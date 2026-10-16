use super::commands::{display_message, run_tmux};

/// Returns (pane_active, window_active, session_attached, width, height) for
/// the sidebar pane. `session_attached` is true when a client is attached to
/// this pane's session — it distinguishes the session the user is actually
/// viewing from background sessions, whose active window/pane still report
/// `window_active`/`pane_active` = 1 even though no one is looking at them.
pub fn get_sidebar_pane_info(tmux_pane: &str) -> (bool, bool, bool, u16, u16) {
    let out = display_message(
        tmux_pane,
        "#{pane_active} #{window_active} #{session_attached} #{pane_width} #{pane_height}",
    );
    let parts: Vec<&str> = out.splitn(5, ' ').collect();
    if parts.len() >= 5 {
        (
            parts[0] == "1",
            parts[1] == "1",
            parts[2] != "0" && !parts[2].is_empty(),
            parts[3].parse().unwrap_or(28),
            parts[4].parse().unwrap_or(24),
        )
    } else {
        (false, false, false, 28, 24)
    }
}

pub fn get_pane_path(pane_id: &str) -> Option<String> {
    Some(display_message(pane_id, "#{pane_current_path}")).filter(|s| !s.is_empty())
}

/// Query tmux for all panes in the active window, returning
/// (pane_id, pane_active, pane_last, path). `pane_last` marks tmux's
/// previously-active pane ({last}), used to recover focus when the sidebar
/// pane itself is the active one. NOT filtered by agent type, so it includes
/// all panes (shell, editor, etc.) — not just agent panes.
pub fn query_active_window_panes() -> Vec<(String, bool, bool, String)> {
    // List panes in the current (active) window across all sessions
    let output = match run_tmux(&[
        "list-panes",
        "-F",
        "#{pane_id}|#{pane_active}|#{pane_last}|#{pane_current_path}",
    ]) {
        Some(s) => s,
        None => return vec![],
    };
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() < 4 {
                return None;
            }
            Some((
                parts[0].to_string(),
                parts[1] == "1",
                parts[2] == "1",
                parts[3].to_string(),
            ))
        })
        .collect()
}

/// Find the focused (non-sidebar) pane ID and path by querying tmux directly.
/// Returns all panes regardless of agent type, so activity/git info can be shown
/// even for non-agent panes.
pub fn find_active_pane(sidebar_pane: &str) -> Option<(String, String)> {
    pick_active_pane(sidebar_pane, &query_active_window_panes())
}

/// Pure logic: pick the focused non-sidebar pane from a list of
/// (pane_id, pane_active, pane_last, path).
///
/// Prefers the genuinely active non-sidebar pane. When the sidebar pane
/// itself is active (common: the sidebar holds focus in its window), there is
/// no active non-sidebar pane, so fall back to tmux's previously-active pane
/// ({last}) within this same window. This keeps the focus marker on a pane of
/// the *current* window instead of stranding it on a stale pane from a
/// different window/session — which otherwise leaves the wrong agent
/// highlighted for seconds after switching here.
///
/// Returns None only when no valid non-sidebar pane is found, so callers can
/// preserve the previously focused pane.
pub(crate) fn pick_active_pane(
    sidebar_pane: &str,
    panes: &[(String, bool, bool, String)],
) -> Option<(String, String)> {
    let valid = |p: &&(String, bool, bool, String)| p.0 != sidebar_pane && !p.3.is_empty();
    panes
        .iter()
        .find(|p| p.1 && valid(p))
        .or_else(|| panes.iter().find(|p| p.2 && valid(p)))
        .map(|p| (p.0.clone(), p.3.clone()))
}

/// Find the focused pane's working directory by querying tmux directly.
/// Used by the background git thread which doesn't have access to AppState.
/// Queries all panes (not just agent panes) so git info is available
/// even when the focused pane has no agent running.
pub fn focused_pane_path(sidebar_pane: &str) -> Option<String> {
    find_active_pane(sidebar_pane).map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tuples are (pane_id, pane_active, pane_last, path).
    #[test]
    fn pick_active_pane_returns_active_non_sidebar() {
        let panes = vec![
            ("%1".into(), false, false, "/home".into()),
            ("%2".into(), true, false, "/work".into()),
            ("%3".into(), false, false, "/tmp".into()),
        ];
        assert_eq!(
            pick_active_pane("%99", &panes),
            Some(("%2".into(), "/work".into()))
        );
    }

    #[test]
    fn pick_active_pane_falls_back_to_last_when_sidebar_active() {
        // Sidebar pane %99 is the active pane; the previously-active claude
        // pane (%2, pane_last) should be picked instead of returning None.
        let panes = vec![
            ("%99".into(), true, false, "/sidebar".into()),
            ("%1".into(), false, false, "/home".into()),
            ("%2".into(), false, true, "/work".into()),
        ];
        assert_eq!(
            pick_active_pane("%99", &panes),
            Some(("%2".into(), "/work".into()))
        );
    }

    #[test]
    fn pick_active_pane_prefers_active_over_last() {
        let panes = vec![
            ("%1".into(), true, false, "/active".into()),
            ("%2".into(), false, true, "/last".into()),
        ];
        assert_eq!(
            pick_active_pane("%99", &panes),
            Some(("%1".into(), "/active".into()))
        );
    }

    #[test]
    fn pick_active_pane_skips_sidebar_even_when_marked_active() {
        let panes = vec![("%99".into(), true, false, "/a".into())];
        assert!(pick_active_pane("%99", &panes).is_none());
    }

    #[test]
    fn pick_active_pane_skips_panes_with_empty_path() {
        let panes = vec![
            ("%1".into(), true, false, "".into()),
            ("%2".into(), true, false, "/ok".into()),
        ];
        assert_eq!(
            pick_active_pane("%99", &panes),
            Some(("%2".into(), "/ok".into()))
        );
    }

    #[test]
    fn pick_active_pane_returns_none_for_empty_list() {
        assert!(pick_active_pane("%99", &[]).is_none());
    }

    #[test]
    fn pick_active_pane_returns_none_when_no_active_or_last() {
        let panes = vec![
            ("%1".into(), false, false, "/x".into()),
            ("%2".into(), false, false, "/y".into()),
        ];
        assert!(pick_active_pane("%99", &panes).is_none());
    }
}
