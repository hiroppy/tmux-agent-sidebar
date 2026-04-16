#[allow(dead_code, unused_imports)]
mod test_helpers;

use test_helpers::*;
use tmux_agent_sidebar::activity::{ActivityEntry, TaskProgress, TaskStatus};
use tmux_agent_sidebar::group::{PaneGitInfo, RepoGroup};
use tmux_agent_sidebar::state::{Focus, PopupState, StatusFilter};
use tmux_agent_sidebar::tmux::{
    AgentType, PaneInfo, PaneStatus, PermissionMode, SessionInfo, WindowInfo,
};

// ─── UI Snapshot Tests ─────────────────────────────────────────────

#[test]
fn snapshot_single_agent_idle_ui() {
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

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    ┃ ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// Locks down the secondary header layout when there are no notices —
// `make_state()` injects a Claude missing-hook notice as the shared
// baseline so the ⓘ badge is on every other snapshot, which means a
// regression in the no-notices path would slip past unnoticed without
// this dedicated coverage.
#[test]
fn snapshot_secondary_header_without_notices() {
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
    state.notices.missing_hook_groups.clear();
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
                             — ▾
    ┃ ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_version_banner_does_not_duplicate_in_scroll_area() {
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
    state.version_notice = Some(tmux_agent_sidebar::version::UpdateNotice {
        local_version: "0.2.6".into(),
        latest_version: "0.2.7".into(),
    });
    state.bottom_panel_height = 0;
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 10);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    project
    ┃ ○ claude
        Waiting for prompt…
    ");
}

#[test]
fn snapshot_single_agent_running_with_elapsed() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.started_at = Some(FIXED_NOW - 125); // 2m5s ago

    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "dotfiles".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("dotfiles", vec![pane])];
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    dotfiles
    ┃ ● claude              2m5s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn running_spinner_different_frame() {
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
    state.spinner_frame = 0;

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_agent_with_prompt_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.prompt = "fix the bug".into();

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

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    ┃ ○ claude
        fix the bug
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_agent_with_japanese_prompt_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.prompt = "これって今1時間経っているけど、起動して確認しても問題ない？".into();

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

    let output = render_to_string(&mut state, 28, 27);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    ┃ ● claude
        こ れ っ て 今 1時 間 経 っ て い
        る け ど 、 起 動 し て 確 認 し て
        も 問 題 な い ？
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_two_agents_same_window_ui() {
    let pane1 = PaneInfo {
        pane_id: "%1".into(),
        pane_active: true,
        status: PaneStatus::Running,
        attention: false,
        agent: AgentType::Claude,
        path: "/home/user/project".into(),
        current_command: String::new(),
        prompt: "fix the bug".into(),
        prompt_is_response: false,
        started_at: None,
        wait_reason: String::new(),
        permission_mode: tmux_agent_sidebar::tmux::PermissionMode::Default,
        subagents: vec![],
        pane_pid: None,
        worktree_name: String::new(),
        worktree_branch: String::new(),
        session_id: None,
        session_name: String::new(),
        sidebar_spawned: false,
    };
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        attention: false,
        agent: AgentType::Codex,
        path: "/home/user/project".into(),
        current_command: String::new(),
        prompt: String::new(),
        prompt_is_response: false,
        started_at: None,
        wait_reason: String::new(),
        permission_mode: tmux_agent_sidebar::tmux::PermissionMode::Default,
        subagents: vec![],
        pane_pid: None,
        worktree_name: String::new(),
        worktree_branch: String::new(),
        session_id: None,
        session_name: String::new(),
        sidebar_spawned: false,
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

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                        — ▾
    ┃ ● claude
        fix the bug
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_two_windows_ui() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut pane2 = make_pane(AgentType::Codex, PaneStatus::Idle);
    pane2.pane_id = "%2".into();
    pane2.pane_active = false;

    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![
            WindowInfo {
                window_id: "@1".into(),
                window_name: "project-a".into(),
                window_active: true,
                auto_rename: false,
                panes: vec![pane1.clone()],
            },
            WindowInfo {
                window_id: "@2".into(),
                window_name: "project-b".into(),
                window_active: false,
                auto_rename: false,
                panes: vec![pane2.clone()],
            },
        ],
    }]);
    // Two different windows → two repo groups
    let mut group1 = make_repo_group("project-a", vec![pane1]);
    group1.has_focus = true;
    let mut group2 = make_repo_group("project-b", vec![pane2]);
    group2.has_focus = false;
    state.repo_groups = vec![group1, group2];
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                        — ▾
    project-a
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_multi_session_ui() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut pane2 = make_pane(AgentType::Codex, PaneStatus::Idle);
    pane2.pane_id = "%2".into();
    pane2.pane_active = false;

    let mut state = make_state(vec![
        SessionInfo {
            session_name: "main".into(),
            windows: vec![WindowInfo {
                window_id: "@1".into(),
                window_name: "dotfiles".into(),
                window_active: true,
                auto_rename: false,
                panes: vec![pane1.clone()],
            }],
        },
        SessionInfo {
            session_name: "work".into(),
            windows: vec![WindowInfo {
                window_id: "@2".into(),
                window_name: "api".into(),
                window_active: false,
                auto_rename: false,
                panes: vec![pane2.clone()],
            }],
        },
    ]);
    // Multi-session → two repo groups (sessions don't matter for rendering)
    let mut group1 = make_repo_group("dotfiles", vec![pane1]);
    group1.has_focus = true;
    let mut group2 = make_repo_group("api", vec![pane2]);
    group2.has_focus = false;
    state.repo_groups = vec![group1, group2];
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                        — ▾
    dotfiles
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_wait_reason_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Waiting);
    pane.wait_reason = "permission_prompt".into();

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

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐1  ○0  ✕0
    ⓘ                        — ▾
    ┃ ◐ claude
        permission required
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_auto_rename_window_title_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);

    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "fish".into(),
            window_active: true,
            auto_rename: true,
            panes: vec![pane.clone()],
        }],
    }]);
    // auto_rename=true: box title comes from RepoGroup.name (path basename = "project")
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    ┃ ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_activity_log_ui() {
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

    state.activity_entries = vec![
        ActivityEntry {
            timestamp: "10:32".into(),
            tool: "Edit".into(),
            label: "src/main.rs".into(),
        },
        ActivityEntry {
            timestamp: "10:31".into(),
            tool: "Bash".into(),
            label: "cargo build".into(),
        },
        ActivityEntry {
            timestamp: "10:30".into(),
            tool: "Read".into(),
            label: "Cargo.toml".into(),
        },
    ];

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │10:32                 Edit│
    │  src/main.rs             │
    │10:31                 Bash│
    │  cargo build             │
    │10:30                 Read│
    │  Cargo.toml              │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_activity_log_long_label_ui() {
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

    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:32".into(),
        tool: "Read".into(),
        label: "config/tmux-agent-sidebar-rs/src/very-long-filename.rs".into(),
    }];

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │10:32                 Read│
    │  config/tmux-agent-sideba│
    │  r-rs/src/very-long-filen│
    │  ame.rs                  │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_prompt_wrapping_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.prompt =
        "Please fix the authentication bug in the login flow that causes users to be logged out"
            .into();

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

    let output = render_to_string(&mut state, 28, 27);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    ┃ ○ claude
        Please fix the
        authentication bug in
        the login flow that cau…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_selected_unfocused_ui() {
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
fn snapshot_error_state_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Error);
    pane.prompt = "something broke".into();

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

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○0  ✕1
    ⓘ                        — ▾
    ┃ ✕ claude
        something broke
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_narrow_width_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.prompt = "hello world".into();

    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "p".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane.clone()],
        }],
    }]);
    state.repo_groups = vec![make_repo_group("project", vec![pane])];
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 18, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕
    ⓘ              — ▾
    ┃ ○ claude
        hello world
    ╭ Activity │ Git ╮
    │ No activity yet│
    ╰────────────────╯
    ");
}

/// Create a state with a dummy session so draw() doesn't show "No agent panes found"
fn make_state_with_groups(
    groups: Vec<tmux_agent_sidebar::group::RepoGroup>,
) -> tmux_agent_sidebar::state::AppState {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state(vec![SessionInfo {
        session_name: "main".into(),
        windows: vec![WindowInfo {
            window_id: "@1".into(),
            window_name: "dummy".into(),
            window_active: true,
            auto_rename: false,
            panes: vec![pane],
        }],
    }]);
    state.repo_groups = groups;
    state.rebuild_row_targets();
    state
}

// ─── Worktree Branch Display ──────────────────────────────────────

#[test]
fn snapshot_worktree_branch_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.prompt = "fix bug".into();
    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("feature/sidebar".into()),
        is_worktree: true,
        worktree_name: None,
    };
    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);

    let output = render_to_string(&mut state, 28, 26);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    ┃ ● claude
    ┃   + feature/sidebar
        fix bug
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_worktree_long_branch_truncated_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("feature/very-long-branch-name-that-overflows".into()),
        is_worktree: true,
        worktree_name: None,
    };
    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                        — ▾
    ┃   + feature/very-long-bra…
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_long_branch_with_ports_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("feature/sidebar/really-long-branch-name-that-should-truncate".into()),
        is_worktree: false,
        worktree_name: None,
    };
    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);
    state.set_pane_ports("%1", vec![3000, 5173]);

    let output = render_to_string(&mut state, 40, 24);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                                    — ▾
    ┃   feature/sidebar/really…  :3000, 5173
    ╭ Activity │ Git ──────────────────────╮
    │            No activity yet           │
    ╰──────────────────────────────────────╯
    ");
}

// ─── Task Progress Variations ─────────────────────────────────────

#[test]
fn snapshot_task_progress_partial_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.prompt = "working".into();
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);
    state.set_pane_task_progress(
        "%1",
        Some(TaskProgress {
            tasks: vec![
                ("Task A".into(), TaskStatus::Completed),
                ("Task B".into(), TaskStatus::InProgress),
                ("Task C".into(), TaskStatus::Pending),
            ],
        }),
    );

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
        ✔◼◻ 1/3
        working
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_task_progress_all_completed_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);
    state.set_pane_task_progress(
        "%1",
        Some(TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::Completed),
            ],
        }),
    );

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    ┃ ● claude
        ✔✔ 2/2
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_task_progress_all_pending_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);
    state.set_pane_task_progress(
        "%1",
        Some(TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Pending),
                ("B".into(), TaskStatus::Pending),
                ("C".into(), TaskStatus::Pending),
            ],
        }),
    );

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    ┃ ● claude
        ◻◻◻ 0/3
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// ─── Combined Elements ────────────────────────────────────────────

#[test]
fn snapshot_all_elements_combined_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Waiting);
    pane.prompt = "fixing the bug".into();
    pane.wait_reason = "permission_prompt".into();
    pane.subagents = vec!["Explore".into(), "Plan".into()];
    pane.permission_mode = PermissionMode::Auto;

    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("main".into()),
        is_worktree: false,
        worktree_name: None,
    };

    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);
    state.set_pane_task_progress(
        "%1",
        Some(TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::InProgress),
            ],
        }),
    );

    let output = render_to_string(&mut state, 30, 32);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐1  ○0  ✕0
    ⓘ                          — ▾
    project                      +
    ┃ ◐ claude auto
    ┃   main
        ✔◼ 1/2
        ├ Explore #1
        └ Plan #2
        permission required
        fixing the bug
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

// ─── Response Display ─────────────────────────────────────────────

#[test]
fn snapshot_response_japanese_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.prompt = "修正が完了しました。テストも全て通っています。".into();
    pane.prompt_is_response = true;
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 30, 27);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                          — ▾
    project
    ┃ ○ claude
      ▷ 修 正 が 完 了 し ま し た 。 テ ス ト
        も 全 て 通 っ て い ま す 。
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

// ─── Three Groups with Focus ─────────────────────────────────────

#[test]
fn snapshot_three_groups_middle_focused_ui() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut pane2 = make_pane(AgentType::Codex, PaneStatus::Idle);
    pane2.pane_id = "%2".into();
    pane2.pane_active = false;
    let mut pane3 = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane3.pane_id = "%3".into();
    pane3.pane_active = false;

    let mut group1 = make_repo_group("repo-a", vec![pane1]);
    group1.has_focus = false;
    let mut group2 = make_repo_group("repo-b", vec![pane2]);
    group2.has_focus = false;
    let mut group3 = make_repo_group("repo-c", vec![pane3]);
    group3.has_focus = false;
    let mut state = make_state_with_groups(vec![group1, group2, group3]);
    state.focused_pane_id = Some("%2".into());

    let output = render_to_string(&mut state, 28, 33);
    insta::assert_snapshot!(output, @"
     ≡3  ●1  ◐0  ○2  ✕0
    ⓘ                        — ▾
    repo-a
      ● claude
    repo-b
    ┃ ○ codex
        Waiting for prompt…
    repo-c
      ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// ─── PermissionMode Badges ────────────────────────────────────────

#[test]
fn snapshot_bypass_all_badge_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.permission_mode = PermissionMode::BypassPermissions;

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude !
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_full_auto_badge_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.permission_mode = PermissionMode::Auto;

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude auto
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_plan_badge_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.permission_mode = PermissionMode::Plan;

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude plan
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_accept_edits_badge_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.permission_mode = PermissionMode::AcceptEdits;

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude edit
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_response_with_branch_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    pane.prompt = "Done. All tests are green.".into();
    pane.prompt_is_response = true;
    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("feature/ui-v2".into()),
        is_worktree: false,
        worktree_name: None,
    };
    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);

    let output = render_to_string(&mut state, 34, 27);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                              — ▾
    project                          +
    ┃ ○ claude
    ┃   feature/ui-v2
      ▷ Done. All tests are green.
    ╭ Activity │ Git ────────────────╮
    │         No activity yet        │
    ╰────────────────────────────────╯
    ");
}

// ─── Multiple Wait Reasons ────────────────────────────────────────

#[test]
fn snapshot_wait_reason_elicitation_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Waiting);
    pane.wait_reason = "elicitation_dialog".into();

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐1  ○0  ✕0
    ⓘ                        — ▾
    ┃ ◐ claude
        waiting for selection
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_wait_reason_unknown_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Waiting);
    pane.wait_reason = "some_future_reason".into();

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐1  ○0  ✕0
    ⓘ                        — ▾
    ┃ ◐ claude
        some_future_reason
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// ─── Permission Denied ───────────────────────────────────────────

#[test]
fn snapshot_wait_reason_permission_denied_ui() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Waiting);
    pane.wait_reason = "permission_denied".into();

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐1  ○0  ✕0
    ⓘ                        — ▾
    ┃ ◐ claude
        permission denied
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// ─── Worktree Name Display ──────────────────────────────────────

#[test]
fn snapshot_worktree_with_name_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("feat/auth".into()),
        is_worktree: true,
        worktree_name: Some("auth-wt".into()),
    };
    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    ┃ ● claude
    ┃   + auth-wt: feat/auth
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_worktree_name_same_as_branch_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let git_info = PaneGitInfo {
        repo_root: Some("/home/user/project".into()),
        branch: Some("feat/auth".into()),
        is_worktree: true,
        worktree_name: Some("feat/auth".into()),
    };
    let mut state = make_state_with_groups(vec![tmux_agent_sidebar::group::RepoGroup {
        name: "project".into(),
        has_focus: true,
        panes: vec![(pane, git_info)],
    }]);

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    ┃ ● claude
    ┃   + feat/auth
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// ─── Activity Log Tool Types ──────────────────────────────────────

#[test]
fn snapshot_activity_all_tool_types_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    state.activity_entries = vec![
        ActivityEntry {
            timestamp: "10:07".into(),
            tool: "Agent".into(),
            label: "Explore codebase".into(),
        },
        ActivityEntry {
            timestamp: "10:06".into(),
            tool: "Skill".into(),
            label: "commit".into(),
        },
        ActivityEntry {
            timestamp: "10:05".into(),
            tool: "ToolSearch".into(),
            label: "select:Read".into(),
        },
        ActivityEntry {
            timestamp: "10:04".into(),
            tool: "TaskCreate".into(),
            label: "#1 Fix bug".into(),
        },
        ActivityEntry {
            timestamp: "10:03".into(),
            tool: "WebFetch".into(),
            label: "docs.rs/ratatui".into(),
        },
        ActivityEntry {
            timestamp: "10:02".into(),
            tool: "Grep".into(),
            label: "run_git".into(),
        },
        ActivityEntry {
            timestamp: "10:01".into(),
            tool: "Write".into(),
            label: "new_file.rs".into(),
        },
    ];

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │10:07                Agent│
    │  Explore codebase        │
    │10:06                Skill│
    │  commit                  │
    │10:05           ToolSearch│
    │  select:Read             │
    │10:04           TaskCreate│
    │  #1 Fix bug              │
    │10:03             WebFetch│
    │  docs.rs/ratatui         │
    │10:02                 Grep│
    │  run_git                 │
    │10:01                Write│
    │  new_file.rs             │
    ╰──────────────────────────╯
    ");
}

// ─── Focus Transitions ───────────────────────────────────────────

#[test]
fn snapshot_focus_activity_log_ui() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Running);
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);
    state.focus = Focus::ActivityLog;
    state.sidebar_focused = true;
    state.activity_entries = vec![ActivityEntry {
        timestamp: "10:00".into(),
        tool: "Read".into(),
        label: "file.rs".into(),
    }];

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │10:00                 Read│
    │  file.rs                 │
    ╰──────────────────────────╯
    ");
}

// ─── Right Border Integrity ──────────────────────────────────────

#[test]
fn right_border_narrow_width_with_badge() {
    let mut pane = make_pane(AgentType::Claude, PaneStatus::Running);
    pane.started_at = Some(FIXED_NOW - 7200); // 2h ago
    pane.permission_mode = PermissionMode::BypassPermissions;
    pane.prompt = "fix the issue".into();

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

    // Snapshot locks in the `!` badge visibility at narrow width plus a
    // fully-drawn right border.
    let output = render_to_string(&mut state, 22, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                  — ▾
    ┃ ● claude !    2h0m0s
        fix the issue
    ╭ Activity │ Git ────╮
    │   No activity yet  │
    ╰────────────────────╯
    ");
    // Structural invariant (width-agnostic): every line that starts with a
    // border glyph must also end with one. Kept alongside the snapshot so
    // border regressions are caught even if someone regenerates the snapshot.
    assert_right_border_intact(&output);
}

#[test]
fn right_border_all_permission_modes_and_agents() {
    let modes: &[PermissionMode] = &[
        PermissionMode::Default,
        PermissionMode::Auto,
        PermissionMode::DontAsk,
        PermissionMode::Plan,
        PermissionMode::AcceptEdits,
        PermissionMode::BypassPermissions,
    ];
    let agents = [AgentType::Claude, AgentType::Codex];
    let now = FIXED_NOW;

    // Render every (agent, mode) combination into a single composite string
    // so one inline snapshot covers the full matrix. A regression in any
    // single cell surfaces as a diff that names the exact combo.
    // Each render is also passed through `assert_right_border_intact`, the
    // structural invariant that catches width-agnostic border breakage.
    let mut composite = String::new();
    for agent in &agents {
        for mode in modes {
            let mut pane = make_pane(agent.clone(), PaneStatus::Running);
            pane.permission_mode = mode.clone();
            pane.started_at = Some(now - 5432); // ~1h30m

            let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);
            let rendered = render_to_string(&mut state, 28, 25);
            assert_right_border_intact(&rendered);
            composite.push_str(&format!("=== {:?} / {:?} ===\n", agent, mode));
            composite.push_str(&rendered);
            composite.push_str("\n\n");
        }
    }
    insta::assert_snapshot!(composite, @"
    === Claude / Default ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude          1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Claude / Auto ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude auto     1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Claude / DontAsk ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude dontAsk  1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Claude / Plan ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude plan     1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Claude / AcceptEdits ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude edit     1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Claude / BypassPermissions ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude !        1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Codex / Default ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● codex           1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Codex / Auto ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● codex auto      1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Codex / DontAsk ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● codex dontAsk   1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Codex / Plan ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● codex plan      1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Codex / AcceptEdits ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● codex edit      1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯

    === Codex / BypassPermissions ===
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● codex !         1h30m32s
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

// ─── Filter Bar Tests ────────────────────────────────────────────

#[test]
fn snapshot_filter_bar_shows_counts() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    let output = render_to_string(&mut state, 30, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                          — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_running_hides_idle() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    state.global.status_filter = StatusFilter::Running;
    let output = render_to_string(&mut state, 30, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                          — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_idle_hides_running() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    state.global.status_filter = StatusFilter::Idle;
    let output = render_to_string(&mut state, 30, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                          — ▾
      ○ codex
        Waiting for prompt…
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_hides_empty_groups() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };

    let mut state = make_state_with_groups(vec![
        make_repo_group("repo-a", vec![pane1]),
        make_repo_group("repo-b", vec![pane2]),
    ]);
    state.global.status_filter = StatusFilter::Running;
    let output = render_to_string(&mut state, 30, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                          — ▾
    repo-a
    ┃ ● claude
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_all_shows_everything() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    state.global.status_filter = StatusFilter::All;
    let output = render_to_string(&mut state, 30, 30);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○1  ✕0
    ⓘ                          — ▾
    project
    ┃ ● claude
      ○ codex
        Waiting for prompt…
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_bar_icons_use_selected_and_inactive_colors() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);

    let styled = render_to_styled_string(&mut state, 30, 25);
    let line = styled.lines().next().unwrap();
    insta::assert_snapshot!(line, @" ≡[fg:111]2[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]1[fg:255]  ✕[fg:245]0[fg:245]");
}

#[test]
fn snapshot_filter_bar_stays_fixed_on_scroll() {
    // Many agents to force scrolling, verify filter bar always present
    let panes: Vec<_> = (0..6)
        .map(|i| {
            let mut p = make_pane(AgentType::Claude, PaneStatus::Running);
            p.pane_id = format!("%{i}");
            p.pane_active = i == 0;
            p
        })
        .collect();
    let mut state = make_state_with_groups(vec![make_repo_group("project", panes)]);
    state.panes_scroll.offset = 3; // scroll down

    let output = render_to_string(&mut state, 30, 15);
    insta::assert_snapshot!(output, @"
     ≡6  ●6  ◐0  ○0  ✕0
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_selected_icon_has_color_without_underline() {
    let pane1 = make_pane(AgentType::Claude, PaneStatus::Running);
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };
    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    state.global.status_filter = StatusFilter::Running;

    // The inline snapshot captures the styled filter bar; any underline
    // modifier on the selected filter would surface in the snapshot diff.
    let styled = render_to_styled_string(&mut state, 30, 25);
    let line = styled.lines().next().unwrap();
    insta::assert_snapshot!(line, @" ≡[fg:245]2[fg:255]  ●[fg:114]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]1[fg:255]  ✕[fg:245]0[fg:245]");
}

#[test]
fn snapshot_filter_error_shows_agents() {
    let mut pane1 = make_pane(AgentType::Claude, PaneStatus::Error);
    pane1.prompt = "something broke".into();
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Running,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Running)
    };

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    state.global.status_filter = StatusFilter::Error;
    let output = render_to_string(&mut state, 30, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●1  ◐0  ○0  ✕1
    ⓘ                          — ▾
    ┃ ✕ claude
        something broke
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_filter_waiting_shows_only_waiting() {
    let mut pane1 = make_pane(AgentType::Claude, PaneStatus::Waiting);
    pane1.wait_reason = "permission_prompt".into();
    let pane2 = PaneInfo {
        pane_id: "%2".into(),
        pane_active: false,
        status: PaneStatus::Idle,
        agent: AgentType::Codex,
        ..make_pane(AgentType::Codex, PaneStatus::Idle)
    };

    let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane1, pane2])]);
    state.global.status_filter = StatusFilter::Waiting;
    let output = render_to_string(&mut state, 30, 25);
    insta::assert_snapshot!(output, @"
     ≡2  ●0  ◐1  ○1  ✕0
    ⓘ                          — ▾
    ┃ ◐ claude
        permission required
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

// ─── Spawn / remove popup snapshots ─────────────────────────────────

fn repo_group_with_root(name: &str, panes: Vec<PaneInfo>) -> RepoGroup {
    RepoGroup {
        name: name.into(),
        has_focus: true,
        panes: panes
            .into_iter()
            .map(|p| {
                (
                    p,
                    PaneGitInfo {
                        repo_root: Some(format!("/home/u/{name}")),
                        branch: Some("main".into()),
                        is_worktree: false,
                        worktree_name: None,
                    },
                )
            })
            .collect(),
    }
}

/// Shrink the bottom panel so the default 20-row bottom block leaves
/// enough room for popup rendering in narrow test backends.
fn make_state_for_popup_tests(groups: Vec<RepoGroup>) -> tmux_agent_sidebar::state::AppState {
    let mut state = make_state_with_groups(groups);
    state.bottom_panel_height = 3;
    state
}

#[test]
fn snapshot_repo_header_shows_spawn_plus_button() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    let output = render_to_string(&mut state, 30, 15);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                          — ▾
    proj                         +
    ┃ ○ claude
    ┃   main
        Waiting for prompt…
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_default_state() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    let output = render_to_string(&mut state, 34, 18);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ╭ Spawn worktree ──────────────╮▾
    p│                              │+
    ┃│ NAME                         │
    ┃│ █                            │
     │ AGENT                        │
     │ claude                       │
     │ MODE                         │
     │ default                      │
     ╰──────────────────────────────╯
    ╭ Activity │ Git ────────────────╮
    │         No activity yet        │
    ╰────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_anchors_directly_below_repo_header() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    // Render once so layout.repo_spawn_targets is populated, then
    // drive the same code path the keyboard `n` handler takes — it
    // must resolve the anchor from the rendered `+` target so the
    // popup opens right below the repo header row (row 2).
    let _ = render_to_string(&mut state, 34, 18);
    state.global.selected_pane_row = 0;
    state.open_spawn_input_from_selection();
    let output = render_to_string(&mut state, 34, 18);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                              — ▾
    ╭ Spawn worktree ──────────────╮ +
    │ NAME                         │
    │ █                            │
    │ AGENT                        │
    │ claude                       │
    │ MODE                         │
    │ default                      │
    ╰──────────────────────────────╯
    ╭ Activity │ Git ────────────────╮
    │         No activity yet        │
    ╰────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_advance_fields_cycles_agent_and_mode() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    for c in "add login".chars() {
        state.spawn_input_push_char(c);
    }
    state.spawn_input_next_field();
    state.spawn_input_cycle(1); // claude → codex
    state.spawn_input_next_field();
    state.spawn_input_cycle(2); // default → bypassPermissions
    let output = render_to_string(&mut state, 34, 18);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ╭ Spawn worktree ──────────────╮▾
    p│                              │+
    ┃│ NAME                         │
    ┃│ add login                    │
     │ AGENT                        │
     │ codex                        │
     │ MODE                         │
     │ bypassPermissions            │
     ╰──────────────────────────────╯
    ╭ Activity │ Git ────────────────╮
    │         No activity yet        │
    ╰────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_tail_fits_long_task_name() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    for c in "refactor-the-entire-authentication-pipeline".chars() {
        state.spawn_input_push_char(c);
    }
    let output = render_to_string(&mut state, 34, 18);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ╭ Spawn worktree ──────────────╮▾
    p│                              │+
    ┃│ NAME                         │
    ┃│ …re-authentication-pipeline█ │
     │ AGENT                        │
     │ claude                       │
     │ MODE                         │
     │ default                      │
     ╰──────────────────────────────╯
    ╭ Activity │ Git ────────────────╮
    │         No activity yet        │
    ╰────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_narrow_width_still_fits() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    for c in "hi".chars() {
        state.spawn_input_push_char(c);
    }
    let output = render_to_string(&mut state, 18, 18);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕
    ╭ Spawn worktree ╮
    │ NAME           │
    │ hi█            │
    │ AGENT          │
    │ claude         │
    │ MODE           │
    │ default        │
    ╰────────────────╯
    ╭ Activity │ Git ╮
    │ No activity yet│
    ╰────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_compact_layout_in_short_agent_area() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    // Default bottom_panel_height is 3 in `make_state_for_popup_tests`.
    // A 14-row terminal leaves 11 rows for the agents panel — below
    // SPAWN_MODAL_EXPANDED_MIN_HEIGHT (12), so the popup must fall
    // back to the label-less compact layout.
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    for c in "hi".chars() {
        state.spawn_input_push_char(c);
    }
    let output = render_to_string(&mut state, 40, 14);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                                    — ▾
    proj╭ Spawn worktree ──────────────╮   +
    ┃ ○ │ hi█                          │
    ┃   │ claude                       │
        │ default                      │
        ╰──────────────────────────────╯
    ╭ Activity │ Git ──────────────────────╮
    │            No activity yet           │
    ╰──────────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_compact_layout_shows_inline_error() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    state.confirm_spawn_input();
    let output = render_to_string(&mut state, 40, 14);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ   ╭ Spawn worktree ──────────────╮ — ▾
    proj│ █                            │   +
    ┃ ○ │ claude                       │
    ┃   │ default                      │
        │ name is empty                │
        ╰──────────────────────────────╯
    ╭ Activity │ Git ──────────────────────╮
    │            No activity yet           │
    ╰──────────────────────────────────────╯
    ");
}

#[test]
fn snapshot_sidebar_spawned_pane_appends_trailing_remove_marker() {
    let mut manual = make_pane(AgentType::Claude, PaneStatus::Idle);
    manual.pane_id = "%1".into();
    let mut spawned = make_pane(AgentType::Claude, PaneStatus::Idle);
    spawned.pane_id = "%2".into();
    spawned.sidebar_spawned = true;

    let group = RepoGroup {
        name: "proj".into(),
        has_focus: true,
        panes: vec![
            (
                manual,
                PaneGitInfo {
                    repo_root: Some("/home/u/proj".into()),
                    branch: Some("main".into()),
                    is_worktree: true,
                    worktree_name: None,
                },
            ),
            (
                spawned,
                PaneGitInfo {
                    repo_root: Some("/home/u/proj".into()),
                    branch: Some("feat/x".into()),
                    is_worktree: true,
                    worktree_name: None,
                },
            ),
        ],
    };
    let mut state = make_state_with_groups(vec![group]);
    state.bottom_panel_height = 3;
    let output = render_to_string(&mut state, 30, 20);
    insta::assert_snapshot!(output, @r"
     ≡2  ●0  ◐0  ○2  ✕0
    ⓘ                          — ▾
    proj                         +
    ┃ ○ claude
    ┃   + main
        Waiting for prompt…
      ○ claude
        + feat/x                 ×
        Waiting for prompt…
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_sidebar_spawned_pane_registers_click_target() {
    let mut spawned = make_pane(AgentType::Claude, PaneStatus::Idle);
    spawned.pane_id = "%7".into();
    spawned.sidebar_spawned = true;
    let group = RepoGroup {
        name: "proj".into(),
        has_focus: true,
        panes: vec![(
            spawned,
            PaneGitInfo {
                repo_root: Some("/home/u/proj".into()),
                branch: Some("feat/abc".into()),
                is_worktree: true,
                worktree_name: None,
            },
        )],
    };
    let mut state = make_state_with_groups(vec![group]);
    let _ = render_to_string(&mut state, 30, 28);
    let targets = &state.layout.spawn_remove_targets;
    assert_eq!(
        targets.len(),
        1,
        "exactly one × target should be registered: {targets:?}"
    );
    assert_eq!(targets[0].pane_id, "%7");
    // `×` is pinned to the rightmost row column (col 29 for a
    // 30-wide panel: inner_width(28) + marker(1) + space(1) - 1).
    // The hit region extends leftward so the glyph sits at its
    // right edge with two columns of slack on the left →
    // rect.x = 29 - 2 = 27, rect.width = 3.
    assert_eq!(targets[0].rect.x, 27);
    assert_eq!(targets[0].rect.width, 3);
}

#[test]
fn snapshot_sidebar_spawned_click_target_is_invariant_to_branch_length() {
    // Right-edge pinning means the × column is determined by the
    // panel width, not the branch name length. A short branch and
    // a long branch must produce the exact same click target.
    let make_group = |branch: &str| RepoGroup {
        name: "proj".into(),
        has_focus: true,
        panes: vec![(
            {
                let mut p = make_pane(AgentType::Claude, PaneStatus::Idle);
                p.pane_id = "%7".into();
                p.sidebar_spawned = true;
                p
            },
            PaneGitInfo {
                repo_root: Some("/home/u/proj".into()),
                branch: Some(branch.into()),
                is_worktree: true,
                worktree_name: None,
            },
        )],
    };

    let mut short_state = make_state_with_groups(vec![make_group("x")]);
    let _ = render_to_string(&mut short_state, 30, 28);
    let short = short_state.layout.spawn_remove_targets[0].rect;

    let mut long_state = make_state_with_groups(vec![make_group("feature/x")]);
    let _ = render_to_string(&mut long_state, 30, 28);
    let long = long_state.layout.spawn_remove_targets[0].rect;

    assert_eq!(
        short, long,
        "click target must be invariant to branch length"
    );
    assert_eq!(short.x, 27, "target x should be pinned to right edge");
}

#[test]
fn snapshot_non_spawned_pane_does_not_register_click_target() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let group = RepoGroup {
        name: "proj".into(),
        has_focus: true,
        panes: vec![(
            pane,
            PaneGitInfo {
                repo_root: Some("/home/u/proj".into()),
                branch: Some("feat/abc".into()),
                is_worktree: true,
                worktree_name: None,
            },
        )],
    };
    let mut state = make_state_with_groups(vec![group]);
    let _ = render_to_string(&mut state, 30, 28);
    assert!(
        state.layout.spawn_remove_targets.is_empty(),
        "manual worktree must not register × click targets"
    );
}

#[test]
fn snapshot_sidebar_spawned_long_branch_truncates_and_keeps_x() {
    // When the branch name is longer than the row can fit, the
    // branch text must truncate (ellipsis) to leave room for the
    // trailing `×` — the action affordance cannot be the thing that
    // gets clipped off-screen.
    let mut spawned = make_pane(AgentType::Claude, PaneStatus::Idle);
    spawned.pane_id = "%8".into();
    spawned.sidebar_spawned = true;
    let group = RepoGroup {
        name: "proj".into(),
        has_focus: true,
        panes: vec![(
            spawned,
            PaneGitInfo {
                repo_root: Some("/home/u/proj".into()),
                branch: Some("feature/really-long-branch-name".into()),
                is_worktree: true,
                worktree_name: None,
            },
        )],
    };
    let mut state = make_state_with_groups(vec![group]);
    state.bottom_panel_height = 3;
    let output = render_to_string(&mut state, 24, 20);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                    — ▾
    proj                   +
      ○ claude
        + feature/really-l…×
        Waiting for prompt…
    ╭ Activity │ Git ──────╮
    │    No activity yet   │
    ╰──────────────────────╯
    ");
    // The click target must still be registered even after
    // truncation — the × is at the right edge of the branch row.
    let targets = &state.layout.spawn_remove_targets;
    assert_eq!(targets.len(), 1, "click target must still be registered");
}

#[test]
fn snapshot_sidebar_spawned_coexists_with_port_display() {
    // Sanity check: when ports are active on the pane, both the
    // trailing `×` AND the port list must render. The `×` stays
    // pinned to the end of the branch text (left side), ports on
    // the far right — they do not overwrite each other.
    let mut spawned = make_pane(AgentType::Claude, PaneStatus::Idle);
    spawned.pane_id = "%9".into();
    spawned.sidebar_spawned = true;
    let group = RepoGroup {
        name: "proj".into(),
        has_focus: true,
        panes: vec![(
            spawned,
            PaneGitInfo {
                repo_root: Some("/home/u/proj".into()),
                branch: Some("feat/srv".into()),
                is_worktree: true,
                worktree_name: None,
            },
        )],
    };
    let mut state = make_state_with_groups(vec![group]);
    state.bottom_panel_height = 3;
    state.pane_state_mut("%9").ports = vec![3000];
    let output = render_to_string(&mut state, 30, 20);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                          — ▾
    proj                         +
      ○ claude
        + feat/srv         :3000 ×
        Waiting for prompt…
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}

#[test]
fn snapshot_remove_confirm_modal_shows_three_options() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.popup = PopupState::RemoveConfirm {
        pane_id: "%42".into(),
        branch: "add-login".into(),
        error: None,
        area: None,
    };
    let output = render_to_string(&mut state, 50, 18);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                                              — ▾
    proj                                             +
    ┃ ○ claude
    ┃   main   ╭ add-login ───────────────╮
        Waiting│[y] remove worktree       │
               │[c] close window only     │
               │[n] cancel                │
               ╰──────────────────────────╯
    ╭ Activity │ Git ────────────────────────────────╮
    │                 No activity yet                │
    ╰────────────────────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_shows_inline_error_when_task_empty() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    // Press Enter with no input — should set the inline error row,
    // NOT close the popup.
    state.confirm_spawn_input();
    assert!(state.is_spawn_input_open(), "popup must stay open on error");
    let output = render_to_string(&mut state, 34, 18);
    insta::assert_snapshot!(output, @"
     ╭ Spawn worktree ──────────────╮
    ⓘ│                              │▾
    p│ NAME                         │+
    ┃│ █                            │
    ┃│                              │
     │ AGENT                        │
     │ claude                       │
     │ MODE                         │
     │ default                      │
     │ name is empty                │
     ╰──────────────────────────────╯
    ╭ Activity │ Git ────────────────╮
    │         No activity yet        │
    ╰────────────────────────────────╯
    ");
}

#[test]
fn snapshot_spawn_modal_clears_error_after_typing() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.open_spawn_input_for_repo("proj".into(), "/home/u/proj".into(), None);
    state.confirm_spawn_input(); // triggers the "name is empty" error
    // Typing a character should clear the error so the user isn't
    // staring at a stale message while they fix their input.
    state.spawn_input_push_char('x');
    match &state.popup {
        PopupState::SpawnInput { error, .. } => {
            assert!(error.is_none(), "error must clear on edit")
        }
        _ => panic!("spawn popup should still be open"),
    }
}

#[test]
fn snapshot_remove_confirm_modal_shows_inline_error() {
    let pane = make_pane(AgentType::Claude, PaneStatus::Idle);
    let mut state = make_state_for_popup_tests(vec![repo_group_with_root("proj", vec![pane])]);
    state.popup = PopupState::RemoveConfirm {
        pane_id: "%42".into(),
        branch: "add-login".into(),
        error: Some("git: worktree has uncommitted changes".into()),
        area: None,
    };
    let output = render_to_string(&mut state, 50, 18);
    insta::assert_snapshot!(output, @r"
     ≡1  ●0  ◐0  ○1  ✕0
    ⓘ                                              — ▾
    proj                                             +
    ┃ ○ claude ╭ add-login ───────────────╮
    ┃   main   │[y] remove worktree       │
        Waiting│[c] close window only     │
               │[n] cancel                │
               │git: worktree has uncommi…│
               ╰──────────────────────────╯
    ╭ Activity │ Git ────────────────────────────────╮
    │                 No activity yet                │
    ╰────────────────────────────────────────────────╯
    ");
}
