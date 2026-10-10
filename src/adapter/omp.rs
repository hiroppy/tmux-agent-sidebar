use serde_json::Value;

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::OMP_AGENT;

use super::opencode::normalize_tool_name;
use super::{json_str, json_value_or_null, optional_str};

/// Adapter for events normalized by the Oh My Pi extension bridge.
///
/// OMP hooks are observational: no parsed event returns a response to the
/// extension, so sidebar ingestion cannot alter prompt or tool execution.
pub struct OmpAdapter;

impl EventAdapter for OmpAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: OMP_AGENT.into(),
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
                agent: OMP_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: OMP_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: json_str(input, "last_message").into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: OMP_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                wait_reason: json_str(input, "wait_reason").into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "activity-log" => {
                let raw_name = json_str(input, "tool_name");
                if raw_name.is_empty() {
                    return None;
                }
                Some(AgentEvent::ActivityLog {
                    tool_name: normalize_tool_name(raw_name),
                    tool_input: json_value_or_null(input, "tool_input"),
                    tool_response: Value::Null,
                })
            }
            // `agent-start` follows `user-prompt-submit` for the same OMP
            // turn. The latter already marks the pane running; mapping both
            // would reset run timestamps and notification identity twice.
            _ => None,
        }
    }
}
