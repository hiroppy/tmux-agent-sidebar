#[allow(dead_code, unused_imports)]
mod test_helpers;

use test_helpers::*;
use tmux_agent_sidebar::activity::ActivityEntry;
use tmux_agent_sidebar::state::{BottomTab, Focus};
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
    state.global.selected_pane_row = 0;
    state.bottom_panel_height = 0;

    let output = render_to_styled_string(&mut state, 28, 10);
    // Verify the selected agent row gets the selection background style.
    assert!(
        output.lines().any(|l| l.starts_with("┃[fg:153,bg:237]")),
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
    // Focused group header should use accent (fg:153)
    assert!(
        output.contains("fg:153"),
        "focused group header should use accent (fg:153)"
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
    state.focus = Focus::Panes; // not activity
    state.sidebar_focused = true;
    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:32".into(),
        tool: "Edit".into(),
        label: "src/main.rs".into(),
    }];

    let output = render_to_styled_string(&mut state, 28, 14);
    // Bottom panel border should use border_inactive (fg:240) when unfocused
    assert!(
        output.contains("fg:240"),
        "activity unfocused border should use BORDER_INACTIVE (fg:240)"
    );
}

#[test]
fn bottom_tab_activity_uses_accent_when_selected() {
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
    state.bottom_tab = BottomTab::Activity;

    let output = render_to_styled_string(&mut state, 28, 14);
    let title_line = output
        .lines()
        .find(|line| line.contains('╭'))
        .expect("bottom title line should be present");

    assert!(
        title_line.contains("A[fg:153]"),
        "selected Activity tab should use accent color"
    );
    assert!(
        title_line.contains("G[fg:252]"),
        "unselected Git tab should remain muted"
    );
}

#[test]
fn bottom_tab_git_uses_accent_when_selected() {
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
    state.bottom_tab = BottomTab::GitStatus;

    let output = render_to_styled_string(&mut state, 28, 14);
    let title_line = output
        .lines()
        .find(|line| line.contains('╭'))
        .expect("bottom title line should be present");

    assert!(
        title_line.contains("G[fg:153]"),
        "selected Git tab should use accent color"
    );
    assert!(
        title_line.contains("A[fg:252]"),
        "unselected Activity tab should remain muted"
    );
}

// ─── Selection Background Border Tests ───────────────────────────────

#[test]
fn selection_marker_uses_accent_color_with_selection_bg() {
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
    state.focus = Focus::Panes;
    state.global.selected_pane_row = 0;

    let output = render_to_styled_string(&mut state, 28, 24);

    // Selected pane rows start with the ┃ marker styled with the
    // accent fg (117) and the selection bg (237), and NEVER contain
    // the old group frame │.
    let selected_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.starts_with("┃") && l.contains("bg:237"))
        .collect();

    assert!(
        !selected_lines.is_empty(),
        "should have at least one line with selection bg"
    );

    for line in &selected_lines {
        assert!(
            line.starts_with("┃[fg:153,bg:237]"),
            "selected row must start with the ┃ marker in accent+selection styles: {}",
            line
        );
        assert!(
            !line.contains('│'),
            "selected row should no longer carry the old frame │: {}",
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
    state.focus = Focus::Panes;
    state.global.selected_pane_row = 0;

    let output = render_to_styled_string(&mut state, 28, 24);

    let selected_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.starts_with("┃") && l.contains("bg:237"))
        .collect();

    for line in &selected_lines {
        // The space right after the left │ should have bg:237
        // Pattern: │[fg:153] [bg:237]
        assert!(
            line.contains(" [bg:237]"),
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
        !output
            .lines()
            .any(|l| l.starts_with("┃") && l.contains("bg:237")),
        "should not have selection bg on agent rows when sidebar is not focused"
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
        accent: Color::Indexed(196),       // red accent
        agent_claude: Color::Indexed(226), // yellow agent
        status_idle: Color::Indexed(46),   // green idle
        port: Color::Indexed(39),          // cyan port
        ..ColorTheme::default()
    };
    // Unfocus sidebar so selected row doesn't use REVERSED (which hides colors)
    state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    let output = render_to_styled_string(&mut state, 28, 10);

    // Verify custom colors are applied
    assert!(
        output.contains("fg:196"),
        "custom accent (196) should be used"
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

    // Verify defaults match shell version's agent-sidebar.conf
    assert_eq!(theme.accent, Color::Indexed(153));
    assert_eq!(theme.border_inactive, Color::Indexed(240));
    assert_eq!(theme.status_running, Color::Indexed(114));
    assert_eq!(theme.status_waiting, Color::Indexed(221));
    assert_eq!(theme.status_idle, Color::Indexed(110));
    assert_eq!(theme.status_error, Color::Indexed(203));
    assert_eq!(theme.agent_claude, Color::Indexed(174));
    assert_eq!(theme.agent_codex, Color::Indexed(141));
    assert_eq!(theme.text_active, Color::Indexed(255));
    assert_eq!(theme.text_muted, Color::Indexed(252));
    assert_eq!(theme.session_header, Color::Indexed(39));
    assert_eq!(theme.wait_reason, Color::Indexed(221));
}
