use serde_json::{Map, Value};

use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::CURSOR_AGENT;
use crate::tool_name::CanonicalTool;

use super::{HookRegistration, json_str, optional_str};

pub struct CursorAdapter;

impl CursorAdapter {
    /// Single source of truth for Cursor CLI (`cursor-agent`) hook wiring.
    ///
    /// Cursor documents a much larger hook surface than the CLI actually
    /// fires — the agent-loop hooks below are the ones confirmed to run in
    /// the terminal client. Everything else is either an editor-only hook
    /// (Tab completions, `workspaceOpen`) or a known CLI gap.
    ///
    /// Deliberate omissions:
    /// - `beforeSubmitPrompt` would give us `UserPromptSubmit` (prompt text
    ///   plus an immediate `running` flip at turn start), but it does not
    ///   fire in the CLI. Until it does, a Cursor pane leaves `idle` on its
    ///   first tool call instead (see `handle_activity_log`).
    /// - `afterAgentResponse` would give `Stop` a `last_message` for the
    ///   `▷ …` response preview. Also CLI-inert today, so Cursor rows show
    ///   no response text.
    /// - `afterFileEdit` fires, but `postToolUse` already reports Cursor's
    ///   `Write` / `Delete` tools. Wiring both would write two activity
    ///   entries per edit.
    /// - `beforeShellExecution` / `beforeMCPExecution` are permission gates
    ///   whose reply decides whether the call proceeds. The sidebar is a
    ///   read-only observer, so it stays out of that path.
    pub const HOOK_REGISTRATIONS: &'static [HookRegistration] = &[
        HookRegistration {
            trigger: "sessionStart",
            matcher: None,
            kind: AgentEventKind::SessionStart,
        },
        HookRegistration {
            trigger: "sessionEnd",
            matcher: None,
            kind: AgentEventKind::SessionEnd,
        },
        HookRegistration {
            trigger: "stop",
            matcher: None,
            kind: AgentEventKind::Stop,
        },
        HookRegistration {
            trigger: "postToolUse",
            matcher: None,
            kind: AgentEventKind::ActivityLog,
        },
    ];
}

/// Cursor only puts `cwd` on the tool hooks; `sessionStart` / `sessionEnd` /
/// `stop` carry the workspace via the shared `workspace_roots` array. Fall
/// back to its first entry so every event still resolves a repository path
/// for grouping and git lookups.
fn cwd(input: &Value) -> String {
    let explicit = json_str(input, "cwd");
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    input
        .get("workspace_roots")
        .and_then(Value::as_array)
        .and_then(|roots| roots.first())
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// `conversation_id` identifies the run throughout Cursor's hooks. Lifecycle
/// payloads can also include `session_id`, but stop events use
/// `conversation_id`, so it is the stable value for correlating panes.
fn session_id(input: &Value) -> Option<String> {
    optional_str(input, "conversation_id").or_else(|| optional_str(input, "session_id"))
}

/// Map Cursor's tool vocabulary onto [`CanonicalTool`] so the activity log
/// and the label strategy table in `src/cli/label.rs` share one vocabulary
/// across agents. MCP calls arrive as `MCP:<tool>`; rewriting them to the
/// `mcp__<tool>` shape Claude uses picks up the violet MCP color in
/// `ActivityEntry::tool_color_index`.
fn normalize_tool_name(raw: &str) -> String {
    if let Some(tool) = raw.strip_prefix("MCP:") {
        return format!("mcp__{tool}");
    }
    match raw {
        "Shell" => CanonicalTool::Bash.as_str().to_string(),
        "Task" => CanonicalTool::Agent.as_str().to_string(),
        // `Read`, `Write`, `Grep` are already canonical; `Delete` and any
        // future tool pass through and render with an empty label.
        other => other.to_string(),
    }
}

/// Copy Cursor's tool-argument keys onto the snake_case names the label
/// extractor expects. Aliases are added alongside the originals rather than
/// replacing them, so anything downstream that wants the raw payload still
/// sees it. Cursor does not publish a per-tool `tool_input` schema, so the
/// path aliases cover every spelling its tools are known to use
/// (`target_file` in the edit tools, `path` / `filePath` elsewhere).
fn normalize_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut map) = input else {
        return input;
    };
    const PATH_ALIASES: &[(&str, &str)] = &[
        ("target_file", "file_path"),
        ("path", "file_path"),
        ("filePath", "file_path"),
    ];
    let rewrites: &[(&str, &str)] = match tool_name {
        "Read" | "Write" | "Edit" | "Delete" => PATH_ALIASES,
        "Grep" => &[("query", "pattern")],
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

/// Cursor hands `tool_output` over as a JSON-encoded *string*, not an
/// object. Decode it so label extractors can index into it like they do for
/// Claude; a payload that is not valid JSON is kept verbatim as a string.
fn tool_response(input: &Value) -> Value {
    match input.get("tool_output") {
        Some(Value::String(raw)) => {
            serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.clone()))
        }
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

impl EventAdapter for CursorAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: CURSOR_AGENT.into(),
                cwd: cwd(input),
                permission_mode: String::new(),
                // Cursor has no `startup` / `resume` / `compact` source; its
                // `composer_mode` describes the chat mode instead, which is
                // not what `on_session_start` keys off.
                source: String::new(),
                worktree: None,
                agent_id: None,
                session_id: session_id(input),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: json_str(input, "reason").into(),
            }),
            // Cursor's `stop` carries the turn outcome instead of exposing a
            // separate failure hook, so an errored turn is re-routed to
            // `StopFailure` — that is what paints the row red and records a
            // wait reason. `completed` / `aborted` / a missing status all
            // land on the normal `Stop` path.
            "stop" => {
                let status = json_str(input, "status");
                if status == "error" {
                    return Some(AgentEvent::StopFailure {
                        agent: CURSOR_AGENT.into(),
                        cwd: cwd(input),
                        permission_mode: String::new(),
                        error: status.into(),
                        worktree: None,
                        agent_id: None,
                        session_id: session_id(input),
                    });
                }
                Some(AgentEvent::Stop {
                    agent: CURSOR_AGENT.into(),
                    cwd: cwd(input),
                    permission_mode: String::new(),
                    // `afterAgentResponse` (the only hook carrying the
                    // assistant's text) does not fire in the CLI, so there is
                    // no response preview to store.
                    last_message: String::new(),
                    response: None,
                    worktree: None,
                    agent_id: None,
                    session_id: session_id(input),
                })
            }
            "activity-log" => {
                let raw_name = json_str(input, "tool_name");
                if raw_name.is_empty() {
                    return None;
                }
                let tool_name = normalize_tool_name(raw_name);
                let tool_input = normalize_tool_input(
                    &tool_name,
                    super::json_value_or_null(input, "tool_input"),
                );
                Some(AgentEvent::ActivityLog {
                    tool_name,
                    tool_input,
                    tool_response: tool_response(input),
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
        super::super::assert_table_drift_free("cursor", CursorAdapter::HOOK_REGISTRATIONS);
    }

    #[test]
    fn session_start_uses_workspace_root_as_cwd() {
        let event = CursorAdapter
            .parse(
                "session-start",
                &json!({
                    "hook_event_name": "sessionStart",
                    "session_id": "ses-1",
                    "is_background_agent": false,
                    "composer_mode": "agent",
                    "workspace_roots": ["/home/user/repo"],
                    "conversation_id": "conv-1",
                }),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: CURSOR_AGENT.into(),
                cwd: "/home/user/repo".into(),
                permission_mode: "".into(),
                source: "".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("conv-1".into()),
            }
        );
    }

    #[test]
    fn session_start_missing_fields_default_to_empty() {
        let event = CursorAdapter.parse("session-start", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: CURSOR_AGENT.into(),
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
    fn session_end_carries_reason() {
        let event = CursorAdapter
            .parse(
                "session-end",
                &json!({"session_id": "ses-1", "reason": "user_close", "duration_ms": 4200}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionEnd {
                end_reason: "user_close".into()
            }
        );
    }

    #[test]
    fn stop_completed_has_no_response_and_no_last_message() {
        let event = CursorAdapter
            .parse(
                "stop",
                &json!({
                    "status": "completed",
                    "loop_count": 0,
                    "conversation_id": "conv-9",
                    "workspace_roots": ["/repo"],
                }),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: CURSOR_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "".into(),
                last_message: "".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: Some("conv-9".into()),
            }
        );
    }

    #[test]
    fn stop_aborted_stays_on_the_normal_stop_path() {
        let event = CursorAdapter
            .parse("stop", &json!({"status": "aborted"}))
            .unwrap();
        assert!(matches!(event, AgentEvent::Stop { .. }));
    }

    #[test]
    fn stop_error_routes_to_stop_failure() {
        let event = CursorAdapter
            .parse(
                "stop",
                &json!({"status": "error", "workspace_roots": ["/repo"], "conversation_id": "c1"}),
            )
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::StopFailure {
                agent: CURSOR_AGENT.into(),
                cwd: "/repo".into(),
                permission_mode: "".into(),
                error: "error".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("c1".into()),
            }
        );
    }

    #[test]
    fn stop_failure_event_name_is_not_accepted() {
        // Cursor has no dedicated failure hook — the only way to reach
        // `StopFailure` is an errored `stop`. Accepting the external name
        // directly would imply a registration the table does not declare.
        assert!(CursorAdapter.parse("stop-failure", &json!({})).is_none());
    }

    #[test]
    fn activity_log_shell_maps_to_bash() {
        let event = CursorAdapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "Shell",
                    "tool_input": {"command": "npm test"},
                    "tool_output": "{\"exitCode\":0,\"stdout\":\"ok\"}",
                    "cwd": "/repo",
                    "duration": 5432,
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
                assert_eq!(tool_input["command"], "npm test");
                // The stringified `tool_output` is decoded into real JSON.
                assert_eq!(tool_response["exitCode"], 0);
                assert_eq!(tool_response["stdout"], "ok");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_task_maps_to_agent() {
        let event = CursorAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "Task", "tool_input": {"description": "Explore repo"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, "Agent"),
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_mcp_tool_gets_claude_style_prefix() {
        let event = CursorAdapter
            .parse("activity-log", &json!({"tool_name": "MCP:search_docs"}))
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => {
                assert_eq!(tool_name, "mcp__search_docs");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_normalizes_target_file_to_file_path() {
        let event = CursorAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "Write", "tool_input": {"target_file": "/repo/src/main.rs"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "Write");
                assert_eq!(tool_input["file_path"], "/repo/src/main.rs");
                assert_eq!(tool_input["target_file"], "/repo/src/main.rs");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_existing_file_path_wins_over_alias() {
        let event = CursorAdapter
            .parse(
                "activity-log",
                &json!({
                    "tool_name": "Read",
                    "tool_input": {"file_path": "/real.rs", "path": "/alias.rs"},
                }),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_input, .. } => {
                assert_eq!(tool_input["file_path"], "/real.rs");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_grep_query_maps_to_pattern() {
        let event = CursorAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "Grep", "tool_input": {"query": "fn main"}}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_input, .. } => {
                assert_eq!(tool_input["pattern"], "fn main");
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_unparseable_tool_output_kept_as_string() {
        let event = CursorAdapter
            .parse(
                "activity-log",
                &json!({"tool_name": "Shell", "tool_output": "not json at all"}),
            )
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_response, .. } => {
                assert_eq!(tool_response, json!("not json at all"));
            }
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_unknown_tool_passes_through() {
        let event = CursorAdapter
            .parse("activity-log", &json!({"tool_name": "Delete"}))
            .unwrap();
        match event {
            AgentEvent::ActivityLog { tool_name, .. } => assert_eq!(tool_name, "Delete"),
            other => panic!("expected ActivityLog, got {:?}", other),
        }
    }

    #[test]
    fn activity_log_empty_tool_name_rejected() {
        assert!(CursorAdapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unwired_events_are_ignored() {
        // Every event the table does not declare must be rejected so the
        // drift test's parse → table direction stays meaningful.
        for event in [
            "user-prompt-submit",
            "notification",
            "permission-denied",
            "cwd-changed",
            "subagent-start",
            "subagent-stop",
            "task-created",
            "task-completed",
            "teammate-idle",
            "worktree-create",
            "worktree-remove",
            "something-else",
        ] {
            assert!(
                CursorAdapter.parse(event, &json!({})).is_none(),
                "{event} should not be handled by the Cursor adapter"
            );
        }
    }
}
