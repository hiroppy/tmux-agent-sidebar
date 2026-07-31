use std::collections::HashSet;

use crate::tmux;

pub(crate) fn cmd_toggle(args: &[String]) -> i32 {
    let mut create_only = false;
    let mut positional = Vec::new();

    for arg in args {
        if arg == "--create-only" {
            create_only = true;
        } else {
            positional.push(arg.as_str());
        }
    }

    let window_id = match positional.first() {
        Some(id) => *id,
        None => return 0,
    };
    let pane_path = positional.get(1).copied().unwrap_or("~");

    // Check sidebar width setting
    let sidebar_width_setting = {
        let s = tmux::display_message(window_id, &format!("#{{{}}}", tmux::SIDEBAR_WIDTH));
        if s.is_empty() { "30".to_string() } else { s }
    };

    let sidebar_width = if sidebar_width_setting.ends_with('%') {
        let window_width: u32 = tmux::display_message(window_id, "#{window_width}")
            .parse()
            .unwrap_or(0);
        let pct: u32 = sidebar_width_setting
            .trim_end_matches('%')
            .parse()
            .unwrap_or(15);
        if window_width > 0 && pct > 0 {
            let w = window_width * pct / 100;
            if w < 1 {
                "1".to_string()
            } else {
                w.to_string()
            }
        } else {
            sidebar_width_setting
        }
    } else {
        sidebar_width_setting
    };

    let sidebar_position = SidebarPosition::from_setting(&tmux::display_message(
        window_id,
        &format!("#{{{}}}", tmux::SIDEBAR_POSITION),
    ));

    // Check for existing sidebar
    let pane_id_role_format = pane_id_role_format();
    let panes_output = tmux::run_tmux(&["list-panes", "-t", window_id, "-F", &pane_id_role_format])
        .unwrap_or_default();

    let existing_sidebar = panes_output.lines().find_map(|line| {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() >= 2 && parts[1] == "sidebar" {
            Some(parts[0].to_string())
        } else {
            None
        }
    });

    if let Some(sidebar_pane) = existing_sidebar {
        if create_only {
            return 0;
        }
        let _ = tmux::run_tmux(&["kill-pane", "-t", &sidebar_pane]);
        return 0;
    }

    let pane_geometry_output = tmux::run_tmux(&[
        "list-panes",
        "-t",
        window_id,
        "-F",
        "#{pane_left} #{pane_width} #{pane_id}",
    ])
    .unwrap_or_default();

    let target_pane = target_pane_for_position(&pane_geometry_output, sidebar_position)
        .unwrap_or_else(|| window_id.to_string());
    let split_flags = split_window_flags(sidebar_position);

    // Remember active pane
    let active_pane = tmux::display_message(window_id, "#{pane_id}");

    // Find our own binary path
    let self_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "tmux-agent-sidebar".to_string());

    // Create sidebar pane
    let sidebar_pane = tmux::run_tmux(&[
        "split-window",
        split_flags,
        "-l",
        &sidebar_width,
        "-t",
        &target_pane,
        "-c",
        pane_path,
        "-P",
        "-F",
        "#{pane_id}",
        &self_bin,
    ])
    .map(|s| s.trim().to_string())
    .unwrap_or_default();

    if !sidebar_pane.is_empty() {
        tmux::set_pane_option(&sidebar_pane, tmux::PANE_ROLE, "sidebar");

        // Sync sidebar widths across windows if enabled
        let sync_on = tmux::get_option(tmux::SIDEBAR_SYNC_SIZE)
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("on"));
        if sync_on {
            sync_all_sidebar_widths(window_id, ReferenceSource::OtherWindow);
        }
    }

    // Restore focus
    if !active_pane.is_empty() {
        let _ = tmux::run_tmux(&["select-pane", "-t", &active_pane]);
    } else {
        let _ = tmux::run_tmux(&["select-pane", "-t", window_id, "-l"]);
    }

    0
}

pub(crate) fn cmd_toggle_all(_args: &[String]) -> i32 {
    let pane_id_role_format = pane_id_role_format();
    let has_sidebar = tmux::run_tmux(&["list-panes", "-a", "-F", &pane_id_role_format])
        .map(|output| any_sidebar_pane(&output))
        .unwrap_or(false);

    if has_sidebar {
        let all_panes =
            tmux::run_tmux(&["list-panes", "-a", "-F", &pane_id_role_format]).unwrap_or_default();
        for line in all_panes.lines() {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            if parts.len() >= 2 && parts[1] == "sidebar" {
                let _ = tmux::run_tmux(&["kill-pane", "-t", parts[0]]);
            }
        }
    } else {
        let all_windows = tmux::run_tmux(&[
            "list-panes",
            "-a",
            "-F",
            "#{window_id}|#{pane_current_path}",
        ])
        .unwrap_or_default();
        for (window_id, pane_path) in unique_window_paths(&all_windows) {
            let args = vec!["--create-only".to_string(), window_id, pane_path];
            cmd_toggle(&args);
        }
    }

    0
}

fn any_sidebar_pane(output: &str) -> bool {
    output.lines().any(|line| {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        parts.len() >= 2 && parts[1] == "sidebar"
    })
}

fn unique_window_paths(output: &str) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut windows = Vec::new();

    for line in output.lines() {
        let Some((window_id, pane_path)) = line.split_once('|') else {
            continue;
        };
        if seen.insert(window_id.to_string()) {
            windows.push((window_id.to_string(), pane_path.to_string()));
        }
    }

    windows
}

/// Which side of the window the sidebar pane is created on, driven by
/// the `@sidebar_position` tmux option.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SidebarPosition {
    Left,
    Right,
}

impl SidebarPosition {
    /// Parse the raw `@sidebar_position` option value. Only an explicit
    /// (case-insensitive, whitespace-tolerant) `right` selects the right
    /// side; everything else — including unset, empty, or invalid values
    /// — falls back to the historical default of `left`, so a typo never
    /// moves the sidebar somewhere unexpected.
    fn from_setting(setting: &str) -> Self {
        if setting.trim().eq_ignore_ascii_case("right") {
            Self::Right
        } else {
            Self::Left
        }
    }
}

/// Horizontal placement of one pane, parsed from a
/// `#{pane_left} #{pane_width} #{pane_id}` formatted `list-panes` line.
#[derive(Debug, Eq, PartialEq)]
struct PaneGeometry {
    left: u32,
    width: u32,
    pane_id: String,
}

/// Parse a single `list-panes` output line into a [`PaneGeometry`].
/// Returns `None` for malformed lines so callers can simply skip them.
fn parse_pane_geometry(line: &str) -> Option<PaneGeometry> {
    let mut parts = line.split_whitespace();
    let left = parts.next()?.parse().ok()?;
    let width = parts.next()?.parse().ok()?;
    let pane_id = parts.next()?.to_string();
    Some(PaneGeometry {
        left,
        width,
        pane_id,
    })
}

/// Pick the pane the sidebar splits from: the leftmost pane for a left
/// sidebar, or the pane with the largest right edge (`left + width`) for
/// a right sidebar, so the new pane always lands at the window's outer
/// edge. Returns `None` when no line of `output` parses as geometry.
fn target_pane_for_position(output: &str, position: SidebarPosition) -> Option<String> {
    let panes = output.lines().filter_map(parse_pane_geometry);
    match position {
        SidebarPosition::Left => panes.min_by_key(|pane| pane.left),
        SidebarPosition::Right => panes.max_by_key(|pane| pane.left.saturating_add(pane.width)),
    }
    .map(|pane| pane.pane_id)
}

/// `split-window` flags for each placement: `-hfb` inserts the new pane
/// before the target (left of it), `-hf` after it (right of it). Both
/// `f` variants span the full window height.
fn split_window_flags(position: SidebarPosition) -> &'static str {
    match position {
        SidebarPosition::Left => "-hfb",
        SidebarPosition::Right => "-hf",
    }
}

/// Decide whether `cmd_auto_close` should kill the window, given the raw
/// outputs of the tmux queries it performs. Extracted as a pure function
/// so the guard logic is directly unit-testable without a running tmux
/// server.
///
/// - `list_panes_output`: `Some(stdout)` from `list-panes -F <pane role format>`,
///   or `None` if the tmux call failed.
/// - `session_windows`: parsed value of `#{session_windows}`, or `None`
///   if the tmux call failed or the value was unparseable.
/// - `session_attached`: parsed value of `#{session_attached}`, or `None`
///   if the tmux call failed or the value was unparseable.
fn should_kill_window(
    list_panes_output: Option<&str>,
    session_windows: Option<u32>,
    session_attached: Option<u32>,
) -> bool {
    // `list-panes` failed or returned nothing: the window is either gone
    // already or tmux is too busy to answer. Do NOT treat "no output"
    // as "no non-sidebar panes" — that would let us kill a live window
    // whose query happened to race with another tmux command.
    let Some(output) = list_panes_output else {
        return false;
    };
    if output.trim().is_empty() {
        return false;
    }

    let non_sidebar = output.lines().filter(|line| *line != "sidebar").count();
    if non_sidebar != 0 {
        return false;
    }

    let Some(windows) = session_windows else {
        return false;
    };

    // Last window in the session: killing it destroys the session and
    // drops every attached client. One attached client is fine — that
    // matches normal tmux `exit` behaviour on the last pane. Two or
    // more means a shared session (e.g. several terminal tabs attached
    // to `main`) where we cannot tell which clients are "wanted", so
    // preserve the sidebar instead. A missing `session_attached` errs
    // on the side of preservation.
    match windows {
        0 => false,
        1 => matches!(session_attached, Some(n) if n <= 1),
        _ => true,
    }
}

pub(crate) fn cmd_auto_close(args: &[String]) -> i32 {
    let window_id = match args.first() {
        Some(id) => id.as_str(),
        None => return 0,
    };

    let pane_role_format = format!("#{{{}}}", tmux::PANE_ROLE);
    let list_panes_output =
        tmux::run_tmux(&["list-panes", "-t", window_id, "-F", &pane_role_format]);

    let session_windows = tmux::run_tmux(&[
        "display-message",
        "-t",
        window_id,
        "-p",
        "#{session_windows}",
    ])
    .and_then(|s| s.trim().parse().ok());

    let session_attached = tmux::run_tmux(&[
        "display-message",
        "-t",
        window_id,
        "-p",
        "#{session_attached}",
    ])
    .and_then(|s| s.trim().parse().ok());

    if should_kill_window(
        list_panes_output.as_deref(),
        session_windows,
        session_attached,
    ) {
        let _ = tmux::run_tmux(&["kill-window", "-t", window_id]);
    }

    0
}

fn pane_id_role_format() -> String {
    format!("#{{pane_id}}|#{{{}}}", tmux::PANE_ROLE)
}

/// Which sidebar to use as the reference width when syncing.
#[derive(Clone, Copy)]
pub(crate) enum ReferenceSource {
    /// Use the first sidebar found in a window OTHER than the current one.
    /// Correct for `cmd_toggle`: a newly created sidebar should match
    /// the width already in use by other windows.
    OtherWindow,
    /// Use the sidebar in the CURRENT window as the reference.
    /// Correct for `cmd_sync_size`: the user just resized this window's
    /// sidebar (or switched here), so propagate ITS width outward.
    CurrentWindow,
}

pub(crate) fn cmd_sync_size(args: &[String]) -> i32 {
    let window_id = match args.first() {
        Some(id) => id.as_str(),
        None => return 0,
    };

    let sync_on = tmux::get_option(tmux::SIDEBAR_SYNC_SIZE)
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("on"));
    if !sync_on {
        return 0;
    }

    // sync-size hooks after window switch: use current window as reference
    sync_all_sidebar_widths(window_id, ReferenceSource::CurrentWindow);
    0
}

/// Sync all sidebar pane widths across all windows TO a common size.
/// The reference width is determined by `source`.
pub(crate) fn sync_all_sidebar_widths(current_window_id: &str, source: ReferenceSource) {
    // Scope to this session only — avoid pulling in detached/other sessions
    let session_id = tmux::display_message(current_window_id, "#{session_id}");
    if session_id.is_empty() {
        return;
    }

    let fmt = format!(
        "#{{session_id}}|#{{window_id}}|#{{pane_id}}|#{{pane_width}}|#{{{}}}",
        tmux::PANE_ROLE,
    );
    // list-panes -a returns from all sessions; we filter by session_id below
    let output = match tmux::run_tmux(&["list-panes", "-a", "-F", &fmt]) {
        Some(s) => s,
        None => return,
    };

    let resizes = compute_sync_resizes(current_window_id, &session_id, &output, source);
    for (pane_id, target_width) in &resizes {
        let _ = tmux::run_tmux(&[
            "resize-pane",
            "-t",
            pane_id,
            "-x",
            &target_width.to_string(),
        ]);
    }
}

/// Pure function: parse `list-panes -a` output and return (pane_id, target_width)
/// pairs that need resizing. Only panes in `expected_session` are considered.
/// `current_window_id` is excluded when building the reference (for `OtherWindow` source).
fn compute_sync_resizes(
    current_window_id: &str,
    expected_session: &str,
    list_panes_output: &str,
    source: ReferenceSource,
) -> Vec<(String, u32)> {
    // Collect sidebar panes in this session: (window_id, pane_id, width)
    let sidebars: Vec<(String, String, u32)> = list_panes_output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(5, '|');
            let session_id = parts.next()?;
            // Only panes in the expected session
            if session_id != expected_session {
                return None;
            }
            let window_id = parts.next()?;
            let pane_id = parts.next()?;
            let width = parts.next()?.parse().ok()?;
            let role = parts.next().unwrap_or("");
            if role.trim() == "sidebar" {
                Some((window_id.to_string(), pane_id.to_string(), width))
            } else {
                None
            }
        })
        .collect();

    if sidebars.len() <= 1 {
        return vec![];
    }

    // Reference width strategy depends on source
    let Some(reference) = (match source {
        // toggle: new sidebar → inherit from other windows
        ReferenceSource::OtherWindow => sidebars
            .iter()
            .find(|(wid, _, _)| wid != current_window_id)
            .map(|(_, _, w)| *w)
            .or_else(|| sidebars.first().map(|(_, _, w)| *w)),
        // sync-size: propagating FROM current window → use its sidebar
        ReferenceSource::CurrentWindow => sidebars
            .iter()
            .find(|(wid, _, _)| wid == current_window_id)
            .map(|(_, _, w)| *w),
        // No fallback: if the current window has no sidebar there is
        // nothing to propagate — returning None lets this be a no-op.
    }) else {
        return vec![];
    };

    // Skip if all already match
    if sidebars.iter().all(|(_, _, w)| *w == reference) {
        return vec![];
    }

    // Return panes that need resizing to match reference
    sidebars
        .iter()
        .filter(|(_, _, w)| *w != reference)
        .map(|(_, pid, _)| (pid.clone(), reference))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_sidebar_pane_detects_sidebar_anywhere() {
        let output = "%1|pane\n%2|sidebar\n%3|pane";
        assert!(any_sidebar_pane(output));
    }

    #[test]
    fn any_sidebar_pane_returns_false_without_sidebar() {
        let output = "%1|pane\n%2|main";
        assert!(!any_sidebar_pane(output));
    }

    #[test]
    fn unique_window_paths_deduplicates_windows_and_keeps_spaces() {
        let output = "%1|/Users/me/My Project\n%1|/Users/me/My Project\n%2|/tmp/another project";
        assert_eq!(
            unique_window_paths(output),
            vec![
                ("%1".to_string(), "/Users/me/My Project".to_string()),
                ("%2".to_string(), "/tmp/another project".to_string()),
            ]
        );
    }

    #[test]
    fn unique_window_paths_skips_malformed_lines() {
        let output = "bad-line\n%1|/tmp";
        assert_eq!(
            unique_window_paths(output),
            vec![("%1".to_string(), "/tmp".to_string())]
        );
    }

    // ─── sidebar placement ───────────────────────────────────────────

    #[test]
    fn sidebar_position_parses_right_only() {
        assert_eq!(
            SidebarPosition::from_setting("right"),
            SidebarPosition::Right
        );
        assert_eq!(
            SidebarPosition::from_setting(" RIGHT "),
            SidebarPosition::Right
        );
        assert_eq!(SidebarPosition::from_setting("left"), SidebarPosition::Left);
        assert_eq!(SidebarPosition::from_setting(""), SidebarPosition::Left);
        assert_eq!(
            SidebarPosition::from_setting("invalid"),
            SidebarPosition::Left
        );
    }

    #[test]
    fn target_pane_for_left_position_uses_leftmost_pane() {
        let output = "40 80 %3\n0 20 %1\n20 20 %2";

        assert_eq!(
            target_pane_for_position(output, SidebarPosition::Left),
            Some("%1".to_string())
        );
    }

    #[test]
    fn target_pane_for_right_position_uses_largest_right_edge() {
        let output = "0 20 %1\n20 20 %2\n40 80 %3";

        assert_eq!(
            target_pane_for_position(output, SidebarPosition::Right),
            Some("%3".to_string())
        );
    }

    #[test]
    fn target_pane_for_position_skips_malformed_lines() {
        let output = "bad-line\n0 nope %1\n12 30 %2";

        assert_eq!(
            target_pane_for_position(output, SidebarPosition::Left),
            Some("%2".to_string())
        );
        assert_eq!(target_pane_for_position("", SidebarPosition::Right), None);
    }

    #[test]
    fn split_window_flags_match_tmux_side_semantics() {
        assert_eq!(split_window_flags(SidebarPosition::Left), "-hfb");
        assert_eq!(split_window_flags(SidebarPosition::Right), "-hf");
    }

    // ─── should_kill_window ───────────────────────────────────────────

    #[test]
    fn should_kill_window_kills_when_only_sidebar_and_other_windows_exist() {
        // Classic intended path: sidebar alone in a window, session has
        // other windows to fall back on. Attached-client count is
        // irrelevant because killing this window does not end the
        // session.
        assert!(should_kill_window(Some("sidebar"), Some(2), None));
        assert!(should_kill_window(Some("sidebar"), Some(2), Some(0)));
        assert!(should_kill_window(Some("sidebar"), Some(2), Some(5)));
    }

    #[test]
    fn should_kill_window_skips_when_non_sidebar_pane_remains() {
        // Another pane with `@pane_role` explicitly set to something
        // non-sidebar (e.g. a spawn-marked pane) keeps the window alive.
        assert!(!should_kill_window(Some("sidebar\npane"), Some(5), Some(1)));
        // `@pane_role` unset renders as an empty line — that pane is
        // a regular user pane, not a sidebar, so the window must stay.
        // The real tmux output for [sidebar pane, regular pane] is
        // "sidebar\n\n" (sidebar's role, then the regular pane's empty
        // role followed by the final record separator).
        assert!(!should_kill_window(Some("sidebar\n\n"), Some(5), Some(1)));
        assert!(!should_kill_window(Some("\nsidebar\n"), Some(5), Some(1)));
    }

    #[test]
    fn should_kill_window_skips_when_list_panes_failed() {
        // `list-panes` failure must never be treated as "window is empty" —
        // that used to let a busy-tmux race kill a live window.
        assert!(!should_kill_window(None, Some(5), Some(1)));
    }

    #[test]
    fn should_kill_window_skips_when_list_panes_empty() {
        // Whitespace-only output (e.g. window already gone) must not
        // trigger a kill either.
        assert!(!should_kill_window(Some(""), Some(5), Some(1)));
        assert!(!should_kill_window(Some("   \n"), Some(5), Some(1)));
    }

    #[test]
    fn should_kill_window_kills_last_window_when_single_client_attached() {
        // One client attached to a single-window session: destroying
        // the session only detaches the same client that just kept the
        // session alive, which matches tmux's standard `exit` behaviour
        // on the last pane — the user expects the sidebar to go with it.
        assert!(should_kill_window(Some("sidebar"), Some(1), Some(1)));
    }

    #[test]
    fn should_kill_window_kills_last_window_when_detached() {
        // No clients attached: killing the session harms no one, and
        // a stranded sidebar in a detached session is pointless anyway.
        assert!(should_kill_window(Some("sidebar"), Some(1), Some(0)));
    }

    #[test]
    fn should_kill_window_preserves_last_window_when_multiple_clients_attached() {
        // Core regression guard (0dc6e99): killing the last window of
        // a session drops every attached client. With multiple terminal
        // tabs sharing a single `main` session, that manifested as every
        // tab dying at once. Keep the sidebar stranded rather than nuke
        // the session.
        assert!(!should_kill_window(Some("sidebar"), Some(1), Some(2)));
        assert!(!should_kill_window(Some("sidebar"), Some(1), Some(7)));
    }

    #[test]
    fn should_kill_window_preserves_last_window_when_attached_query_failed() {
        // Without knowing how many clients are attached we cannot prove
        // the kill is safe. Better a lingering sidebar pane than a
        // mass-disconnect.
        assert!(!should_kill_window(Some("sidebar"), Some(1), None));
    }

    #[test]
    fn should_kill_window_skips_when_session_windows_query_failed() {
        // If we cannot prove the session has other windows, err on the
        // side of preservation. Better to leave a lingering sidebar
        // pane than to destroy a live workspace.
        assert!(!should_kill_window(Some("sidebar"), None, Some(1)));
        assert!(!should_kill_window(Some("sidebar"), Some(0), Some(1)));
    }

    // ─── sync-size ────────────────────────────────────────────────────

    #[test]
    fn sync_resizes_returns_empty_when_only_one_sidebar() {
        let resizes = compute_sync_resizes(
            "@1",
            "s1",
            "s1|@1|%1|35|sidebar",
            ReferenceSource::OtherWindow,
        );
        assert!(resizes.is_empty());
    }

    #[test]
    fn sync_resizes_returns_empty_when_none_are_sidebars() {
        let resizes =
            compute_sync_resizes("@1", "s1", "s1|@1|%1|35|main", ReferenceSource::OtherWindow);
        assert!(resizes.is_empty());
    }

    #[test]
    fn sync_resizes_returns_empty_when_all_same_width() {
        let output = "s1|@1|%1|35|sidebar\ns1|@2|%2|35|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert!(resizes.is_empty());
    }

    #[test]
    fn sync_resizes_uses_other_window_width_as_reference() {
        let output = "s1|@1|%1|35|sidebar\ns1|@2|%2|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert_eq!(resizes.len(), 1);
        assert_eq!(resizes[0], ("%1".to_string(), 50));
    }

    #[test]
    fn sync_resizes_multi_window_resizes_others_to_reference() {
        let output = "s1|@1|%1|50|sidebar\ns1|@2|%2|35|sidebar\ns1|@3|%3|35|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert_eq!(resizes.len(), 1);
        assert_eq!(resizes[0], ("%1".to_string(), 35));
    }

    #[test]
    fn sync_resizes_all_in_current_window_falls_back_to_first() {
        // All sidebars in @1 → no "other" window, fallback to first (35)
        let output = "s1|@1|%1|35|sidebar\ns1|@1|%2|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert_eq!(resizes.len(), 1);
        assert_eq!(resizes[0], ("%2".to_string(), 35));
    }

    #[test]
    fn sync_resizes_skips_malformed_lines() {
        let output = "s1|@1|%1|35|sidebar\nbad-line\ns1|@2|%2|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert_eq!(resizes.len(), 1);
        assert_eq!(resizes[0], ("%1".to_string(), 50));
    }

    #[test]
    fn sync_resizes_ignores_non_sidebar_role() {
        let output = "s1|@1|%1|35|sidebar\ns1|@2|%2|50|main\ns1|@3|%3|60|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert_eq!(resizes.len(), 1);
        assert_eq!(resizes[0], ("%1".to_string(), 60));
    }

    // ─── CurrentWindow source ──────────────────────────────────────────

    #[test]
    fn sync_current_window_uses_own_sidebar_as_reference() {
        // Current window @1 has sidebar at 50 → propagate 50 to @2
        let output = "s1|@1|%1|50|sidebar\ns1|@2|%2|35|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::CurrentWindow);
        assert_eq!(resizes.len(), 1);
        assert_eq!(resizes[0], ("%2".to_string(), 50));
    }

    #[test]
    fn sync_current_window_no_sidebar_returns_empty() {
        // Current window @1 has no sidebar, others have 2+ with different widths —
        // no fallback, no-op because there is no valid "current window width"
        let output = "s1|@2|%2|35|sidebar\ns1|@3|%3|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::CurrentWindow);
        assert!(
            resizes.is_empty(),
            "current window has no sidebar, nothing to propagate"
        );
    }

    #[test]
    fn sync_current_window_propagates_to_multiple_others() {
        // @1 (60) is reference, @2 (35) and @3 (50) both need resizing
        let output = "s1|@1|%1|60|sidebar\ns1|@2|%2|35|sidebar\ns1|@3|%3|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::CurrentWindow);
        assert_eq!(resizes.len(), 2);
        assert!(resizes.contains(&("%2".to_string(), 60)));
        assert!(resizes.contains(&("%3".to_string(), 60)));
    }

    #[test]
    fn sync_other_window_propagates_to_all_mismatched() {
        // @1 current, first other @2 (35) = reference, both @1 (60) and @3 (50) mismatch
        let output = "s1|@1|%1|60|sidebar\ns1|@2|%2|35|sidebar\ns1|@3|%3|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert_eq!(resizes.len(), 2);
        assert!(resizes.contains(&("%1".to_string(), 35)));
        assert!(resizes.contains(&("%3".to_string(), 35)));
    }

    #[test]
    fn sync_ignores_panes_from_other_session() {
        // s2's sidebar should be ignored — only s1 panes considered
        let output = "s1|@1|%1|35|sidebar\ns2|@1|%2|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert!(resizes.is_empty(), "only one sidebar in session s1");
    }

    #[test]
    fn sync_ignores_panes_from_other_session_cross_session() {
        // s1: @1 sidebar 35, s2: @2 sidebar 50. Only s1 counted → alone → empty
        let output = "s1|@1|%1|35|sidebar\ns2|@2|%2|50|sidebar";
        let resizes = compute_sync_resizes("@1", "s1", output, ReferenceSource::OtherWindow);
        assert!(resizes.is_empty(), "only one sidebar in session s1");
    }
}
