use tmux_agent_sidebar::cli::capture::ansi::parse_ansi;
use tmux_agent_sidebar::cli::capture::tmux_probe::PaneGeom;

#[test]
fn pane_geom_parses_tmux_format_line() {
    let line = "%1,0,0,80,40,1";
    let pane = PaneGeom::parse(line).unwrap();
    assert_eq!(pane.pane_id, "%1");
    assert_eq!(pane.left, 0);
    assert_eq!(pane.top, 0);
    assert_eq!(pane.width, 80);
    assert_eq!(pane.height, 40);
    assert!(pane.active);
}

#[test]
fn parse_ansi_emits_cell_with_256_color() {
    // "\x1b[38;5;117mhi" -> two cells 'h' and 'i' with fg=117
    let bytes = b"\x1b[38;5;117mhi";
    let grid = parse_ansi(bytes, 4, 1);
    assert_eq!(grid.len(), 1);
    assert_eq!(grid[0].len(), 4);
    assert_eq!(grid[0][0].ch, 'h');
    assert_eq!(grid[0][0].fg, Some(117));
    assert_eq!(grid[0][1].ch, 'i');
    assert_eq!(grid[0][1].fg, Some(117));
    assert_eq!(grid[0][2].ch, ' ');
    assert_eq!(grid[0][2].fg, None);
}

#[test]
fn parse_ansi_handles_newlines_as_row_advance() {
    let bytes = b"ab\r\ncd";
    let grid = parse_ansi(bytes, 2, 2);
    assert_eq!(grid[0][0].ch, 'a');
    assert_eq!(grid[0][1].ch, 'b');
    assert_eq!(grid[1][0].ch, 'c');
    assert_eq!(grid[1][1].ch, 'd');
}

#[test]
fn parse_ansi_honors_reset_sgr() {
    let bytes = b"\x1b[38;5;117ma\x1b[0mb";
    let grid = parse_ansi(bytes, 4, 1);
    assert_eq!(grid[0][0].fg, Some(117));
    assert_eq!(grid[0][1].fg, None);
}

#[test]
fn parse_ansi_sample_pane_snapshot() {
    let bytes = std::fs::read("tests/fixtures/capture/sample-pane.ansi").unwrap();
    let grid = parse_ansi(&bytes, 40, 5);
    insta::assert_debug_snapshot!(grid);
}

#[test]
fn canvas_assembles_two_panes_side_by_side_with_border() {
    use tmux_agent_sidebar::cli::capture::ansi::StyledCell;
    use tmux_agent_sidebar::cli::capture::canvas::{PaneContent, WindowGeom, assemble};
    use tmux_agent_sidebar::cli::capture::tmux_probe::PaneGeom;

    let left_pane = PaneGeom {
        pane_id: "%1".into(),
        left: 0,
        top: 0,
        width: 4,
        height: 2,
        active: true,
    };
    let right_pane = PaneGeom {
        pane_id: "%2".into(),
        left: 5,
        top: 0,
        width: 4,
        height: 2,
        active: false,
    };

    let make = |ch: char| {
        vec![
            vec![
                StyledCell {
                    ch,
                    ..Default::default()
                };
                4
            ];
            2
        ]
    };

    let panes = vec![
        PaneContent {
            geom: left_pane,
            cells: make('L'),
        },
        PaneContent {
            geom: right_pane,
            cells: make('R'),
        },
    ];

    let geom = WindowGeom { cols: 9, rows: 2 };
    let canvas = assemble(&geom, &panes);

    let row_to_string = |row: &[StyledCell]| -> String { row.iter().map(|c| c.ch).collect() };

    // Row 0: "LLLL│RRRR"
    assert_eq!(row_to_string(&canvas[0]), "LLLL│RRRR");
    assert_eq!(row_to_string(&canvas[1]), "LLLL│RRRR");
}

#[test]
fn canvas_assembles_2x2_grid_resolves_center_cross() {
    use tmux_agent_sidebar::cli::capture::ansi::StyledCell;
    use tmux_agent_sidebar::cli::capture::canvas::{PaneContent, WindowGeom, assemble};
    use tmux_agent_sidebar::cli::capture::tmux_probe::PaneGeom;

    // 4 panes in a 2x2 grid, each 2x2, with 1-cell borders at col=2 and row=2
    // Window is 5x5: panes at (0,0),(3,0),(0,3),(3,3)
    let make_pane = |id: &str, left: u16, top: u16, ch: char| -> PaneContent {
        PaneContent {
            geom: PaneGeom {
                pane_id: id.into(),
                left,
                top,
                width: 2,
                height: 2,
                active: false,
            },
            cells: vec![
                vec![
                    StyledCell {
                        ch,
                        ..Default::default()
                    };
                    2
                ];
                2
            ],
        }
    };

    let panes = vec![
        make_pane("%1", 0, 0, 'A'),
        make_pane("%2", 3, 0, 'B'),
        make_pane("%3", 0, 3, 'C'),
        make_pane("%4", 3, 3, 'D'),
    ];

    let geom = WindowGeom { cols: 5, rows: 5 };
    let canvas = assemble(&geom, &panes);

    let row_to_string = |row: &[StyledCell]| -> String { row.iter().map(|c| c.ch).collect() };

    assert_eq!(row_to_string(&canvas[0]), "AA│BB");
    assert_eq!(row_to_string(&canvas[1]), "AA│BB");
    assert_eq!(row_to_string(&canvas[2]), "──┼──");
    assert_eq!(row_to_string(&canvas[3]), "CC│DD");
    assert_eq!(row_to_string(&canvas[4]), "CC│DD");
}

#[test]
fn render_html_emits_pre_with_per_cell_spans() {
    use tmux_agent_sidebar::cli::capture::ansi::StyledCell;
    use tmux_agent_sidebar::cli::capture::render_html::render_html;

    let cells = vec![vec![
        StyledCell {
            ch: 'a',
            fg: Some(117),
            ..Default::default()
        },
        StyledCell {
            ch: 'b',
            fg: Some(180),
            bold: true,
            ..Default::default()
        },
    ]];
    let html = render_html(&cells);
    insta::assert_snapshot!(html);
}

#[test]
#[ignore = "requires local tmux"]
fn capture_frames_sequence_integration() {
    use std::process::Command;
    use tmux_agent_sidebar::cli;

    Command::new("tmux")
        .args(["new-session", "-d", "-s", "cap-seq", "-x", "40", "-y", "10"])
        .status()
        .expect("tmux new-session");
    let _cleanup = scopeguard::guard((), |_| {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", "cap-seq"])
            .status();
    });

    Command::new("tmux")
        .args(["send-keys", "-t", "cap-seq", "printf 'hi\\n'", "Enter"])
        .status()
        .ok();
    std::thread::sleep(std::time::Duration::from_millis(100));

    let tmp = tempfile::tempdir().unwrap();
    let code = cli::run(&[
        "capture".into(),
        "--session".into(),
        "cap-seq".into(),
        "--frames-out".into(),
        tmp.path().to_string_lossy().into(),
        "--duration-ms".into(),
        "300".into(),
        "--fps".into(),
        "10".into(),
    ])
    .unwrap();
    assert_eq!(code, 0);

    assert!(tmp.path().join("frame0000.html").exists());
    assert!(tmp.path().join("frame0002.html").exists());
    let manifest_path = tmp.path().join("manifest.json");
    assert!(manifest_path.exists());

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["fps"], 10);
    assert_eq!(manifest["duration_ms"], 300);
    assert_eq!(manifest["frame_count"], 3);
}

#[test]
#[ignore = "requires local tmux"]
fn capture_single_frame_integration() {
    use std::process::Command;
    use tmux_agent_sidebar::cli;

    Command::new("tmux")
        .args(["new-session", "-d", "-s", "cap-it", "-x", "40", "-y", "10"])
        .status()
        .expect("tmux new-session");
    let _cleanup = scopeguard::guard((), |_| {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", "cap-it"])
            .status();
    });

    Command::new("tmux")
        .args([
            "send-keys",
            "-t",
            "cap-it",
            "printf '\\033[38;5;117mhi\\n'",
            "Enter",
        ])
        .status()
        .ok();
    std::thread::sleep(std::time::Duration::from_millis(200));

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("frame.html");
    // --window omitted: capture defaults to the session's active window,
    // sidestepping any `base-index` setting the developer's tmux may have.
    let code = cli::run(&[
        "capture".into(),
        "--session".into(),
        "cap-it".into(),
        "--frame-out".into(),
        out.to_string_lossy().into(),
    ])
    .unwrap();
    assert_eq!(code, 0);
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("<pre"));
}
