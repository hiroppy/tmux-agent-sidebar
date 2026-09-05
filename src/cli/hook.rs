use crate::event::{AgentEvent, resolve_adapter};

use super::{read_stdin_json, tmux_pane};

mod activity;
mod context;
mod handlers;
mod lock;
mod notifications;

use context::sync_pane_location;
use notifications::notification_settings;

// ─── hook subcommand ────────────────────────────────────────────────────────

pub(crate) fn cmd_hook(args: &[String]) -> i32 {
    let agent_name = args.first().map(|s| s.as_str()).unwrap_or("");
    let event_name = args.get(1).map(|s| s.as_str()).unwrap_or("");

    if agent_name.is_empty() || event_name.is_empty() {
        return 0;
    }

    let Some(adapter) = resolve_adapter(agent_name) else {
        return 0;
    };

    let pane = tmux_pane();
    if pane.is_empty() {
        return 0;
    }

    let input = read_stdin_json();
    let Some(event) = adapter.parse(event_name, &input) else {
        return 0;
    };

    // Hooks are independent processes; serialize their read-modify-write
    // over this pane's options so a Stop cannot settle on a child list a
    // concurrent SubagentStart is still appending to.
    let _lock = lock::acquire(&pane);
    handle_event(&pane, agent_name, event)
}

// ─── event handler ──────────────────────────────────────────────────────────

fn handle_event(pane: &str, agent_name: &str, event: AgentEvent) -> i32 {
    match event {
        AgentEvent::SessionStart {
            agent,
            cwd,
            permission_mode,
            source,
            top_level,
            worktree,
            session_id,
            ..
        } => handlers::on_session_start(
            pane,
            &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
            &source,
            top_level,
        ),
        AgentEvent::SessionEnd {
            agent,
            session_id,
            requires_existing_session,
            end_reason,
            top_level,
        } => {
            if requires_existing_session
                && !context::pane_tracks_host_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            let notifications = notification_settings();
            handlers::on_session_end(pane, &agent, &end_reason, top_level, &notifications)
        }
        AgentEvent::UserPromptSubmit {
            agent,
            cwd,
            permission_mode,
            prompt,
            prompt_is_system_message,
            requires_existing_session,
            prompt_id,
            worktree,
            session_id,
            ..
        } => {
            if requires_existing_session
                && !context::pane_tracks_host_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            handlers::on_user_prompt_submit(
                pane,
                &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                &prompt,
                prompt_is_system_message,
                prompt_id.as_deref(),
            )
        }
        AgentEvent::Notification {
            agent,
            cwd,
            permission_mode,
            wait_reason,
            meta_only,
            requires_existing_session,
            worktree,
            session_id,
            ..
        } => {
            let notifications = notification_settings();
            handlers::on_notification(
                pane,
                &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                &wait_reason,
                meta_only,
                requires_existing_session,
                &notifications,
            )
        }
        AgentEvent::Stop {
            agent,
            cwd,
            permission_mode,
            last_message,
            response,
            prompt_id,
            requires_existing_session,
            children_may_outlive_turn,
            worktree,
            session_id,
            ..
        } => {
            if requires_existing_session
                && !context::pane_tracks_host_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            let notifications = notification_settings();
            handlers::on_stop(
                pane,
                &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                &last_message,
                response.as_deref(),
                prompt_id.as_deref(),
                children_may_outlive_turn,
                &notifications,
            )
        }
        AgentEvent::TurnSettled {
            agent,
            cwd,
            permission_mode,
            prompt_id,
            requires_existing_session,
            children_may_outlive_turn,
            worktree,
            session_id,
            ..
        } => {
            if requires_existing_session
                && !context::pane_tracks_host_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            handlers::on_turn_settled(
                pane,
                &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                prompt_id.as_deref(),
                children_may_outlive_turn,
            )
        }
        AgentEvent::StopFailure {
            agent,
            cwd,
            permission_mode,
            error,
            prompt_id,
            requires_existing_session,
            worktree,
            session_id,
            ..
        } => {
            if requires_existing_session
                && !context::pane_tracks_host_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            let notifications = notification_settings();
            handlers::on_stop_failure(
                pane,
                &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                &error,
                prompt_id.as_deref(),
                &notifications,
            )
        }
        AgentEvent::SubagentStart {
            agent,
            session_id,
            requires_existing_session,
            agent_type,
            agent_id,
            display_name,
            children_may_outlive_turn,
        } => {
            if requires_existing_session
                && !context::pane_tracks_host_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            handlers::on_subagent_start(
                pane,
                &agent_type,
                display_name.as_deref(),
                agent_id.as_deref(),
                children_may_outlive_turn,
            )
        }
        AgentEvent::SubagentStop {
            agent_id,
            children_may_outlive_turn,
            ..
        } => {
            let notifications = notification_settings();
            handlers::on_subagent_stop(
                pane,
                agent_id.as_deref(),
                children_may_outlive_turn,
                &notifications,
            )
        }
        AgentEvent::ActivityLog {
            agent,
            session_id,
            requires_existing_session,
            tool_name,
            tool_input,
            tool_response,
        } => {
            if requires_existing_session
                && !context::pane_tracks_session(pane, &agent, session_id.as_deref())
            {
                return 0;
            }
            activity::handle_activity_log(pane, &tool_name, &tool_input, &tool_response)
        }
        AgentEvent::PermissionDenied {
            agent,
            cwd,
            permission_mode,
            requires_existing_session,
            worktree,
            session_id,
            ..
        } => {
            let notifications = notification_settings();
            handlers::on_permission_denied(
                pane,
                &context::make_ctx(&agent, &cwd, &permission_mode, &worktree, &session_id),
                requires_existing_session,
                &notifications,
            )
        }
        AgentEvent::CwdChanged {
            cwd,
            worktree,
            session_id,
            ..
        } => {
            sync_pane_location(pane, &cwd, &worktree, &session_id);
            0
        }
        AgentEvent::TaskCreated { .. } => 0,
        AgentEvent::TaskCompleted {
            task_id,
            task_subject,
        } => {
            super::set_attention(pane, "notification");
            let notifications = notification_settings();
            handlers::on_task_completed(pane, agent_name, &task_id, &task_subject, &notifications)
        }
        AgentEvent::TeammateIdle {
            teammate_name,
            idle_reason,
            ..
        } => handlers::on_teammate_idle(pane, &teammate_name, &idle_reason),
        AgentEvent::WorktreeCreate => 0,
        AgentEvent::WorktreeRemove { .. } => handlers::on_worktree_remove(pane),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};

    use super::*;
    use crate::tmux;

    #[test]
    fn stale_grok_activity_does_not_recreate_state_after_session_end() {
        let _guard = tmux::test_mock::install();
        let pane = "%ACTIVITY_AFTER_SESSION_END";
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::remove_file(&log_path);

        handle_event(
            pane,
            "grok",
            AgentEvent::ActivityLog {
                agent: "grok".into(),
                session_id: Some("ended-session".into()),
                requires_existing_session: true,
                tool_name: "Bash".into(),
                tool_input: json!({"command": "npm run dev", "background": true}),
                tool_response: Value::Null,
            },
        );

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_BG_CMD));
        assert!(!log_path.exists());
    }

    #[test]
    fn grok_activity_accepts_current_tracked_child_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%ACTIVITY_CURRENT_CHILD";
        let log_path = crate::activity::log_file_path(pane);
        let _ = fs::remove_file(&log_path);
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "grok");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "host-session");
        tmux::test_mock::set(pane, tmux::PANE_SUBAGENTS, "Review PR:child-session");

        handle_event(
            pane,
            "grok",
            AgentEvent::ActivityLog {
                agent: "grok".into(),
                session_id: Some("child-session".into()),
                requires_existing_session: true,
                tool_name: "Read".into(),
                tool_input: json!({"file_path": "/repo/src/lib.rs"}),
                tool_response: Value::Null,
            },
        );

        assert!(
            fs::read_to_string(&log_path)
                .unwrap()
                .contains("|Read|lib.rs")
        );
        let _ = fs::remove_file(log_path);
    }

    #[test]
    fn grok_user_prompt_preserves_literal_system_tag_text() {
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_LITERAL_SYSTEM_TAG";
        let adapter = resolve_adapter("grok").unwrap();

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "session-start",
                    &json!({"sessionId": "host-session", "cwd": "/repo"}),
                )
                .unwrap(),
        );
        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "user-prompt-submit",
                    &json!({
                        "sessionId": "host-session",
                        "cwd": "/repo",
                        "prompt": "explain <system-reminder>this literal tag</system-reminder>"
                    }),
                )
                .unwrap(),
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("explain <system-reminder>this literal tag</system-reminder>")
        );
    }

    #[test]
    fn stale_grok_session_end_does_not_clear_reused_pane() {
        let _guard = tmux::test_mock::install();
        let pane = "%STALE_GROK_SESSION_END";
        let adapter = resolve_adapter("grok").unwrap();
        tmux::test_mock::set(pane, tmux::PANE_AGENT, "claude");
        tmux::test_mock::set(pane, tmux::PANE_SESSION_ID, "new-claude-session");
        tmux::test_mock::set(pane, tmux::PANE_STATUS, "running");

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "session-end",
                    &json!({"sessionId": "old-grok-session", "reason": "other"}),
                )
                .unwrap(),
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_AGENT).as_deref(),
            Some("claude")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SESSION_ID).as_deref(),
            Some("new-claude-session")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("running")
        );
    }

    #[test]
    fn stale_grok_prompt_does_not_recreate_ended_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%STALE_GROK_PROMPT";
        let adapter = resolve_adapter("grok").unwrap();

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "user-prompt-submit",
                    &json!({
                        "sessionId": "ended-session",
                        "cwd": "/repo",
                        "prompt": "late prompt"
                    }),
                )
                .unwrap(),
        );

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_AGENT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STATUS));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_PROMPT));
    }

    #[test]
    fn stale_grok_idle_notification_does_not_recreate_ended_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%STALE_GROK_IDLE_NOTIFICATION";
        let adapter = resolve_adapter("grok").unwrap();

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "turn-settled",
                    &json!({
                        "sessionId": "ended-session",
                        "cwd": "/repo",
                        "notificationType": "idle_prompt"
                    }),
                )
                .unwrap(),
        );

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_AGENT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STATUS));
    }

    #[test]
    fn stale_grok_stop_failure_does_not_recreate_ended_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%STALE_GROK_STOP_FAILURE";
        let adapter = resolve_adapter("grok").unwrap();

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "stop-failure",
                    &json!({
                        "sessionId": "ended-session",
                        "cwd": "/repo",
                        "errorDetails": "late failure"
                    }),
                )
                .unwrap(),
        );

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_AGENT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STATUS));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_WAIT_REASON));
    }

    #[test]
    fn stale_grok_stop_does_not_recreate_ended_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%STALE_GROK_STOP";
        let adapter = resolve_adapter("grok").unwrap();

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "stop",
                    &json!({
                        "sessionId": "ended-session",
                        "cwd": "/repo",
                        "lastAssistantMessage": "late response"
                    }),
                )
                .unwrap(),
        );

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_AGENT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_STATUS));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_PROMPT));
    }

    #[test]
    fn stale_grok_subagent_start_does_not_recreate_ended_session() {
        let _guard = tmux::test_mock::install();
        let pane = "%STALE_GROK_SUBAGENT_START";
        let adapter = resolve_adapter("grok").unwrap();

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "subagent-start",
                    &json!({
                        "sessionId": "ended-session",
                        "subagentId": "late-child",
                        "subagentType": "general-purpose",
                        "description": "Late child"
                    }),
                )
                .unwrap(),
        );

        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
    }

    #[test]
    fn delayed_grok_subagent_start_revives_background_through_dispatch() {
        // Covers the dispatch wiring itself: the SubagentStart arm must
        // forward `children_may_outlive_turn` from the adapter. Handler-level
        // tests cannot catch a dispatch that hard-codes `false`.
        let _guard = tmux::test_mock::install();
        let pane = "%GROK_LATE_CHILD_DISPATCH";
        let adapter = resolve_adapter("grok").unwrap();

        for (event_name, payload) in [
            (
                "session-start",
                json!({"sessionId": "host-session", "cwd": "/repo"}),
            ),
            (
                "user-prompt-submit",
                json!({
                    "sessionId": "host-session",
                    "cwd": "/repo",
                    "prompt": "audit the adapters",
                    "promptId": "p-1"
                }),
            ),
            // The host Stop wins the race against the child's start hook.
            (
                "stop",
                json!({
                    "sessionId": "host-session",
                    "cwd": "/repo",
                    "lastAssistantMessage": "done",
                    "promptId": "p-1"
                }),
            ),
        ] {
            handle_event(pane, "grok", adapter.parse(event_name, &payload).unwrap());
        }
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("idle"),
            "Stop settles on an empty child list"
        );

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "subagent-start",
                    &json!({
                        "sessionId": "host-session",
                        "subagentId": "sub-1",
                        "subagentType": "general-purpose",
                        "description": "Late child"
                    }),
                )
                .unwrap(),
        );

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_STATUS).as_deref(),
            Some("background"),
            "the adapter's child-lifetime policy must survive dispatch"
        );
    }

    #[test]
    fn current_grok_host_events_apply_then_session_end_tears_down() {
        let _guard = tmux::test_mock::install();
        let pane = "%CURRENT_GROK_HOST_EVENTS";
        let adapter = resolve_adapter("grok").unwrap();

        for (event_name, payload) in [
            (
                "session-start",
                json!({"sessionId": "host-session", "cwd": "/repo"}),
            ),
            (
                "user-prompt-submit",
                json!({
                    "sessionId": "host-session",
                    "cwd": "/repo",
                    "prompt": "current prompt"
                }),
            ),
            (
                "subagent-start",
                json!({
                    "sessionId": "host-session",
                    "subagentId": "child-session",
                    "subagentType": "general-purpose",
                    "description": "Current child"
                }),
            ),
        ] {
            handle_event(pane, "grok", adapter.parse(event_name, &payload).unwrap());
        }

        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_PROMPT).as_deref(),
            Some("current prompt")
        );
        assert_eq!(
            tmux::test_mock::get(pane, tmux::PANE_SUBAGENTS).as_deref(),
            Some("Current child:child-session")
        );

        handle_event(
            pane,
            "grok",
            adapter
                .parse(
                    "session-end",
                    &json!({"sessionId": "host-session", "reason": "other"}),
                )
                .unwrap(),
        );
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_AGENT));
        assert!(!tmux::test_mock::contains(pane, tmux::PANE_SUBAGENTS));
    }
}
