#[allow(dead_code, unused_imports)]
mod test_helpers;

use test_helpers::*;
use tmux_agent_sidebar::activity::ActivityEntry;
use tmux_agent_sidebar::group::{PaneGitInfo, RepoGroup};
use tmux_agent_sidebar::state::{AppState, BottomTab, Focus, RowTarget};
use tmux_agent_sidebar::tmux::{AgentType, PaneInfo, PaneStatus, SessionInfo, WindowInfo};

// ─── State Transition Tests ────────────────────────────────────────

#[test]
fn test_move_agent_selection_bounds() {
    let mut state = make_state(vec![]);
    state.agent_row_targets = vec![
        RowTarget {
            pane_id: "%1".into(),
        },
        RowTarget {
            pane_id: "%2".into(),
        },
    ];
    state.selected_agent_row = 0;
    state.move_agent_selection(1);
    assert_eq!(state.selected_agent_row, 1);
    state.move_agent_selection(1); // should not go past end
    assert_eq!(state.selected_agent_row, 1);
    state.move_agent_selection(-1);
    assert_eq!(state.selected_agent_row, 0);
    state.move_agent_selection(-1); // should not go below 0
    assert_eq!(state.selected_agent_row, 0);
}

#[test]
fn test_move_agent_selection_empty() {
    let mut state = make_state(vec![]);
    state.move_agent_selection(1);
    assert_eq!(state.selected_agent_row, 0);
}

#[test]
fn test_scroll_activity_bounds() {
    let mut state = make_state(vec![]);
    state.activity_entries = vec![
        ActivityEntry {
            timestamp: "10:00".into(),
            tool: "Read".into(),
            label: "a".into(),
        },
        ActivityEntry {
            timestamp: "10:01".into(),
            tool: "Edit".into(),
            label: "b".into(),
        },
        ActivityEntry {
            timestamp: "10:02".into(),
            tool: "Bash".into(),
            label: "c".into(),
        },
    ];
    state.activity_scroll.total_lines = 6;
    state.activity_scroll.visible_height = 4;
    state.activity_scroll.scroll(1);
    assert_eq!(state.activity_scroll.offset, 1);
    state.activity_scroll.scroll(5);
    assert_eq!(state.activity_scroll.offset, 2); // clamped to 6-4=2
    state.activity_scroll.scroll(-10);
    assert_eq!(state.activity_scroll.offset, 0);
}

// ─── line_to_row Mapping Tests ─────────────────────────────────────

#[test]
fn test_line_to_row_single_agent() {
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
    let _ = render_to_styled_string(&mut state, 28, 10);
    assert_eq!(state.line_to_row[0], None); // box top
    assert_eq!(state.line_to_row[1], Some(0)); // agent status
    assert_eq!(state.line_to_row[2], Some(0)); // idle hint
    assert_eq!(state.line_to_row[3], None); // box bottom
}

#[test]
fn test_line_to_row_two_agents() {
    let pane1 = PaneInfo {
        pane_id: "%1".into(),
        pane_active: true,
        status: PaneStatus::Running,
        attention: false,
        agent: AgentType::Claude,
        path: "/home/user/project".into(),
        prompt: String::new(),
        prompt_is_response: false,
        started_at: None,
        wait_reason: String::new(),
        permission_mode: tmux_agent_sidebar::tmux::PermissionMode::Default,
        subagents: vec![],
        pane_pid: None,
    };
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        attention: false,
        agent: AgentType::Codex,
        path: "/home/user/project".into(),
        prompt: String::new(),
        prompt_is_response: false,
        started_at: None,
        wait_reason: String::new(),
        permission_mode: tmux_agent_sidebar::tmux::PermissionMode::Default,
        subagents: vec![],
        pane_pid: None,
    };

    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane1.clone(), pane2.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane1, pane2])];
    state.rebuild_row_targets();
    let _ = render_to_styled_string(&mut state, 28, 10);
    // box_top=None, agent1=Some(0), separator=None, agent2 status+hint, box_bottom=None
    assert_eq!(state.line_to_row[0], None); // box top
    assert_eq!(state.line_to_row[1], Some(0)); // agent 1
    assert_eq!(state.line_to_row[2], None); // separator
    assert_eq!(state.line_to_row[3], Some(1)); // agent 2 status line
    assert_eq!(state.line_to_row[4], Some(1)); // agent 2 idle hint
    assert_eq!(state.line_to_row[5], None); // box bottom
}

#[test]
fn test_line_to_row_with_prompt() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.prompt = "hello".into();

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
    let _ = render_to_styled_string(&mut state, 28, 10);
    // box_top=None, status=Some(0), prompt=Some(0), box_bottom=None
    assert_eq!(state.line_to_row[0], None); // box top
    assert_eq!(state.line_to_row[1], Some(0)); // agent status line
    assert_eq!(state.line_to_row[2], Some(0)); // prompt line
    assert_eq!(state.line_to_row[3], None); // box bottom
}

// ─── Coverage Gap Tests ─────────────────────────────────────────────

#[test]
fn snapshot_agent_with_attention_styled() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.attention = true;

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
    state.sidebar_focused = false; // unfocused so colors show, not REVERSED

    let output = render_to_styled_string(&mut state, 28, 24);
    // attention=true on idle pane should use waiting color (221), not idle color (250)
    assert!(
        output.contains("fg:221"),
        "attention pane should use waiting color"
    );
}

#[test]
fn test_rebuild_row_targets_clamps_selection() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut p2 = pane.clone();
    p2.pane_id = "%2".into();
    let mut state = make_state(vec![]);
    state.repo_groups = vec![RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![
            (pane.clone(), PaneGitInfo::default()),
            (p2.clone(), PaneGitInfo::default()),
        ],
    }];
    state.selected_agent_row = 1; // select second agent

    // Trigger rebuild
    state.rebuild_row_targets();
    assert_eq!(state.agent_row_targets.len(), 2);

    // Now shrink to 1 agent
    state.repo_groups[0].panes.pop();
    state.selected_agent_row = 1; // still pointing at index 1
    state.rebuild_row_targets();
    // Should be clamped to 0
    assert_eq!(state.selected_agent_row, 0);
}

// find_focused_pane now queries tmux directly, so it can't be tested
// without a tmux session. The underlying logic (pick_active_pane) is
// tested via unit tests in tmux.rs. focused_pane_id is pub, so tests
// can set it directly.

#[test]
fn test_scroll_git_empty_is_noop() {
    let mut state = make_state(vec![]);
    state.git_scroll.offset = 0;
    state.bottom_tab = BottomTab::GitStatus;
    state.scroll_bottom(5);
    assert_eq!(
        state.git_scroll.offset, 0,
        "scrolling empty git should be no-op"
    );
}

// ─── State: scroll_git Tests ────────────────────────────────────────

#[test]
fn test_scroll_git_bounds() {
    let mut state = make_state(vec![]);
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "file.rs".into(),
        additions: 0,
        deletions: 0,
    }];
    state.git_scroll.total_lines = 8;
    state.git_scroll.visible_height = 3;
    state.git_scroll.offset = 0;

    state.git_scroll.scroll(2);
    assert_eq!(state.git_scroll.offset, 2);

    // Clamp to max (8 - 3 = 5)
    state.git_scroll.scroll(10);
    assert_eq!(state.git_scroll.offset, 5);

    // Clamp to 0
    state.git_scroll.scroll(-100);
    assert_eq!(state.git_scroll.offset, 0);
}

// ─── State: apply_git_data Tests ────────────────────────────────────

#[test]
fn test_apply_git_data() {
    use tmux_agent_sidebar::git::{GitData, GitFileEntry};

    let mut state = make_state(vec![]);
    let data = GitData {
        diff_stat: Some((10, 5)),
        branch: "feature/test".into(),
        ahead_behind: Some((2, 1)),
        staged_files: vec![GitFileEntry {
            status: 'M',
            name: "src/lib.rs".into(),
            additions: 10,
            deletions: 5,
        }],
        unstaged_files: vec![],
        untracked_files: vec![],
        remote_url: "https://github.com/user/repo".into(),
        pr_number: Some("42".into()),
    };

    state.apply_git_data(data);

    assert_eq!(state.git.staged_files.len(), 1);
    assert_eq!(state.git.staged_files[0].status, 'M');
    assert_eq!(state.git.staged_files[0].name, "src/lib.rs");
    assert!(state.git.unstaged_files.is_empty());
    assert!(state.git.untracked_files.is_empty());
    assert_eq!(state.git.changed_file_count(), 1);
    assert_eq!(state.git.diff_stat, Some((10, 5)));
    assert_eq!(state.git.branch, "feature/test");
    assert_eq!(state.git.ahead_behind, Some((2, 1)));
    assert_eq!(state.git.remote_url, "https://github.com/user/repo");
    assert_eq!(state.git.pr_number, Some("42".into()));
}

// ─── State: new Tests ───────────────────────────────────────────────

#[test]
fn test_state_new_defaults() {
    let state = AppState::new("%99".into());
    assert_eq!(state.now, 0);
    assert_eq!(state.tmux_pane, "%99");
    assert!(state.sessions.is_empty());
    assert!(!state.sidebar_focused);
    assert_eq!(state.focus, Focus::Agents);
    assert_eq!(state.spinner_frame, 0);
    assert_eq!(state.selected_agent_row, 0);
    assert!(state.agent_row_targets.is_empty());
    assert!(state.activity_entries.is_empty());
    assert_eq!(state.activity_scroll.offset, 0);
    assert_eq!(state.activity_max_entries, 50);
    assert_eq!(state.agents_scroll.offset, 0);
    assert_eq!(state.agents_scroll.total_lines, 0);
    assert_eq!(state.agents_scroll.visible_height, 0);
    assert_eq!(state.bottom_tab, BottomTab::Activity);
    assert!(state.git.branch.is_empty());
    assert_eq!(state.git_scroll.offset, 0);
    assert!(state.git.pr_number.is_none());
}

// ─── State: move_agent_selection return value Tests ─────────────────

#[test]
fn test_move_agent_selection_return_value() {
    let mut state = make_state(vec![]);
    state.agent_row_targets = vec![
        RowTarget {
            pane_id: "%1".into(),
        },
        RowTarget {
            pane_id: "%2".into(),
        },
    ];
    state.selected_agent_row = 0;

    assert!(
        state.move_agent_selection(1),
        "should return true when moved"
    );
    assert!(
        !state.move_agent_selection(1),
        "should return false at boundary"
    );
    assert!(
        state.move_agent_selection(-1),
        "should return true when moved back"
    );
    assert!(
        !state.move_agent_selection(-1),
        "should return false at start"
    );
}

// find_focused_pane edge case tests were removed because the function now
// queries tmux directly. See tmux::find_active_pane tests instead.

// ─── State: scroll_bottom dispatch Tests ────────────────────────────

#[test]
fn test_scroll_bottom_dispatches_to_git() {
    let mut state = make_state(vec![]);
    state.bottom_tab = BottomTab::GitStatus;
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "file.rs".into(),
        additions: 0,
        deletions: 0,
    }];
    state.git_scroll.total_lines = 10;
    state.git_scroll.visible_height = 3;
    state.git_scroll.offset = 0;

    state.scroll_bottom(2);
    assert_eq!(state.git_scroll.offset, 2);
}

#[test]
fn test_scroll_bottom_dispatches_to_activity() {
    let mut state = make_state(vec![]);
    state.bottom_tab = BottomTab::Activity;
    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:00".into(),
        tool: "Read".into(),
        label: "a".into(),
    }];
    state.activity_scroll.total_lines = 10;
    state.activity_scroll.visible_height = 3;
    state.activity_scroll.offset = 0;

    state.scroll_bottom(2);
    assert_eq!(state.activity_scroll.offset, 2);
}

// ─── State: next_bottom_tab cycle Tests ─────────────────────────────

#[test]
fn test_next_bottom_tab_full_cycle() {
    let mut state = make_state(vec![]);
    assert_eq!(state.bottom_tab, BottomTab::Activity);
    state.next_bottom_tab();
    assert_eq!(state.bottom_tab, BottomTab::GitStatus);
    state.next_bottom_tab();
    assert_eq!(state.bottom_tab, BottomTab::Activity);
}

// ─── State: scroll_activity empty Tests ─────────────────────────────

#[test]
fn test_scroll_activity_empty_is_noop() {
    let mut state = make_state(vec![]);
    state.activity_scroll.offset = 0;
    state.activity_scroll.scroll(5);
    assert_eq!(
        state.activity_scroll.offset, 0,
        "scrolling empty activity should be no-op"
    );
}

// ─── State: git tab active flag Tests ───────────────────────────────

#[test]
fn test_git_tab_active_after_tab_switch() {
    let mut state = make_state(vec![]);
    assert_eq!(state.bottom_tab, BottomTab::Activity);

    state.next_bottom_tab();
    assert_eq!(state.bottom_tab, BottomTab::GitStatus);

    state.next_bottom_tab();
    assert_eq!(state.bottom_tab, BottomTab::Activity);
}

#[test]
fn test_git_tab_default_is_activity() {
    let state = make_state(vec![]);
    assert_eq!(state.bottom_tab, BottomTab::Activity);
    assert_ne!(state.bottom_tab, BottomTab::GitStatus);
}

#[test]
fn test_git_tab_equality_check() {
    // Verify BottomTab::GitStatus == check works correctly for the AtomicBool flag
    let tab = BottomTab::GitStatus;
    assert!(tab == BottomTab::GitStatus);
    let tab2 = BottomTab::Activity;
    assert!(tab2 != BottomTab::GitStatus);
}

// ─── State: global sync → rebuild consistency Tests ─────────────

#[test]
fn test_filter_change_rebuilds_row_targets() {
    use tmux_agent_sidebar::state::AgentFilter;

    let running_pane = PaneInfo {
        pane_id: "%1".into(),
        status: PaneStatus::Running,
        ..make_pane(AgentType::Claude, PaneStatus::Running)
    };
    let idle_pane = PaneInfo {
        pane_id: "%2".into(),
        status: PaneStatus::Idle,
        ..make_pane(AgentType::Claude, PaneStatus::Idle)
    };
    let mut state = make_state(vec![]);
    state.repo_groups = vec![make_repo_group(
        "project",
        vec![running_pane, idle_pane],
    )];

    // All filter shows both
    state.agent_filter = AgentFilter::All;
    state.rebuild_row_targets();
    assert_eq!(state.agent_row_targets.len(), 2);

    // Simulates sync_global_state setting filter to Running
    state.agent_filter = AgentFilter::Running;
    state.rebuild_row_targets();
    assert_eq!(state.agent_row_targets.len(), 1);
    assert_eq!(state.agent_row_targets[0].pane_id, "%1");

    // Simulates sync_global_state setting filter to Idle
    state.agent_filter = AgentFilter::Idle;
    state.rebuild_row_targets();
    assert_eq!(state.agent_row_targets.len(), 1);
    assert_eq!(state.agent_row_targets[0].pane_id, "%2");
}

#[test]
fn test_cursor_sync_clamped_by_rebuild() {
    use tmux_agent_sidebar::state::AgentFilter;

    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state(vec![]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];

    // Simulates sync_global_state setting cursor beyond bounds
    state.selected_agent_row = 5;
    state.agent_filter = AgentFilter::All;
    state.rebuild_row_targets();
    // Should be clamped to last valid index
    assert_eq!(state.selected_agent_row, 0);
}
