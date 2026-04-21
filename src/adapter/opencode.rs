use serde_json::Value;

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::OPENCODE_AGENT;

use super::{json_str, json_value_or_null, optional_str};

pub struct OpenCodeAdapter;

impl EventAdapter for OpenCodeAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: OPENCODE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: OPENCODE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: OPENCODE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                wait_reason: json_str(input, "wait_reason").into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: OPENCODE_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: json_str(input, "last_message").into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop-failure" => Some(AgentEvent::StopFailure {
                agent: OPENCODE_AGENT.into(),
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
    fn session_start() {
        let adapter = OpenCodeAdapter;
        let event = adapter
            .parse(
                "session-start",
                &json!({"cwd": "/tmp", "session_id": "ses-1", "source": "startup"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: OPENCODE_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                source: "startup".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = OpenCodeAdapter;
        let event = adapter
            .parse(
                "user-prompt-submit",
                &json!({"cwd": "/tmp", "prompt": "hello"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: OPENCODE_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "hello".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn activity_log() {
        let adapter = OpenCodeAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "Bash",
                    "tool_input": {"command": "ls"},
                    "tool_response": {"stdout": "file.txt"}
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(tool_input["command"], "ls");
                assert_eq!(tool_response["stdout"], "file.txt");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn stop_failure() {
        let adapter = OpenCodeAdapter;
        let event = adapter
            .parse(
                "stop-failure",
                &json!({"cwd": "/tmp", "error": "boom", "session_id": "ses-1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::StopFailure {
                agent: OPENCODE_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                error: "boom".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }
}
