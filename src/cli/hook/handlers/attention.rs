use crate::cli::{set_attention, set_status};
use crate::desktop_notification;
use crate::desktop_notification::DesktopNotificationKind;
use crate::tmux;

use super::super::context::{AgentContext, set_agent_meta};
use super::super::notifications::{
    NotifyLabels, NotifyPayload, notification_body, notification_fingerprint, notify_lifecycle,
};

pub(in crate::cli::hook) fn on_notification(
    pane: &str,
    ctx: &AgentContext<'_>,
    wait_reason: &str,
    meta_only: bool,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    if meta_only {
        return 0;
    }
    set_status(pane, "waiting");
    set_attention(pane, "notification");
    if wait_reason.is_empty() {
        // An explicit-but-empty wait_reason is the hook's way of saying
        // "no reason"; drop any prior value so the sidebar doesn't keep
        // rendering a stale cause from the previous notification.
        tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
    } else {
        tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, wait_reason);
    }
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
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_status(pane, "waiting");
    set_attention(pane, "notification");
    tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, "permission_denied");
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
        on_notification(pane, &ctx, "", /* meta_only */ false, &notifications);

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
        on_permission_denied(pane, &ctx, &notifications);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("permission_denied")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("waiting")
        );
    }
}
