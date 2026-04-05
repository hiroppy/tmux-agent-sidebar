#[allow(dead_code, unused_imports)]
mod test_helpers;

use test_helpers::*;
use tmux_agent_sidebar::activity::ActivityEntry;
use tmux_agent_sidebar::state::Focus;
use tmux_agent_sidebar::tmux::{AgentType, PaneStatus, SessionInfo, WindowInfo};

// ─── Styled Snapshot Tests for Selection and Focus ─────────────────

#[test]
fn snapshot_selected_focused_styled() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();
    state.sidebar_focused = true;
    state.selected_agent_row = 0;

    let output = render_to_styled_string(&mut state, 28, 24);
    // Verify output contains selection background style with selection_bg color (236)
    assert!(
        output.contains("bg:239"),
        "selected focused row should have selection background color"
    );
}

#[test]
fn snapshot_activity_focused_styled() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:32".into(),
        tool: "Edit".into(),
        label: "src/main.rs".into(),
    }];

    let output = render_to_styled_string(&mut state, 28, 14);
    // Activity border should contain "fg:117" (BORDER_ACTIVE)
    assert!(
        output.contains("fg:117"),
        "activity focused border should use BORDER_ACTIVE (fg:117)"
    );
}

#[test]
fn snapshot_activity_unfocused_styled() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();
    state.focus = Focus::Agents; // not activity
    state.sidebar_focused = true;
    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:32".into(),
        tool: "Edit".into(),
        label: "src/main.rs".into(),
    }];

    let output = render_to_styled_string(&mut state, 28, 14);
    // Bottom panel border uses border_active (fg:117) regardless of focus
    assert!(
        output.contains("fg:117"),
        "activity unfocused border should use BORDER_ACTIVE (fg:117)"
    );
}

// ─── Selection Background Border Tests ───────────────────────────────

#[test]
fn selection_bg_does_not_bleed_into_border() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();
    state.sidebar_focused = true;
    state.focus = Focus::Agents;
    state.selected_agent_row = 0;

    let output = render_to_styled_string(&mut state, 28, 24);

    // Find content lines with selection bg (bg:239)
    let selected_lines: Vec<&str> = output.lines().filter(|l| l.contains("bg:239")).collect();

    assert!(
        !selected_lines.is_empty(),
        "should have at least one line with selection bg"
    );

    for line in &selected_lines {
        // Left border: "│" should NOT have bg:239
        // The line starts with │[fg:...] (border style, no bg),
        // followed by a space with bg:239
        assert!(
            !line.starts_with("│[fg:117,bg:239]"),
            "left border │ should not have selection bg: {}",
            line
        );

        // Right border: last │ should NOT have bg:239
        // Find the last │ in the line and check it doesn't have bg
        let last_border = line.rfind('│').expect("should have right border");
        let after_border = &line[last_border..];
        assert!(
            !after_border.contains("bg:239"),
            "right border │ should not have selection bg: {}",
            line
        );
    }
}

#[test]
fn selection_bg_covers_inner_padding() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();
    state.sidebar_focused = true;
    state.focus = Focus::Agents;
    state.selected_agent_row = 0;

    let output = render_to_styled_string(&mut state, 28, 24);

    let selected_lines: Vec<&str> = output.lines().filter(|l| l.contains("bg:239")).collect();

    for line in &selected_lines {
        // The space right after the left │ should have bg:239
        // Pattern: │[fg:117] [bg:239]
        assert!(
            line.contains(" [bg:239]"),
            "inner space should have selection bg: {}",
            line
        );
    }
}

#[test]
fn no_selection_bg_when_not_selected() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();
    state.sidebar_focused = false; // not focused → no selection

    let output = render_to_styled_string(&mut state, 28, 24);

    assert!(
        !output.contains("bg:239"),
        "should not have selection bg when sidebar is not focused"
    );
}

// ─── Custom Theme Tests ─────────────────────────────────────────────

#[test]
fn snapshot_custom_theme_colors() {
    use ratatui::style::Color;
    use tmux_agent_sidebar::ui::colors::ColorTheme;

    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();

    // Override theme with custom colors
    state.theme = ColorTheme {
        border_active: Color::Indexed(196), // red border
        agent_claude: Color::Indexed(226),  // yellow agent
        status_idle: Color::Indexed(46),    // green idle
        ..ColorTheme::default()
    };
    // Unfocus sidebar so selected row doesn't use REVERSED (which hides colors)
    state.sidebar_focused = false;

    let output = render_to_styled_string(&mut state, 28, 24);

    // Verify custom colors are applied
    assert!(
        output.contains("fg:196"),
        "custom border_active (196) should be used"
    );
    assert!(
        output.contains("fg:226"),
        "custom agent_claude (226) should be used"
    );
    assert!(
        output.contains("fg:46"),
        "custom status_idle (46) should be used"
    );
}

#[test]
fn test_theme_default_matches_shell_colors() {
    use ratatui::style::Color;
    use tmux_agent_sidebar::ui::colors::ColorTheme;

    let theme = ColorTheme::default();

    // Verify defaults match shell版's agent-sidebar.conf
    assert_eq!(theme.border_active, Color::Indexed(117));
    assert_eq!(theme.border_inactive, Color::Indexed(240));
    assert_eq!(theme.status_running, Color::Indexed(114));
    assert_eq!(theme.status_waiting, Color::Indexed(221));
    assert_eq!(theme.status_idle, Color::Indexed(109));
    assert_eq!(theme.status_error, Color::Indexed(203));
    assert_eq!(theme.agent_claude, Color::Indexed(174));
    assert_eq!(theme.agent_codex, Color::Indexed(141));
    assert_eq!(theme.text_active, Color::Indexed(255));
    assert_eq!(theme.text_muted, Color::Indexed(244));
    assert_eq!(theme.session_header, Color::Indexed(39));
    assert_eq!(theme.wait_reason, Color::Indexed(221));
    assert_eq!(theme.activity_border, Color::Indexed(39));
}
