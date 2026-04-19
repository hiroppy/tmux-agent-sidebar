use std::process::Command;

/// Per-pane geometry as emitted by `tmux list-panes -F`.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneGeom {
    pub pane_id: String,
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
    pub active: bool,
}

impl PaneGeom {
    /// Parse a single line of the form `%1,0,0,80,40,1`
    /// (pane_id,left,top,width,height,active).
    pub fn parse(line: &str) -> Result<Self, String> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 6 {
            return Err(format!("expected 6 fields, got {}: {line}", parts.len()));
        }
        let parse_u16 = |s: &str| s.parse::<u16>().map_err(|e| format!("{s}: {e}"));
        Ok(Self {
            pane_id: parts[0].to_string(),
            left: parse_u16(parts[1])?,
            top: parse_u16(parts[2])?,
            width: parse_u16(parts[3])?,
            height: parse_u16(parts[4])?,
            active: match parts[5] {
                "1" => true,
                "0" => false,
                other => return Err(format!("invalid active flag: {other}")),
            },
        })
    }
}

/// Query tmux for all panes in the given window.
///
/// `window` is optional: if `None`, the session's currently active window is
/// used, which sidesteps any user-specific `base-index` setting (some
/// configurations start window numbering at 1 instead of 0).
pub fn list_panes(session: &str, window: Option<&str>) -> Result<Vec<PaneGeom>, String> {
    let target = match window {
        // Fully-qualified window id (e.g. `@3`) — tmux resolves it
        // globally, so we skip the session prefix.
        Some(w) if w.starts_with('@') => w.to_string(),
        Some(w) => format!("{session}:{w}"),
        None => session.to_string(),
    };
    let out = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            &target,
            "-F",
            "#{pane_id},#{pane_left},#{pane_top},#{pane_width},#{pane_height},#{pane_active}",
        ])
        .output()
        .map_err(|e| format!("spawning tmux: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "tmux list-panes failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.lines().map(PaneGeom::parse).collect()
}

/// Capture one pane as ANSI-coloured bytes via `capture-pane -p -e`.
pub fn capture_pane(pane_id: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-e", "-t", pane_id])
        .output()
        .map_err(|e| format!("spawning tmux: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "tmux capture-pane {pane_id} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_wrong_arity() {
        assert!(PaneGeom::parse("a,b,c").is_err());
    }

    #[test]
    fn parse_rejects_non_numeric() {
        assert!(PaneGeom::parse("%1,x,0,80,40,1").is_err());
    }

    #[test]
    fn parse_handles_inactive_pane() {
        let p = PaneGeom::parse("%2,10,5,60,30,0").unwrap();
        assert_eq!(p.pane_id, "%2");
        assert!(!p.active);
    }
}
