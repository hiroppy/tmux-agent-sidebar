use crate::cli::{sanitize_tmux_value, set_attention, set_status};
use crate::desktop_notification;
use crate::desktop_notification::DesktopNotificationKind;
use crate::tmux;

use crate::time::now_epoch_secs;

use super::super::context::{AgentContext, clear_run_state, mark_task_reset, set_agent_meta};
use super::super::notifications::{
    NotifyLabels, NotifyPayload, notify_lifecycle, notify_stop, set_notification_run_id, stop_body,
    stop_failure_body, stop_failure_fingerprint, task_completed_body, task_completed_fingerprint,
};
use super::status_priority::resolve_stop_status;

pub(in crate::cli::hook) fn on_user_prompt_submit(
    pane: &str,
    ctx: &AgentContext<'_>,
    prompt: &str,
    prompt_is_system_message: bool,
    prompt_id: Option<&str>,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    set_status(pane, "running");
    set_notification_run_id(pane);
    if !prompt.is_empty() && !prompt_is_system_message {
        let p = sanitize_tmux_value(prompt);
        tmux::set_pane_option(pane, tmux::PANE_PROMPT, &p);
        tmux::set_pane_option(pane, tmux::PANE_PROMPT_SOURCE, "user");
    }
    tmux::set_pane_option(pane, tmux::PANE_STARTED_AT, &now_epoch_secs().to_string());
    tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
    tmux::set_pane_option(pane, tmux::PANE_TURN_ACTIVE, "1");
    tmux::unset_pane_option(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY);
    if let Some(prompt_id) = prompt_id {
        tmux::set_pane_option(pane, tmux::PANE_PROMPT_ID, &encode_prompt_id(prompt_id));
    } else {
        tmux::unset_pane_option(pane, tmux::PANE_PROMPT_ID);
    }
    0
}

fn encode_prompt_id(prompt_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(prompt_id.len() * 2);
    for byte in prompt_id.bytes() {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn turn_end_is_current(pane: &str, prompt_id: Option<&str>) -> bool {
    let Some(prompt_id) = prompt_id else {
        // Grok's session-level idle_prompt backstop intentionally has no
        // promptId and must settle unconditionally. Grok cancels its pending
        // idle timer when a newer turn starts, so this is not a stale-turn path.
        return true;
    };
    let current = tmux::get_pane_option_value(pane, tmux::PANE_PROMPT_ID);
    // Identified terminal events are valid only when a matching submit
    // identity is still tracked. SessionStart clears that identity, so a
    // delayed terminal event from the previous session must be rejected.
    if current.is_empty() {
        return false;
    }
    let active = !tmux::get_pane_option_value(pane, tmux::PANE_TURN_ACTIVE).is_empty();
    active && current == encode_prompt_id(prompt_id)
}

fn mark_turn_settled(pane: &str, prompt_id: Option<&str>) {
    if let Some(prompt_id) = prompt_id {
        tmux::set_pane_option(pane, tmux::PANE_PROMPT_ID, &encode_prompt_id(prompt_id));
    }
    tmux::unset_pane_option(pane, tmux::PANE_TURN_ACTIVE);
}

#[derive(Clone, Copy)]
struct BackgroundWork {
    shell_live: bool,
    subagent_live: bool,
}

impl BackgroundWork {
    fn any(self) -> bool {
        self.shell_live || self.subagent_live
    }
}

fn settle_turn_state(
    pane: &str,
    ctx: &AgentContext<'_>,
    prompt_id: Option<&str>,
    children_may_outlive_turn: bool,
) -> BackgroundWork {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");

    // Adapters normalize whether a later child stop can drain this list.
    if !children_may_outlive_turn {
        tmux::unset_pane_option(pane, tmux::PANE_SUBAGENTS);
    }

    let bg_shell_live = !tmux::get_pane_option_value(pane, tmux::PANE_BG_CMD).is_empty();
    let subagent_live = !tmux::get_pane_option_value(pane, tmux::PANE_SUBAGENTS).is_empty();
    let background = BackgroundWork {
        shell_live: bg_shell_live,
        subagent_live,
    };
    let background_live = background.any();
    if background_live {
        tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
    } else {
        clear_run_state(pane);
    }
    mark_task_reset(pane);
    set_status(pane, resolve_stop_status(background_live));
    mark_turn_settled(pane, prompt_id);
    background
}

pub(in crate::cli::hook) fn on_stop(
    pane: &str,
    ctx: &AgentContext<'_>,
    last_message: &str,
    response: Option<&str>,
    prompt_id: Option<&str>,
    children_may_outlive_turn: bool,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    if !turn_end_is_current(pane, prompt_id) {
        if let Some(resp) = response {
            println!("{resp}");
        }
        return 0;
    }
    if !last_message.is_empty() {
        let msg = sanitize_tmux_value(last_message);
        tmux::set_pane_option(pane, tmux::PANE_PROMPT, &msg);
        tmux::set_pane_option(pane, tmux::PANE_PROMPT_SOURCE, "response");
    }
    let background = settle_turn_state(pane, ctx, prompt_id, children_may_outlive_turn);
    let notification_body = stop_body(last_message);

    if background.subagent_live
        && notifications.enabled
        && notifications.event_enabled(desktop_notification::DesktopNotificationEvent::Stop)
    {
        tmux::set_pane_option(
            pane,
            tmux::PANE_PENDING_STOP_NOTIFICATION_BODY,
            &sanitize_tmux_value(&notification_body),
        );
    } else {
        tmux::unset_pane_option(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY);
    }

    if !background.any() {
        let _ = notify_stop(
            pane,
            NotifyLabels::FromCtx(ctx),
            notifications,
            &notification_body,
        );
    }
    if let Some(resp) = response {
        println!("{resp}");
    }
    0
}

pub(in crate::cli::hook) fn on_turn_settled(
    pane: &str,
    ctx: &AgentContext<'_>,
    prompt_id: Option<&str>,
    children_may_outlive_turn: bool,
) -> i32 {
    if turn_end_is_current(pane, prompt_id) {
        settle_turn_state(pane, ctx, prompt_id, children_may_outlive_turn);
    }
    0
}

pub(in crate::cli::hook) fn on_stop_failure(
    pane: &str,
    ctx: &AgentContext<'_>,
    error: &str,
    prompt_id: Option<&str>,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    if !turn_end_is_current(pane, prompt_id) {
        return 0;
    }
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
    mark_task_reset(pane);
    if !error.is_empty() {
        tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, error);
    }
    set_status(pane, "error");
    mark_turn_settled(pane, prompt_id);
    let _ = notify_lifecycle(
        pane,
        NotifyLabels::FromCtx(ctx),
        notifications,
        None,
        NotifyPayload {
            kind: DesktopNotificationKind::TaskFailed,
            event: desktop_notification::DesktopNotificationEvent::StopFailure,
            fingerprint_suffix: stop_failure_fingerprint(error),
            body: &stop_failure_body(error),
        },
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
    let _ = notify_lifecycle(
        pane,
        NotifyLabels::FromPane { agent: agent_name },
        notifications,
        None,
        NotifyPayload {
            kind: DesktopNotificationKind::TaskCompleted,
            event: desktop_notification::DesktopNotificationEvent::TaskCompleted,
            fingerprint_suffix: task_completed_fingerprint(task_id, task_subject),
            body: &task_completed_body(task_subject),
        },
    );
    0
}

#[cfg(test)]
mod tests {
    use super::super::session::on_session_start;
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
        let exit = on_user_prompt_submit(pane, &ctx, "fix the bug", false, None);
        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("fix the bug")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_SOURCE).as_deref(),
            Some("user")
        );
        assert!(tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
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
        on_user_prompt_submit(
            pane,
            &ctx,
            "<system-reminder>ignore me</system-reminder>",
            true,
            None,
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_PROMPT),
            "system messages should not be stored as user prompt"
        );
        // But status should still advance to running.
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
    }

    #[test]
    fn on_user_prompt_submit_clears_stale_wait_reason_but_preserves_bg_cmd() {
        let _guard = tmux::test_mock::install();
        let pane = "%PROMPT_CLEAR_WAIT";
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "permission");
        tmux::test_mock::set(pane, tmux::PANE_BG_CMD, "npm run dev");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_user_prompt_submit(pane, &ctx, "new prompt", false, None);
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_BG_CMD).as_deref(),
            Some("npm run dev"),
            "bg command must survive a new user turn — the shell is still running",
        );
    }

    #[test]
    fn on_stop_with_background_shell_sets_background_status() {
        let _guard = tmux::test_mock::install();
        let pane = "%STOP_BG";
        tmux::test_mock::set(pane, tmux::PANE_BG_CMD, "npm run dev");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "123");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };

        let exit = on_stop(
            pane,
            &ctx,
            "",
            None,
            None,
            false,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background")
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY),
            "shell-only background work must not queue a completion notification"
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STARTED_AT).as_deref(),
            Some("123")
        );
    }

    #[test]
    fn on_stop_without_background_shell_sets_idle_status() {
        let _guard = tmux::test_mock::install();
        let pane = "%STOP_IDLE";
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "123");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };

        on_stop(
            pane,
            &ctx,
            "",
            None,
            None,
            false,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
    }

    #[test]
    fn on_stop_clears_stale_subagents_for_non_grok_agents() {
        let _guard = tmux::test_mock::install();
        for (agent, pane) in [
            (tmux::CLAUDE_AGENT, "%STOP_STALE_CLAUDE"),
            (tmux::CODEX_AGENT, "%STOP_STALE_CODEX"),
            (tmux::OPENCODE_AGENT, "%STOP_STALE_OPENCODE"),
        ] {
            tmux::test_mock::set(
                pane,
                tmux::PANE_SUBAGENTS,
                "general-purpose:sub-1,general-purpose:sub-2",
            );
            let ctx = AgentContext {
                agent,
                cwd: "/repo",
                permission_mode: "default",
                worktree: &None,
                session_id: &None,
            };

            on_stop(
                pane,
                &ctx,
                "",
                None,
                None,
                false,
                &desktop_notification::DesktopNotificationSettings {
                    enabled: false,
                    events: Default::default(),
                },
            );

            assert!(
                !tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS),
                "{agent} Stop must clear stale subagent list"
            );
            assert_eq!(
                tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
                Some("idle"),
                "{agent} Stop must not remain in background"
            );
        }
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
            None,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );
        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("error")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("rate_limit")
        );
    }

    #[test]
    fn user_prompt_tracks_current_prompt_id() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_PROMPT_ID";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };

        on_user_prompt_submit(pane, &ctx, "new turn", false, Some("prompt-new"));

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_ID).as_deref(),
            Some(encode_prompt_id("prompt-new").as_str())
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_TURN_ACTIVE).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn opaque_prompt_ids_do_not_collide_after_tmux_storage() {
        let _guard = tmux::test_mock::install();
        let pane = "%OPAQUE_PROMPT_ID";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_user_prompt_submit(pane, &ctx, "new", false, Some("prompt 1"));
        assert!(!turn_end_is_current(pane, Some("prompt|1")));

        on_turn_settled(pane, &ctx, Some("prompt|1"), true);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running"),
            "a stale opaque id must not settle the current turn"
        );
    }

    #[test]
    fn turn_settled_keeps_prompt_as_user_input_and_clears_run_state() {
        let _guard = tmux::test_mock::install();
        let pane = "%TURN_SETTLED";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_user_prompt_submit(pane, &ctx, "cancel me", false, Some("prompt-1"));
        on_turn_settled(pane, &ctx, Some("prompt-1"), true);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("cancel me")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_SOURCE).as_deref(),
            Some("user")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_ID).as_deref(),
            Some(encode_prompt_id("prompt-1").as_str())
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_TURN_ACTIVE));
    }

    #[test]
    fn turn_settled_preserves_grok_background_subagent() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_BACKGROUND_SUBAGENT";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "explore:explore");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };

        on_turn_settled(pane, &ctx, None, true);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("explore:explore")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background")
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY),
            "non-notifying TurnSettled events must not queue a completion notification"
        );
    }

    #[test]
    fn stop_with_grok_subagent_defers_completion_notification() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_DEFERRED_STOP_NOTIFICATION";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "explore:sub-1");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: true,
            events: [desktop_notification::DesktopNotificationEvent::Stop]
                .into_iter()
                .collect(),
        };

        on_stop(
            pane,
            &ctx,
            "background child still working",
            None,
            None,
            true,
            &notifications,
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY).as_deref(),
            Some("background child still working")
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_OS_NOTIFY_TASK_COMPLETED),
            "parent Stop must not notify before the final child exits"
        );
    }

    #[test]
    fn stale_turn_end_does_not_settle_newer_prompt() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_STALE_STOP";
        tmux::test_mock::set(pane, tmux::PANE_PROMPT_ID, &encode_prompt_id("prompt-new"));
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "123");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };

        on_stop(
            pane,
            &ctx,
            "old response",
            None,
            Some("prompt-old"),
            true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_ID).as_deref(),
            Some(encode_prompt_id("prompt-new").as_str())
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STARTED_AT).as_deref(),
            Some("123")
        );
    }

    #[test]
    fn matching_turn_end_settles_and_retains_prompt_identity() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_MATCHING_STOP";
        tmux::test_mock::set(
            pane,
            tmux::PANE_PROMPT_ID,
            &encode_prompt_id("prompt-current"),
        );
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        tmux::test_mock::set(pane, tmux::PANE_TURN_ACTIVE, "1");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };

        on_stop(
            pane,
            &ctx,
            "done",
            None,
            Some("prompt-current"),
            true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_ID).as_deref(),
            Some(encode_prompt_id("prompt-current").as_str())
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_TURN_ACTIVE));
    }

    #[test]
    fn stale_turn_failure_does_not_replace_newer_running_state() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_STALE_FAILURE";
        tmux::test_mock::set(pane, tmux::PANE_PROMPT_ID, &encode_prompt_id("prompt-new"));
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };

        on_stop_failure(
            pane,
            &ctx,
            "old failure",
            Some("prompt-old"),
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_ID).as_deref(),
            Some(encode_prompt_id("prompt-new").as_str())
        );
    }

    #[test]
    fn stale_failure_does_not_replace_newer_settled_turn() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_STALE_AFTER_SETTLED";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };

        on_user_prompt_submit(pane, &ctx, "new turn", false, Some("prompt-new"));
        on_stop(
            pane,
            &ctx,
            "new response",
            None,
            Some("prompt-new"),
            true,
            &notifications,
        );
        on_stop_failure(
            pane,
            &ctx,
            "old failure",
            Some("prompt-old"),
            &notifications,
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("new response")
        );
    }

    #[test]
    fn duplicate_failure_does_not_replace_successful_stop() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_DUPLICATE_AFTER_STOP";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };

        on_user_prompt_submit(pane, &ctx, "turn", false, Some("prompt-1"));
        on_stop(
            pane,
            &ctx,
            "success",
            None,
            Some("prompt-1"),
            true,
            &notifications,
        );
        on_stop_failure(pane, &ctx, "late failure", Some("prompt-1"), &notifications);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("success")
        );
    }

    #[test]
    fn identified_end_does_not_settle_active_idless_turn() {
        let _guard = tmux::test_mock::install();
        let pane = "%ACTIVE_IDLESS_TURN";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };

        on_user_prompt_submit(pane, &ctx, "current idless turn", false, None);
        on_stop(
            pane,
            &ctx,
            "stale identified response",
            None,
            Some("prompt-old"),
            true,
            &notifications,
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("current idless turn")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_TURN_ACTIVE).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn identified_stop_does_not_mutate_reset_session_before_first_prompt() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_RESET_STALE_STOP";
        let old_session_id = Some("session-old".into());
        let old_ctx = AgentContext {
            agent: "grok",
            cwd: "/repo/old",
            permission_mode: "auto",
            worktree: &None,
            session_id: &old_session_id,
        };
        let new_session_id = Some("session-new".into());
        let new_ctx = AgentContext {
            agent: "grok",
            cwd: "/repo/new",
            permission_mode: "auto",
            worktree: &None,
            session_id: &new_session_id,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };

        on_user_prompt_submit(pane, &old_ctx, "old prompt", false, Some("prompt-old"));
        on_session_start(pane, &new_ctx, "clear", true);
        on_stop(
            pane,
            &old_ctx,
            "old response",
            None,
            Some("prompt-old"),
            true,
            &notifications,
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_PROMPT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_PROMPT_ID));
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_CWD).as_deref(),
            Some("/repo/new")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SESSION_ID).as_deref(),
            Some("session-new")
        );
    }

    #[test]
    fn identified_failure_does_not_mutate_reset_session_before_first_prompt() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_RESET_STALE_FAILURE";
        let old_session_id = Some("session-old".into());
        let old_ctx = AgentContext {
            agent: "grok",
            cwd: "/repo/old",
            permission_mode: "auto",
            worktree: &None,
            session_id: &old_session_id,
        };
        let new_session_id = Some("session-new".into());
        let new_ctx = AgentContext {
            agent: "grok",
            cwd: "/repo/new",
            permission_mode: "auto",
            worktree: &None,
            session_id: &new_session_id,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };

        on_user_prompt_submit(pane, &old_ctx, "old prompt", false, Some("prompt-old"));
        on_session_start(pane, &new_ctx, "clear", true);
        on_stop_failure(
            pane,
            &old_ctx,
            "old failure",
            Some("prompt-old"),
            &notifications,
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_PROMPT_ID));
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_CWD).as_deref(),
            Some("/repo/new")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SESSION_ID).as_deref(),
            Some("session-new")
        );
    }

    #[test]
    fn idless_session_backstop_settles_active_identified_turn() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_IDLE_BACKSTOP";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &None,
        };

        on_user_prompt_submit(
            pane,
            &ctx,
            "turn missing its end report",
            false,
            Some("prompt-1"),
        );
        on_turn_settled(pane, &ctx, None, true);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_TURN_ACTIVE));
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT_ID).as_deref(),
            Some(encode_prompt_id("prompt-1").as_str())
        );
    }
}
