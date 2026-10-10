use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::DEVIN_AGENT;
use serde_json::Value;

use super::{HookRegistration, json_str, json_value_or_null, optional_str};

pub struct DevinAdapter;

impl DevinAdapter {
    /// Single source of truth for Devin CLI hook wiring. Devin CLI's hook
    /// system is deliberately modeled on Claude Code's hook format
    /// (command hooks receive JSON on stdin, config files use the same
    /// `{ trigger: [{ matcher, hooks: [{ type: "command", command }] }] }`
    /// shape), but Devin CLI does not have Claude-style subagents, so there
    /// is no `SubagentStart`/`SubagentStop` equivalent.
    ///
    /// Devin CLI's hook event enum: `PreToolUse`, `PostToolUse`,
    /// `PermissionRequest`, `UserPromptSubmit`, `Stop`, `PostCompaction`,
    /// `SessionStart`, `SessionEnd`.
    ///
    /// Caveats:
    /// - `PreToolUse` is registered here *only* for Devin's two
    ///   interactive tools, `ask_user_question` (a multi-choice question
    ///   the agent poses mid-turn) and `exit_plan_mode` (presenting a
    ///   plan for approval). Both block the CLI on a selection UI with no
    ///   corresponding `PostToolUse`-adjacent "resolved" signal of their
    ///   own until the user answers, and neither is a permission check on
    ///   an existing tool, so nothing else in the hook set reports this
    ///   state. `PreToolUse` for every other tool is intentionally left
    ///   unwired (see below) and routed to `None` in `parse()`.
    /// - `PostCompaction` is not wired — there is no existing
    ///   `AgentEventKind` for compaction/summary events, and none of the
    ///   other adapters (Claude included) model one either.
    /// - `PermissionRequest` and the two interactive `PreToolUse` cases
    ///   above both map to `AgentEventKind::PermissionDenied`. This is the
    ///   closest existing kind for "the sidebar should show waiting on a
    ///   pane blocked on a human decision"; Devin's `PermissionRequest`
    ///   hook fires when a decision is needed rather than only on denial,
    ///   and the interactive tools aren't permission checks at all, but
    ///   reusing the existing kind avoids introducing a near-duplicate
    ///   variant. Both native triggers are wired to the same
    ///   `hook.sh devin permission-denied` command; `parse()`
    ///   distinguishes them via `hook_event_name` + `tool_name` so a
    ///   `PreToolUse` for e.g. `exec` or `write` is correctly ignored.
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
            trigger: "PermissionRequest",
            matcher: None,
            kind: AgentEventKind::PermissionDenied,
        },
        HookRegistration {
            trigger: "PreToolUse (ask_user_question / exit_plan_mode only)",
            matcher: None,
            kind: AgentEventKind::PermissionDenied,
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

    /// Devin tools that block the CLI on a human decision via `PreToolUse`
    /// rather than via `PermissionRequest`. Kept in sync with
    /// `agent-tmux-manager`'s `atm-devin-adapter`, which validated these
    /// tool names against real Devin CLI behavior.
    fn is_interactive_tool(tool_name: &str) -> bool {
        matches!(tool_name, "ask_user_question" | "exit_plan_mode")
    }
}

impl EventAdapter for DevinAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: DEVIN_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: json_str(input, "reason").into(),
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: DEVIN_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: optional_str(input, "session_id"),
            }),
            "permission-denied" => {
                // Devin's real PermissionRequest hook and its two
                // interactive tools (ask_user_question, exit_plan_mode,
                // fired via PreToolUse) are both wired to this same
                // command. A PreToolUse for any other tool must be
                // ignored -- see the HOOK_REGISTRATIONS doc comment.
                let tool_name = json_str(input, "tool_name");
                if json_str(input, "hook_event_name") == "PreToolUse"
                    && !Self::is_interactive_tool(tool_name)
                {
                    return None;
                }
                Some(AgentEvent::PermissionDenied {
                    agent: DEVIN_AGENT.into(),
                    cwd: json_str(input, "cwd").into(),
                    permission_mode: String::new(),
                    worktree: None,
                    agent_id: None,
                    session_id: optional_str(input, "session_id"),
                })
            }
            "stop" => Some(AgentEvent::Stop {
                agent: DEVIN_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: String::new(),
                response: None,
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
    fn hook_registrations_match_parse_arms() {
        super::super::assert_table_drift_free("devin", DevinAdapter::HOOK_REGISTRATIONS);
    }

    #[test]
    fn session_start() {
        let adapter = DevinAdapter;
        let input = json!({"cwd": "/home/user", "session_id": "sess-devin-1"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: DEVIN_AGENT.into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
                source: "".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-devin-1".into()),
            }
        );
    }

    #[test]
    fn session_start_captures_source() {
        let adapter = DevinAdapter;
        let input = json!({"cwd": "/tmp", "source": "startup"});
        let event = adapter.parse("session-start", &input).unwrap();
        match event {
            AgentEvent::SessionStart { source, .. } => assert_eq!(source, "startup"),
            other => panic!("expected SessionStart, got {other:?}"),
        }
    }

    #[test]
    fn session_start_missing_fields_default_to_empty() {
        let adapter = DevinAdapter;
        let event = adapter.parse("session-start", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: "devin".into(),
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
    fn session_end_captures_reason() {
        let adapter = DevinAdapter;
        let event = adapter
            .parse("session-end", &json!({"reason": "user_exit"}))
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionEnd {
                end_reason: "user_exit".into()
            }
        );
    }

    #[test]
    fn session_end_missing_reason_defaults_to_empty() {
        let adapter = DevinAdapter;
        assert_eq!(
            adapter.parse("session-end", &json!({})).unwrap(),
            AgentEvent::SessionEnd {
                end_reason: "".into()
            }
        );
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = DevinAdapter;
        let input = json!({"cwd": "/tmp", "prompt": "fix the bug", "session_id": "sess-devin-2"});
        let event = adapter.parse("user-prompt-submit", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: DEVIN_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "fix the bug".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-devin-2".into()),
            }
        );
    }

    #[test]
    fn permission_denied_round_trip() {
        let adapter = DevinAdapter;
        let input = json!({
            "cwd": "/tmp",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf /"},
            "session_id": "sess-devin-3"
        });
        let event = adapter.parse("permission-denied", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::PermissionDenied {
                agent: DEVIN_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-devin-3".into()),
            }
        );
    }

    #[test]
    fn permission_denied_missing_fields_default_to_empty() {
        let adapter = DevinAdapter;
        assert_eq!(
            adapter.parse("permission-denied", &json!({})).unwrap(),
            AgentEvent::PermissionDenied {
                agent: "devin".into(),
                cwd: "".into(),
                permission_mode: "".into(),
                worktree: None,
                agent_id: None,
                session_id: None,
            }
        );
    }

    #[test]
    fn pre_tool_use_ask_user_question_becomes_permission_denied() {
        // The multi-choice question case: Devin blocks on a selection UI
        // via a real ask_user_question tool call, with PreToolUse as the
        // only signal.
        let adapter = DevinAdapter;
        let input = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "ask_user_question",
            "cwd": "/tmp",
            "session_id": "sess-devin-question"
        });
        let event = adapter.parse("permission-denied", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::PermissionDenied {
                agent: DEVIN_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("sess-devin-question".into()),
            }
        );
    }

    #[test]
    fn pre_tool_use_exit_plan_mode_becomes_permission_denied() {
        let adapter = DevinAdapter;
        let input = json!({"hook_event_name": "PreToolUse", "tool_name": "exit_plan_mode"});
        assert!(adapter.parse("permission-denied", &input).is_some());
    }

    #[test]
    fn pre_tool_use_non_interactive_tool_ignored() {
        // A normal tool starting (e.g. exec, write) must not be treated
        // as "waiting on the user" just because it shares the
        // permission-denied command with the interactive tools.
        let adapter = DevinAdapter;
        for tool_name in ["exec", "write", "edit", "read"] {
            let input = json!({"hook_event_name": "PreToolUse", "tool_name": tool_name});
            assert!(
                adapter.parse("permission-denied", &input).is_none(),
                "tool {tool_name} should not produce an event"
            );
        }
    }

    #[test]
    fn stop_has_no_response() {
        let adapter = DevinAdapter;
        let input = json!({"cwd": "/tmp", "stop_hook_active": false, "session_id": "sess-devin-4"});
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: DEVIN_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: Some("sess-devin-4".into()),
            }
        );
    }

    #[test]
    fn activity_log_from_post_tool_use() {
        let adapter = DevinAdapter;
        let input = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"},
            "tool_response": {"success": true, "output": "file.txt\n", "error": null}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(
                    tool_input.get("command").and_then(|v| v.as_str()),
                    Some("ls -la")
                );
                assert_eq!(tool_response.get("success"), Some(&json!(true)));
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_empty_tool_name_rejected() {
        let adapter = DevinAdapter;
        assert!(adapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn pre_tool_use_kebab_name_unused() {
        // PreToolUse is wired via the shared "permission-denied" command
        // (see HOOK_REGISTRATIONS doc comment), not its own "pre-tool-use"
        // event name -- that string is never sent by hook.sh and must not
        // accidentally match anything.
        assert!(DevinAdapter.parse("pre-tool-use", &json!({})).is_none());
    }

    #[test]
    fn post_compaction_not_supported() {
        // Devin's PostCompaction has no existing AgentEventKind equivalent.
        assert!(
            DevinAdapter
                .parse("post-compaction", &json!({"summary": "compacted"}))
                .is_none()
        );
    }

    #[test]
    fn subagent_events_not_supported() {
        // Devin CLI does not emit SubagentStart/SubagentStop.
        assert!(
            DevinAdapter
                .parse("subagent-start", &json!({"agent_type": "X"}))
                .is_none()
        );
        assert!(
            DevinAdapter
                .parse("subagent-stop", &json!({"agent_type": "X"}))
                .is_none()
        );
    }

    #[test]
    fn cwd_changed_not_supported() {
        assert!(DevinAdapter.parse("cwd-changed", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        assert!(DevinAdapter.parse("something-else", &json!({})).is_none());
    }
}
