use serde_json::{Value, json};

use crate::event::{AgentEvent, AgentEventKind, EventAdapter, ExecutionState};
use crate::tmux::AGY_AGENT;

use super::{HookRegistration, json_str, optional_str};

pub struct AntigravityAdapter;

impl AntigravityAdapter {
    // Native CLI schema: https://antigravity.google/docs/hooks/
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "PreInvocation",
            matcher: None,
            kind: AgentEventKind::ExecutionUpdate,
        },
        HookRegistration {
            trigger: "PostToolUse",
            matcher: Some(""),
            kind: AgentEventKind::ActivityLog,
        },
        HookRegistration {
            trigger: "Stop",
            matcher: None,
            kind: AgentEventKind::ExecutionUpdate,
        },
    ];
}

impl EventAdapter for AntigravityAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        let session_id = optional_str(input, "conversationId")?;
        match event_name {
            "execution-update" => {
                let state = if input.get("terminationReason").is_some() {
                    let reason = input.get("terminationReason")?.as_str()?;
                    if reason.is_empty() {
                        return None;
                    }
                    let error = json_str(input, "error");
                    if !error.is_empty() || matches!(reason, "error" | "max_steps_exceeded") {
                        ExecutionState::Failed(if error.is_empty() { reason } else { error }.into())
                    } else if input.get("fullyIdle")?.as_bool()? {
                        ExecutionState::Idle
                    } else {
                        ExecutionState::Background
                    }
                } else {
                    // Distinguish PreInvocation from an incomplete Stop payload.
                    input.get("invocationNum")?.as_u64()?;
                    ExecutionState::Running
                };
                let cwd = input
                    .get("workspacePaths")
                    .and_then(Value::as_array)
                    .and_then(|paths| paths.first())
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into();
                Some(AgentEvent::ExecutionUpdate {
                    agent: AGY_AGENT.into(),
                    cwd,
                    session_id,
                    state,
                })
            }
            "activity-log" => {
                let call = input.get("toolCall")?;
                let name = optional_str(call, "name")?;
                let args = call.get("args").unwrap_or(&Value::Null);
                let (tool_name, tool_input) = match name.as_str() {
                    "run_command" => ("Bash", json!({"command":json_str(args, "CommandLine")})),
                    "view_file" => ("Read", json!({"file_path":json_str(args, "AbsolutePath")})),
                    "write_to_file" => ("Write", json!({"file_path":json_str(args, "TargetFile")})),
                    "replace_file_content" | "multi_replace_file_content" => {
                        ("Edit", json!({"file_path":json_str(args, "TargetFile")}))
                    }
                    "grep_search" => ("Grep", json!({"pattern":json_str(args, "Query")})),
                    "find_by_name" => ("Glob", json!({"pattern":json_str(args, "Pattern")})),
                    "search_web" => ("WebSearch", json!({"query":json_str(args, "query")})),
                    "read_url_content" => ("WebFetch", json!({"url":json_str(args, "Url")})),
                    _ => (name.as_str(), args.clone()),
                };
                Some(AgentEvent::ActivityLog {
                    tool_name: tool_name.into(),
                    tool_input,
                    tool_response: Value::Null,
                })
            }
            _ => None,
        }
    }
}
