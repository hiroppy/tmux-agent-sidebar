//! Central resolver for the sidebar's pane status priority. Hook handlers
//! consult these helpers rather than hard-coding the precedence, so the
//! ordering lives in exactly one place.
//!
//! Priority (highest → lowest):
//!
//! ```text
//! running > permission > background > waiting > idle
//! ```
//!
//! Who enforces each tier:
//! - `running` is set unconditionally by `activity.rs::handle_activity_log`
//!   on any tool use and does not flow through this module — tool activity
//!   always wins because the agent is actively doing work.
//! - `permission` class wait reasons survive as `waiting` even when a
//!   background shell is alive (`resolve_notification_status`), because the
//!   user still has to act on the prompt.
//! - `background` is picked over `waiting` for softer notifications (auth
//!   success, rate limit, teammate idle, session resumed, …) and over
//!   `idle` at Stop time, whenever `@pane_bg_cmd` is non-empty.
//! - `waiting` is the Notification fallback when no bg shell is live.
//! - `idle` is the Stop fallback when no bg shell is live.

/// Wait reasons that demand direct user action and must stay visible as
/// `waiting` even with a live background shell.
pub(in crate::cli::hook) fn is_permission_wait_reason(wait_reason: &str) -> bool {
    matches!(
        wait_reason,
        "permission" | "permission_prompt" | "permission_denied" | "elicitation_dialog"
    )
}

/// Status `Stop` should land in.
pub(in crate::cli::hook) fn resolve_stop_status(bg_shell_live: bool) -> &'static str {
    if bg_shell_live { "background" } else { "idle" }
}

/// Status `Notification` should land in.
pub(in crate::cli::hook) fn resolve_notification_status(
    wait_reason: &str,
    bg_shell_live: bool,
) -> &'static str {
    if bg_shell_live && !is_permission_wait_reason(wait_reason) {
        "background"
    } else {
        "waiting"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_permission_wait_reason_allowlist() {
        assert!(is_permission_wait_reason("permission"));
        assert!(is_permission_wait_reason("permission_prompt"));
        assert!(is_permission_wait_reason("permission_denied"));
        assert!(is_permission_wait_reason("elicitation_dialog"));

        assert!(!is_permission_wait_reason("auth_success"));
        assert!(!is_permission_wait_reason("rate_limit"));
        assert!(!is_permission_wait_reason("session_resumed"));
        assert!(!is_permission_wait_reason("teammate_idle:alice"));
        assert!(!is_permission_wait_reason(""));
    }

    #[test]
    fn resolve_stop_status_prefers_background_when_bg_shell_live() {
        assert_eq!(resolve_stop_status(true), "background");
        assert_eq!(resolve_stop_status(false), "idle");
    }

    #[test]
    fn resolve_notification_status_permission_always_waiting() {
        // Permission class survives regardless of bg shell state.
        assert_eq!(resolve_notification_status("permission", true), "waiting");
        assert_eq!(resolve_notification_status("permission", false), "waiting");
        assert_eq!(
            resolve_notification_status("permission_prompt", true),
            "waiting"
        );
    }

    #[test]
    fn resolve_notification_status_soft_reason_downgrades_to_background_when_bg_live() {
        assert_eq!(
            resolve_notification_status("auth_success", true),
            "background"
        );
        assert_eq!(
            resolve_notification_status("rate_limit", true),
            "background"
        );
        assert_eq!(resolve_notification_status("", true), "background");
    }

    #[test]
    fn resolve_notification_status_soft_reason_without_bg_stays_waiting() {
        assert_eq!(
            resolve_notification_status("auth_success", false),
            "waiting"
        );
        assert_eq!(resolve_notification_status("", false), "waiting");
    }
}
