use serde_json::Value;

use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::PI_AGENT;

use super::{HookRegistration, json_str, json_value_or_null};

/// Read a field from JSON, trying `camelCase` then `snake_case` keys.
/// This lets the drift test (`assert_table_drift_free`, which sends
/// snake_case keys) and real Pi extension calls (camelCase) both work.
fn flex_str<'a>(val: &'a Value, primary: &str, fallback: &str) -> &'a str {
    let v = json_str(val, primary);
    if !v.is_empty() {
        v
    } else {
        json_str(val, fallback)
    }
}

fn flex_opt(val: &Value, primary: &str, fallback: &str) -> Option<String> {
    let v = flex_str(val, primary, fallback);
    if v.is_empty() {
        None
    } else {
        Some(v.into())
    }
}

fn flex_value(val: &Value, primary: &str, fallback: &str) -> Value {
    let v = json_value_or_null(val, primary);
    if v.is_null() {
        json_value_or_null(val, fallback)
    } else {
        v
    }
}

/// Normalize Pi's camelCase tool names to the PascalCase the sidebar activity
/// log expects. Pi events come through as: executeCommand, readFile, writeFile,
/// editFile, globFiles, grepFiles, searchWeb, fetchUrl, askUser, etc.
fn normalize_tool_name(raw: &str) -> String {
    match raw {
        "executeCommand" | "Bash" => "Bash".into(),
        "readFile" | "Read" => "Read".into(),
        "writeFile" | "Write" => "Write".into(),
        "editFile" | "Edit" => "Edit".into(),
        "globFiles" | "Glob" => "Glob".into(),
        "grepFiles" | "Grep" => "Grep".into(),
        "searchWeb" | "WebSearch" => "WebSearch".into(),
        "fetchUrl" | "WebFetch" => "WebFetch".into(),
        "askUser" | "AskUser" => "AskUser".into(),
        other => other.to_string(),
    }
}

pub struct PiAdapter;

impl PiAdapter {
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "session-start",
            matcher: None,
            kind: AgentEventKind::SessionStart,
        },
        HookRegistration {
            trigger: "session-end",
            matcher: None,
            kind: AgentEventKind::SessionEnd,
        },
        HookRegistration {
            trigger: "user-prompt-submit",
            matcher: None,
            kind: AgentEventKind::UserPromptSubmit,
        },
        HookRegistration {
            trigger: "stop",
            matcher: None,
            kind: AgentEventKind::Stop,
        },
        HookRegistration {
            trigger: "stop-failure",
            matcher: None,
            kind: AgentEventKind::StopFailure,
        },
        HookRegistration {
            trigger: "notification",
            matcher: None,
            kind: AgentEventKind::Notification,
        },
        HookRegistration {
            trigger: "activity-log",
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
                cwd: flex_str(input, "projectPath", "cwd").into(),
                permission_mode: String::new(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: flex_opt(input, "sessionId", "session_id"),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: flex_str(input, "endReason", "end_reason").into(),
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: PI_AGENT.into(),
                cwd: flex_str(input, "projectPath", "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: flex_opt(input, "sessionId", "session_id"),
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: PI_AGENT.into(),
                cwd: flex_str(input, "projectPath", "cwd").into(),
                permission_mode: String::new(),
                last_message: flex_str(input, "lastMessage", "last_message").into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: flex_opt(input, "sessionId", "session_id"),
            }),
            "stop-failure" => Some(AgentEvent::StopFailure {
                agent: PI_AGENT.into(),
                cwd: flex_str(input, "projectPath", "cwd").into(),
                permission_mode: String::new(),
                error: json_str(input, "error").into(),
                worktree: None,
                agent_id: None,
                session_id: flex_opt(input, "sessionId", "session_id"),
            }),
            "notification" => Some(AgentEvent::Notification {
                agent: PI_AGENT.into(),
                cwd: flex_str(input, "projectPath", "cwd").into(),
                permission_mode: String::new(),
                wait_reason: flex_str(input, "notificationType", "notification_type").into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: flex_opt(input, "sessionId", "session_id"),
            }),
            "activity-log" => {
                let raw_name = flex_str(input, "toolName", "tool_name");
                if raw_name.is_empty() {
                    return None;
                }
                let tool_name = normalize_tool_name(raw_name);
                Some(AgentEvent::ActivityLog {
                    tool_name,
                    tool_input: flex_value(input, "toolArgs", "tool_input"),
                    tool_response: flex_value(input, "result", "tool_response"),
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

    // ─── supported events ───────────────────────────────────────────

    #[test]
    fn session_start() {
        let adapter = PiAdapter;
        let input = json!({"projectPath": "/home/user", "sessionId": "sess-pi-1"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
                source: "".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-pi-1".into()),
            }
        );
    }

    #[test]
    fn session_start_no_project_path_defaults_empty() {
        let adapter = PiAdapter;
        let event = adapter.parse("session-start", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: PI_AGENT.into(),
                cwd: "".into(),
                permission_mode: "".into(),
                source: "".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn session_end() {
        let adapter = PiAdapter;
        assert_eq!(
            adapter
                .parse("session-end", &json!({"endReason": "finished"}))
                .unwrap(),
            AgentEvent::SessionEnd {
                end_reason: "finished".into(),
            }
        );
    }

    #[test]
    fn session_end_empty_reason() {
        let adapter = PiAdapter;
        assert_eq!(
            adapter.parse("session-end", &json!({})).unwrap(),
            AgentEvent::SessionEnd {
                end_reason: "".into(),
            }
        );
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = PiAdapter;
        let input = json!({
            "projectPath": "/tmp",
            "prompt": "fix bug",
            "sessionId": "sess-pi-2"
        });
        let event = adapter.parse("user-prompt-submit", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: PI_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "fix bug".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-pi-2".into()),
            }
        );
    }

    #[test]
    fn user_prompt_submit_missing_fields_default_to_empty() {
        let adapter = PiAdapter;
        let event = adapter.parse("user-prompt-submit", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: PI_AGENT.into(),
                cwd: "".into(),
                permission_mode: "".into(),
                prompt: "".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn stop() {
        let adapter = PiAdapter;
        let input = json!({
            "projectPath": "/tmp",
            "lastMessage": "done",
            "sessionId": "sess-pi-3"
        });
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: PI_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "done".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: Some("sess-pi-3".into()),
            }
        );
    }

    #[test]
    fn stop_empty_last_message() {
        let adapter = PiAdapter;
        let event = adapter.parse("stop", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: PI_AGENT.into(),
                cwd: "".into(),
                permission_mode: "".into(),
                last_message: "".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn stop_has_no_response() {
        let adapter = PiAdapter;
        let event = adapter.parse("stop", &json!({})).unwrap();
        match event {
            AgentEvent::Stop { response, .. } => assert!(response.is_none()),
            other => panic!("expected Stop, got {:?}", other),
        }
    }

    #[test]
    fn stop_failure() {
        let adapter = PiAdapter;
        let input = json!({
            "projectPath": "/tmp",
            "error": "rate_limit",
            "sessionId": "sess-pi-4"
        });
        let event = adapter.parse("stop-failure", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::StopFailure {
                agent: PI_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                error: "rate_limit".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-pi-4".into()),
            }
        );
    }

    #[test]
    fn stop_failure_empty_error() {
        let adapter = PiAdapter;
        let event = adapter.parse("stop-failure", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::StopFailure {
                agent: PI_AGENT.into(),
                cwd: "".into(),
                permission_mode: "".into(),
                error: "".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn notification() {
        let adapter = PiAdapter;
        let input = json!({"projectPath": "/tmp", "notificationType": "permission"});
        let event = adapter.parse("notification", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Notification {
                agent: PI_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                wait_reason: "permission".into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn notification_empty_type() {
        let adapter = PiAdapter;
        let event = adapter.parse("notification", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::Notification {
                agent: PI_AGENT.into(),
                cwd: "".into(),
                permission_mode: "".into(),
                wait_reason: "".into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn activity_log() {
        let adapter = PiAdapter;
        let input = json!({
            "toolName": "readFile",
            "toolArgs": {"filePath": "/a/b.rs"},
            "result": {"content": "fn main() {}"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_input["filePath"], "/a/b.rs");
                assert_eq!(tool_response["content"], "fn main() {}");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_normalizes_execute_command() {
        let adapter = PiAdapter;
        let input = json!({
            "toolName": "executeCommand",
            "toolArgs": {"command": "ls -la"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => {
                assert_eq!(tool_name, "Bash");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_normalizes_edit_file() {
        let adapter = PiAdapter;
        let input = json!({
            "toolName": "editFile",
            "toolArgs": {"filePath": "/a/b.rs", "oldString": "foo", "newString": "bar"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => {
                assert_eq!(tool_name, "Edit");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_unknown_tool_passes_through() {
        let adapter = PiAdapter;
        let input = json!({
            "toolName": "customMcpTool",
            "toolArgs": {"foo": "bar"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => {
                assert_eq!(tool_name, "customMcpTool");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_accepts_snake_case_keys() {
        // The drift test sends snake_case; this verifies flex_str handles it
        let adapter = PiAdapter;
        let input = json!({
            "tool_name": "readFile",
            "tool_input": {"file_path": "/a/b.rs"},
            "tool_response": {"content": "fn main() {}"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_input["file_path"], "/a/b.rs");
                assert_eq!(tool_response["content"], "fn main() {}");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_empty_tool_name_rejected() {
        let adapter = PiAdapter;
        assert!(adapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn activity_log_without_result() {
        let adapter = PiAdapter;
        let event = adapter
            .parse(
                "activity-log",
                &json!({"toolName": "readFile", "toolArgs": {"filePath": "/a/b.rs"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_input["filePath"], "/a/b.rs");
                assert_eq!(tool_response, Value::Null);
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    // ─── unsupported events ─────────────────────────────────────────

    #[test]
    fn permission_denied_not_supported() {
        assert!(PiAdapter.parse("permission-denied", &json!({})).is_none());
    }

    #[test]
    fn cwd_changed_not_supported() {
        assert!(PiAdapter.parse("cwd-changed", &json!({})).is_none());
    }

    #[test]
    fn subagent_start_not_supported() {
        assert!(PiAdapter
            .parse("subagent-start", &json!({"agent_type": "X"}))
            .is_none());
    }

    #[test]
    fn subagent_stop_not_supported() {
        assert!(PiAdapter
            .parse("subagent-stop", &json!({"agent_type": "X"}))
            .is_none());
    }

    #[test]
    fn task_created_not_supported() {
        assert!(PiAdapter.parse("task-created", &json!({})).is_none());
    }

    #[test]
    fn task_completed_not_supported() {
        assert!(PiAdapter.parse("task-completed", &json!({})).is_none());
    }

    #[test]
    fn teammate_idle_not_supported() {
        assert!(PiAdapter.parse("teammate-idle", &json!({})).is_none());
    }

    #[test]
    fn worktree_create_not_supported() {
        assert!(PiAdapter.parse("worktree-create", &json!({})).is_none());
    }

    #[test]
    fn worktree_remove_not_supported() {
        assert!(PiAdapter.parse("worktree-remove", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        assert!(PiAdapter.parse("something-else", &json!({})).is_none());
    }
}
