use crate::cli::{set_attention, set_status};
use crate::desktop_notification;
use crate::desktop_notification::DesktopNotificationKind;
use crate::tmux;

use super::super::context::{AgentContext, pane_tracks_session, set_agent_meta};
use super::super::notifications::{
    NotifyLabels, NotifyPayload, notification_body, notification_fingerprint, notify_lifecycle,
};
use super::status_priority::resolve_notification_status;

fn settled_failed_parent_with_children(pane: &str) -> bool {
    // Child permission hooks still raise attention and notifications, but
    // must not erase the parent turn's terminal failure state.
    tmux::get_pane_option_value(pane, tmux::PANE_STATUS) == "error"
        && tmux::get_pane_option_value(pane, tmux::PANE_TURN_ACTIVE).is_empty()
        && !tmux::get_pane_option_value(pane, tmux::PANE_SUBAGENTS).is_empty()
}

pub(in crate::cli::hook) fn on_notification(
    pane: &str,
    ctx: &AgentContext<'_>,
    wait_reason: &str,
    meta_only: bool,
    requires_existing_session: bool,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    if requires_existing_session && !pane_tracks_session(pane, ctx.agent, ctx.session_id.as_deref())
    {
        return 0;
    }
    set_agent_meta(pane, ctx);
    if meta_only {
        return 0;
    }
    let preserve_parent_failure = settled_failed_parent_with_children(pane);
    if !preserve_parent_failure {
        let bg_shell_live = !tmux::get_pane_option_value(pane, tmux::PANE_BG_CMD).is_empty();
        set_status(
            pane,
            resolve_notification_status(wait_reason, bg_shell_live),
        );
        if wait_reason.is_empty() {
            tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
        } else {
            tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, wait_reason);
        }
    }
    set_attention(pane, "notification");
    let _ = notify_lifecycle(
        pane,
        NotifyLabels::FromCtx(ctx),
        notifications,
        None,
        NotifyPayload {
            kind: DesktopNotificationKind::PermissionRequired,
            event: desktop_notification::DesktopNotificationEvent::Notification,
            fingerprint_suffix: notification_fingerprint(wait_reason),
            body: &notification_body(wait_reason),
        },
    );
    0
}

pub(in crate::cli::hook) fn on_permission_denied(
    pane: &str,
    ctx: &AgentContext<'_>,
    requires_existing_session: bool,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    if requires_existing_session && !pane_tracks_session(pane, ctx.agent, ctx.session_id.as_deref())
    {
        return 0;
    }
    set_agent_meta(pane, ctx);
    let preserve_parent_failure = settled_failed_parent_with_children(pane);
    if !preserve_parent_failure {
        set_status(pane, "waiting");
        tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, "permission_denied");
    }
    set_attention(pane, "notification");
    let _ = notify_lifecycle(
        pane,
        NotifyLabels::FromCtx(ctx),
        notifications,
        None,
        NotifyPayload {
            kind: DesktopNotificationKind::PermissionRequired,
            event: desktop_notification::DesktopNotificationEvent::PermissionDenied,
            fingerprint_suffix: "permission_denied",
            body: "Permission required",
        },
    );
    0
}

pub(in crate::cli::hook) fn on_teammate_idle(
    pane: &str,
    teammate_name: &str,
    idle_reason: &str,
) -> i32 {
    set_attention(pane, "notification");
    let reason = if idle_reason.is_empty() {
        format!("teammate_idle:{teammate_name}")
    } else {
        format!("teammate_idle:{teammate_name}:{idle_reason}")
    };
    tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, &reason);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_teammate_idle_sets_attention_and_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%TEAM";
        let exit = on_teammate_idle(pane, "alice", "");
        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_ATTENTION).as_deref(),
            Some("notification")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("teammate_idle:alice")
        );
    }

    #[test]
    fn on_teammate_idle_includes_idle_reason_when_present() {
        let _guard = tmux::test_mock::install();
        let pane = "%TEAM_REASON";
        on_teammate_idle(pane, "alice", "tokens_exhausted");
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("teammate_idle:alice:tokens_exhausted")
        );
    }

    #[test]
    fn on_notification_meta_only_skips_status_and_attention() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_META";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "permission",
            /* meta_only */ true,
            /* requires_existing_session */ false,
            &notifications,
        );
        // meta_only=true must short-circuit before status/attention/wait_reason writes.
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STATUS));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_ATTENTION));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
        // Agent meta should still be applied so the sidebar can render the pane.
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_AGENT).as_deref(),
            Some("claude")
        );
    }

    #[test]
    fn on_notification_sets_waiting_status_and_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_WAIT";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "permission",
            /* meta_only */ false,
            /* requires_existing_session */ false,
            &notifications,
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_ATTENTION).as_deref(),
            Some("notification")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("permission")
        );
    }

    #[test]
    fn notification_requiring_existing_session_does_not_recreate_torn_down_pane() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_AFTER_SESSION_END";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &Some("ended-session".into()),
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };

        on_notification(
            pane,
            &ctx,
            "permission_prompt",
            /* meta_only */ false,
            /* requires_existing_session */ true,
            &notifications,
        );

        for key in [
            tmux::PANE_AGENT,
            tmux::PANE_STATUS,
            tmux::PANE_ATTENTION,
            tmux::PANE_WAIT_REASON,
            tmux::PANE_SESSION_ID,
        ] {
            assert!(
                !tmux::test_mock::contains(pane, key),
                "delayed notification recreated {key}"
            );
        }
    }

    #[test]
    fn notification_requiring_existing_session_rejects_previous_session_id() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_PREVIOUS_SESSION";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "grok");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "current-session");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "idle");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/old-repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &Some("previous-session".into()),
        };

        on_notification(
            pane,
            &ctx,
            "permission_prompt",
            /* meta_only */ false,
            /* requires_existing_session */ true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SESSION_ID).as_deref(),
            Some("current-session")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_ATTENTION));
    }

    #[test]
    fn notification_requiring_existing_session_accepts_matching_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_CURRENT_SESSION";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "grok");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "current-session");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &Some("current-session".into()),
        };

        on_notification(
            pane,
            &ctx,
            "permission_prompt",
            /* meta_only */ false,
            /* requires_existing_session */ true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_ATTENTION).as_deref(),
            Some("notification")
        );
    }

    #[test]
    fn on_notification_keeps_background_when_softer_reason_and_bg_shell_live() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_BG_PREEMPT";
        tmux::test_mock::set(pane, tmux::PANE_BG_CMD, "cargo test");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "auth_success",
            /* meta_only */ false,
            /* requires_existing_session */ false,
            &notifications,
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background"),
        );
    }

    #[test]
    fn on_notification_permission_reason_preempts_background() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_PERM_OVER_BG";
        tmux::test_mock::set(pane, tmux::PANE_BG_CMD, "cargo test");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "permission_prompt",
            /* meta_only */ false,
            /* requires_existing_session */ false,
            &notifications,
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting"),
        );
    }

    #[test]
    fn on_notification_plain_permission_preempts_background() {
        // Claude's real `notification_type: "permission"` payload must
        // stay in `waiting` even with a live bg shell — the user has to
        // act on the prompt regardless.
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_PERM_PLAIN_OVER_BG";
        tmux::test_mock::set(pane, tmux::PANE_BG_CMD, "cargo test");
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "permission",
            /* meta_only */ false,
            /* requires_existing_session */ false,
            &notifications,
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting"),
        );
    }

    #[test]
    fn on_notification_soft_reason_without_bg_still_sets_waiting() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_SOFT_NO_BG";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "auth_success",
            /* meta_only */ false,
            /* requires_existing_session */ false,
            &notifications,
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting"),
        );
    }

    #[test]
    fn on_notification_empty_wait_reason_clears_stale_value() {
        // Regression: an empty wait_reason used to be a no-op, which
        // left the previously-written reason on the pane. A later
        // notification that genuinely has no reason must drop the
        // stale one so the sidebar does not keep rendering the wrong
        // cause.
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_STALE";
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "permission");

        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_notification(
            pane,
            &ctx,
            "",
            /* meta_only */ false,
            /* requires_existing_session */ false,
            &notifications,
        );

        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON),
            "empty wait_reason must clear a prior value"
        );
    }

    #[test]
    fn on_permission_denied_records_permission_denied_wait_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%PD";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        let notifications = desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        };
        on_permission_denied(
            pane,
            &ctx,
            /* requires_existing_session */ false,
            &notifications,
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("permission_denied")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting")
        );
    }

    #[test]
    fn permission_denied_requiring_existing_session_does_not_recreate_torn_down_pane() {
        let _guard = tmux::test_mock::install();
        let pane = "%PD_AFTER_SESSION_END";
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &Some("ended-session".into()),
        };

        on_permission_denied(
            pane,
            &ctx,
            /* requires_existing_session */ true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        for key in [
            tmux::PANE_AGENT,
            tmux::PANE_STATUS,
            tmux::PANE_ATTENTION,
            tmux::PANE_WAIT_REASON,
            tmux::PANE_SESSION_ID,
        ] {
            assert!(
                !tmux::test_mock::contains(pane, key),
                "delayed permission denial recreated {key}"
            );
        }
    }

    #[test]
    fn permission_denied_requiring_existing_session_accepts_tracked_child() {
        let _guard = tmux::test_mock::install();
        let pane = "%PD_CURRENT_CHILD";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "grok");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "host-session");
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Review PR:child-session");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "auto",
            worktree: &None,
            session_id: &Some("child-session".into()),
        };

        on_permission_denied(
            pane,
            &ctx,
            /* requires_existing_session */ true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_ATTENTION).as_deref(),
            Some("notification")
        );
    }

    #[test]
    fn on_notification_preserves_settled_parent_failure() {
        let _guard = tmux::test_mock::install();
        let pane = "%NOTIF_FAILED_PARENT";
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "error");
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "rate_limit");
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "grok");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "host-session");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &Some("host-session".into()),
        };

        on_notification(
            pane,
            &ctx,
            "permission",
            /* meta_only */ false,
            /* requires_existing_session */ true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("error")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("rate_limit")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_ATTENTION).as_deref(),
            Some("notification")
        );
    }

    #[test]
    fn on_permission_denied_preserves_settled_parent_failure() {
        let _guard = tmux::test_mock::install();
        let pane = "%PD_FAILED_PARENT";
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "error");
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "rate_limit");
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "grok");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "host-session");
        let ctx = AgentContext {
            agent: "grok",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &Some("host-session".into()),
        };

        on_permission_denied(
            pane,
            &ctx,
            /* requires_existing_session */ true,
            &desktop_notification::DesktopNotificationSettings {
                enabled: false,
                events: Default::default(),
            },
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("error")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("rate_limit")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_ATTENTION).as_deref(),
            Some("notification")
        );
    }
}
