use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::PI_AGENT;
use serde_json::Value;

use super::{HookRegistration, json_str, json_value_or_null, optional_str};

pub struct PiAdapter;

impl PiAdapter {
    /// Single source of truth for the Pi extension bridge. Unlike Claude Code
    /// and Codex, Pi has no per-event hook config: the extension in
    /// `.pi/extensions/tmux-agent-sidebar.ts` subscribes to Pi's extension
    /// events and forwards them to `tmux-agent-sidebar hook pi <event>`.
    ///
    /// The `trigger` field is the Pi extension event name the bridge listens
    /// to. `parse()` keys off the normalized `AgentEventKind::external_name()`
    /// instead, so the two sides stay decoupled.
    ///
    /// Covered events:
    /// - `session_start` → SessionStart (Pi session loaded/created)
    /// - `session_shutdown` → SessionEnd (quitting/reload/session switch)
    /// - `before_agent_start` (messageType `user`) → UserPromptSubmit
    /// - `agent_start` → UserPromptSubmit with an empty prompt (keeps the
    ///   previous prompt, only advances status to running)
    /// - `ui_prompt_start` → Notification (Pi is blocked on a user prompt)
    /// - `agent_end` / `agent_settled` → Stop
    /// - `turn_end` (outcome `error`) → StopFailure
    /// - `session_compact_failed` → StopFailure
    /// - `tool_execution_start` → ActivityLog
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "session_start",
            matcher: None,
            kind: AgentEventKind::SessionStart,
        },
        HookRegistration {
            trigger: "session_shutdown",
            matcher: None,
            kind: AgentEventKind::SessionEnd,
        },
        HookRegistration {
            trigger: "before_agent_start",
            matcher: None,
            kind: AgentEventKind::UserPromptSubmit,
        },
        HookRegistration {
            trigger: "ui_prompt_start",
            matcher: None,
            kind: AgentEventKind::Notification,
        },
        HookRegistration {
            trigger: "agent_end",
            matcher: None,
            kind: AgentEventKind::Stop,
        },
        HookRegistration {
            trigger: "turn_end",
            matcher: None,
            kind: AgentEventKind::StopFailure,
        },
        HookRegistration {
            trigger: "tool_execution_start",
            matcher: None,
            kind: AgentEventKind::ActivityLog,
        },
    ];
}

impl EventAdapter for PiAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: json_str(input, "end_reason").into(),
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                wait_reason: json_str(input, "wait_reason").into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: json_str(input, "last_message").into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop-failure" => Some(AgentEvent::StopFailure {
                agent: PI_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                error: json_str(input, "error").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "activity-log" => {
                let tool_name = json_str(input, "tool_name");
                if tool_name.is_empty() {
                    return None;
                }
                Some(AgentEvent::ActivityLog {
                    tool_name: tool_name.into(),
                    tool_input: json_value_or_null(input, "tool_input"),
                    tool_response: json_value_or_null(input, "tool_response"),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hook_registrations_match_parse_arms() {
        super::super::assert_table_drift_free("pi", PiAdapter::HOOK_REGISTRATIONS);
    }

    #[test]
    fn session_start_sets_agent_and_source() {
        let event = PiAdapter
            .parse(
                "session-start",
                &json!({"cwd": "/tmp", "source": "startup", "session_id": "s1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                source: "startup".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("s1".into()),
            }
        );
    }

    #[test]
    fn session_end_captures_reason() {
        let event = PiAdapter
            .parse("session-end", &json!({"end_reason": "quit"}))
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionEnd {
                end_reason: "quit".into()
            }
        );
    }

    #[test]
    fn user_prompt_submit_captures_prompt() {
        let event = PiAdapter
            .parse(
                "user-prompt-submit",
                &json!({"cwd": "/tmp", "prompt": "hello"}),
            )
            .unwrap();
        match event {
            AgentEvent::UserPromptSubmit { agent, prompt, .. } => {
                assert_eq!(agent, PI_AGENT);
                assert_eq!(prompt, "hello");
            }
            other => panic!("expected UserPromptSubmit, got {other:?}"),
        }
    }

    #[test]
    fn empty_prompt_is_preserved_as_empty() {
        // The bridge sends an empty prompt from `agent_start`; the handler
        // treats it as "keep the previous prompt".
        let event = PiAdapter.parse("user-prompt-submit", &json!({})).unwrap();
        match event {
            AgentEvent::UserPromptSubmit { prompt, .. } => assert!(prompt.is_empty()),
            other => panic!("expected UserPromptSubmit, got {other:?}"),
        }
    }

    #[test]
    fn notification_carries_wait_reason() {
        let event = PiAdapter
            .parse(
                "notification",
                &json!({"cwd": "/tmp", "wait_reason": "ui_prompt"}),
            )
            .unwrap();
        match event {
            AgentEvent::Notification {
                wait_reason,
                meta_only,
                ..
            } => {
                assert_eq!(wait_reason, "ui_prompt");
                assert!(!meta_only);
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    #[test]
    fn stop_captures_last_message_and_has_no_response() {
        let event = PiAdapter
            .parse("stop", &json!({"cwd": "/tmp", "last_message": "done"}))
            .unwrap();
        match event {
            AgentEvent::Stop {
                last_message,
                response,
                ..
            } => {
                assert_eq!(last_message, "done");
                assert!(response.is_none());
            }
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn stop_failure_captures_error() {
        let event = PiAdapter
            .parse("stop-failure", &json!({"error": "compaction failed"}))
            .unwrap();
        match event {
            AgentEvent::StopFailure { error, .. } => assert_eq!(error, "compaction failed"),
            other => panic!("expected StopFailure, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_tool_name_and_payloads() {
        let event = PiAdapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "bash",
                    "tool_input": {"command": "ls"},
                    "tool_response": {"stdout": "ok"}
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "bash");
                assert_eq!(
                    tool_input.get("command").and_then(|v| v.as_str()),
                    Some("ls")
                );
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_without_tool_name_is_rejected() {
        assert!(PiAdapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unsupported_events_are_ignored() {
        for name in [
            "permission-denied",
            "cwd-changed",
            "subagent-start",
            "subagent-stop",
            "task-created",
            "task-completed",
            "teammate-idle",
            "worktree-create",
            "worktree-remove",
            "something-else",
        ] {
            assert!(
                PiAdapter.parse(name, &json!({})).is_none(),
                "{name} should not be handled by the pi adapter"
            );
        }
    }
}
