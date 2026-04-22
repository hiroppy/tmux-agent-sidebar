use crate::tmux;
use crate::ui::text::wait_reason_label;
use crate::{desktop_notification, desktop_notification::DesktopNotificationKind};

use super::context::{
    AgentContext, branch_label_from_ctx, branch_label_from_pane, now_epoch_millis,
    repo_label_from_ctx, repo_label_from_pane,
};

/// How to resolve the repo/branch labels and agent name that appear in
/// the desktop-notification title. The handlers split cleanly in two:
/// SessionStart-ish events have a full [`AgentContext`] from the
/// payload; later-in-lifecycle events (TaskCompleted, SessionEnd) only
/// have the pane id and an agent-name string, so they read the labels
/// back out of the pane options that earlier events wrote.
pub(super) enum NotifyLabels<'a> {
    FromCtx(&'a AgentContext<'a>),
    FromPane { agent: &'a str },
}

/// Content half of [`notify_lifecycle`]: what to say, not who to say it
/// to. Bundled into a struct so the helper stays under clippy's
/// `too_many_arguments` threshold.
pub(super) struct NotifyPayload<'a> {
    pub kind: DesktopNotificationKind,
    pub event: desktop_notification::DesktopNotificationEvent,
    pub fingerprint_suffix: &'a str,
    pub body: &'a str,
}

/// Fire a lifecycle desktop notification (`Notification`,
/// `PermissionDenied`, `Stop`, `StopFailure`, `TaskCompleted`,
/// `SessionEnd`). Resolves labels, computes the run-scoped fingerprint,
/// and delegates to [`notify_desktop`].
///
/// `run_id: None` triggers a fresh `notification_run_id(pane)` lookup;
/// `Some(id)` reuses a caller-cached value (on_stop already fetches it
/// to check `has_run_scoped_stamp`, so re-fetching here would be an
/// extra tmux subprocess call per Stop event).
pub(super) fn notify_lifecycle(
    pane: &str,
    labels: NotifyLabels<'_>,
    settings: &desktop_notification::DesktopNotificationSettings,
    run_id: Option<u64>,
    payload: NotifyPayload<'_>,
) -> bool {
    let (repo, branch, agent) = match labels {
        NotifyLabels::FromCtx(ctx) => (
            repo_label_from_ctx(ctx),
            branch_label_from_ctx(ctx),
            ctx.agent,
        ),
        NotifyLabels::FromPane { agent } => (
            repo_label_from_pane(pane),
            branch_label_from_pane(pane),
            agent,
        ),
    };
    let run_id = run_id.or_else(|| notification_run_id(pane));
    let fingerprint =
        desktop_notification::run_scoped_fingerprint(run_id, payload.fingerprint_suffix);
    notify_desktop(
        pane,
        payload.kind,
        payload.event,
        settings,
        &fingerprint,
        &desktop_notification::format_title(repo.as_deref(), branch.as_deref(), agent),
        payload.body,
    )
}

pub(super) fn notification_settings() -> desktop_notification::DesktopNotificationSettings {
    desktop_notification::DesktopNotificationSettings::from_tmux()
}

pub(super) fn set_notification_run_id(pane: &str) {
    tmux::set_pane_option(
        pane,
        tmux::PANE_NOTIFICATION_RUN_ID,
        &now_epoch_millis().to_string(),
    );
}

pub(super) fn notification_run_id(pane: &str) -> Option<u64> {
    tmux::get_pane_option_value(pane, tmux::PANE_NOTIFICATION_RUN_ID)
        .parse::<u64>()
        .ok()
}

pub(super) fn notify_desktop(
    pane: &str,
    kind: DesktopNotificationKind,
    event: desktop_notification::DesktopNotificationEvent,
    settings: &desktop_notification::DesktopNotificationSettings,
    fingerprint: &str,
    title: &str,
    body: &str,
) -> bool {
    desktop_notification::notify_if_allowed(settings, pane, kind, event, fingerprint, title, body)
}

pub(super) fn task_completed_fingerprint<'a>(task_id: &'a str, task_subject: &'a str) -> &'a str {
    if !task_id.is_empty() {
        task_id
    } else if !task_subject.is_empty() {
        task_subject
    } else {
        "task-completed"
    }
}

pub(super) fn task_completed_body(task_subject: &str) -> String {
    if task_subject.is_empty() {
        "Task completed".to_string()
    } else {
        format!("Task completed: {task_subject}")
    }
}

pub(super) const NOTIFICATION_BODY_MAX_CHARS: usize = 240;

pub(super) fn stop_body(last_message: &str) -> String {
    let trimmed = last_message.trim();
    if trimmed.is_empty() {
        "Task completed".to_string()
    } else {
        truncate_body(trimmed)
    }
}

pub(super) fn truncate_body(text: &str) -> String {
    if text.chars().count() <= NOTIFICATION_BODY_MAX_CHARS {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(NOTIFICATION_BODY_MAX_CHARS).collect();
        format!("{truncated}…")
    }
}

pub(super) fn notification_fingerprint(wait_reason: &str) -> &str {
    if wait_reason.is_empty() {
        "notification"
    } else {
        wait_reason
    }
}

pub(super) fn notification_body(wait_reason: &str) -> String {
    if wait_reason.is_empty() {
        "Permission required".to_string()
    } else {
        wait_reason_label(wait_reason)
    }
}

pub(super) fn stop_failure_fingerprint(error: &str) -> &str {
    if error.is_empty() {
        "task-failed"
    } else {
        error
    }
}

pub(super) fn stop_failure_body(error: &str) -> String {
    if error.is_empty() {
        "Task failed".to_string()
    } else {
        format!("Task failed: {error}")
    }
}

pub(super) fn session_end_fingerprint(end_reason: &str) -> String {
    if end_reason.is_empty() {
        "session-ended".to_string()
    } else {
        format!("session-ended:{end_reason}")
    }
}

pub(super) fn session_end_body(end_reason: &str) -> String {
    if end_reason.is_empty() {
        "Session ended".to_string()
    } else {
        format!("Session ended: {end_reason}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_run_id_reads_tmux_option() {
        let _guard = tmux::test_mock::install();
        let pane = "%PANE_STARTED";
        tmux::test_mock::set(pane, tmux::PANE_NOTIFICATION_RUN_ID, "1700000123456");
        assert_eq!(notification_run_id(pane), Some(1_700_000_123_456));
    }

    #[test]
    fn notification_task_completed_helpers_choose_expected_values() {
        assert_eq!(task_completed_fingerprint("id-1", "subject"), "id-1");
        assert_eq!(task_completed_fingerprint("", "subject"), "subject");
        assert_eq!(task_completed_fingerprint("", ""), "task-completed");
        assert_eq!(task_completed_body("subject"), "Task completed: subject");
        assert_eq!(task_completed_body(""), "Task completed");
    }

    #[test]
    fn notification_stop_failure_helpers_choose_expected_values() {
        assert_eq!(stop_failure_fingerprint("boom"), "boom");
        assert_eq!(stop_failure_fingerprint(""), "task-failed");
        assert_eq!(stop_failure_body("boom"), "Task failed: boom");
        assert_eq!(stop_failure_body(""), "Task failed");
    }

    #[test]
    fn notification_session_end_helpers_choose_expected_values() {
        assert_eq!(session_end_fingerprint("logout"), "session-ended:logout");
        assert_eq!(session_end_fingerprint(""), "session-ended");
        assert_eq!(session_end_body("logout"), "Session ended: logout");
        assert_eq!(session_end_body(""), "Session ended");
    }

    #[test]
    fn stop_body_falls_back_to_placeholder_when_empty() {
        assert_eq!(stop_body(""), "Task completed");
        assert_eq!(stop_body("   \n"), "Task completed");
    }

    #[test]
    fn stop_body_uses_last_message_when_present() {
        assert_eq!(
            stop_body("Fixed the bug in main.rs"),
            "Fixed the bug in main.rs"
        );
    }

    #[test]
    fn stop_body_truncates_long_message() {
        let long = "a".repeat(NOTIFICATION_BODY_MAX_CHARS + 50);
        let body = stop_body(&long);
        assert_eq!(body.chars().count(), NOTIFICATION_BODY_MAX_CHARS + 1);
        assert!(body.ends_with('…'));
    }

    #[test]
    fn set_notification_run_id_writes_millis_value() {
        let _guard = tmux::test_mock::install();
        let pane = "%PANE_SET_RUN_ID";
        set_notification_run_id(pane);
        let written = tmux::test_mock::get(pane, tmux::PANE_NOTIFICATION_RUN_ID);
        assert!(
            written
                .as_deref()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some(),
            "expected a millisecond timestamp to be written"
        );
    }
}
