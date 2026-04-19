pub mod ansi;
pub mod canvas;
pub mod render_html;
pub mod tmux_probe;

use std::fs;

use canvas::{PaneContent, WindowGeom, assemble};
use tmux_probe::{capture_pane, list_panes};

pub fn cmd_capture(args: &[String]) -> i32 {
    let opts = match parse_args(args) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("capture: {msg}");
            return 2;
        }
    };
    let result = if opts.frames_out.is_some() {
        run_loop(&opts)
    } else {
        run_single(&opts)
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("capture: {e}");
            1
        }
    }
}

fn run_loop(opts: &Opts) -> Result<(), String> {
    // Safe unwrap: cmd_capture only dispatches here when frames_out is set.
    let dir = opts
        .frames_out
        .as_ref()
        .expect("run_loop called without frames_out");
    fs::create_dir_all(dir).map_err(|e| format!("create {dir}: {e}"))?;

    if opts.fps == 0 {
        return Err("--fps must be > 0".into());
    }
    // Clamp to at least one frame — the default `duration_ms=1, fps=1`
    // would otherwise truncate to zero and write only the manifest.
    // Cap at u32::MAX before the loop so the iteration count and the
    // number written to `manifest.json` can't disagree (the loop index
    // is u32; an uncapped u64 cast would silently truncate).
    let raw_frame_count = ((opts.duration_ms as u64 * opts.fps as u64) / 1000).max(1);
    let frame_count: u32 = raw_frame_count
        .try_into()
        .map_err(|_| format!("too many frames requested: {raw_frame_count}"))?;
    let interval = std::time::Duration::from_nanos(1_000_000_000u64 / opts.fps as u64);
    let start = std::time::Instant::now();

    for frame in 0..frame_count {
        let html = capture_window_html(opts)?;
        let path = std::path::PathBuf::from(dir).join(format!("frame{frame:04}.html"));
        fs::write(&path, html).map_err(|e| format!("write {}: {e}", path.display()))?;

        let next = start + interval * (frame + 1);
        if let Some(delta) = next.checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(delta);
        }
    }

    let manifest = serde_json::json!({
        "fps": opts.fps,
        "duration_ms": opts.duration_ms,
        "frame_count": frame_count,
    });
    let manifest_path = std::path::PathBuf::from(dir).join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;

    Ok(())
}

fn run_single(opts: &Opts) -> Result<(), String> {
    let html = capture_window_html(opts)?;
    if let Some(path) = &opts.frame_out {
        fs::write(path, html).map_err(|e| format!("write {path}: {e}"))?;
    } else {
        print!("{html}");
    }
    Ok(())
}

fn capture_window_html(opts: &Opts) -> Result<String, String> {
    let panes = list_panes(&opts.session, opts.window.as_deref())?;
    let (cols, rows) = window_bounds(&panes)?;
    let mut contents = Vec::with_capacity(panes.len());
    for geom in panes {
        let bytes = capture_pane(&geom.pane_id)?;
        let cells = ansi::parse_ansi(&bytes, geom.width, geom.height);
        contents.push(PaneContent { geom, cells });
    }
    let mut grid = assemble(&WindowGeom { cols, rows }, &contents);

    // Optional crop: --crop-rows START:END and/or --crop-cols START:END
    // (END is exclusive) trim the grid before rendering so scenarios
    // can emit just the Activity/Git tab region, just a popup, etc.
    if let Some((r0, r1)) = opts.crop_rows {
        let r0 = (r0 as usize).min(grid.len());
        let r1 = (r1 as usize).min(grid.len()).max(r0);
        grid = grid[r0..r1].to_vec();
    }
    if let Some((c0, c1)) = opts.crop_cols {
        for row in grid.iter_mut() {
            let c0 = (c0 as usize).min(row.len());
            let c1 = (c1 as usize).min(row.len()).max(c0);
            *row = row[c0..c1].to_vec();
        }
    }

    Ok(render_html::render_html(&grid))
}

fn window_bounds(panes: &[tmux_probe::PaneGeom]) -> Result<(u16, u16), String> {
    // Use checked_add so a malformed `tmux list-panes` row where
    // left+width (or top+height) wraps past u16::MAX fails loudly
    // instead of silently wrapping to a tiny canvas.
    let mut cols: u16 = 0;
    let mut rows: u16 = 0;
    for p in panes {
        let right = p
            .left
            .checked_add(p.width)
            .ok_or_else(|| format!("pane {} geometry overflows u16 width", p.pane_id))?;
        let bottom = p
            .top
            .checked_add(p.height)
            .ok_or_else(|| format!("pane {} geometry overflows u16 height", p.pane_id))?;
        cols = cols.max(right);
        rows = rows.max(bottom);
    }
    Ok((cols, rows))
}

#[derive(Debug)]
struct Opts {
    session: String,
    window: Option<String>,
    frame_out: Option<String>,
    frames_out: Option<String>,
    duration_ms: u32,
    fps: u32,
    crop_rows: Option<(u16, u16)>,
    crop_cols: Option<(u16, u16)>,
}

/// Parse "N:M" → (N, M). Both ends must be valid u16 and start < end.
fn parse_range(s: &str) -> Result<(u16, u16), String> {
    let (a, b) = s
        .split_once(':')
        .ok_or_else(|| format!("expected START:END, got {s}"))?;
    let a: u16 = a.parse().map_err(|e| format!("range start: {e}"))?;
    let b: u16 = b.parse().map_err(|e| format!("range end: {e}"))?;
    if a >= b {
        return Err(format!("range start must be < end ({a} >= {b})"));
    }
    Ok((a, b))
}

fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut session = None;
    let mut window: Option<String> = None;
    let mut frame_out = None;
    let mut frames_out = None;
    let mut duration_ms = 1u32;
    let mut fps = 1u32;
    let mut crop_rows: Option<(u16, u16)> = None;
    let mut crop_cols: Option<(u16, u16)> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--session" => {
                session = Some(args.get(i + 1).ok_or("--session needs value")?.clone());
                i += 2;
            }
            "--window" => {
                window = Some(args.get(i + 1).ok_or("--window needs value")?.clone());
                i += 2;
            }
            "--frame-out" => {
                frame_out = Some(args.get(i + 1).ok_or("--frame-out needs value")?.clone());
                i += 2;
            }
            "--frames-out" => {
                frames_out = Some(args.get(i + 1).ok_or("--frames-out needs value")?.clone());
                i += 2;
            }
            "--duration-ms" => {
                duration_ms = args
                    .get(i + 1)
                    .ok_or("--duration-ms needs value")?
                    .parse()
                    .map_err(|e| format!("--duration-ms: {e}"))?;
                i += 2;
            }
            "--fps" => {
                fps = args
                    .get(i + 1)
                    .ok_or("--fps needs value")?
                    .parse()
                    .map_err(|e| format!("--fps: {e}"))?;
                i += 2;
            }
            "--crop-rows" => {
                let v = args.get(i + 1).ok_or("--crop-rows needs value")?;
                crop_rows = Some(parse_range(v).map_err(|e| format!("--crop-rows: {e}"))?);
                i += 2;
            }
            "--crop-cols" => {
                let v = args.get(i + 1).ok_or("--crop-cols needs value")?;
                crop_cols = Some(parse_range(v).map_err(|e| format!("--crop-cols: {e}"))?);
                i += 2;
            }
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    if frame_out.is_some() && frames_out.is_some() {
        return Err("--frame-out and --frames-out are mutually exclusive".into());
    }
    Ok(Opts {
        session: session.ok_or("--session required")?,
        window,
        frame_out,
        frames_out,
        duration_ms,
        fps,
        crop_rows,
        crop_cols,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_requires_session() {
        let err = parse_args(&[]).unwrap_err();
        assert!(err.contains("--session required"));
    }

    #[test]
    fn parse_args_parses_fps() {
        let opts =
            parse_args(&["--session".into(), "x".into(), "--fps".into(), "30".into()]).unwrap();
        assert_eq!(opts.fps, 30);
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let err = parse_args(&["--session".into(), "x".into(), "--weird".into()]).unwrap_err();
        assert!(err.contains("unknown flag"));
    }

    #[test]
    fn parse_args_defaults_fps_and_duration() {
        let opts = parse_args(&["--session".into(), "x".into()]).unwrap();
        assert_eq!(opts.fps, 1);
        assert_eq!(opts.duration_ms, 1);
        assert_eq!(opts.window, None);
    }

    #[test]
    fn parse_args_accepts_frames_out_and_duration() {
        let opts = parse_args(&[
            "--session".into(),
            "s".into(),
            "--frames-out".into(),
            "/tmp/out".into(),
            "--duration-ms".into(),
            "8000".into(),
            "--fps".into(),
            "30".into(),
        ])
        .unwrap();
        assert_eq!(opts.frames_out.as_deref(), Some("/tmp/out"));
        assert_eq!(opts.duration_ms, 8000);
        assert_eq!(opts.fps, 30);
    }
}
