#[allow(dead_code, unused_imports)]
mod test_helpers;

use test_helpers::*;
use tmux_agent_sidebar::activity::{ActivityEntry, TaskProgress, TaskStatus};
use tmux_agent_sidebar::group::PaneGitInfo;
use tmux_agent_sidebar::state::{Focus, StatusFilter};
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
                             — ▾
    ┃ ○ claude
        Waiting for prompt…
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
}

#[test]
fn snapshot_version_banner_replaces_repo_filter_ui() {
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
    state.rebuild_row_targets();

    let output = render_to_string(&mut state, 28, 25);
    insta::assert_snapshot!(output, @"
     ≡1  ●0  ◐0  ○1  ✕0
             new release v0.2.7!
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
             new release v0.2.7!
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                   — ▾
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
                             — ▾
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
                             — ▾
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
                                         — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                               — ▾
    project
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
                               — ▾
    project
    ┃ ○ claude
      ▶ 修 正 が 完 了 し ま し た 。 テ ス ト
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                                   — ▾
    project
    ┃ ○ claude
    ┃   feature/ui-v2
      ▶ Done. All tests are green.
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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
                             — ▾
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

    let output = render_to_string(&mut state, 22, 25);
    assert!(
        output.contains("!"),
        "badge should remain visible at narrow width"
    );
    assert_right_border_intact(&output);
}

#[test]
fn right_border_all_permission_modes_and_agents() {
    let modes_and_badges: &[(PermissionMode, &str)] = &[
        (PermissionMode::Default, ""),
        (PermissionMode::Auto, "auto"),
        (PermissionMode::DontAsk, "dontAsk"),
        (PermissionMode::Plan, "plan"),
        (PermissionMode::AcceptEdits, "edit"),
        (PermissionMode::BypassPermissions, "!"),
    ];
    let agents = [AgentType::Claude, AgentType::Codex];
    let now = FIXED_NOW;

    for agent in &agents {
        for (mode, expected_badge) in modes_and_badges {
            let mut pane = make_pane(agent.clone(), PaneStatus::Running);
            pane.permission_mode = mode.clone();
            pane.started_at = Some(now - 5432); // ~1h30m

            let mut state = make_state_with_groups(vec![make_repo_group("project", vec![pane])]);

            let output = render_to_string(&mut state, 28, 25);
            assert_right_border_intact(&output);
            if !expected_badge.is_empty() {
                assert!(
                    output.contains(expected_badge),
                    "{:?} {:?} should show badge {:?}",
                    agent,
                    mode,
                    expected_badge,
                );
            }
        }
    }
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
                               — ▾
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
                               — ▾
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
                               — ▾
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
                               — ▾
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
                               — ▾
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

    let styled = render_to_styled_string(&mut state, 30, 25);
    assert!(
        !styled.contains("underline"),
        "selected filter should not be underlined"
    );

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
                               — ▾
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
                               — ▾
    ┃ ◐ claude
        permission required
    ╭ Activity │ Git ────────────╮
    │       No activity yet      │
    ╰────────────────────────────╯
    ");
}
