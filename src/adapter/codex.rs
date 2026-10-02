use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::CODEX_AGENT;
use serde_json::Value;

use super::{HookRegistration, json_str, json_value_or_null, optional_str};

pub struct CodexAdapter;

impl CodexAdapter {
    /// Single source of truth for Codex CLI hook wiring. Verified against
    /// the hook input schemas in `openai/codex:codex-rs/hooks/src/schema.rs`
    /// at `rust-v0.155.1`, which defines `PreToolUse`, `PermissionRequest`,
    /// `PostToolUse`, `PreCompact`, `PostCompact`, `SessionStart`,
    /// `SessionEnd`, `UserPromptSubmit`, `Stop`, `SubagentStart`,
    /// `SubagentStop` and `Interrupt`.
    ///
    /// Caveats:
    /// - `PermissionRequest` runs before the approval UI. Our hook prints
    ///   nothing and exits 0, which Codex treats as "no verdict", so the
    ///   normal approval prompt still shows. Mapped to `Notification` so
    ///   the pane lands in `waiting` until the next tool event.
    /// - Subagent `UserPromptSubmit` events carry `agent_id` and are dropped
    ///   so a child prompt cannot overwrite the parent's prompt preview.
    /// - `Interrupt` fires when the user aborts a turn (never for subagents)
    ///   and is the only signal that ends it: `Stop` does not follow. Its
    ///   output schema rejects unknown fields, so the hook prints nothing.
    /// - `PreToolUse`, the compact events and the subagent lifecycle events
    ///   are supported by Codex but not yet wired.
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "SessionStart",
            matcher: Some("startup|resume"),
            kind: AgentEventKind::SessionStart,
        },
        HookRegistration {
            trigger: "UserPromptSubmit",
            matcher: None,
            kind: AgentEventKind::UserPromptSubmit,
        },
        HookRegistration {
            trigger: "Stop",
            matcher: None,
            kind: AgentEventKind::Stop,
        },
        HookRegistration {
            trigger: "PostToolUse",
            matcher: None,
            kind: AgentEventKind::ActivityLog,
        },
        HookRegistration {
            trigger: "PermissionRequest",
            matcher: None,
            kind: AgentEventKind::Notification,
        },
        HookRegistration {
            trigger: "SessionEnd",
            matcher: None,
            kind: AgentEventKind::SessionEnd,
        },
        HookRegistration {
            trigger: "Interrupt",
            matcher: None,
            kind: AgentEventKind::Interrupt,
        },
    ];
}

impl EventAdapter for CodexAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "user-prompt-submit" if optional_str(input, "agent_id").is_some() => None,
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                last_message: json_str(input, "last_assistant_message").into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                wait_reason: "permission_prompt".into(),
                meta_only: false,
                worktree: None,
                agent_id: optional_str(input, "agent_id"),
                session_id: optional_str(input, "session_id"),
            }),
            "interrupt" => Some(AgentEvent::Interrupt {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                worktree: None,
                session_id: optional_str(input, "session_id"),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: json_str(input, "reason").into(),
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
        super::super::assert_table_drift_free("codex", CodexAdapter::HOOK_REGISTRATIONS);
    }

    #[test]
    fn session_start() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/home/user", "session_id": "sess-codex-1"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: CODEX_AGENT.into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
                source: "".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-codex-1".into()),
            }
        );
    }

    #[test]
    fn session_end_carries_reason() {
        let input = json!({
            "hook_event_name": "SessionEnd",
            "session_id": "sess-codex-9",
            "cwd": "/tmp",
            "transcript_path": null,
            "reason": "other",
        });
        assert_eq!(
            CodexAdapter.parse("session-end", &input),
            Some(AgentEvent::SessionEnd {
                end_reason: "other".into(),
            })
        );
    }

    #[test]
    fn interrupt_ends_the_turn() {
        let input = json!({
            "hook_event_name": "Interrupt",
            "session_id": "sess-codex-7",
            "turn_id": "turn-3",
            "transcript_path": null,
            "cwd": "/tmp",
            "model": "gpt-5.5",
            "permission_mode": "default",
        });
        assert_eq!(
            CodexAdapter.parse("interrupt", &input),
            Some(AgentEvent::Interrupt {
                agent: CODEX_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "default".into(),
                worktree: None,
                session_id: Some("sess-codex-7".into()),
            })
        );
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "prompt": "hello", "session_id": "sess-codex-2"});
        let event = adapter.parse("user-prompt-submit", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: CODEX_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "hello".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-codex-2".into()),
            }
        );
    }

    #[test]
    fn stop_has_continue_response() {
        let adapter = CodexAdapter;
        let input = json!({
            "cwd": "/tmp",
            "last_assistant_message": "done",
            "session_id": "sess-codex-3",
        });
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: CODEX_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "done".into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-codex-3".into()),
            }
        );
    }

    /// Realistic Stop payload matching the upstream Codex hook input schema
    /// (`codex-rs/hooks/schema/generated/stop.command.input.schema.json`),
    /// which declares `session_id` as a required top-level string.
    #[test]
    fn stop_extracts_session_id_from_upstream_schema_payload() {
        let adapter = CodexAdapter;
        let input = json!({
            "hook_event_name": "Stop",
            "cwd": "/home/user/project",
            "session_id": "01HXYZABCDEF0123456789",
            "model": "gpt-5-codex",
            "permission_mode": "default",
            "last_assistant_message": "all tests pass",
            "stop_hook_active": false,
            "transcript_path": null,
            "turn_id": "turn-42",
        });
        let event = adapter.parse("stop", &input).unwrap();
        match event {
            AgentEvent::Stop {
                session_id,
                permission_mode,
                last_message,
                ..
            } => {
                assert_eq!(session_id.as_deref(), Some("01HXYZABCDEF0123456789"));
                assert_eq!(permission_mode, "default");
                assert_eq!(last_message, "all tests pass");
            }
            other => panic!("expected Stop, got {:?}", other),
        }
    }

    /// PermissionRequest payload shape from
    /// `codex-rs/hooks/schema/generated/permission-request.command.input.schema.json`.
    #[test]
    fn permission_request_maps_to_permission_prompt_notification() {
        let input = json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "sess-codex-4",
            "turn_id": "turn-7",
            "cwd": "/repo",
            "transcript_path": null,
            "model": "gpt-5.5",
            "permission_mode": "default",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf build"},
        });
        assert_eq!(
            CodexAdapter.parse("notification", &input),
            Some(AgentEvent::Notification {
                agent: CODEX_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "default".into(),
                wait_reason: "permission_prompt".into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: Some("sess-codex-4".into()),
            })
        );
    }

    #[test]
    fn subagent_user_prompt_submit_is_dropped() {
        let input = json!({
            "cwd": "/tmp",
            "prompt": "child task",
            "session_id": "sess-codex-5",
            "agent_id": "agent-1",
            "agent_type": "worker",
        });
        assert!(CodexAdapter.parse("user-prompt-submit", &input).is_none());
    }

    #[test]
    fn stop_failure_not_supported() {
        assert!(CodexAdapter.parse("stop-failure", &json!({})).is_none());
    }

    #[test]
    fn subagent_start_not_supported() {
        assert!(CodexAdapter.parse("subagent-start", &json!({})).is_none());
    }

    #[test]
    fn activity_log_bash_command() {
        let adapter = CodexAdapter;
        let input = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"},
            "tool_response": {"stdout": "file.txt\n"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(
                    tool_input.get("command").and_then(|v| v.as_str()),
                    Some("ls -la")
                );
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_empty_tool_name_rejected() {
        assert!(CodexAdapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        assert!(CodexAdapter.parse("something-else", &json!({})).is_none());
    }

    /// Defensive path: upstream schema requires `session_id`, but the adapter
    /// must not panic if a malformed payload omits it. Parse returns the event
    /// with `session_id: None` and downstream handlers treat it as missing.
    #[test]
    fn stop_without_session_id_falls_back_to_none() {
        let adapter = CodexAdapter;
        let event = adapter.parse("stop", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: "codex".into(),
                cwd: "".into(),
                permission_mode: "".into(),
                last_message: "".into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn subagent_stop_not_supported() {
        assert!(CodexAdapter.parse("subagent-stop", &json!({})).is_none());
    }

    #[test]
    fn permission_denied_not_supported() {
        assert!(
            CodexAdapter
                .parse("permission-denied", &json!({}))
                .is_none()
        );
    }

    #[test]
    fn cwd_changed_not_supported() {
        assert!(CodexAdapter.parse("cwd-changed", &json!({})).is_none());
    }

    #[test]
    fn session_start_has_no_worktree() {
        let event = CodexAdapter
            .parse("session-start", &json!({"cwd": "/tmp"}))
            .unwrap();
        match event {
            AgentEvent::SessionStart {
                worktree, agent_id, ..
            } => {
                assert!(worktree.is_none());
                assert!(agent_id.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn session_start_captures_source() {
        let event = CodexAdapter
            .parse("session-start", &json!({"cwd": "/tmp", "source": "resume"}))
            .unwrap();
        match event {
            AgentEvent::SessionStart { source, .. } => assert_eq!(source, "resume"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn task_created_not_supported() {
        assert!(CodexAdapter.parse("task-created", &json!({})).is_none());
    }

    #[test]
    fn task_completed_not_supported() {
        assert!(CodexAdapter.parse("task-completed", &json!({})).is_none());
    }

    #[test]
    fn teammate_idle_not_supported() {
        assert!(CodexAdapter.parse("teammate-idle", &json!({})).is_none());
    }

    #[test]
    fn worktree_create_not_supported() {
        assert!(CodexAdapter.parse("worktree-create", &json!({})).is_none());
    }

    #[test]
    fn worktree_remove_not_supported() {
        assert!(CodexAdapter.parse("worktree-remove", &json!({})).is_none());
    }

    #[test]
    fn session_start_missing_fields_default_to_empty() {
        let adapter = CodexAdapter;
        let event = adapter.parse("session-start", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: "codex".into(),
                cwd: "".into(),
                permission_mode: "".into(),
                source: "".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }
}
