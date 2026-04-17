use crate::tmux;
use crate::ui::text::wait_reason_label;
use crate::{desktop_notification, desktop_notification::DesktopNotificationKind};

use super::context::now_epoch_millis;

pub(super) fn notification_settings() -> desktop_notification::DesktopNotificationSettings {
    desktop_notification::DesktopNotificationSettings::from_tmux()
}

pub(super) fn set_notification_run_id(pane: &str) {
    tmux::set_pane_option(
        pane,
        "@pane_notification_run_id",
        &now_epoch_millis().to_string(),
    );
}

pub(super) fn notification_run_id(pane: &str) -> Option<u64> {
    tmux::get_pane_option_value(pane, "@pane_notification_run_id")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_run_id_reads_tmux_option() {
        let _guard = tmux::test_mock::install();
        let pane = "%PANE_STARTED";
        tmux::test_mock::set(pane, "@pane_notification_run_id", "1700000123456");
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
        let written = tmux::test_mock::get(pane, "@pane_notification_run_id");
        assert!(
            written
                .as_deref()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some(),
            "expected a millisecond timestamp to be written"
        );
    }
}
