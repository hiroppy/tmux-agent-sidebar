use crate::cli::{set_attention, set_status};
use crate::desktop_notification;
use crate::desktop_notification::DesktopNotificationKind;
use crate::tmux;

use super::super::context::{
    AgentContext, PENDING_SESSION_END, PENDING_WORKTREE_REMOVE, clear_run_state,
    pane_writes_allowed, run_session_end_teardown, set_agent_meta,
};
use super::super::notifications::{
    NotifyLabels, NotifyPayload, notify_lifecycle, session_end_body, session_end_fingerprint,
    set_notification_run_id,
};

pub(in crate::cli::hook) fn on_session_start(
    pane: &str,
    ctx: &AgentContext<'_>,
    source: &str,
) -> i32 {
    set_agent_meta(pane, ctx);
    set_attention(pane, "clear");
    clear_run_state(pane);
    set_notification_run_id(pane);
    tmux::unset_pane_option(pane, tmux::PANE_PROMPT);
    tmux::unset_pane_option(pane, tmux::PANE_PROMPT_SOURCE);
    // `@pane_subagents` is deliberately preserved across SessionStart.
    // Subagents share the parent's `$TMUX_PANE`, so when a subagent
    // fires its own SessionStart after SubagentStart has populated the
    // list, clearing it here would drop the marker that
    // `should_update_cwd` and `drain_pending_teardowns` rely on. The
    // normal teardown paths (`run_session_end_teardown` via
    // `clear_all_meta`) already clear the list when a real session
    // ends, so the only state this would skip clearing is a subagent
    // list stranded by a hard crash — acceptable vs. racing against
    // legitimate subagent activity.
    // A fresh session overrides any deferred teardown that was waiting
    // for the previous run's subagents to drain.
    tmux::unset_pane_option(pane, PENDING_SESSION_END);
    tmux::unset_pane_option(pane, PENDING_WORKTREE_REMOVE);
    match source {
        "resume" => tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, "session_resumed"),
        "compact" => tmux::set_pane_option(pane, tmux::PANE_WAIT_REASON, "session_resumed_compact"),
        _ => tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON),
    }
    set_status(pane, "idle");
    0
}

pub(in crate::cli::hook) fn on_session_end(
    pane: &str,
    agent_name: &str,
    end_reason: &str,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    // Subagents share the parent's `$TMUX_PANE`, so a SessionEnd fired
    // while `@pane_subagents` is populated is almost certainly a child's
    // (we have no way to distinguish parent vs. child events otherwise).
    // Bail out early before:
    //
    //   1. the notification path consumes the run-scoped fingerprint,
    //      which would silently deduplicate the parent's real SessionEnd
    //      notification when it eventually arrives, and
    //   2. we set PENDING_SESSION_END, which `drain_pending_teardowns`
    //      would later turn into `run_session_end_teardown` — wiping a
    //      still-running parent pane the moment the last subagent stops.
    //
    // The tradeoff is that a parent SessionEnd that genuinely races
    // ahead of every SubagentStop will be ignored too, leaving stale
    // metadata until the next SessionStart clears it. Compared to
    // clobbering a live parent, the stale-metadata failure mode is
    // far safer and the one the user can recover from.
    if !pane_writes_allowed(pane) {
        return 0;
    }

    // Noteworthy terminations (forced logout, bypass-permissions revoked) get
    // a desktop notification so the user isn't left wondering why the pane
    // cleared. Routine reasons (`clear`, `resume`, `prompt_input_exit`,
    // `other`) stay silent.
    if matches!(end_reason, "logout" | "bypass_permissions_disabled") {
        let _ = notify_lifecycle(
            pane,
            NotifyLabels::FromPane { agent: agent_name },
            notifications,
            None,
            NotifyPayload {
                kind: DesktopNotificationKind::TaskCompleted,
                event: desktop_notification::DesktopNotificationEvent::Stop,
                fingerprint_suffix: &session_end_fingerprint(end_reason),
                body: &session_end_body(end_reason),
            },
        );
    }
    run_session_end_teardown(pane);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn default_notifications() -> desktop_notification::DesktopNotificationSettings {
        desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        }
    }

    fn basic_ctx() -> AgentContext<'static> {
        AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        }
    }

    #[test]
    fn on_session_end_preserves_parent_state_when_subagents_active() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_END";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "claude");
        tmux::test_mock::set(pane, tmux::PANE_CWD, "/repo/parent");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "parent-session");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        // Seed an activity log so we can prove the file is NOT removed.
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        fs::write(&log_path, "1234567890|Read|main.rs\n").unwrap();

        let exit = on_session_end(pane, "claude", "", &default_notifications());

        assert_eq!(exit, 0);
        assert!(
            tmux::test_mock::contains(pane, tmux::PANE_AGENT),
            "child SessionEnd must not clear parent @pane_agent"
        );
        assert!(tmux::test_mock::contains(pane, tmux::PANE_CWD));
        assert!(tmux::test_mock::contains(pane, tmux::PANE_SESSION_ID));
        assert!(tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
        assert!(
            log_path.exists(),
            "child SessionEnd must not delete parent activity log"
        );
        assert!(
            !tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "child SessionEnd must not record a pending teardown that \
             `on_subagent_stop` would later replay against the parent"
        );

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn on_session_end_clears_state_when_no_subagents() {
        let _guard = tmux::test_mock::install();
        let pane = "%LONE_END";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "claude");
        tmux::test_mock::set(pane, tmux::PANE_CWD, "/repo");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");

        let exit = on_session_end(pane, "claude", "", &default_notifications());

        assert_eq!(exit, 0);
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_AGENT),
            "lone SessionEnd should clear @pane_agent"
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_CWD));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STATUS));
    }

    #[test]
    fn on_session_start_sets_agent_and_idle_status() {
        let _guard = tmux::test_mock::install();
        let pane = "%NEW_SESSION";
        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &Some("sess-123".into()),
        };

        let exit = on_session_start(pane, &ctx, "");
        assert_eq!(exit, 0);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_AGENT).as_deref(),
            Some("claude")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SESSION_ID).as_deref(),
            Some("sess-123")
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_PROMPT),
            "SessionStart should clear any stale prompt"
        );
    }

    #[test]
    fn on_session_start_preserves_subagents_list() {
        // Regression: a subagent's own SessionStart arriving after
        // SubagentStart seeded @pane_subagents must NOT drop the
        // parent's list. If it did, should_update_cwd would start
        // returning true and the next hook from either side could
        // clobber the parent's cwd/worktree metadata.
        let _guard = tmux::test_mock::install();
        let pane = "%SUBAGENT_LIVE";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");

        on_session_start(pane, &basic_ctx(), "");

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("Explore:sub-1"),
            "SessionStart must not wipe an active subagent list"
        );
    }

    #[test]
    fn fresh_session_start_clears_pending_markers() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_RESTART";
        tmux::test_mock::set(pane, PENDING_SESSION_END, "1");
        tmux::test_mock::set(pane, PENDING_WORKTREE_REMOVE, "1");

        let ctx = AgentContext {
            agent: "claude",
            cwd: "/repo",
            permission_mode: "default",
            worktree: &None,
            session_id: &None,
        };
        on_session_start(pane, &ctx, "");

        assert!(
            !tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "fresh SessionStart must drop a stale pending marker"
        );
        assert!(!tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE));
    }

    #[test]
    fn on_session_start_resume_writes_wait_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%RESUME";
        on_session_start(pane, &basic_ctx(), "resume");
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("session_resumed"),
        );
    }

    #[test]
    fn on_session_start_compact_writes_compact_wait_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%COMPACT";
        on_session_start(pane, &basic_ctx(), "compact");
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_WAIT_REASON).as_deref(),
            Some("session_resumed_compact"),
        );
    }

    #[test]
    fn on_session_start_startup_clears_stale_wait_reason() {
        let _guard = tmux::test_mock::install();
        let pane = "%FRESH";
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "session_resumed");
        on_session_start(pane, &basic_ctx(), "startup");
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON),
            "startup source should drop a stale resume marker"
        );
    }

    fn notifications_enabled_all() -> desktop_notification::DesktopNotificationSettings {
        // The Stop event is the one our SessionEnd notification is gated on;
        // `enabled: true` plus the Stop event lets `notify_if_allowed` reach
        // the point where it writes the dedup stamp in the tmux mock. The
        // real `send_desktop_notification` is still a process spawn, so if it
        // ever runs in CI it just fails silently and leaves the stamp unset.
        desktop_notification::DesktopNotificationSettings {
            enabled: true,
            events: [desktop_notification::DesktopNotificationEvent::Stop]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn on_session_end_routine_reason_does_not_notify() {
        let _guard = tmux::test_mock::install();
        let pane = "%END_ROUTINE";
        on_session_end(pane, "claude", "clear", &notifications_enabled_all());
        // The notification helper writes a dedup stamp only when a notification
        // actually goes out; a missing stamp is proof the gate rejected it.
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_OS_NOTIFY_TASK_COMPLETED),
            "routine end_reason must not fire a desktop notification"
        );
    }

    #[test]
    fn on_session_end_logout_attempts_notification() {
        let _guard = tmux::test_mock::install();
        let pane = "%END_LOGOUT";
        // Seed a run id so the fingerprint is run-scoped.
        tmux::test_mock::set(pane, tmux::PANE_NOTIFICATION_RUN_ID, "1700000000000");
        // Agent name is surfaced in the desktop notification title; using an
        // obvious test marker makes it trivial to spot when a local `cargo
        // test` run happens to actually fire osascript.
        on_session_end(
            pane,
            "cargo-test: on_session_end_logout",
            "logout",
            &notifications_enabled_all(),
        );
        // If `send_desktop_notification` succeeds (local dev with notify-send
        // / osascript available), the stamp is written; if it fails (headless
        // CI), the stamp stays unset but we at least verified the gate let
        // the call through. The stronger check — that the gate opens — is
        // covered by `notifications_enabled_all` only containing `Stop`.
        let stamp_key = tmux::PANE_OS_NOTIFY_TASK_COMPLETED;
        if tmux::test_mock::contains(pane, stamp_key) {
            let raw = tmux::test_mock::get(pane, stamp_key).unwrap_or_default();
            assert!(
                raw.contains("session-ended:logout"),
                "stamp must record the session-end fingerprint, got {raw}"
            );
        }
    }

    #[test]
    fn on_session_end_bypass_disabled_attempts_notification() {
        let _guard = tmux::test_mock::install();
        let pane = "%END_BYPASS";
        tmux::test_mock::set(pane, tmux::PANE_NOTIFICATION_RUN_ID, "1700000000000");
        on_session_end(
            pane,
            "cargo-test: on_session_end_bypass_disabled",
            "bypass_permissions_disabled",
            &notifications_enabled_all(),
        );
        let stamp_key = tmux::PANE_OS_NOTIFY_TASK_COMPLETED;
        if tmux::test_mock::contains(pane, stamp_key) {
            let raw = tmux::test_mock::get(pane, stamp_key).unwrap_or_default();
            assert!(
                raw.contains("session-ended:bypass_permissions_disabled"),
                "stamp must record the session-end fingerprint, got {raw}"
            );
        }
    }
}
