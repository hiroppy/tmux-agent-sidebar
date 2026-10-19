use serde_json::{Map, Value};

use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::OMP_AGENT;
use crate::tool_name::CanonicalTool;

use super::{json_str, json_value_or_null, optional_str};

/// Adapter for oh-my-pi (`omp`).
///
/// Like pi and OpenCode, omp has no native hooks-JSON config — events are
/// bridged from an omp extension (`.omp/extensions/tmux-agent-sidebar.ts`)
/// that shells out to `hook.sh`. omp exposes no permission-mode concept to
/// extensions, so `permission_mode` is always empty and there is no
/// `PermissionDenied`/worktree support.
pub struct OmpAdapter;

/// omp's built-in tool names are lowercase (`bash`, `read`, `web_search`,
/// `task`, `todo`, …). Normalise to the Claude-style vocabulary the label
/// extractor in `src/cli/label.rs` keys off. Unknown tools pass through and
/// simply get no label.
fn normalize_tool_name(raw: &str) -> String {
    let canonical = match raw {
        "bash" => CanonicalTool::Bash,
        "read" => CanonicalTool::Read,
        "write" => CanonicalTool::Write,
        "edit" => CanonicalTool::Edit,
        "grep" => CanonicalTool::Grep,
        "glob" => CanonicalTool::Glob,
        "web_search" => CanonicalTool::WebSearch,
        "task" => CanonicalTool::Agent,
        "todo" => CanonicalTool::TodoWrite,
        other => return other.to_string(),
    };
    canonical.as_str().to_string()
}

/// omp keys file targets as `path` (and holds the glob pattern in `path`
/// for `glob`). Keys are added alongside the originals so downstream
/// consumers still see the raw payload. omp's `edit` takes hashline text
/// with no `path` key, so Edit rows get no file label.
fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };
    let rewrites: &[(&str, &str)] = match tool_name {
        "Read" | "Write" | "Edit" => &[("path", "file_path")],
        "Glob" => &[("path", "pattern")],
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
            "stop-failure" => Some(AgentEvent::StopFailure {
                agent: OMP_AGENT.into(),
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
        let event = OmpAdapter
            .parse(
                "session-start",
                &json!({"cwd": "/tmp", "session_id": "ses-1", "source": "resume"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: OMP_AGENT.into(),
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
        let event = OmpAdapter
            .parse("session-end", &json!({"end_reason": "quit"}))
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionEnd {
                end_reason: "quit".into(),
            }
        );
    }

    #[test]
    fn user_prompt_submit() {
        let event = OmpAdapter
            .parse(
                "user-prompt-submit",
                &json!({"cwd": "/tmp", "prompt": "hi", "session_id": "ses-1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: OMP_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "hi".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }

    #[test]
    fn notification() {
        let event = OmpAdapter
            .parse(
                "notification",
                &json!({"cwd": "/tmp", "wait_reason": "permission", "session_id": "ses-1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::Notification {
                agent: OMP_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                wait_reason: "permission".into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }

    #[test]
    fn stop() {
        let event = OmpAdapter
            .parse(
                "stop",
                &json!({"cwd": "/tmp", "last_message": "done", "session_id": "ses-1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: OMP_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "done".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }

    #[test]
    fn stop_failure() {
        let event = OmpAdapter
            .parse(
                "stop-failure",
                &json!({"cwd": "/tmp", "error": "boom", "session_id": "ses-1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::StopFailure {
                agent: OMP_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                error: "boom".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-1".into()),
            }
        );
    }

    #[test]
    fn activity_log_normalizes_lowercase_bash() {
        let event = OmpAdapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "bash",
                    "tool_input": {"command": "ls"},
                    "tool_response": {"isError": false},
                }),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::ActivityLog {
                tool_name: "Bash".into(),
                tool_input: json!({"command": "ls"}),
                tool_response: json!({"isError": false}),
            }
        );
    }

    #[test]
    fn activity_log_read_gains_file_path_and_keeps_path() {
        let event = OmpAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "read", "tool_input": {"path": "/tmp/a.rs"}}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::ActivityLog {
                tool_name: "Read".into(),
                tool_input: json!({"path": "/tmp/a.rs", "file_path": "/tmp/a.rs"}),
                tool_response: Value::Null,
            }
        );
    }

    #[test]
    fn activity_log_glob_path_becomes_pattern() {
        let event = OmpAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "glob", "tool_input": {"path": "src/**/*.rs"}}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::ActivityLog {
                tool_name: "Glob".into(),
                tool_input: json!({"path": "src/**/*.rs", "pattern": "src/**/*.rs"}),
                tool_response: Value::Null,
            }
        );
    }

    #[test]
    fn activity_log_maps_task_todo_and_web_search() {
        for (raw, canonical) in [
            ("task", "Agent"),
            ("todo", "TodoWrite"),
            ("web_search", "WebSearch"),
            ("grep", "Grep"),
        ] {
            let event = OmpAdapter
                .parse("activity-log", &json!({"tool_name": raw}))
                .unwrap();
            match event {
                AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, canonical),
                other => panic!("expected ActivityLog, got {:?}", other),
            }
        }
    }

    #[test]
    fn activity_log_unknown_tool_passes_through() {
        let event = OmpAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "custom_tool", "tool_input": {"path": "/x"}}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::ActivityLog {
                tool_name: "custom_tool".into(),
                tool_input: json!({"path": "/x"}),
                tool_response: Value::Null,
            }
        );
    }

    #[test]
    fn activity_log_missing_tool_name_returns_none() {
        assert!(
            OmpAdapter
                .parse("activity-log", &json!({"tool_input": {}}))
                .is_none()
        );
    }

    #[test]
    fn unknown_event_returns_none() {
        assert!(OmpAdapter.parse("nope", &json!({})).is_none());
    }
}
