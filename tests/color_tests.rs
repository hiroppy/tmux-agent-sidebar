#[allow(dead_code, unused_imports)]
mod test_helpers;

use ratatui::style::Color;
use test_helpers::*;
use tmux_agent_sidebar::activity::{ActivityEntry, TaskProgress, TaskStatus};
use tmux_agent_sidebar::state::{BottomTab, Focus};
use tmux_agent_sidebar::tmux::{AgentType, PaneStatus, PermissionMode, SessionInfo, WindowInfo};
use tmux_agent_sidebar::ui::colors::ColorTheme;

// ─── ColorTheme Default Values ──────────────────────────────────────

#[test]
fn test_all_color_theme_defaults() {
    let theme = ColorTheme::default();

    // Core UI colors
    assert_eq!(theme.accent, Color::Indexed(153));
    assert_eq!(theme.border_inactive, Color::Indexed(240));
    assert_eq!(theme.selection_bg, Color::Indexed(237));

    // Status colors
    assert_eq!(theme.status_all, Color::Indexed(111));
    assert_eq!(theme.status_running, Color::Indexed(114));
    assert_eq!(theme.status_waiting, Color::Indexed(221));
    assert_eq!(theme.status_idle, Color::Indexed(110));
    assert_eq!(theme.status_error, Color::Indexed(203));
    assert_eq!(theme.status_unknown, Color::Indexed(244));

    // Agent colors
    assert_eq!(theme.agent_claude, Color::Indexed(174));
    assert_eq!(theme.agent_codex, Color::Indexed(141));

    // Text colors
    assert_eq!(theme.text_active, Color::Indexed(255));
    assert_eq!(theme.text_muted, Color::Indexed(252));
    assert_eq!(theme.text_inactive, Color::Indexed(244));

    // Header/UI element colors
    assert_eq!(theme.session_header, Color::Indexed(39));
    assert_eq!(theme.port, Color::Indexed(246));
    assert_eq!(theme.wait_reason, Color::Indexed(221));
    assert_eq!(theme.branch, Color::Indexed(109));

    // New theme fields
    assert_eq!(theme.badge_danger, Color::Indexed(203));
    assert_eq!(theme.badge_auto, Color::Indexed(221));
    assert_eq!(theme.task_progress, Color::Indexed(223));
    assert_eq!(theme.subagent, Color::Indexed(73));
    assert_eq!(theme.commit_hash, Color::Indexed(221));
    assert_eq!(theme.diff_added, Color::Indexed(114));
    assert_eq!(theme.diff_deleted, Color::Indexed(174));
    assert_eq!(theme.file_change, Color::Indexed(221));
    assert_eq!(theme.pr_link, Color::Indexed(117));
}

// ─── status_color() for all PaneStatus variants ─────────────────────

#[test]
fn test_status_color_all_variants() {
    let theme = ColorTheme::default();

    assert_eq!(
        theme.status_color(&PaneStatus::Running, false),
        Color::Indexed(114)
    );
    assert_eq!(
        theme.status_color(&PaneStatus::Waiting, false),
        Color::Indexed(221)
    );
    assert_eq!(
        theme.status_color(&PaneStatus::Idle, false),
        Color::Indexed(110)
    );
    assert_eq!(
        theme.status_color(&PaneStatus::Error, false),
        Color::Indexed(203)
    );
    assert_eq!(
        theme.status_color(&PaneStatus::Unknown, false),
        Color::Indexed(244)
    );
}

#[test]
fn test_status_color_attention_overrides_all() {
    let theme = ColorTheme::default();

    // attention=true should always return status_waiting regardless of status
    for status in &[
        PaneStatus::Running,
        PaneStatus::Waiting,
        PaneStatus::Idle,
        PaneStatus::Error,
        PaneStatus::Unknown,
    ] {
        assert_eq!(
            theme.status_color(status, true),
            theme.status_waiting,
            "attention=true should override {:?} to waiting color",
            status
        );
    }
}

// ─── agent_color() for all AgentType variants ───────────────────────

#[test]
fn test_agent_color_all_variants() {
    let theme = ColorTheme::default();

    assert_eq!(theme.agent_color(&AgentType::Claude), Color::Indexed(174));
    assert_eq!(theme.agent_color(&AgentType::Codex), Color::Indexed(141));
    assert_eq!(theme.agent_color(&AgentType::Unknown), theme.status_unknown);
}

// ─── PermissionMode badge colors ────────────────────────────────────

#[test]
fn test_permission_mode_bypass_all_renders_danger_color() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.permission_mode = PermissionMode::BypassPermissions;

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
    state.sidebar_focused = false;

    let output = render_to_styled_string(&mut state, 28, 26);
    // BypassAll badge uses badge_danger (203)
    assert!(
        output.contains("fg:203"),
        "BypassAll badge should use badge_danger color (203)"
    );
    // Badge text "!" should appear
    let plain = render_to_string(&mut state, 28, 26);
    assert!(plain.contains("!"), "BypassAll should show ! badge");
}

#[test]
fn test_permission_mode_full_auto_renders_auto_color() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.permission_mode = PermissionMode::Auto;

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
    state.sidebar_focused = false;

    let output = render_to_styled_string(&mut state, 28, 26);
    // Auto badge uses badge_auto (221)
    assert!(
        output.contains("fg:221"),
        "Auto badge should use badge_auto color (221)"
    );
    // Badge text "auto" should appear
    let plain = render_to_string(&mut state, 28, 26);
    assert!(plain.contains("auto"), "Auto should show auto badge");
}

#[test]
fn test_permission_mode_normal_no_badge() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    // permission_mode is Normal by default in make_pane

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
    state.sidebar_focused = false;

    let plain = render_to_string(&mut state, 28, 25);
    // Normal mode should not show any badge (no "!" in status line)
    // Check that the agent line doesn't contain "!" right after the agent name
    // The agent line is like "● claude   2m5s" — no "!" badge
    assert!(
        !plain
            .lines()
            .any(|l| { l.contains("claude") && l.contains("!") }),
        "Normal mode should not show badge"
    );
}

// ─── Activity tool_color_index all branches ─────────────────────────

#[test]
fn test_tool_color_all_branches() {
    let cases = vec![
        ("Edit", 180),
        ("Write", 180),
        ("Bash", 114),
        ("Read", 110),
        ("Glob", 110),
        ("Grep", 110),
        ("Agent", 181),
        ("UnknownTool", 244),
        ("", 244),
    ];
    for (tool, expected) in cases {
        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: tool.into(),
            label: "test".into(),
        };
        assert_eq!(
            entry.tool_color_index(),
            expected,
            "tool_color_index for '{}'",
            tool
        );
    }
}

// ─── Git status summary colors ──────────────────────────────────────

#[test]
fn test_git_summary_modified_uses_badge_auto_color() {
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

    state.bottom_tab = BottomTab::GitStatus;
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "src/lib.rs".into(),
        additions: 5,
        deletions: 2,
    }];

    let styled = render_to_styled_string(&mut state, 28, 25);
    // Modified count uses badge_auto (221)
    assert!(
        styled.contains("fg:221"),
        "Modified summary should use badge_auto color (221)"
    );
}

// ─── Render Tests: verify correct colors in styled output ───────────

#[test]
fn test_task_progress_line_uses_task_progress_color() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.pane_id = "%1".into();

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
    state.sidebar_focused = false;

    // Set task progress for pane %1
    let progress = TaskProgress {
        tasks: vec![
            ("Task A".into(), TaskStatus::Completed),
            ("Task B".into(), TaskStatus::InProgress),
            ("Task C".into(), TaskStatus::Pending),
        ],
    };
    state.set_pane_task_progress("%1", Some(progress));

    let styled = render_to_styled_string(&mut state, 40, 40);
    // task_progress color is 223
    assert!(
        styled.contains("fg:223"),
        "Task progress line should use task_progress color (223)"
    );

    let plain = render_to_string(&mut state, 40, 40);
    // Should show progress icons and count
    assert!(plain.contains("✔"), "Should show completed task icon");
    assert!(plain.contains("◼"), "Should show in-progress task icon");
    assert!(plain.contains("◻"), "Should show pending task icon");
    assert!(plain.contains("1/3"), "Should show completed/total count");
}

#[test]
fn test_subagent_line_uses_subagent_color() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.subagents = vec!["Explore #1".into()];

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
    state.sidebar_focused = false;

    let styled = render_to_styled_string(&mut state, 40, 27);
    // subagent color is 73
    assert!(
        styled.contains("fg:73"),
        "Subagent line should use subagent color (73)"
    );

    let plain = render_to_string(&mut state, 40, 27);
    assert!(plain.contains("Explore #1"), "Subagent name should appear");
}

#[test]
fn test_response_arrow_uses_response_arrow_color() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.pane_active = false;
    pane.prompt = "Task completed successfully".into();
    pane.prompt_is_response = true;

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
    state.sidebar_focused = false;

    let styled = render_to_styled_string(&mut state, 40, 27);
    // Response arrow (▶) uses theme.response_arrow (74) and bold.
    assert!(
        styled.contains("fg:74"),
        "Response arrow should use response_arrow color (74)"
    );
    assert!(styled.contains("bold"), "Response arrow should be bold");
    // The response text itself uses text_active (255) for the focused pane
    assert!(
        styled.contains("fg:255"),
        "Response text should use text_active color (255) for focused pane"
    );

    let plain = render_to_string(&mut state, 40, 27);
    assert!(plain.contains("▶"), "Response should show ▶ arrow");
}

// test_commit_hash_uses_commit_hash_color was removed because
// git_last_commit and commit hash rendering no longer exist.

#[test]
fn test_pr_link_uses_pr_link_color() {
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

    state.bottom_tab = BottomTab::GitStatus;
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.git.branch = "feature/test".into();
    state.git.pr_number = Some("99".into());
    state.git.remote_url = "https://github.com/user/repo".into();

    let styled = render_to_styled_string(&mut state, 28, 40);
    // pr_link color is 117 (pale blue)
    assert!(
        styled.contains("fg:117"),
        "PR link should use pr_link color (117)"
    );
    assert!(styled.contains("underline"), "PR link should be underlined");

    let plain = render_to_string(&mut state, 28, 25);
    assert!(plain.contains("#99"), "PR number should appear");
}

#[test]
fn test_diff_stat_added_uses_diff_added_color() {
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

    state.bottom_tab = BottomTab::GitStatus;
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.diff_stat = Some((42, 10));

    let styled = render_to_styled_string(&mut state, 28, 40);
    // diff_added color is 114
    assert!(
        styled.contains("fg:114"),
        "Diff +additions should use diff_added color (114)"
    );

    let plain = render_to_string(&mut state, 28, 25);
    assert!(plain.contains("+42"), "Insertions count should appear");
}

#[test]
fn test_diff_stat_deleted_uses_diff_deleted_color() {
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

    state.bottom_tab = BottomTab::GitStatus;
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.diff_stat = Some((0, 25));

    let styled = render_to_styled_string(&mut state, 28, 25);
    // diff_deleted color is 174
    assert!(
        styled.contains("fg:174"),
        "Diff -deletions should use diff_deleted color (174)"
    );

    let plain = render_to_string(&mut state, 28, 25);
    assert!(plain.contains("-25"), "Deletions count should appear");
}

#[test]
fn test_file_change_stat_uses_file_change_color() {
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

    state.bottom_tab = BottomTab::GitStatus;
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "lib.rs".into(),
        additions: 40,
        deletions: 10,
    }];

    let styled = render_to_styled_string(&mut state, 28, 25);
    // Modified status uses badge_auto (221)
    assert!(
        styled.contains("fg:221"),
        "Modified file status should use badge_auto color (221)"
    );

    let plain = render_to_string(&mut state, 28, 25);
    assert!(plain.contains("lib.rs"), "Filename should appear");
}

// ─── Custom theme overrides for new fields ──────────────────────────

#[test]
fn test_custom_theme_new_fields_override() {
    let theme = ColorTheme {
        badge_danger: Color::Indexed(196),
        badge_auto: Color::Indexed(226),
        task_progress: Color::Indexed(200),
        subagent: Color::Indexed(100),
        commit_hash: Color::Indexed(150),
        diff_added: Color::Indexed(46),
        diff_deleted: Color::Indexed(160),
        file_change: Color::Indexed(208),
        pr_link: Color::Indexed(33),
        port: Color::Indexed(82),
        ..ColorTheme::default()
    };

    assert_eq!(theme.badge_danger, Color::Indexed(196));
    assert_eq!(theme.badge_auto, Color::Indexed(226));
    assert_eq!(theme.task_progress, Color::Indexed(200));
    assert_eq!(theme.subagent, Color::Indexed(100));
    assert_eq!(theme.commit_hash, Color::Indexed(150));
    assert_eq!(theme.diff_added, Color::Indexed(46));
    assert_eq!(theme.diff_deleted, Color::Indexed(160));
    assert_eq!(theme.file_change, Color::Indexed(208));
    assert_eq!(theme.pr_link, Color::Indexed(33));
    assert_eq!(theme.port, Color::Indexed(82));

    // Original fields should still be default
    assert_eq!(theme.accent, Color::Indexed(153));
    assert_eq!(theme.agent_claude, Color::Indexed(174));
}

// ─── Branch color in styled output ──────────────────────────────────

#[test]
fn test_branch_color_in_agent_panel() {
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
    state.repo_groups = vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(
            pane,
            tmux_agent_sidebar::group::PaneGitInfo {
                repo_root: Some("/home/user/project".into()),
                branch: Some("feature/cool-feature".into()),
                is_worktree: false,
                worktree_name: None,
            },
        )],
    }];
    state.rebuild_row_targets();
    state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    let styled = render_to_styled_string(&mut state, 40, 26);
    // branch color is 109
    assert!(
        styled.contains("fg:109"),
        "Branch name should use branch color (109)"
    );

    let plain = render_to_string(&mut state, 40, 26);
    assert!(
        plain.contains("feature/cool-feature") || plain.contains("feature/cool-feat…"),
        "Branch name should appear in output"
    );
}

// ─── Selection background color ─────────────────────────────────────

#[test]
fn test_selection_bg_color_applied() {
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

    let styled = render_to_styled_string(&mut state, 28, 25);
    // selection_bg is 237
    assert!(
        styled.contains("bg:237"),
        "Selected row should use selection_bg color (237)"
    );
}

// ─── Accent (focused) vs border_inactive colors ─────────────────────

#[test]
fn test_accent_vs_border_inactive_colors() {
    let mut pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    pane1.pane_id = "%1".into();
    let mut pane2 = make_pane(AgentType::Codex, PaneStatus::Idle);
    pane2.pane_id = "%2".into();

    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@0".into(),
            window_name: "fish".into(),
            window_active: true,
            auto_rename: true,
            panes: vec![pane1.clone(), pane2.clone()],
        }],
    }]);
    state.repo_groups = vec![
        tmux_agent_sidebar::group::RepoGroup {
            name: "focused-repo".into(),
            has_focus: true,
            panes: vec![(pane1, tmux_agent_sidebar::group::PaneGitInfo::default())],
        },
        tmux_agent_sidebar::group::RepoGroup {
            name: "unfocused-repo".into(),
            has_focus: false,
            panes: vec![(pane2, tmux_agent_sidebar::group::PaneGitInfo::default())],
        },
    ];
    state.focused_pane_id = Some("%1".into());
    state.rebuild_row_targets();

    let styled = render_to_styled_string(&mut state, 28, 30);
    // Should contain both accent (153) and border_inactive (240) colors
    assert!(
        styled.contains("fg:153"),
        "Focused group should use accent color (153)"
    );
    assert!(
        styled.contains("fg:240"),
        "Unfocused group should use border_inactive color (240)"
    );
}

// ─── Status color rendering for each PaneStatus ─────────────────────

#[test]
fn test_running_status_color_in_output() {
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
    state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    let styled = render_to_styled_string(&mut state, 28, 25);
    // Spinner pulse color at frame 0 is 82 (from SPINNER_PULSE[0])
    assert!(
        styled.contains("fg:82"),
        "Running spinner should use SPINNER_PULSE[0] color (82)"
    );
}

#[test]
fn test_waiting_status_color_in_output() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Waiting);
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
    state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    let styled = render_to_styled_string(&mut state, 28, 25);
    // Waiting status color is 221
    assert!(
        styled.contains("fg:221"),
        "Waiting status should use status_waiting color (221)"
    );
}

#[test]
fn test_error_status_color_in_output() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Error);
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
    state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    let styled = render_to_styled_string(&mut state, 28, 25);
    // Error status color is 203
    assert!(
        styled.contains("fg:203"),
        "Error status should use status_error color (203)"
    );
}

#[test]
fn test_idle_status_color_in_output() {
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
    state.sidebar_focused = false;

    let styled = render_to_styled_string(&mut state, 28, 25);
    // Idle status color is 110
    assert!(
        styled.contains("fg:110"),
        "Idle status should use status_idle color (110)"
    );
}

#[test]
fn test_unknown_status_color_in_output() {
    let theme = tmux_agent_sidebar::ui::colors::ColorTheme::default();
    assert_eq!(
        theme.status_color(&PaneStatus::Unknown, false),
        ratatui::style::Color::Indexed(244)
    );
}
