use serde_json::{Map, Value};

use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::GROK_AGENT;
use crate::tool_name::CanonicalTool;

use super::HookRegistration;

pub struct GrokAdapter;

impl GrokAdapter {
    /// Grok Build lifecycle hooks used by the sidebar. Grok emits camelCase
    /// payloads. Snake-case parser alternatives below are the official Grok
    /// Agent SDK conversions of those fields, not Claude payload aliases.
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "SessionStart",
            matcher: None,
            kind: AgentEventKind::SessionStart,
        },
        HookRegistration {
            trigger: "SessionEnd",
            matcher: None,
            kind: AgentEventKind::SessionEnd,
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
            trigger: "StopFailure",
            matcher: None,
            kind: AgentEventKind::StopFailure,
        },
        HookRegistration {
            trigger: "StopCancelled",
            matcher: None,
            kind: AgentEventKind::TurnSettled,
        },
        HookRegistration {
            trigger: "Notification",
            matcher: Some("permission_prompt"),
            kind: AgentEventKind::Notification,
        },
        HookRegistration {
            trigger: "Notification",
            matcher: Some("idle_prompt"),
            kind: AgentEventKind::TurnSettled,
        },
        HookRegistration {
            trigger: "PermissionDenied",
            matcher: None,
            kind: AgentEventKind::PermissionDenied,
        },
        HookRegistration {
            trigger: "SubagentStart",
            matcher: None,
            kind: AgentEventKind::SubagentStart,
        },
        HookRegistration {
            trigger: "SubagentStop",
            matcher: None,
            kind: AgentEventKind::SubagentStop,
        },
        HookRegistration {
            trigger: "PostToolUse",
            matcher: None,
            kind: AgentEventKind::ActivityLog,
        },
        HookRegistration {
            trigger: "PostToolUseFailure",
            matcher: None,
            kind: AgentEventKind::ActivityLog,
        },
    ];
}

fn json_str<'a>(input: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str))
        .unwrap_or("")
}

fn optional_str(input: &Value, keys: &[&str]) -> Option<String> {
    let value = json_str(input, keys);
    (!value.is_empty()).then(|| value.to_string())
}

fn extract_user_query(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(after_open) = trimmed.strip_prefix("<user_query>") else {
        return raw.to_string();
    };
    let Some(close) = after_open.rfind("</user_query>") else {
        return raw.to_string();
    };
    after_open[..close].trim().to_string()
}

fn is_subagent_event(input: &Value) -> bool {
    !json_str(input, &["subagentType", "subagent_type"]).is_empty()
}

fn value_or_null(input: &Value, keys: &[&str]) -> Value {
    keys.iter()
        .find_map(|key| input.get(*key).cloned())
        .unwrap_or(Value::Null)
}

fn normalize_tool_name(raw: &str) -> String {
    let canonical = match raw {
        "run_terminal_command" => CanonicalTool::Bash,
        "read_file" => CanonicalTool::Read,
        "search_replace" => CanonicalTool::Edit,
        "grep" => CanonicalTool::Grep,
        "list_dir" => CanonicalTool::Glob,
        "web_search" => CanonicalTool::WebSearch,
        "spawn_subagent" => CanonicalTool::Agent,
        other => return other.to_string(),
    };
    canonical.as_str().to_string()
}

fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };

    if matches!(tool_name, "Read" | "Write" | "Edit") {
        copy_key(&mut map, "path", "file_path");
        copy_key(&mut map, "filePath", "file_path");
        copy_key(&mut map, "target_file", "file_path");
    }
    if tool_name == "Glob" {
        copy_key(&mut map, "target_directory", "pattern");
    }

    Value::Object(map)
}

fn copy_key(map: &mut Map<String, Value>, source: &str, destination: &str) {
    if map.contains_key(destination) {
        return;
    }
    if let Some(value) = map.get(source).cloned() {
        map.insert(destination.to_string(), value);
    }
}

impl EventAdapter for GrokAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            // Grok dispatches SessionStart only from the top-level agent path;
            // its typed SessionStart payload has no subagent identity field.
            "session-start" => Some(AgentEvent::SessionStart {
                agent: GROK_AGENT.into(),
                cwd: json_str(input, &["cwd"]).into(),
                permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                source: json_str(input, &["source"]).into(),
                top_level: true,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, &["sessionId", "session_id"]),
            }),
            "session-end" if !is_subagent_event(input) => Some(AgentEvent::SessionEnd {
                agent: GROK_AGENT.into(),
                session_id: optional_str(input, &["sessionId", "session_id"]),
                requires_existing_session: true,
                end_reason: json_str(input, &["reason"]).into(),
                top_level: true,
            }),
            "user-prompt-submit" if !is_subagent_event(input) => {
                Some(AgentEvent::UserPromptSubmit {
                    agent: GROK_AGENT.into(),
                    cwd: json_str(input, &["cwd"]).into(),
                    permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                    prompt: extract_user_query(json_str(input, &["prompt"])),
                    prompt_is_system_message: false,
                    requires_existing_session: true,
                    prompt_id: optional_str(input, &["promptId", "prompt_id"]),
                    worktree: None,
                    agent_id: None,
                    session_id: optional_str(input, &["sessionId", "session_id"]),
                })
            }
            "notification" => Some(AgentEvent::Notification {
                agent: GROK_AGENT.into(),
                cwd: json_str(input, &["cwd"]).into(),
                permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                wait_reason: json_str(input, &["notificationType", "notification_type"]).into(),
                meta_only: false,
                requires_existing_session: true,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, &["sessionId", "session_id"]),
            }),
            "stop" if !matches!(json_str(input, &["reason"]), "channel_closed" | "shutdown") => {
                Some(AgentEvent::Stop {
                    agent: GROK_AGENT.into(),
                    cwd: json_str(input, &["cwd"]).into(),
                    permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                    last_message: json_str(
                        input,
                        &["lastAssistantMessage", "last_assistant_message"],
                    )
                    .into(),
                    response: None,
                    prompt_id: optional_str(input, &["promptId", "prompt_id"]),
                    requires_existing_session: true,
                    children_may_outlive_turn: true,
                    worktree: None,
                    agent_id: None,
                    session_id: optional_str(input, &["sessionId", "session_id"]),
                })
            }
            "turn-settled" if !is_subagent_event(input) => Some(AgentEvent::TurnSettled {
                agent: GROK_AGENT.into(),
                cwd: json_str(input, &["cwd"]).into(),
                permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                prompt_id: optional_str(input, &["promptId", "prompt_id"]),
                requires_existing_session: true,
                children_may_outlive_turn: true,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, &["sessionId", "session_id"]),
            }),
            "stop-failure" if !is_subagent_event(input) => Some(AgentEvent::StopFailure {
                agent: GROK_AGENT.into(),
                cwd: json_str(input, &["cwd"]).into(),
                permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                error: json_str(
                    input,
                    &[
                        "errorDetails",
                        "error_details",
                        "lastAssistantMessage",
                        "last_assistant_message",
                        "error",
                    ],
                )
                .into(),
                prompt_id: optional_str(input, &["promptId", "prompt_id"]),
                requires_existing_session: true,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, &["sessionId", "session_id"]),
            }),
            "permission-denied" => Some(AgentEvent::PermissionDenied {
                agent: GROK_AGENT.into(),
                cwd: json_str(input, &["cwd"]).into(),
                permission_mode: json_str(input, &["permissionMode", "permission_mode"]).into(),
                requires_existing_session: true,
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, &["sessionId", "session_id"]),
            }),
            "subagent-start" => {
                let agent_type = json_str(input, &["subagentType", "subagent_type"]);
                if agent_type.is_empty() {
                    return None;
                }
                let agent_id = optional_str(input, &["subagentId", "subagent_id"])?;
                Some(AgentEvent::SubagentStart {
                    agent: GROK_AGENT.into(),
                    session_id: optional_str(input, &["sessionId", "session_id"]),
                    requires_existing_session: true,
                    agent_type: agent_type.into(),
                    agent_id: Some(agent_id),
                    display_name: optional_str(input, &["description"]),
                    children_may_outlive_turn: true,
                })
            }
            "subagent-stop" => {
                let agent_type = json_str(input, &["subagentType", "subagent_type"]);
                if agent_type.is_empty() {
                    return None;
                }
                let agent_id = optional_str(input, &["subagentId", "subagent_id"])?;
                Some(AgentEvent::SubagentStop {
                    agent_type: agent_type.into(),
                    agent_id: Some(agent_id),
                    last_message: json_str(
                        input,
                        &["lastAssistantMessage", "last_assistant_message"],
                    )
                    .into(),
                    transcript_path: json_str(input, &["transcriptPath", "transcript_path"]).into(),
                    children_may_outlive_turn: true,
                })
            }
            "activity-log" => {
                let raw_name = json_str(input, &["toolName", "tool_name"]);
                if raw_name.is_empty() {
                    return None;
                }
                let tool_name = normalize_tool_name(raw_name);
                let tool_input = normalize_tool_input(
                    &tool_name,
                    value_or_null(input, &["toolInput", "tool_input"]),
                );
                Some(AgentEvent::ActivityLog {
                    agent: GROK_AGENT.into(),
                    session_id: optional_str(input, &["sessionId", "session_id"]),
                    requires_existing_session: true,
                    tool_name,
                    tool_input,
                    tool_response: value_or_null(input, &["toolResult", "tool_result"]),
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
        super::super::assert_table_drift_free("grok", GrokAdapter::HOOK_REGISTRATIONS);
    }

    #[test]
    fn user_prompt_submit_reads_camel_case_context() {
        let event = GrokAdapter
            .parse(
                "user-prompt-submit",
                &json!({
                    "cwd": "/repo",
                    "permissionMode": "plan",
                    "prompt": "fix the tests",
                    "promptId": "prompt-2",
                    "sessionId": "session-2"
                }),
            )
            .unwrap();

        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: GROK_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "plan".into(),
                prompt: "fix the tests".into(),
                prompt_is_system_message: false,
                requires_existing_session: true,
                prompt_id: Some("prompt-2".into()),
                worktree: None,
                agent_id: None,
                session_id: Some("session-2".into()),
            }
        );
    }

    #[test]
    fn user_prompt_submit_extracts_query_from_grok_prompt_envelope() {
        let cases = [
            ("<user_query> ok go </user_query>", "ok go"),
            (
                "<user_query>\nfix the tests\nand update the docs\n</user_query>",
                "fix the tests\nand update the docs",
            ),
            (
                "<user_query>\nexplain <example>this XML</example>\n</user_query>\n\
                 <skill_information>internal instructions</skill_information>\n\n\
                 <system-reminder><attached_files>context</attached_files></system-reminder>",
                "explain <example>this XML</example>",
            ),
            (
                "<user_query>\nexplain the literal </user_query> token\n</user_query>",
                "explain the literal </user_query> token",
            ),
        ];

        for (prompt, expected) in cases {
            let event = GrokAdapter
                .parse("user-prompt-submit", &json!({"prompt": prompt}))
                .unwrap();
            assert!(
                matches!(event, AgentEvent::UserPromptSubmit { prompt, .. } if prompt == expected),
                "failed to extract Grok query from {prompt:?}"
            );
        }
    }

    #[test]
    fn user_prompt_submit_preserves_non_envelope_xml() {
        let cases = [
            "plain prompt",
            "explain <user_query>this example</user_query>",
            "<user_query>unfinished",
            "<system-reminder>literal user-authored example</system-reminder>",
        ];

        for prompt in cases {
            let event = GrokAdapter
                .parse("user-prompt-submit", &json!({"prompt": prompt}))
                .unwrap();
            assert!(
                matches!(event, AgentEvent::UserPromptSubmit { prompt: actual, .. } if actual == prompt),
                "changed non-envelope prompt {prompt:?}"
            );
        }
    }

    #[test]
    fn session_end_reads_reason() {
        assert_eq!(
            GrokAdapter.parse(
                "session-end",
                &json!({"sessionId": "host-session", "reason": "shutdown"}),
            ),
            Some(AgentEvent::SessionEnd {
                agent: GROK_AGENT.into(),
                session_id: Some("host-session".into()),
                requires_existing_session: true,
                end_reason: "shutdown".into(),
                top_level: true,
            })
        );
    }

    #[test]
    fn child_session_end_does_not_clear_host_session() {
        assert!(
            GrokAdapter
                .parse(
                    "session-end",
                    &json!({"reason": "shutdown", "subagentType": "explore"})
                )
                .is_none()
        );
    }

    #[test]
    fn agent_type_is_harness_metadata_not_subagent_identity() {
        let event = GrokAdapter
            .parse(
                "session-start",
                &json!({
                    "agent_type": "grok-build",
                    "cwd": "/repo",
                    "session_id": "session-sdk"
                }),
            )
            .expect("SDK agent_type must not suppress a top-level session");
        assert!(matches!(event, AgentEvent::SessionStart { .. }));

        assert!(
            GrokAdapter
                .parse("subagent-start", &json!({"agent_type": "explore"}))
                .is_none(),
            "Grok child identity is subagentType, not agent_type"
        );
    }

    #[test]
    fn stop_is_observational_and_reads_last_message() {
        let event = GrokAdapter
            .parse(
                "stop",
                &json!({
                    "cwd": "/repo",
                    "permissionMode": "auto",
                    "lastAssistantMessage": "done",
                    "promptId": "prompt-3",
                    "sessionId": "session-3"
                }),
            )
            .unwrap();

        match event {
            AgentEvent::Stop {
                agent,
                last_message,
                response,
                prompt_id,
                session_id,
                requires_existing_session,
                children_may_outlive_turn,
                ..
            } => {
                assert_eq!(agent, GROK_AGENT);
                assert_eq!(last_message, "done");
                assert!(response.is_none(), "Grok Stop must emit no decision output");
                assert_eq!(prompt_id.as_deref(), Some("prompt-3"));
                assert_eq!(session_id.as_deref(), Some("session-3"));
                assert!(requires_existing_session);
                assert!(children_may_outlive_turn);
            }
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn child_stop_failure_does_not_settle_host_session() {
        assert!(
            GrokAdapter
                .parse("stop-failure", &json!({"subagentType": "explore"}))
                .is_none(),
            "StopFailure from a child must be ignored"
        );
    }

    #[test]
    fn stop_failure_prefers_human_readable_details() {
        let event = GrokAdapter
            .parse(
                "stop-failure",
                &json!({
                    "error": "rate_limit",
                    "errorDetails": "capacity temporarily unavailable",
                    "lastAssistantMessage": "request failed",
                    "sessionId": "session-failure"
                }),
            )
            .unwrap();

        match event {
            AgentEvent::StopFailure {
                error,
                session_id,
                requires_existing_session,
                ..
            } => {
                assert_eq!(error, "capacity temporarily unavailable");
                assert_eq!(session_id.as_deref(), Some("session-failure"));
                assert!(requires_existing_session);
            }
            other => panic!("expected StopFailure, got {other:?}"),
        }
    }

    #[test]
    fn permission_prompt_notification_sets_wait_reason() {
        let event = GrokAdapter
            .parse(
                "notification",
                &json!({
                    "cwd": "/repo",
                    "permissionMode": "default",
                    "notificationType": "permission_prompt",
                    "sessionId": "session-4"
                }),
            )
            .unwrap();

        match event {
            AgentEvent::Notification {
                agent,
                wait_reason,
                meta_only,
                requires_existing_session,
                ..
            } => {
                assert_eq!(agent, GROK_AGENT);
                assert_eq!(wait_reason, "permission_prompt");
                assert!(!meta_only);
                assert!(requires_existing_session);
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    #[test]
    fn cancelled_and_idle_backstop_map_to_non_notifying_settlement() {
        let event = GrokAdapter
            .parse(
                "turn-settled",
                &json!({"notificationType": "idle_prompt", "sessionId": "session-5"}),
            )
            .unwrap();

        match event {
            AgentEvent::TurnSettled {
                prompt_id,
                session_id,
                requires_existing_session,
                children_may_outlive_turn,
                ..
            } => {
                assert!(prompt_id.is_none());
                assert_eq!(session_id.as_deref(), Some("session-5"));
                assert!(requires_existing_session);
                assert!(children_may_outlive_turn);
            }
            other => panic!("expected TurnSettled, got {other:?}"),
        }
    }

    #[test]
    fn session_end_stop_is_ignored_in_favor_of_session_end_hook() {
        assert!(
            GrokAdapter
                .parse("stop", &json!({"reason": "shutdown"}))
                .is_none()
        );
        assert!(
            GrokAdapter
                .parse("stop", &json!({"reason": "channel_closed"}))
                .is_none()
        );
    }

    #[test]
    fn permission_denied_preserves_agent_context() {
        let event = GrokAdapter
            .parse(
                "permission-denied",
                &json!({
                    "cwd": "/repo",
                    "permissionMode": "plan",
                    "sessionId": "session-6"
                }),
            )
            .unwrap();

        assert_eq!(
            event,
            AgentEvent::PermissionDenied {
                agent: GROK_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "plan".into(),
                requires_existing_session: true,
                worktree: None,
                agent_id: None,
                session_id: Some("session-6".into()),
            }
        );
    }

    #[test]
    fn subagent_lifecycle_uses_documented_type_and_id() {
        assert_eq!(
            GrokAdapter.parse(
                "subagent-start",
                &json!({
                    "subagentId": "subagent-1",
                    "subagentType": "explore",
                    "description": "Review error handling",
                    "sessionId": "host-session",
                    "promptId": "child-turn-1"
                }),
            ),
            Some(AgentEvent::SubagentStart {
                agent: GROK_AGENT.into(),
                session_id: Some("host-session".into()),
                requires_existing_session: true,
                agent_type: "explore".into(),
                agent_id: Some("subagent-1".into()),
                display_name: Some("Review error handling".into()),
                children_may_outlive_turn: true,
            })
        );
        assert_eq!(
            GrokAdapter.parse(
                "subagent-stop",
                &json!({
                    "subagentId": "subagent-1",
                    "subagentType": "explore",
                    "promptId": "child-turn-1",
                    "lastAssistantMessage": "found it",
                    "transcriptPath": "/tmp/subagent.jsonl"
                }),
            ),
            Some(AgentEvent::SubagentStop {
                agent_type: "explore".into(),
                agent_id: Some("subagent-1".into()),
                last_message: "found it".into(),
                transcript_path: "/tmp/subagent.jsonl".into(),
                children_may_outlive_turn: true,
            })
        );
    }

    #[test]
    fn subagent_lifecycle_requires_documented_id() {
        for event_name in ["subagent-start", "subagent-stop"] {
            assert!(
                GrokAdapter
                    .parse(event_name, &json!({"subagentType": "explore"}))
                    .is_none(),
                "{event_name} must not invent a missing subagentId"
            );
        }
    }

    #[test]
    fn claude_only_field_names_are_not_treated_as_grok_payloads() {
        assert_eq!(
            GrokAdapter.parse("session-end", &json!({"endReason": "shutdown"})),
            Some(AgentEvent::SessionEnd {
                agent: GROK_AGENT.into(),
                session_id: None,
                requires_existing_session: true,
                end_reason: "".into(),
                top_level: true,
            })
        );

        let notification = GrokAdapter
            .parse("notification", &json!({"wait_reason": "permission_prompt"}))
            .unwrap();
        assert!(matches!(
            notification,
            AgentEvent::Notification { wait_reason, .. } if wait_reason.is_empty()
        ));

        assert!(
            GrokAdapter
                .parse(
                    "subagent-start",
                    &json!({"agentId": "claude-child", "subagentType": "explore"}),
                )
                .is_none()
        );

        let activity = GrokAdapter
            .parse(
                "activity-log",
                &json!({"toolName": "read_file", "tool_response": {"content": "x"}}),
            )
            .unwrap();
        assert!(matches!(
            activity,
            AgentEvent::ActivityLog {
                tool_response: Value::Null,
                ..
            }
        ));
    }

    #[test]
    fn empty_subagent_type_is_ignored() {
        assert!(
            GrokAdapter
                .parse("subagent-start", &json!({"promptId": "child-turn-1"}))
                .is_none()
        );
        assert!(
            GrokAdapter
                .parse("subagent-stop", &json!({"promptId": "child-turn-1"}))
                .is_none()
        );
    }

    #[test]
    fn native_terminal_tool_maps_to_bash() {
        let event = GrokAdapter
            .parse(
                "activity-log",
                &json!({
                    "sessionId": "child-session-1",
                    "toolName": "run_terminal_command",
                    "toolInput": {"command": "cargo test"}
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                agent,
                session_id,
                requires_existing_session,
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(agent, GROK_AGENT);
                assert_eq!(session_id.as_deref(), Some("child-session-1"));
                assert!(requires_existing_session);
                assert_eq!(tool_name, "Bash");
                assert_eq!(tool_input["command"], "cargo test");
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn native_file_and_directory_keys_feed_shared_labels() {
        let read = GrokAdapter
            .parse(
                "activity-log",
                &json!({
                    "toolName": "read_file",
                    "toolInput": {"target_file": "/repo/src/main.rs"}
                }),
            )
            .unwrap();
        let list = GrokAdapter
            .parse(
                "activity-log",
                &json!({
                    "toolName": "list_dir",
                    "toolInput": {"target_directory": "/repo/src"}
                }),
            )
            .unwrap();

        match read {
            AgentEvent::ActivityLog { tool_input, .. } => {
                assert_eq!(tool_input["file_path"], "/repo/src/main.rs")
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
        match list {
            AgentEvent::ActivityLog { tool_input, .. } => {
                assert_eq!(tool_input["pattern"], "/repo/src")
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn unknown_native_tool_name_passes_through() {
        let event = GrokAdapter
            .parse("activity-log", &json!({"toolName": "custom__tool"}))
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, "custom__tool"),
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn empty_tool_name_is_ignored() {
        assert!(GrokAdapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_is_ignored() {
        assert!(GrokAdapter.parse("not-an-event", &json!({})).is_none());
    }
}
