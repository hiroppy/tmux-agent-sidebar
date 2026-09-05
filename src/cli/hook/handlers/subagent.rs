use crate::cli::{set_attention, set_status};
use crate::desktop_notification;
use crate::time::now_epoch_secs;
use crate::tmux;

use super::super::context::{
    append_subagent, clear_run_state, drain_pending_teardowns, remove_subagent,
};
use super::super::notifications::{NotifyLabels, notify_stop};

pub(in crate::cli::hook) fn on_subagent_start(
    pane: &str,
    agent_type: &str,
    display_name: Option<&str>,
    agent_id: Option<&str>,
    children_may_outlive_turn: bool,
) -> i32 {
    // Claude Code always sends agent_id per the hooks spec; drop the
    // event silently if it's missing so the tree never gains an
    // untrackable entry.
    let Some(id) = agent_id.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let current = tmux::get_pane_option_value(pane, tmux::PANE_SUBAGENTS);
    let new_val = append_subagent(&current, agent_type, display_name, id);
    tmux::set_pane_option(pane, tmux::PANE_SUBAGENTS, &new_val);
    revive_background_for_late_child(pane, children_may_outlive_turn);
    0
}

/// A child's hook process can be delayed past the host `Stop` that ends the
/// same turn. The stop handler then saw an empty child list, settled the
/// pane to `idle` and cleared its timer — so this registration is the first
/// evidence that background work outlived the turn, and the child would
/// otherwise sit invisible behind a "completed" pane until some unrelated
/// event repainted it.
///
/// Only adapters that declare children may outlive the turn revive the
/// lifecycle: where children die with their turn, a start arriving after
/// settlement is stale and must never resurrect a finished pane.
///
/// Only `idle` is upgraded. `running` belongs to a turn that is still live,
/// `waiting` still needs the user (see `status_priority`), `error` must stay
/// visible, and an empty status means the pane is not tracked at all.
///
/// The completion notification `Stop` already fired cannot be retracted, but
/// this path adds no second one: `Stop` left no pending body behind, so the
/// final child stop settles the pane silently.
fn revive_background_for_late_child(pane: &str, children_may_outlive_turn: bool) {
    if !children_may_outlive_turn {
        return;
    }
    let turn_settled = tmux::get_pane_option_value(pane, tmux::PANE_TURN_ACTIVE).is_empty();
    if !turn_settled || tmux::get_pane_option_value(pane, tmux::PANE_STATUS) != "idle" {
        return;
    }
    // `clear_run_state` dropped the submit timestamp when the turn settled;
    // re-arm it so the background elapsed label counts from the moment the
    // sidebar learned the child exists.
    if tmux::get_pane_option_value(pane, tmux::PANE_STARTED_AT).is_empty() {
        tmux::set_pane_option(pane, tmux::PANE_STARTED_AT, &now_epoch_secs().to_string());
    }
    // A resumed or compacted session settles to `idle` while still carrying
    // its `session_resumed*` reason. That reason belongs to the turn that
    // just ended, not to the child starting now, so drop it exactly as the
    // sibling background branch in `on_subagent_stop` does.
    tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
    set_status(pane, "background");
}

pub(in crate::cli::hook) fn on_subagent_stop(
    pane: &str,
    agent_id: Option<&str>,
    children_may_outlive_turn: bool,
    notifications: &desktop_notification::DesktopNotificationSettings,
) -> i32 {
    let Some(id) = agent_id.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let current = tmux::get_pane_option_value(pane, tmux::PANE_SUBAGENTS);
    let drained_to_empty = match remove_subagent(&current, id) {
        None => false,
        Some(new_val) if new_val.is_empty() => {
            tmux::unset_pane_option(pane, tmux::PANE_SUBAGENTS);
            true
        }
        Some(new_val) => {
            tmux::set_pane_option(pane, tmux::PANE_SUBAGENTS, &new_val);
            false
        }
    };
    // Once the last subagent stops, replay any teardown that was deferred
    // because subagents were active when SessionEnd / WorktreeRemove fired.
    if drained_to_empty {
        // Child hooks can temporarily replace `background` with `waiting` or
        // `running`; adapters declare whether the turn marker is a durable
        // parent-settlement signal for this child lifecycle.
        let agent = tmux::get_pane_option_value(pane, tmux::PANE_AGENT);
        let parent_turn_settled = children_may_outlive_turn
            && tmux::get_pane_option_value(pane, tmux::PANE_TURN_ACTIVE).is_empty();
        let deferred_body =
            tmux::get_pane_option_value(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY);
        tmux::unset_pane_option(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY);
        drain_pending_teardowns(pane);

        let status = tmux::get_pane_option_value(pane, tmux::PANE_STATUS);
        let bg_shell_live = !tmux::get_pane_option_value(pane, tmux::PANE_BG_CMD).is_empty();
        if parent_turn_settled && !status.is_empty() && status != "error" {
            set_attention(pane, "clear");
            if bg_shell_live {
                tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
                set_status(pane, "background");
            } else {
                clear_run_state(pane);
                set_status(pane, "idle");
                if !deferred_body.is_empty() {
                    let _ = notify_stop(
                        pane,
                        NotifyLabels::FromPane { agent: &agent },
                        notifications,
                        &deferred_body,
                    );
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::super::run::{on_stop, on_user_prompt_submit};
    use super::super::session::on_session_end;
    use super::super::worktree::on_worktree_remove;
    use super::*;
    use crate::cli::hook::context::{AgentContext, PENDING_SESSION_END, PENDING_WORKTREE_REMOVE};
    use crate::desktop_notification;
    use std::fs;

    fn default_notifications() -> desktop_notification::DesktopNotificationSettings {
        desktop_notification::DesktopNotificationSettings {
            enabled: false,
            events: Default::default(),
        }
    }

    #[test]
    fn on_subagent_start_appends_to_list() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_START";
        on_subagent_start(pane, "Explore", None, Some("sub-1"), false);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("Explore:sub-1")
        );
        on_subagent_start(pane, "Plan", None, Some("sub-2"), false);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("Explore:sub-1,Plan:sub-2")
        );
    }

    #[test]
    fn on_subagent_start_prefers_safe_display_name() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_DESCRIPTION";

        on_subagent_start(
            pane,
            "general-purpose",
            Some("Code review: tests, types\nand errors"),
            Some("01a0380d-9cc4-7312-a767-351c89120226"),
            false,
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("Code review tests types and errors:01a0380d-9cc4-7312-a767-351c89120226")
        );
    }

    #[test]
    fn on_subagent_start_drops_event_without_id() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_NO_ID";
        on_subagent_start(pane, "Explore", None, None, false);
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
        on_subagent_start(pane, "Explore", None, Some(""), false);
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
    }

    // ─── late child-start race ──────────────────────────────────────
    //
    // A child's hook process can lose the race against the host `Stop`
    // that ends the same turn. `Stop` then settles the pane on an empty
    // child list, so the registration that lands afterwards is the only
    // chance to put the pane back into its background lifecycle.

    #[test]
    fn late_subagent_start_revives_background_when_children_outlive_the_turn() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LATE_REVIVE";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "idle");

        let before = crate::time::now_epoch_secs();
        on_subagent_start(pane, "Explore", None, Some("sub-1"), true);
        let after = crate::time::now_epoch_secs();

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background"),
            "a child registering after settlement must not stay hidden behind an idle pane"
        );
        // Pin the value, not just the key: a timer re-armed to 0 or to
        // milliseconds would still "contain" a timestamp while rendering a
        // nonsense elapsed label.
        let started_at: u64 = tmux::test_mock::get(pane, tmux::PANE_STARTED_AT)
            .expect("the background elapsed label needs a re-armed timer")
            .parse()
            .expect("@pane_started_at must stay parseable epoch seconds");
        assert!(
            (before..=after).contains(&started_at),
            "expected a wall-clock re-arm in [{before}, {after}], got {started_at}"
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("Explore:sub-1")
        );
    }

    #[test]
    fn late_subagent_start_leaves_pane_idle_when_children_die_with_the_turn() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LATE_STALE";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "claude");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "idle");

        on_subagent_start(pane, "Explore", None, Some("sub-1"), false);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle"),
            "a child that cannot outlive its turn must never resurrect a settled pane"
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
    }

    #[test]
    fn subagent_start_during_an_active_turn_keeps_the_running_status() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_START_ACTIVE_TURN";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        tmux::test_mock::set(pane, tmux::PANE_TURN_ACTIVE, "1");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "1700");

        on_subagent_start(pane, "Explore", None, Some("sub-1"), true);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STARTED_AT).as_deref(),
            Some("1700"),
            "a live turn keeps the timestamp its prompt submit set"
        );
    }

    #[test]
    fn subagent_start_during_an_active_turn_leaves_an_idle_pane_alone() {
        // Isolates the `@pane_turn_active` gate: the status is already
        // `idle`, so only the turn marker can hold the revival back. The
        // sibling `running` test cannot pin this — its status guard
        // short-circuits first.
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_START_IDLE_ACTIVE_TURN";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "idle");
        tmux::test_mock::set(pane, tmux::PANE_TURN_ACTIVE, "1");

        on_subagent_start(pane, "Explore", None, Some("sub-1"), true);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle"),
            "a live turn owns the status; only a settled turn may be revived"
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
    }

    #[test]
    fn late_subagent_start_drops_a_stale_wait_reason_from_the_settled_turn() {
        // A resumed session settles to `idle` while still carrying its
        // `session_resumed*` reason; that reason belongs to the turn that
        // ended, not to the child starting now.
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LATE_STALE_WAIT_REASON";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "idle");
        tmux::test_mock::set(pane, tmux::PANE_WAIT_REASON, "session_resumed_compact");

        on_subagent_start(pane, "Explore", None, Some("sub-1"), true);

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background")
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON),
            "a revived background pane must not keep the settled turn's reason"
        );
    }

    #[test]
    fn subagent_start_never_downgrades_a_non_idle_settled_pane() {
        let _guard = tmux::test_mock::install();

        // `waiting` still needs the user, `error` must stay visible,
        // `background` is already correct, and an empty status means the
        // pane is not tracked at all.
        for status in ["waiting", "error", "background"] {
            let pane = format!("%SUB_START_KEEP_{}", status.to_uppercase());
            tmux::test_mock::set(&pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
            tmux::test_mock::set(&pane, tmux::PANE_STATUS, status);

            on_subagent_start(&pane, "Explore", None, Some("sub-1"), true);

            assert_eq!(
                tmux::test_mock::get(&pane, tmux::PANE_STATUS).as_deref(),
                Some(status),
                "a settled {status} pane must survive a child registration"
            );
        }

        let untracked = "%SUB_START_KEEP_UNTRACKED";
        on_subagent_start(untracked, "Explore", None, Some("sub-1"), true);
        assert!(
            !tmux::test_mock::contains(untracked, tmux::PANE_STATUS),
            "an untracked pane must not gain a status from a child registration"
        );
        assert!(!tmux::test_mock::contains(untracked, tmux::PANE_STARTED_AT));
    }

    #[test]
    fn delayed_child_start_after_stop_recovers_background_and_settles_once() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LATE_START_RACE";
        let agent = tmux::GROK_AGENT.to_string();
        let cwd = "/repo".to_string();
        let permission_mode = "default".to_string();
        let worktree = None;
        let session_id = None;
        let ctx = AgentContext {
            agent: &agent,
            cwd: &cwd,
            permission_mode: &permission_mode,
            worktree: &worktree,
            session_id: &session_id,
        };

        on_user_prompt_submit(pane, &ctx, "audit the adapters", false, Some("prompt-1"));

        // The host Stop wins the race: the child's start hook has not landed
        // yet, so the pane settles on an empty child list and the completion
        // notification fires now — that one cannot be taken back.
        on_stop(
            pane,
            &ctx,
            "done",
            None,
            Some("prompt-1"),
            true,
            &default_notifications(),
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(
            pane,
            tmux::PANE_PENDING_STOP_NOTIFICATION_BODY
        ));

        // The delayed registration arrives and restores the background
        // lifecycle instead of leaving live work displayed as completed.
        on_subagent_start(pane, "Explore", None, Some("sub-1"), true);
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background")
        );
        assert!(tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));

        // The child finishes: the pane settles back to idle, and because
        // Stop left no pending body there is no second notification.
        on_subagent_stop(pane, Some("sub-1"), true, &default_notifications());
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
        assert!(!tmux::test_mock::contains(
            pane,
            tmux::PANE_PENDING_STOP_NOTIFICATION_BODY
        ));

        fs::remove_file(crate::activity::log_file_path(pane)).ok();
    }

    #[test]
    fn last_grok_subagent_stop_settles_background_without_shell() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LAST_BACKGROUND";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "background");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "1700");

        on_subagent_stop(pane, Some("sub-1"), true, &default_notifications());

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STARTED_AT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
    }

    #[test]
    fn last_grok_subagent_stop_consumes_deferred_completion_notification() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LAST_DEFERRED_NOTIFICATION";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "background");
        tmux::test_mock::set(
            pane,
            tmux::PANE_PENDING_STOP_NOTIFICATION_BODY,
            "parent response",
        );

        on_subagent_stop(pane, Some("sub-1"), true, &default_notifications());

        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY),
            "the final child must consume the pending Stop notification"
        );
    }

    #[test]
    fn last_grok_subagent_stop_settles_inactive_parent_after_child_state_transitions() {
        let _guard = tmux::test_mock::install();

        for status in ["waiting", "running"] {
            let pane = format!("%SUB_LAST_{}", status.to_uppercase());
            tmux::test_mock::set(&pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
            tmux::test_mock::set(&pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
            tmux::test_mock::set(&pane, tmux::PANE_STATUS, status);
            tmux::test_mock::set(&pane, tmux::PANE_STARTED_AT, "1700");
            tmux::test_mock::set(&pane, tmux::PANE_ATTENTION, "notification");
            tmux::test_mock::set(
                &pane,
                tmux::PANE_PENDING_STOP_NOTIFICATION_BODY,
                "parent response",
            );

            on_subagent_stop(&pane, Some("sub-1"), true, &default_notifications());

            assert_eq!(
                tmux::test_mock::get(&pane, tmux::PANE_STATUS).as_deref(),
                Some("idle"),
                "an inactive parent must settle after its final child stops from {status}"
            );
            assert!(!tmux::test_mock::contains(&pane, tmux::PANE_STARTED_AT));
            assert!(!tmux::test_mock::contains(
                &pane,
                tmux::PANE_PENDING_STOP_NOTIFICATION_BODY
            ));
            assert!(!tmux::test_mock::contains(&pane, tmux::PANE_ATTENTION));
        }
    }

    #[test]
    fn last_grok_subagent_stop_keeps_background_shell_running() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LAST_WITH_SHELL";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_BG_CMD, "sleep 300");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "background");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "1700");
        tmux::test_mock::set(
            pane,
            tmux::PANE_PENDING_STOP_NOTIFICATION_BODY,
            "parent response",
        );

        on_subagent_stop(pane, Some("sub-1"), true, &default_notifications());

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STARTED_AT).as_deref(),
            Some("1700")
        );
        assert!(
            !tmux::test_mock::contains(pane, tmux::PANE_PENDING_STOP_NOTIFICATION_BODY),
            "shell-backed background work keeps the existing no-notification behavior"
        );
    }

    #[test]
    fn last_grok_subagent_stop_restores_background_after_child_transition_with_live_shell() {
        let _guard = tmux::test_mock::install();

        for status in ["waiting", "running"] {
            let pane = format!("%SUB_LAST_SHELL_{}", status.to_uppercase());
            tmux::test_mock::set(&pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
            tmux::test_mock::set(&pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
            tmux::test_mock::set(&pane, tmux::PANE_BG_CMD, "sleep 300");
            tmux::test_mock::set(&pane, tmux::PANE_STATUS, status);
            tmux::test_mock::set(&pane, tmux::PANE_STARTED_AT, "1700");
            tmux::test_mock::set(&pane, tmux::PANE_WAIT_REASON, "permission");
            tmux::test_mock::set(&pane, tmux::PANE_ATTENTION, "notification");

            on_subagent_stop(&pane, Some("sub-1"), true, &default_notifications());

            assert_eq!(
                tmux::test_mock::get(&pane, tmux::PANE_STATUS).as_deref(),
                Some("background"),
                "a settled parent with a live shell must return to background from {status}"
            );
            assert_eq!(
                tmux::test_mock::get(&pane, tmux::PANE_STARTED_AT).as_deref(),
                Some("1700")
            );
            assert!(!tmux::test_mock::contains(&pane, tmux::PANE_WAIT_REASON));
            assert!(!tmux::test_mock::contains(&pane, tmux::PANE_ATTENTION));
        }
    }

    #[test]
    fn last_grok_subagent_stop_does_not_settle_running_parent() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LAST_RUNNING";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, tmux::GROK_AGENT);
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "1700");
        tmux::test_mock::set(pane, tmux::PANE_TURN_ACTIVE, "1");

        on_subagent_stop(pane, Some("sub-1"), true, &default_notifications());

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STARTED_AT).as_deref(),
            Some("1700")
        );
    }

    #[test]
    fn last_subagent_stop_does_not_infer_settlement_for_claude_without_turn_marker() {
        let _guard = tmux::test_mock::install();
        let pane = "%SUB_LAST_CLAUDE_UNINITIALIZED_TURN_MARKER";
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "claude");
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        tmux::test_mock::set(pane, tmux::PANE_STARTED_AT, "1700");

        on_subagent_stop(pane, Some("sub-1"), false, &default_notifications());

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STARTED_AT).as_deref(),
            Some("1700")
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
    }

    // ─── deferred teardown regression tests ─────────────────────────
    //
    // These pin the invariant that WorktreeRemove fired while subagents
    // are active must not be lost forever — it is recorded as a pending
    // marker and replayed by `on_subagent_stop` once the subagent list
    // drains to empty.
    //
    // SessionEnd does NOT participate in the deferred-drain dance: we
    // can't tell a parent SessionEnd from a child's, and letting the
    // drain replay one on the wrong side risks wiping a live parent.

    #[test]
    fn session_end_while_subagents_active_is_a_no_op() {
        // Regression: previously `on_session_end` set PENDING_SESSION_END
        // whenever `@pane_subagents` was non-empty, and the next
        // `on_subagent_stop` would turn that marker into
        // `run_session_end_teardown`. Because subagents share the
        // parent's `$TMUX_PANE`, there is no way to guarantee the
        // SessionEnd came from the parent — so the safer default is to
        // skip the event entirely and leave the parent's state alone.
        let _guard = tmux::test_mock::install();
        let pane = "%CHILD_SESSIONEND";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "claude");
        tmux::test_mock::set(pane, tmux::PANE_CWD, "/repo/parent");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::create_dir_all(log_path.parent().unwrap());
        fs::write(&log_path, "1234567890|Read|main.rs\n").unwrap();

        on_session_end(pane, "claude", "", false, &default_notifications());
        assert!(
            !tmux::test_mock::contains(pane, PENDING_SESSION_END),
            "child SessionEnd must not record a pending teardown"
        );
        // Every parent field must survive.
        assert!(tmux::test_mock::contains(pane, tmux::PANE_AGENT));
        assert!(tmux::test_mock::contains(pane, tmux::PANE_CWD));
        assert!(tmux::test_mock::contains(pane, tmux::PANE_STATUS));
        assert!(log_path.exists());

        // Subsequent subagent stop must not trigger a teardown either.
        on_subagent_stop(pane, Some("sub-1"), false, &default_notifications());
        assert!(
            tmux::test_mock::contains(pane, tmux::PANE_AGENT),
            "SubagentStop draining an empty list must not tear down a live parent"
        );
        assert!(log_path.exists());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn pending_worktree_remove_drains_when_last_subagent_stops() {
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_WT_DEFER";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1");
        tmux::test_mock::set(pane, tmux::PANE_WORKTREE_NAME, "feat");
        tmux::test_mock::set(pane, tmux::PANE_WORKTREE_BRANCH, "feat");
        tmux::test_mock::set(pane, tmux::PANE_CWD, "/wt/feat");

        on_worktree_remove(pane);
        assert!(
            tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "WorktreeRemove must be deferred via the pending marker"
        );
        assert!(tmux::test_mock::contains(pane, tmux::PANE_WORKTREE_NAME));

        on_subagent_stop(pane, Some("sub-1"), false, &default_notifications());

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WORKTREE_NAME));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WORKTREE_BRANCH));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_CWD));
        assert!(
            !tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE),
            "pending marker must be cleared once teardown runs"
        );
    }

    #[test]
    fn pending_worktree_remove_waits_for_last_subagent() {
        // Equivalent of the old `pending_teardown_does_not_fire_until_subagents_empty`
        // but anchored on WorktreeRemove, which still uses the deferred
        // drain (SessionEnd dropped it intentionally — see the comment
        // above `session_end_while_subagents_active_is_a_no_op`).
        let _guard = tmux::test_mock::install();
        let pane = "%PARENT_WT_PARTIAL";
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Explore:sub-1,Plan:sub-2");
        tmux::test_mock::set(pane, tmux::PANE_WORKTREE_NAME, "feat");
        tmux::test_mock::set(pane, tmux::PANE_WORKTREE_BRANCH, "feat");
        tmux::test_mock::set(pane, tmux::PANE_CWD, "/wt/feat");

        on_worktree_remove(pane);
        assert!(tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE));

        // First child stops — list still has sub-2, teardown must NOT fire.
        on_subagent_stop(pane, Some("sub-1"), false, &default_notifications());
        assert!(
            tmux::test_mock::contains(pane, tmux::PANE_WORKTREE_NAME),
            "teardown must wait for the LAST subagent"
        );
        assert!(tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE));

        // Last child stops — now teardown fires.
        on_subagent_stop(pane, Some("sub-2"), false, &default_notifications());
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WORKTREE_NAME));
        assert!(!tmux::test_mock::contains(pane, PENDING_WORKTREE_REMOVE));
    }
}
