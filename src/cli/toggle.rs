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
        let s = tmux::display_message(window_id, "#{@sidebar_width}");
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

    // Check for existing sidebar
    let panes_output = tmux::run_tmux(&[
        "list-panes",
        "-t",
        window_id,
        "-F",
        "#{pane_id}|#{@pane_role}",
    ])
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

    // Find leftmost pane
    let leftmost_output = tmux::run_tmux(&[
        "list-panes",
        "-t",
        window_id,
        "-F",
        "#{pane_left} #{pane_id}",
    ])
    .unwrap_or_default();

    let leftmost_pane = leftmost_output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() >= 2 {
                let left: u32 = parts[0].parse().unwrap_or(u32::MAX);
                Some((left, parts[1].to_string()))
            } else {
                None
            }
        })
        .min_by_key(|(left, _)| *left)
        .map(|(_, id)| id)
        .unwrap_or_else(|| window_id.to_string());

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
        "-hfb",
        "-l",
        &sidebar_width,
        "-t",
        &leftmost_pane,
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
        tmux::set_pane_option(&sidebar_pane, "@pane_role", "sidebar");
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
    let has_sidebar = tmux::run_tmux(&["list-panes", "-a", "-F", "#{pane_id}|#{@pane_role}"])
        .map(|output| any_sidebar_pane(&output))
        .unwrap_or(false);

    if has_sidebar {
        let all_panes = tmux::run_tmux(&["list-panes", "-a", "-F", "#{pane_id}|#{@pane_role}"])
            .unwrap_or_default();
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

/// Decide whether `cmd_auto_close` should kill the window, given the raw
/// outputs of the two tmux queries it performs. Extracted as a pure
/// function so the guard logic is directly unit-testable without a
/// running tmux server.
///
/// - `list_panes_output`: `Some(stdout)` from `list-panes -F #{@pane_role}`,
///   or `None` if the tmux call failed.
/// - `session_windows`: parsed value of `#{session_windows}`, or `None`
///   if the tmux call failed or the value was unparseable.
fn should_kill_window(list_panes_output: Option<&str>, session_windows: Option<u32>) -> bool {
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

    // Guard against destroying the session. If this window is the only
    // one left in its session, killing it drops every client attached
    // to that session (e.g. multiple Ghostty tabs sharing the same
    // `main` session all close at once). Leave the window alone and
    // let the user close the sidebar manually.
    match session_windows {
        Some(n) if n > 1 => true,
        _ => false,
    }
}

pub(crate) fn cmd_auto_close(args: &[String]) -> i32 {
    let window_id = match args.first() {
        Some(id) => id.as_str(),
        None => return 0,
    };

    let list_panes_output = tmux::run_tmux(&["list-panes", "-t", window_id, "-F", "#{@pane_role}"]);

    let session_windows = tmux::run_tmux(&[
        "display-message",
        "-t",
        window_id,
        "-p",
        "#{session_windows}",
    ])
    .and_then(|s| s.trim().parse().ok());

    if should_kill_window(list_panes_output.as_deref(), session_windows) {
        let _ = tmux::run_tmux(&["kill-window", "-t", window_id]);
    }

    0
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

    // ─── should_kill_window ───────────────────────────────────────────

    #[test]
    fn should_kill_window_kills_when_only_sidebar_and_other_windows_exist() {
        // Classic intended path: sidebar alone in a window, session has
        // other windows to fall back on.
        assert!(should_kill_window(Some("sidebar"), Some(2)));
    }

    #[test]
    fn should_kill_window_skips_when_non_sidebar_pane_remains() {
        // Another pane with `@pane_role` explicitly set to something
        // non-sidebar (e.g. a spawn-marked pane) keeps the window alive.
        assert!(!should_kill_window(Some("sidebar\npane"), Some(5)));
        // `@pane_role` unset renders as an empty line — that pane is
        // a regular user pane, not a sidebar, so the window must stay.
        // The real tmux output for [sidebar pane, regular pane] is
        // "sidebar\n\n" (sidebar's role, then the regular pane's empty
        // role followed by the final record separator).
        assert!(!should_kill_window(Some("sidebar\n\n"), Some(5)));
        assert!(!should_kill_window(Some("\nsidebar\n"), Some(5)));
    }

    #[test]
    fn should_kill_window_skips_when_list_panes_failed() {
        // `list-panes` failure must never be treated as "window is empty" —
        // that used to let a busy-tmux race kill a live window.
        assert!(!should_kill_window(None, Some(5)));
    }

    #[test]
    fn should_kill_window_skips_when_list_panes_empty() {
        // Whitespace-only output (e.g. window already gone) must not
        // trigger a kill either.
        assert!(!should_kill_window(Some(""), Some(5)));
        assert!(!should_kill_window(Some("   \n"), Some(5)));
    }

    #[test]
    fn should_kill_window_skips_when_session_has_only_this_window() {
        // Core regression guard: killing the last window of a session
        // drops every attached client. With multiple Ghostty tabs
        // sharing a single `main` session this manifested as every tab
        // dying at once. Keep the sidebar stranded rather than nuke
        // the session.
        assert!(!should_kill_window(Some("sidebar"), Some(1)));
    }

    #[test]
    fn should_kill_window_skips_when_session_windows_query_failed() {
        // If we cannot prove the session has other windows, err on
        // the side of preserving the session. Better to leave a
        // lingering sidebar pane than to destroy a live workspace.
        assert!(!should_kill_window(Some("sidebar"), None));
        assert!(!should_kill_window(Some("sidebar"), Some(0)));
    }
}
