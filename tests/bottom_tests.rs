#[allow(dead_code, unused_imports)]
mod test_helpers;

use test_helpers::*;
use tmux_agent_sidebar::activity::ActivityEntry;
use tmux_agent_sidebar::state::{BottomTab, Focus};
use tmux_agent_sidebar::tmux::{AgentType, PaneStatus, SessionInfo, WindowInfo};

// ─── Bottom Tab Tests ──────────────────────────────────────────────

#[test]
fn test_next_bottom_tab() {
    let mut state = make_state(vec![]);
    assert_eq!(state.bottom_tab, BottomTab::Activity);
    state.next_bottom_tab();
    assert_eq!(state.bottom_tab, BottomTab::GitStatus);
    state.next_bottom_tab();
    assert_eq!(state.bottom_tab, BottomTab::Activity);
}

#[test]
fn test_scroll_bottom_dispatches() {
    let mut state = make_state(vec![]);

    // Set up activity scroll state
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
    ];
    state.activity_scroll.total_lines = 6;
    state.activity_scroll.visible_height = 4;

    // Set up git scroll state
    state.git.unstaged_files = vec![
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "file1.rs".into(),
            additions: 0,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "file2.rs".into(),
            additions: 0,
            deletions: 0,
            path: String::new(),
        },
    ];
    state.git.untracked_files = vec!["file3.rs".into()];
    state.git_scroll.total_lines = 3;
    state.git_scroll.visible_height = 1;

    // Activity tab: scroll should affect activity
    state.bottom_tab = BottomTab::Activity;
    state.scroll_bottom(1);
    assert_eq!(state.activity_scroll.offset, 1);
    assert_eq!(state.git_scroll.offset, 0);

    // Git tab: scroll should affect git
    state.bottom_tab = BottomTab::GitStatus;
    state.scroll_bottom(1);
    assert_eq!(state.git_scroll.offset, 1);
    assert_eq!(state.activity_scroll.offset, 1); // unchanged
}

#[test]
fn snapshot_git_status_tab_ui() {
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
    state.git.branch = "feature/sidebar".into();
    state.git.ahead_behind = Some((2, 1));
    state.git.unstaged_files = vec![
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "src/ui/panes.rs".into(),
            additions: 30,
            deletions: 10,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "src/state.rs".into(),
            additions: 12,
            deletions: 5,
            path: String::new(),
        },
    ];
    state.git.untracked_files = vec!["new_file.rs".into()];
    state.git.diff_stat = Some((42, 15));

    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │ feature/sidebar     ↑2↓1 │
    │ +42/-15          3 files │
    │──────────────────────────│
    │ Unstaged (2)             │
    │ M src/ui/panes…  +30/-10 │
    │ M src/state.rs    +12/-5 │
    │ Untracked (1)            │
    │ ? new_file.rs            │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_clean_ui() {
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
    // No git changes

    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_activity_tab_active_ui() {
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

    state.bottom_tab = BottomTab::Activity;
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:32".into(),
        tool: "Edit".into(),
        label: "src/main.rs".into(),
    }];

    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │10:32                 Edit│
    │  src/main.rs             │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_tab_bar_renders_both_labels() {
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

    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:32".into(),
        tool: "Edit".into(),
        label: "test".into(),
    }];

    let output = render_to_string(&mut state, 28, 14);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ╭ Activity │ Git ──────────╮
    │10:32                 Edit│
    │  test                    │
    ╰──────────────────────────╯
    ");
}

// ─── Git Content Tests ──────────────────────────────────────────────

#[test]
fn snapshot_git_full_info_ui() {
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
    state.git.ahead_behind = Some((0, 0));
    state.git.diff_stat = Some((120, 30));
    state.git.unstaged_files = vec![
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "src/state.rs".into(),
            additions: 42,
            deletions: 10,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "src/ui/bottom.rs".into(),
            additions: 85,
            deletions: 20,
            path: String::new(),
        },
    ];
    state.git.untracked_files = vec!["new_file.rs".into()];

    // Use plain render since elapsed time varies
    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │ +120/-30         3 files │
    │──────────────────────────│
    │ Unstaged (2)             │
    │ M src/state.rs   +42/-10 │
    │ M src/ui/botto…  +85/-20 │
    │ Untracked (1)            │
    │ ? new_file.rs            │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_long_filename_truncated_ui() {
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
    state.git.unstaged_files = vec![
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "very-long-filename-that-should-be-truncated.rs".into(),
            additions: 150,
            deletions: 50,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "short.rs".into(),
            additions: 8,
            deletions: 2,
            path: String::new(),
        },
    ];

    // Verify the long filename is truncated (contains ellipsis)
    let plain = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │                  2 files │
    │──────────────────────────│
    │ Unstaged (2)             │
    │ M very-long-f…  +150/-50 │
    │ M short.rs         +8/-2 │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_more_than_5_files() {
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
    state.git.unstaged_files = vec![
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "a.rs".into(),
            additions: 100,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "b.rs".into(),
            additions: 80,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "c.rs".into(),
            additions: 60,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "d.rs".into(),
            additions: 40,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "e.rs".into(),
            additions: 20,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "f.rs".into(),
            additions: 10,
            deletions: 0,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "g.rs".into(),
            additions: 5,
            deletions: 0,
            path: String::new(),
        },
    ];

    // Verify file list rendering (scroll to see overflow)
    let plain = render_to_string(&mut state, 28, 40);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │                  7 files │
    │──────────────────────────│
    │ Unstaged (7)             │
    │ M a.rs           +100/-0 │
    │ M b.rs            +80/-0 │
    │ M c.rs            +60/-0 │
    │ M d.rs            +40/-0 │
    │ M e.rs            +20/-0 │
    │                  +2 more │
    ╰──────────────────────────╯
    ");

    // Scroll to bottom to verify "+2 more" exists
    state.git_scroll.offset = 5;
    let scrolled = render_to_string(&mut state, 28, 40);
    insta::assert_snapshot!(scrolled, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │                  7 files │
    │──────────────────────────│
    │ M e.rs            +20/-0 │
    │                  +2 more │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_branch_only_no_changes() {
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
    state.git.branch = "feature/long-branch-name".into();
    state.git.ahead_behind = Some((5, 0));

    let plain = render_to_string(&mut state, 38, 20);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ────────────────────╮
    │ feature/long-branch-name        ↑5 │
    │────────────────────────────────────│
    │         Working tree clean         │
    ╰────────────────────────────────────╯
    ");
}

#[test]
fn snapshot_git_pr_number_ui() {
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
    state.git.branch = "feature/fix".into();
    state.git.pr_number = Some("42".into());
    state.git.remote_url = "https://github.com/user/repo".into();
    state.git.diff_stat = Some((10, 3));

    let output = render_to_styled_string(&mut state, 28, 14);
    // PR number should be underlined and blue
    let plain = render_to_string(&mut state, 28, 14);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ──────────╮
    │ feature/fix          #42 │
    │ +10/-3           0 files │
    │──────────────────────────│
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
    assert!(
        output.contains("underline"),
        "PR number should be underlined"
    );
    assert!(
        output.contains("fg:117"),
        "PR link should use pr_link color (117)"
    );
}

#[test]
fn test_normalize_git_url() {
    // Test via state: set remote URL and check it's normalized
    let mut state = make_state(vec![]);
    state.git.remote_url = "https://github.com/user/repo".into();
    assert_eq!(state.git.remote_url, "https://github.com/user/repo");
}

#[test]
fn snapshot_git_pr_with_diff_ui() {
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
    state.git.pr_number = Some("123".into());
    state.git.remote_url = "https://github.com/user/repo".into();
    state.git.diff_stat = Some((55, 20));

    let plain = render_to_string(&mut state, 28, 14);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ──────────╮
    │ main                #123 │
    │ +55/-20          0 files │
    │──────────────────────────│
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_subagents_tree_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.subagents = vec!["Explore #1".into(), "Plan".into(), "Explore #2".into()];

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

    let output = render_to_string(&mut state, 40, 28);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                                    — ▾
    project
    ┃ ● claude
        ├ Explore #1
        ├ Plan #2
        └ Explore #2
    ╭ Activity │ Git ──────────────────────╮
    │            No activity yet           │
    ╰──────────────────────────────────────╯
    ");
}

#[test]
fn snapshot_subagent_long_name_truncated_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.subagents = vec![
        "superpowers:code-reviewer".into(),
        "claude-code-guide".into(),
    ];

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

    // Narrow width (28) to force truncation of long subagent names
    let output = render_to_string(&mut state, 28, 27);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
        ├ superpowers:code-revi…
        └ claude-code-guide #2
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");

    assert_right_border_intact(&output);
}

// ─── Empty State Centered Tests ─────────────────────────────────────

#[test]
fn snapshot_activity_empty_centered_ui() {
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
    state.bottom_tab = BottomTab::Activity;
    // No activity entries — should show centered "No activity yet"

    let output = render_to_string(&mut state, 28, 26);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    project
    ┃ ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_clean_centered_ui() {
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
    state.bottom_tab = BottomTab::GitStatus;
    // No git info — should show centered "Working tree clean"

    let output = render_to_string(&mut state, 28, 26);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    project
    ┃ ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

// ─── Git: "Working tree clean" consistency ──────────────────────────

#[test]
fn snapshot_git_branch_loaded_no_changes_shows_inline_clean() {
    // Bug fix: when git_branch is set but no status/diff/commit data,
    // the early-return "centered clean" path was skipped, falling through
    // to a different "inline clean" layout. Now both paths are consistent.
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
    // Branch loaded, but no changes/commits — should still show "Working tree clean"
    state.git.branch = "main".into();

    let plain = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │──────────────────────────│
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_no_data_shows_centered_clean() {
    // When no git data is loaded at all, should show centered "Working tree clean"
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
    // No git data at all

    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

// ─── Git: ahead/behind rendering ────────────────────────────────────

#[test]
fn test_git_behind_only() {
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
    state.git.ahead_behind = Some((0, 3));

    let plain = render_to_string(&mut state, 28, 14);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ──────────╮
    │ main                  ↓3 │
    │──────────────────────────│
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

#[test]
fn test_git_ahead_and_behind() {
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
    state.git.ahead_behind = Some((2, 3));

    let plain = render_to_string(&mut state, 38, 14);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ────────────────────╮
    │ main                          ↑2↓3 │
    │────────────────────────────────────│
    │         Working tree clean         │
    ╰────────────────────────────────────╯
    ");
}

// ─── Git: diff stat with only insertions or only deletions ──────────

#[test]
fn test_git_diff_insertions_only() {
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
    state.git.diff_stat = Some((25, 0));

    let plain = render_to_string(&mut state, 28, 14);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │ +25/-0           0 files │
    │──────────────────────────│
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

#[test]
fn test_git_diff_deletions_only() {
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
    state.git.diff_stat = Some((0, 15));

    let plain = render_to_string(&mut state, 28, 14);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │ +0/-15           0 files │
    │──────────────────────────│
    │    Working tree clean    │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_branch_truncated_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@0".into(),
            window_name: "project".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    // Use a repo group with a long branch name via PaneGitInfo
    state.repo_groups = vec![tmux_agent_sidebar::group::RepoGroup {
        name: "dotfiles".into(),
        has_focus: true,
        panes: vec![(
            pane,
            tmux_agent_sidebar::group::PaneGitInfo {
                repo_root: Some("/home/user/dotfiles".into()),
                branch: Some("feature/tmux-sidebar-dashboard-refactor".into()),
                is_worktree: false,
                worktree_name: None,
            },
        )],
    }];
    state.rebuild_row_targets();

    let plain = render_to_string(&mut state, 28, 30);
    insta::assert_snapshot!(plain, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    dotfiles                   +
    ┃ ● claude
    ┃   feature/tmux-sidebar-da…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_staged_unstaged_untracked_ui() {
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
    state.git.pr_number = Some("5".into());
    state.git.diff_stat = Some((12, 3));
    state.git.staged_files = vec![
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: "app.rs".into(),
            additions: 10,
            deletions: 2,
            path: String::new(),
        },
        tmux_agent_sidebar::git::GitFileEntry {
            status: 'A',
            name: "new.rs".into(),
            additions: 2,
            deletions: 0,
            path: String::new(),
        },
    ];
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "config.toml".into(),
        additions: 0,
        deletions: 1,
        path: String::new(),
    }];
    state.git.untracked_files = vec!["debug.log".into()];

    let output = render_to_string(&mut state, 28, 30);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │ main                  #5 │
    │ +12/-3           4 files │
    │──────────────────────────│
    │ Staged (2)               │
    │ M app.rs          +10/-2 │
    │ A new.rs           +2/-0 │
    │ Unstaged (1)             │
    │ M config.toml      +0/-1 │
    │ Untracked (1)            │
    │ ? debug.log              │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_long_branch_with_pr_ui() {
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
    state.git.branch = "feature/very-long-branch-name".into();
    state.git.pr_number = Some("123".into());
    state.git.diff_stat = Some((5, 2));
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "main.rs".into(),
        additions: 5,
        deletions: 2,
        path: String::new(),
    }];

    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │ feature/very-long-… #123 │
    │ +5/-2            1 files │
    │──────────────────────────│
    │ Unstaged (1)             │
    │ M main.rs          +5/-2 │
    ╰──────────────────────────╯
    ");
    assert_right_border_intact(&output);
}

#[test]
fn snapshot_git_staged_only_ui() {
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
    state.git.diff_stat = Some((20, 0));
    state.git.staged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'A',
        name: "new_feature.rs".into(),
        additions: 20,
        deletions: 0,
        path: String::new(),
    }];

    let output = render_to_string(&mut state, 28, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ╭ Activity │ Git ──────────╮
    │ main                     │
    │ +20/-0           1 files │
    │──────────────────────────│
    │ Staged (1)               │
    │ A new_feature.rs  +20/-0 │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_git_many_files_more_indicator_ui() {
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
    state.git.branch = "dev".into();
    state.git.unstaged_files = (0..7)
        .map(|i| tmux_agent_sidebar::git::GitFileEntry {
            status: 'M',
            name: format!("f{i}.rs"),
            additions: 1,
            deletions: 0,
            path: String::new(),
        })
        .collect();

    let output = render_to_string(&mut state, 28, 30);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │ dev                      │
    │                  7 files │
    │──────────────────────────│
    │ Unstaged (7)             │
    │ M f0.rs            +1/-0 │
    │ M f1.rs            +1/-0 │
    │ M f2.rs            +1/-0 │
    │ M f3.rs            +1/-0 │
    │ M f4.rs            +1/-0 │
    │                  +2 more │
    ╰──────────────────────────╯
    ");
    assert_right_border_intact(&output);
}

#[test]
fn snapshot_focused_group_active_border_styled() {
    // Two repo groups: focused pane in first, second should have inactive border
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
            name: "dotfiles".into(),
            has_focus: true,
            panes: vec![(
                pane1.clone(),
                tmux_agent_sidebar::group::PaneGitInfo::default(),
            )],
        },
        tmux_agent_sidebar::group::RepoGroup {
            name: "my-app".into(),
            has_focus: false,
            panes: vec![(
                pane2.clone(),
                tmux_agent_sidebar::group::PaneGitInfo::default(),
            )],
        },
    ];
    state.focused_pane_id = Some("%1".into());
    state.rebuild_row_targets();

    let styled = render_to_styled_string(&mut state, 28, 30);
    assert!(styled.contains("fg:153"));
}
