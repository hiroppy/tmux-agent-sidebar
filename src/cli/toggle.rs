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

pub(crate) fn cmd_auto_close(args: &[String]) -> i32 {
    let window_id = match args.first() {
        Some(id) => id.as_str(),
        None => return 0,
    };

    let output =
        tmux::run_tmux(&["list-panes", "-t", window_id, "-F", "#{@pane_role}"]).unwrap_or_default();

    let non_sidebar = output.lines().filter(|line| *line != "sidebar").count();

    if non_sidebar == 0 {
        let _ = tmux::run_tmux(&["kill-window", "-t", window_id]);
    }

    0
}
