use crate::cli::{set_attention, set_status};
use crate::event::ExecutionState;
use crate::{time::now_epoch_secs, tmux};

use super::super::context::{make_ctx, set_agent_meta};
use super::super::notifications::{notification_settings, set_notification_run_id};
use super::run::{on_stop, on_stop_failure};

pub(in crate::cli::hook) fn on_execution_update(
    pane: &str,
    agent: &str,
    cwd: &str,
    session_id: &str,
    state: &ExecutionState,
) -> i32 {
    let previous_session = tmux::get_pane_option_value(pane, tmux::PANE_SESSION_ID);
    let new_session = previous_session != session_id;
    // A delayed completion from a replaced conversation must not stop its successor.
    if new_session && !previous_session.is_empty() && !matches!(state, ExecutionState::Running) {
        return 0;
    }
    let session = Some(session_id.to_string());
    let ctx = make_ctx(agent, cwd, "", &None, &session);
    match state {
        ExecutionState::Running | ExecutionState::Background => {
            if new_session {
                for key in [
                    tmux::PANE_PROMPT,
                    tmux::PANE_PROMPT_SOURCE,
                    tmux::PANE_BG_CMD,
                    tmux::PANE_SUBAGENTS,
                    tmux::PANE_PERMISSION_MODE,
                ] {
                    tmux::unset_pane_option(pane, key);
                }
            }
            set_agent_meta(pane, &ctx);
            let started = tmux::get_pane_option_value(pane, tmux::PANE_STARTED_AT);
            if new_session || started.is_empty() {
                tmux::set_pane_option(pane, tmux::PANE_STARTED_AT, &now_epoch_secs().to_string());
                set_notification_run_id(pane);
            }
            set_attention(pane, "clear");
            tmux::unset_pane_option(pane, tmux::PANE_WAIT_REASON);
            set_status(pane, "running");
            0
        }
        ExecutionState::Idle => on_stop(pane, &ctx, "", None, &notification_settings()),
        ExecutionState::Failed(error) => {
            on_stop_failure(pane, &ctx, error, &notification_settings())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_invocations_preserve_timer_and_run_id() {
        let _guard = tmux::test_mock::install();
        on_execution_update("%AGY", "agy", "/repo", "c1", &ExecutionState::Running);
        assert_eq!(
            tmux::test_mock::get("%AGY", tmux::PANE_AGENT).as_deref(),
            Some("agy")
        );
        tmux::test_mock::set("%AGY", tmux::PANE_STARTED_AT, "123");
        tmux::test_mock::set("%AGY", tmux::PANE_NOTIFICATION_RUN_ID, "456");
        on_execution_update("%AGY", "agy", "/repo", "c1", &ExecutionState::Running);
        assert_eq!(
            tmux::test_mock::get("%AGY", tmux::PANE_STARTED_AT).as_deref(),
            Some("123")
        );
        assert_eq!(
            tmux::test_mock::get("%AGY", tmux::PANE_NOTIFICATION_RUN_ID).as_deref(),
            Some("456")
        );
    }

    #[test]
    fn a_new_conversation_resets_timer_and_stale_metadata() {
        let _guard = tmux::test_mock::install();
        tmux::test_mock::set("%AGY", tmux::PANE_SESSION_ID, "old");
        tmux::test_mock::set("%AGY", tmux::PANE_STARTED_AT, "123");
        tmux::test_mock::set("%AGY", tmux::PANE_PROMPT, "old prompt");
        on_execution_update("%AGY", "agy", "/repo", "new", &ExecutionState::Running);
        assert_ne!(
            tmux::test_mock::get("%AGY", tmux::PANE_STARTED_AT).as_deref(),
            Some("123")
        );
        assert!(!tmux::test_mock::contains("%AGY", tmux::PANE_PROMPT));
        assert_eq!(
            tmux::test_mock::get("%AGY", tmux::PANE_SESSION_ID).as_deref(),
            Some("new")
        );
        on_execution_update("%AGY", "agy", "/repo", "old", &ExecutionState::Idle);
        on_execution_update("%AGY", "agy", "/repo", "old", &ExecutionState::Background);
        assert_eq!(
            tmux::test_mock::get("%AGY", tmux::PANE_SESSION_ID).as_deref(),
            Some("new")
        );
        assert_eq!(
            tmux::test_mock::get("%AGY", tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
    }
}
