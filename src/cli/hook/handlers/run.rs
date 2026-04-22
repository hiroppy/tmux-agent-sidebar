use crate::cli::{sanitize_tmux_value, set_attention, set_status};
use crate::desktop_notification;
use crate::desktop_notification::DesktopNotificationKind;
use crate::tmux;

use super::super::context::{
    AgentContext, branch_label_from_ctx, branch_label_from_pane, clear_run_state,
    is_system_message, mark_task_reset, now_epoch_secs, repo_label_from_ctx, repo_label_from_pane,
    set_agent_meta,
};
use super::super::notifications::{
    notification_run_id, notify_desktop, set_notification_run_id, stop_body, stop_failure_body,
    stop_failure_fingerprint, task_completed_body, task_completed_fingerprint,
};

pub(in crate::cli::hook) fn on_user_prompt_submit(
    pane: &str,
    ctx: &AgentContext<'_>,
    prompt: &str,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    set_status(pane, "running");
    set_notification_run_id(pane);
    if !prompt.is_empty() && !is_system_message(prompt) {
        let p = sanitize_tmux_value(prompt);
        tmux::set_pane_option(pane, "@pane_prompt", &p);
        tmux::set_pane_option(pane, "@pane_prompt_source", "user");
    }
    tmux::set_pane_option(pane, "@pane_started_at", &now_epoch_secs().to_string());
    tmux::unset_pane_option(pane, "@pane_wait_reason");
    0
}

pub(in crate::cli::hook) fn on_stop(
    pane: &str,
    ctx: &AgentContext<'_>,
    last_message: &str,
    response: Option<&str>,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    if !last_message.is_empty() {
        let msg = sanitize_tmux_value(last_message);
        tmux::set_pane_option(pane, "@pane_prompt", &msg);
        tmux::set_pane_option(pane, "@pane_prompt_source", "response");
    }
    clear_run_state(pane);
    mark_task_reset(pane);
    set_status(pane, "idle");
    let run_id = notification_run_id(pane);
    // Skip the generic Stop notification if an explicit TaskCompleted
    // stamp from the current run has already fired — otherwise Claude
    // Code's `TaskCompleted` → `Stop` sequence produces two desktop
    // notifications for the same logical completion.
    let already_notified = desktop_notification::has_run_scoped_stamp(
        pane,
        DesktopNotificationKind::TaskCompleted,
        run_id,
    );
    if !already_notified {
        let repo = repo_label_from_ctx(ctx);
        let branch = branch_label_from_ctx(ctx);
        let fingerprint = desktop_notification::run_scoped_fingerprint(run_id, "stop");
        let _ = notify_desktop(
            pane,
            DesktopNotificationKind::TaskCompleted,
            desktop_notification::DesktopNotificationEvent::Stop,
            notifications,
            &fingerprint,
            &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), ctx.agent),
            &stop_body(last_message),
        );
    }
    if let Some(resp) = response {
        println!("{resp}");
    }
    0
}

pub(in crate::cli::hook) fn on_stop_failure(
    pane: &str,
    ctx: &AgentContext<'_>,
    error: &str,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
    mark_task_reset(pane);
    if !error.is_empty() {
        tmux::set_pane_option(pane, "@pane_wait_reason", error);
    }
    set_status(pane, "error");
    let fingerprint = desktop_notification::run_scoped_fingerprint(
        notification_run_id(pane),
        stop_failure_fingerprint(error),
    );
    let repo = repo_label_from_ctx(ctx);
    let branch = branch_label_from_ctx(ctx);
    let body = stop_failure_body(error);
    let _ = notify_desktop(
        pane,
        DesktopNotificationKind::TaskFailed,
        desktop_notification::DesktopNotificationEvent::StopFailure,
        notifications,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), ctx.agent),
        &body,
    );
    0
}

pub(in crate::cli::hook) fn on_task_completed(
    pane: &str,
    agent_name: &str,
    task_id: &str,
    task_subject: &str,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    let fingerprint = desktop_notification::run_scoped_fingerprint(
        notification_run_id(pane),
        task_completed_fingerprint(task_id, task_subject),
    );
    let repo = repo_label_from_pane(pane);
    let branch = branch_label_from_pane(pane);
    let body = task_completed_body(task_subject);
    let _ = notify_desktop(
        pane,
        DesktopNotificationKind::TaskCompleted,
        desktop_notification::DesktopNotificationEvent::TaskCompleted,
        notifications,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), agent_name),
        &body,
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_user_prompt_submit_sets_running_and_stores_prompt() {
        let _guard = tmux::test_mock::install();
        let pane = "%PROMPT";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let exit = on_user_prompt_submit(pane, &ctx, "fix the bug");
        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_status").as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_prompt").as_deref(),
            Some("fix the bug")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_prompt_source").as_deref(),
            Some("user")
        );
        assert!(tmux::test_mock::contains(pane, "@pane_started_at"));
    }

    #[test]
    fn on_user_prompt_submit_ignores_system_messages() {
        let _guard = tmux::test_mock::install();
        let pane = "%SYS_PROMPT";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_user_prompt_submit(pane, &ctx, "<system-reminder>ignore me</system-reminder>");
        assert!(
            !tmux::test_mock::contains(pane, "@pane_prompt"),
            "system messages should not be stored as user prompt"
        );
        // But status should still advance to running.
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_status").as_deref(),
            Some("running")
        );
    }

    #[test]
    fn on_user_prompt_submit_clears_stale_wait_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%PROMPT_CLEAR_WAIT";
        tmux::test_mock::set(pane, "@pane_wait_reason", "permission");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_user_prompt_submit(pane, &ctx, "new prompt");
        assert!(!tmux::test_mock::contains(pane, "@pane_wait_reason"));
    }

    #[test]
    fn on_stop_failure_records_error_wait_reason_and_error_status() {
        let _guard = tmux::test_mock::install();
        let pane = "%STOP_FAIL";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let exit = on_stop_failure(
            pane,
            &ctx,
            "rate_limit",
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );
        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_status").as_deref(),
            Some("error")
        );
        assert_eq!(
            tmux::test_mock::get(pane, "@pane_wait_reason").as_deref(),
            Some("rate_limit")
        );
    }
}
