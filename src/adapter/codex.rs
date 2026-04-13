use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::CODEX_AGENT;
use serde_json::Value;

use super::{HookRegistration, json_str};

pub struct CodexAdapter;

impl CodexAdapter {
    /// Single source of truth for Codex CLI hook wiring. Verified against
    /// Codex CLI's official hook event enum in
    /// `openai/codex:codex-rs/hooks/src/engine/config.rs`, which currently
    /// defines only: `PreToolUse`, `PostToolUse`, `SessionStart`,
    /// `UserPromptSubmit`, `Stop`.
    ///
    /// Caveats:
    /// - `PostToolUse` fires only for Bash (Codex's `PostToolUseToolInput`
    ///   is a typed `{ command: String }` struct); the resulting activity
    ///   log is Bash-only.
    /// - `PreToolUse` is supported by Codex but not yet wired.
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
    ];
}

impl EventAdapter for CodexAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                worktree: None,
                agent_id: None,
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                last_message: json_str(input, "last_assistant_message").into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
            }),
            // Codex's PostToolUse currently fires only for Bash (tool_input is
            // typed `{ command: String }`). Other tools do not emit the hook,
            // so the resulting activity log is Bash-only.
            "activity-log" => {
                let tool_name = json_str(input, "tool_name");
                if tool_name.is_empty() {
                    return None;
                }
                let tool_input = input.get("tool_input").cloned().unwrap_or(Value::Null);
                let tool_response = input.get("tool_response").cloned().unwrap_or(Value::Null);
                Some(AgentEvent::ActivityLog {
                    tool_name: tool_name.into(),
                    tool_input,
                    tool_response,
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
        let input = json!({"cwd": "/home/user"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: CODEX_AGENT.into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
                worktree: None,
                agent_id: None,
            }
        );
    }

    #[test]
    fn session_end_not_supported() {
        // Codex CLI does not fire SessionEnd (verified against
        // openai/codex:codex-rs/hooks/src/engine/config.rs).
        assert!(CodexAdapter.parse("session-end", &json!({})).is_none());
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "prompt": "hello"});
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
            }
        );
    }

    #[test]
    fn stop_has_continue_response() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "last_assistant_message": "done"});
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
            }
        );
    }

    #[test]
    fn notification_not_supported() {
        assert!(CodexAdapter.parse("notification", &json!({})).is_none());
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

    #[test]
    fn stop_empty_fields() {
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
                worktree: None,
                agent_id: None,
            }
        );
    }
}
