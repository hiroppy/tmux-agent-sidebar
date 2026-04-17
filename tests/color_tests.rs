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
    assert_eq!(theme.selection_bg, Color::Indexed(239));

    // Status colors
    assert_eq!(theme.status_all, Color::Indexed(111));
    assert_eq!(theme.status_running, Color::Indexed(114));
    assert_eq!(theme.status_waiting, Color::Indexed(221));
    assert_eq!(theme.status_idle, Color::Indexed(110));
    assert_eq!(theme.status_error, Color::Indexed(167));
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
    assert_eq!(theme.badge_danger, Color::Indexed(167));
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
        Color::Indexed(167)
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
    state.focus_state.sidebar_focused = false;

    // Styled snapshot locks in the BypassAll `!` badge rendered with
    // badge_danger (fg:167).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 26), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174] [fg:167]![fg:167]


    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.sidebar_focused = false;

    // Styled snapshot locks in the Auto `auto` badge rendered with
    // badge_auto (fg:221).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 26), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174] [fg:221]a[fg:221]u[fg:221]t[fg:221]o[fg:221]


    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.sidebar_focused = false;

    // Snapshot locks in that the agent row shows no badge in Normal mode.
    insta::assert_snapshot!(render_to_string(&mut state, 28, 25), @"
     ≡1  ●1  ◐0  ○0  ✕0
    ⓘ                        — ▾
    project
    ┃ ● claude
    ╭ Activity │ Git ──────────╮
    │      No activity yet     │
    ╰──────────────────────────╯
    ");
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
    state.focus_state.focus = Focus::ActivityLog;
    state.focus_state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "src/lib.rs".into(),
        additions: 5,
        deletions: 2,
        path: String::new(),
    }];

    // Styled snapshot locks in the Modified file badge color
    // (badge_auto fg:221).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]

    ╭[fg:153] [fg:153]A[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:240]│[fg:240] [fg:240]G[fg:153]i[fg:153]t[fg:153] [fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╮[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]m[fg:255]a[fg:255]i[fg:255]n[fg:255] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]1[fg:252] [fg:252]f[fg:252]i[fg:252]l[fg:252]e[fg:252]s[fg:252]│[fg:153]
    │[fg:153]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]│[fg:153]
    │[fg:153]U[fg:109]n[fg:109]s[fg:109]t[fg:109]a[fg:109]g[fg:109]e[fg:109]d[fg:109] [fg:109]([fg:109]1[fg:109])[fg:109] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]M[fg:221] [fg:153]s[fg:252]r[fg:252]c[fg:252]/[fg:252]l[fg:252]i[fg:252]b[fg:252].[fg:252]r[fg:252]s[fg:252] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]+[fg:114]5[fg:114]/[fg:252]-[fg:174]2[fg:174]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    ╰[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╯[fg:153]
    ");
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
    state.focus_state.sidebar_focused = false;

    // Set task progress for pane %1
    let progress = TaskProgress {
        tasks: vec![
            ("Task A".into(), TaskStatus::Completed),
            ("Task B".into(), TaskStatus::InProgress),
            ("Task C".into(), TaskStatus::Pending),
        ],
    };
    state.set_pane_task_progress("%1", Some(progress));

    // Styled snapshot locks in both the task_progress color (fg:223) and
    // the progress glyphs (✔/◼/◻) with the "1/3" count.
    insta::assert_snapshot!(render_to_styled_string(&mut state, 40, 40), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                                    —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
       [fg:223] [fg:223]✔[fg:223]◼[fg:223]◻[fg:223] [fg:223]1[fg:223]/[fg:223]3[fg:223]















    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.sidebar_focused = false;

    // Styled snapshot locks in the subagent line color (fg:73) plus the
    // rendered "Explore #1" label.
    insta::assert_snapshot!(render_to_styled_string(&mut state, 40, 27), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                                    —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
       [fg:252] [fg:252]└[fg:252] [fg:252]E[fg:73]x[fg:73]p[fg:73]l[fg:73]o[fg:73]r[fg:73]e[fg:73] [fg:73]#[fg:73]1[fg:73]


    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.sidebar_focused = false;

    // Styled snapshot locks in:
    //   • response_arrow color (fg:81) + bold on the ▷ glyph
    //   • text_active color (fg:255) on the focused response text
    insta::assert_snapshot!(render_to_styled_string(&mut state, 40, 27), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]0[fg:245]  ◐[fg:245]0[fg:245]  ○[fg:245]1[fg:255]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                                    —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ○[fg:110] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
      ▷[fg:81,bold] [fg:81,bold]T[fg:255]a[fg:255]s[fg:255]k[fg:255] [fg:255]c[fg:255]o[fg:255]m[fg:255]p[fg:255]l[fg:255]e[fg:255]t[fg:255]e[fg:255]d[fg:255] [fg:255]s[fg:255]u[fg:255]c[fg:255]c[fg:255]e[fg:255]s[fg:255]s[fg:255]f[fg:255]u[fg:255]l[fg:255]l[fg:255]y[fg:255]


    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.focus = Focus::ActivityLog;
    state.focus_state.sidebar_focused = true;
    state.git.branch = "feature/test".into();
    state.git.pr_number = Some("99".into());
    state.git.remote_url = "https://github.com/user/repo".into();

    // Styled snapshot locks in the PR link: pr_link color (fg:117) plus
    // the underline modifier on the `#99` glyphs.
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 40), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
















    ╭[fg:153] [fg:153]A[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:240]│[fg:240] [fg:240]G[fg:153]i[fg:153]t[fg:153] [fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╮[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]f[fg:255]e[fg:255]a[fg:255]t[fg:255]u[fg:255]r[fg:255]e[fg:255]/[fg:255]t[fg:255]e[fg:255]s[fg:255]t[fg:255] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]#[fg:117,underline]9[fg:117,underline]9[fg:117,underline]│[fg:153]
    │[fg:153]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153]W[fg:252]o[fg:252]r[fg:252]k[fg:252]i[fg:252]n[fg:252]g[fg:252] [fg:252]t[fg:252]r[fg:252]e[fg:252]e[fg:252] [fg:252]c[fg:252]l[fg:252]e[fg:252]a[fg:252]n[fg:252] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    ╰[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╯[fg:153]
    ");
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
    state.focus_state.focus = Focus::ActivityLog;
    state.focus_state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.diff_stat = Some((42, 10));

    // Styled snapshot locks in the `+42` additions rendered with
    // diff_added color (fg:114).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 40), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
















    ╭[fg:153] [fg:153]A[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:240]│[fg:240] [fg:240]G[fg:153]i[fg:153]t[fg:153] [fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╮[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]m[fg:255]a[fg:255]i[fg:255]n[fg:255] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]+[fg:114]4[fg:114]2[fg:114]/[fg:252]-[fg:174]1[fg:174]0[fg:174] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]0[fg:252] [fg:252]f[fg:252]i[fg:252]l[fg:252]e[fg:252]s[fg:252]│[fg:153]
    │[fg:153]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153]W[fg:252]o[fg:252]r[fg:252]k[fg:252]i[fg:252]n[fg:252]g[fg:252] [fg:252]t[fg:252]r[fg:252]e[fg:252]e[fg:252] [fg:252]c[fg:252]l[fg:252]e[fg:252]a[fg:252]n[fg:252] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    ╰[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╯[fg:153]
    ");
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
    state.focus_state.focus = Focus::ActivityLog;
    state.focus_state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.diff_stat = Some((0, 25));

    // Styled snapshot locks in the `-25` deletions rendered with
    // diff_deleted color (fg:174).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]

    ╭[fg:153] [fg:153]A[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:240]│[fg:240] [fg:240]G[fg:153]i[fg:153]t[fg:153] [fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╮[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]m[fg:255]a[fg:255]i[fg:255]n[fg:255] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]+[fg:114]0[fg:114]/[fg:252]-[fg:174]2[fg:174]5[fg:174] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]0[fg:252] [fg:252]f[fg:252]i[fg:252]l[fg:252]e[fg:252]s[fg:252]│[fg:153]
    │[fg:153]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153]W[fg:252]o[fg:252]r[fg:252]k[fg:252]i[fg:252]n[fg:252]g[fg:252] [fg:252]t[fg:252]r[fg:252]e[fg:252]e[fg:252] [fg:252]c[fg:252]l[fg:252]e[fg:252]a[fg:252]n[fg:252] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    ╰[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╯[fg:153]
    ");
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
    state.focus_state.focus = Focus::ActivityLog;
    state.focus_state.sidebar_focused = true;
    state.git.branch = "main".into();
    state.git.unstaged_files = vec![tmux_agent_sidebar::git::GitFileEntry {
        status: 'M',
        name: "lib.rs".into(),
        additions: 40,
        deletions: 10,
        path: String::new(),
    }];

    // Styled snapshot locks in the `M lib.rs` row rendered with
    // badge_auto (fg:221) on the status glyph.
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]

    ╭[fg:153] [fg:153]A[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:240]│[fg:240] [fg:240]G[fg:153]i[fg:153]t[fg:153] [fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╮[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]m[fg:255]a[fg:255]i[fg:255]n[fg:255] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]1[fg:252] [fg:252]f[fg:252]i[fg:252]l[fg:252]e[fg:252]s[fg:252]│[fg:153]
    │[fg:153]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]│[fg:153]
    │[fg:153]U[fg:109]n[fg:109]s[fg:109]t[fg:109]a[fg:109]g[fg:109]e[fg:109]d[fg:109] [fg:109]([fg:109]1[fg:109])[fg:109] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153]M[fg:221] [fg:153]l[fg:252]i[fg:252]b[fg:252].[fg:252]r[fg:252]s[fg:252] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]+[fg:114]4[fg:114]0[fg:114]/[fg:252]-[fg:174]1[fg:174]0[fg:174]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    │[fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153] [fg:153]│[fg:153]
    ╰[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]─[fg:153]╯[fg:153]
    ");
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
    state.focus_state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    // Styled snapshot locks in the branch name rendered with branch color
    // (fg:109).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 40, 26), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                                    —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]                                +[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
    ┃[fg:153]  [fg:109] [fg:109]f[fg:109]e[fg:109]a[fg:109]t[fg:109]u[fg:109]r[fg:109]e[fg:109]/[fg:109]c[fg:109]o[fg:109]o[fg:109]l[fg:109]-[fg:109]f[fg:109]e[fg:109]a[fg:109]t[fg:109]u[fg:109]r[fg:109]e[fg:109]
    ");
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
    state.focus_state.sidebar_focused = true;
    state.global.selected_pane_row = 0;

    // Styled snapshot locks in the selected agent row's selection
    // background (bg:239).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]0[fg:245]  ◐[fg:245]0[fg:245]  ○[fg:245]1[fg:255]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    ┃[fg:153,bg:239] [bg:239]○[fg:110,bg:239] [fg:174,bg:239]c[fg:174,bg:239]l[fg:174,bg:239]a[fg:174,bg:239]u[fg:174,bg:239]d[fg:174,bg:239]e[fg:174,bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239]
       [fg:255] [fg:255]W[fg:255]a[fg:255]i[fg:255]t[fg:255]i[fg:255]n[fg:255]g[fg:255] [fg:255]f[fg:255]o[fg:255]r[fg:255] [fg:255]p[fg:255]r[fg:255]o[fg:255]m[fg:255]p[fg:255]t[fg:255]…[fg:255]

    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.focused_pane_id = Some("%1".into());
    state.rebuild_row_targets();

    // Styled snapshot locks in:
    //   • focused group header rendered with accent (fg:153)
    //   • unfocused group header rendered with border_inactive (fg:240)
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 30), @"
     ≡[fg:111]2[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]1[fg:255]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    f[fg:153]o[fg:153]c[fg:153]u[fg:153]s[fg:153]e[fg:153]d[fg:153]-[fg:153]r[fg:153]e[fg:153]p[fg:153]o[fg:153]
    ┃[fg:153,bg:239] [bg:239]●[fg:82,bg:239] [fg:174,bg:239]c[fg:174,bg:239]l[fg:174,bg:239]a[fg:174,bg:239]u[fg:174,bg:239]d[fg:174,bg:239]e[fg:174,bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239] [bg:239]

    u[fg:255]n[fg:255]f[fg:255]o[fg:255]c[fg:255]u[fg:255]s[fg:255]e[fg:255]d[fg:255]-[fg:255]r[fg:255]e[fg:255]p[fg:255]o[fg:255]
      ○[fg:110] [fg:141]c[fg:141]o[fg:141]d[fg:141]e[fg:141]x[fg:141]
       [fg:244] [fg:244]W[fg:244]a[fg:244]i[fg:244]t[fg:244]i[fg:244]n[fg:244]g[fg:244] [fg:244]f[fg:244]o[fg:244]r[fg:244] [fg:244]p[fg:244]r[fg:244]o[fg:244]m[fg:244]p[fg:244]t[fg:244]…[fg:244]


    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
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
    state.focus_state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    // Styled snapshot locks in the running spinner using SPINNER_PULSE[0]
    // color (fg:82).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]1[fg:255]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ●[fg:82] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
    ");
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
    state.focus_state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    // Styled snapshot locks in the waiting status using status_waiting
    // color (fg:221).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]0[fg:245]  ◐[fg:245]1[fg:255]  ○[fg:245]0[fg:245]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ◐[fg:221] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
    ");
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
    state.focus_state.sidebar_focused = false;
    state.bottom_panel_height = 0;

    // Styled snapshot locks in the error status using status_error
    // color (fg:167).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]0[fg:245]  ◐[fg:245]0[fg:245]  ○[fg:245]0[fg:245]  ✕[fg:245]1[fg:255]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ✕[fg:167] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]
    ");
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
    state.focus_state.sidebar_focused = false;

    // Styled snapshot locks in the idle status using status_idle
    // color (fg:110).
    insta::assert_snapshot!(render_to_styled_string(&mut state, 28, 25), @"
     ≡[fg:111]1[fg:255]  ●[fg:245]0[fg:245]  ◐[fg:245]0[fg:245]  ○[fg:245]1[fg:255]  ✕[fg:245]0[fg:245]
    ⓘ[fg:221]                        —[fg:252] ▾[fg:252]
    p[fg:153]r[fg:153]o[fg:153]j[fg:153]e[fg:153]c[fg:153]t[fg:153]
    ┃[fg:153] ○[fg:110] [fg:174]c[fg:174]l[fg:174]a[fg:174]u[fg:174]d[fg:174]e[fg:174]

    ╭[fg:240] [fg:240]A[fg:153]c[fg:153]t[fg:153]i[fg:153]v[fg:153]i[fg:153]t[fg:153]y[fg:153] [fg:240]│[fg:240] [fg:240]G[fg:252]i[fg:252]t[fg:252] [fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╮[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]N[fg:252]o[fg:252] [fg:252]a[fg:252]c[fg:252]t[fg:252]i[fg:252]v[fg:252]i[fg:252]t[fg:252]y[fg:252] [fg:252]y[fg:252]e[fg:252]t[fg:252] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    │[fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240] [fg:240]│[fg:240]
    ╰[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]╯[fg:240]
    ");
}

#[test]
fn test_unknown_status_color_in_output() {
    let theme = tmux_agent_sidebar::ui::colors::ColorTheme::default();
    assert_eq!(
        theme.status_color(&PaneStatus::Unknown, false),
        ratatui::style::Color::Indexed(244)
    );
}
