use serde_json::{Map, Value};

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::PI_AGENT;
use crate::tool_name::CanonicalTool;

use super::{json_str, json_value_or_null, optional_str};

pub struct PiAdapter;

fn normalize_tool_name(raw: &str) -> String {
    let stripped = raw
        .strip_prefix("mcp_pi_")
        .or_else(|| raw.strip_prefix("mcp__"))
        .unwrap_or(raw);
    let canonical = match stripped {
        "bash" => CanonicalTool::Bash,
        "read" => CanonicalTool::Read,
        "write" => CanonicalTool::Write,
        "edit" | "multiedit" => CanonicalTool::Edit,
        "glob" => CanonicalTool::Glob,
        "grep" => CanonicalTool::Grep,
        "webfetch" => CanonicalTool::WebFetch,
        "websearch" => CanonicalTool::WebSearch,
        "task" | "subagent" => CanonicalTool::Agent,
        "skill" => CanonicalTool::Skill,
        "lsp" => CanonicalTool::Lsp,
        "todowrite" => CanonicalTool::TodoWrite,
        other => return other.to_string(),
    };
    canonical.as_str().to_string()
}

fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };
    let rewrites: &[(&str, &str)] = match tool_name {
        "Read" | "Write" | "Edit" => &[("filePath", "file_path"), ("path", "file_path")],
        _ => &[],
    };
    copy_keys(&mut map, rewrites);
    Value::Object(map)
}

fn copy_keys(map: &mut Map<String, Value>, pairs: &[(&str, &str)]) {
    for (src, dst) in pairs {
        if map.contains_key(*dst) {
            continue;
        }
        if let Some(value) = map.get(*src).cloned() {
            map.insert((*dst).to_string(), value);
        }
    }
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
                let raw_name = json_str(input, "tool_name");
                if raw_name.is_empty() {
                    return None;
                }
                let tool_name = normalize_tool_name(raw_name);
                let tool_input =
                    normalize_tool_input(&tool_name, json_value_or_null(input, "tool_input"));
                Some(AgentEvent::ActivityLog {
                    tool_name,
                    tool_input,
                    tool_response: json_value_or_null(input, "tool_response"),
                })
            }
            "subagent-start" => {
                let agent_type = json_str(input, "agent_type");
                if agent_type.is_empty() {
                    return None;
                }
                Some(AgentEvent::SubagentStart {
                    agent_type: agent_type.into(),
                    agent_id: optional_str(input, "agent_id"),
                })
            }
            "subagent-stop" => {
                let agent_type = json_str(input, "agent_type");
                if agent_type.is_empty() {
                    return None;
                }
                Some(AgentEvent::SubagentStop {
                    agent_type: agent_type.into(),
                    agent_id: optional_str(input, "agent_id"),
                    last_message: json_str(input, "last_assistant_message").into(),
                    transcript_path: json_str(input, "agent_transcript_path").into(),
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
    fn activity_log_normalizes_mcp_pi_bash() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "mcp_pi_bash",
                    "tool_input": {"command": "ls"},
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, "Bash"),
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn subagent_start_round_trip() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "subagent-start",
                &json!({"agent_type": "review-code", "agent_id": "call-1"}),
            )
            .unwrap();
        match event {
            AgentEvent::SubagentStart {
                agent_type,
                agent_id,
            } => {
                assert_eq!(agent_type, "review-code");
                assert_eq!(agent_id.as_deref(), Some("call-1"));
            }
            other => panic!("expected SubagentStart, got {:?}", other),
        }
    }

    #[test]
    fn subagent_stop_round_trip() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "subagent-stop",
                &json!({
                    "agent_type": "review-code",
                    "agent_id": "call-1",
                    "last_assistant_message": "done",
                }),
            )
            .unwrap();
        match event {
            AgentEvent::SubagentStop {
                agent_type,
                agent_id,
                last_message,
                ..
            } => {
                assert_eq!(agent_type, "review-code");
                assert_eq!(agent_id.as_deref(), Some("call-1"));
                assert_eq!(last_message, "done");
            }
            other => panic!("expected SubagentStop, got {:?}", other),
        }
    }
}
