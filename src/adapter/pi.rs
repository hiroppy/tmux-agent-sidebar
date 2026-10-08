use serde_json::{Map, Value};

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::PI_AGENT;
use crate::tool_name::CanonicalTool;

use super::{json_str, json_value_or_null, optional_str};

/// Adapter for the pi coding agent (`@earendil-works/pi-coding-agent`).
///
/// Unlike Claude Code and Codex, pi has no native hooks-JSON config —
/// events are bridged from a pi extension (`.pi/extensions/tmux-agent-sidebar.ts`)
/// that listens on pi's extension event bus and shells out to `hook.sh`,
/// the same fire-and-forget pattern OpenCode's plugin bridge uses. See
/// `adapter/opencode.rs` for the sibling implementation.
///
/// pi has no native permission-mode or tool-approval concept exposed to
/// extensions, so (like OpenCode) `permission_mode` is always empty and
/// there is no `PermissionDenied`/worktree support.
pub struct PiAdapter;

/// pi's built-in tools are lowercase (`bash`, `read`, `write`, `edit`, ...)
/// and file tools key their target path as `path` rather than the
/// Claude-style `file_path` the label extractor in `src/cli/label.rs`
/// expects. Normalise here so the activity log and its strategy table
/// share one vocabulary across agents.
///
/// Only tools whose input schema is verified against pi's documented
/// extension API (`docs/extensions.md`) are mapped; anything else passes
/// through unmapped and simply won't get a label — the same trade-off
/// Codex's Bash-only `PostToolUse` makes.
fn normalize_tool_name(raw: &str) -> String {
    let canonical = match raw {
        "bash" => CanonicalTool::Bash,
        "read" => CanonicalTool::Read,
        "write" => CanonicalTool::Write,
        "edit" => CanonicalTool::Edit,
        "powershell" => CanonicalTool::PowerShell,
        "grep" => CanonicalTool::Grep,
        other => return other.to_string(),
    };
    canonical.as_str().to_string()
}

/// Translate pi's `path` tool argument into the `file_path` key the label
/// extractor expects for Read/Write/Edit. Keys are added alongside the
/// originals rather than replacing them so downstream consumers that want
/// the raw payload still see it.
fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };
    let rewrites: &[(&str, &str)] = match tool_name {
        "Read" | "Write" | "Edit" => &[("path", "file_path")],
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
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "session-start",
                &json!({"cwd": "/tmp", "session_id": "ses-1", "source": "resume"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                source: "resume".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }

    #[test]
    fn session_end() {
        let adapter = PiAdapter;
        let event = adapter
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
    fn user_prompt_submit() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "user-prompt-submit",
                &json!({"cwd": "/tmp", "prompt": "hello"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: PI_AGENT.into(),
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
    fn stop_has_no_response() {
        let adapter = PiAdapter;
        let event = adapter
            .parse("stop", &json!({"last_message": "done"}))
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
            other => panic!("expected Stop, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_normalizes_lowercase_bash() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "bash",
                    "tool_input": {"command": "ls"},
                    "tool_response": {"content": [], "isError": false}
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(
                    tool_input.get("command").and_then(|v| v.as_str()),
                    Some("ls")
                );
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_rewrites_path_to_file_path_for_read() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "read",
                    "tool_input": {"path": "/tmp/foo.rs"},
                    "tool_response": {}
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(
                    tool_input.get("file_path").and_then(|v| v.as_str()),
                    Some("/tmp/foo.rs")
                );
                // Original key is preserved alongside the rewritten one.
                assert_eq!(
                    tool_input.get("path").and_then(|v| v.as_str()),
                    Some("/tmp/foo.rs")
                );
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_missing_tool_name_returns_none() {
        let adapter = PiAdapter;
        assert!(adapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn activity_log_normalizes_lowercase_grep() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({"tool_name": "grep", "tool_input": {"pattern": "fn main"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, "Grep"),
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_passes_through_unmapped_tool_names() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({"tool_name": "find", "tool_input": {"pattern": "*.rs"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, "find"),
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn unknown_event_returns_none() {
        let adapter = PiAdapter;
        assert!(adapter.parse("permission-denied", &json!({})).is_none());
        assert!(adapter.parse("cwd-changed", &json!({})).is_none());
    }
}
