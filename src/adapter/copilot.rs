use crate::event::{AgentEvent, AgentEventKind, EventAdapter};
use crate::tmux::COPILOT_AGENT;
use serde_json::Value;

use super::{HookRegistration, json_str, json_value_or_null, optional_str};

pub struct CopilotAdapter;

/// Read the session id, accepting both Copilot's native camelCase
/// `sessionId` and the "VS Code compatible" snake_case `session_id`.
fn session_id_of(input: &Value) -> Option<String> {
    optional_str(input, "sessionId").or_else(|| optional_str(input, "session_id"))
}

impl CopilotAdapter {
    /// Single source of truth for GitHub Copilot CLI hook wiring.
    ///
    /// Copilot CLI hooks use a config shape distinct from Claude/Codex/
    /// Devin (`{ "version": 1, "hooks": { <camelCase event>: [{ "type":
    /// "command", "bash": ..., "powershell": ... }] } }` — no `matcher`
    /// field on the hook entries, and command hooks use `bash`/`powershell`
    /// instead of a single `command` field) and camelCase event/field
    /// names, verified against the official reference at
    /// <https://docs.github.com/en/copilot/reference/hooks-reference>.
    ///
    /// Copilot CLI's hook event enum (subset wired here): `sessionStart`,
    /// `sessionEnd`, `userPromptSubmitted`, `preToolUse`, `postToolUse`,
    /// `agentStop`, `notification`. Copilot also exposes `permissionRequest`,
    /// `postToolUseFailure`, `preCompact`, `subagentStart`, `subagentStop`,
    /// `errorOccurred`, and `userPromptTransformed`, but those are out of
    /// scope for this adapter.
    ///
    /// Caveats:
    /// - `preToolUse` is supported by Copilot CLI but not yet wired — same
    ///   rationale as Codex/Devin's unwired `PreToolUse`: `postToolUse`
    ///   already covers the activity log, and there is no existing
    ///   `AgentEventKind` for a pre-execution moment on its own.
    /// - `agentStop`'s payload has no assistant-response text field (only
    ///   `transcriptPath`), so `AgentEvent::Stop.last_message` is always
    ///   empty for Copilot.
    /// - `notification` is the event that actually covers "the agent is
    ///   waiting on you" states: per GitHub's hooks reference its
    ///   `notification_type` field takes values `permission_prompt` (a
    ///   tool wants approval) and `elicitation_dialog` (the agent is
    ///   asking a clarifying/multi-choice question) among others. Both
    ///   values already match the existing `is_permission_wait_reason`
    ///   allowlist used by Claude's own Notification handling, so mapping
    ///   this straight onto `AgentEventKind::Notification` (same as
    ///   Claude) makes the sidebar show "waiting" for Copilot's
    ///   permission prompts *and* its multi-choice questions with no
    ///   further handler changes needed. `permissionRequest` (the
    ///   separate hook that can programmatically allow/deny) is left
    ///   unwired — it fires for every tool call, not just the ones that
    ///   end up needing a human decision, so it isn't a useful
    ///   "waiting" signal on its own.
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
            trigger: "userPromptSubmitted",
            matcher: None,
            kind: AgentEventKind::UserPromptSubmit,
        },
        HookRegistration {
            trigger: "notification",
            matcher: None,
            kind: AgentEventKind::Notification,
        },
        HookRegistration {
            trigger: "agentStop",
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

impl EventAdapter for CopilotAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: COPILOT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                source: json_str(input, "source").into(),
                worktree: None,
                agent_id: None,
                session_id: session_id_of(input),
            }),
            "session-end" => Some(AgentEvent::SessionEnd {
                end_reason: json_str(input, "reason").into(),
            }),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: COPILOT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
                session_id: session_id_of(input),
            }),
            "notification" => {
                let wait_reason = json_str(input, "notification_type");
                Some(AgentEvent::Notification {
                    agent: COPILOT_AGENT.into(),
                    cwd: json_str(input, "cwd").into(),
                    permission_mode: String::new(),
                    wait_reason: wait_reason.into(),
                    meta_only: false,
                    worktree: None,
                    agent_id: None,
                    session_id: session_id_of(input),
                })
            }
            "stop" => Some(AgentEvent::Stop {
                agent: COPILOT_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: String::new(),
                last_message: String::new(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: session_id_of(input),
            }),
            "activity-log" => {
                // Copilot CLI supports two payload shapes for the same
                // trigger depending on how the hook event name is cased in
                // the config file: the native camelCase format
                // (`toolName`/`toolArgs`/`toolResult`) and the "VS Code
                // compatible" format (`tool_name`/`tool_input`/
                // `tool_result`), which uses snake_case field names when
                // the trigger is configured in PascalCase (`PostToolUse`
                // instead of `postToolUse`). Accept both.
                let mut tool_name = json_str(input, "toolName");
                if tool_name.is_empty() {
                    tool_name = json_str(input, "tool_name");
                }
                if tool_name.is_empty() {
                    return None;
                }
                let tool_input = match input.get("toolArgs") {
                    Some(v) => v.clone(),
                    None => json_value_or_null(input, "tool_input"),
                };
                let tool_response = match input.get("toolResult") {
                    Some(v) => v.clone(),
                    None => json_value_or_null(input, "tool_result"),
                };
                Some(AgentEvent::ActivityLog {
                    tool_name: tool_name.into(),
                    tool_input,
                    tool_response,
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
        super::super::assert_table_drift_free("copilot", CopilotAdapter::HOOK_REGISTRATIONS);
    }

    #[test]
    fn session_start() {
        let adapter = CopilotAdapter;
        let input = json!({"cwd": "/home/user", "sessionId": "ses-copilot-1", "source": "startup"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: COPILOT_AGENT.into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
                source: "startup".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-copilot-1".into()),
            }
        );
    }

    #[test]
    fn session_start_missing_fields_default_to_empty() {
        let adapter = CopilotAdapter;
        let event = adapter.parse("session-start", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: "copilot".into(),
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
        let adapter = CopilotAdapter;
        let event = adapter
            .parse("session-end", &json!({"reason": "complete"}))
            .unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionEnd {
                end_reason: "complete".into()
            }
        );
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = CopilotAdapter;
        let input = json!({"cwd": "/tmp", "prompt": "hello", "sessionId": "ses-copilot-2"});
        let event = adapter.parse("user-prompt-submit", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: COPILOT_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "hello".into(),
                worktree: None,
                agent_id: None,
                session_id: Some("ses-copilot-2".into()),
            }
        );
    }

    #[test]
    fn notification_captures_permission_prompt_wait_reason() {
        let adapter = CopilotAdapter;
        let input = json!({
            "sessionId": "ses-copilot-notif",
            "cwd": "/tmp",
            "message": "Permission needed to run a command",
            "title": "Permission needed",
            "notification_type": "permission_prompt"
        });
        let event = adapter.parse("notification", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Notification {
                agent: COPILOT_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                wait_reason: "permission_prompt".into(),
                meta_only: false,
                worktree: None,
                agent_id: None,
                session_id: Some("ses-copilot-notif".into()),
            }
        );
    }

    #[test]
    fn notification_captures_elicitation_dialog_wait_reason() {
        // This is the multi-choice / clarifying-question case: Copilot's
        // hooks reference documents `elicitation_dialog` as firing when
        // "the agent requests additional information from the user."
        let adapter = CopilotAdapter;
        let input = json!({
            "notification_type": "elicitation_dialog",
            "message": "Which dotfile manager should we use?"
        });
        let event = adapter.parse("notification", &input).unwrap();
        match event {
            AgentEvent::Notification { wait_reason, .. } => {
                assert_eq!(wait_reason, "elicitation_dialog");
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    #[test]
    fn notification_missing_type_defaults_to_empty_wait_reason() {
        let adapter = CopilotAdapter;
        let event = adapter.parse("notification", &json!({})).unwrap();
        match event {
            AgentEvent::Notification { wait_reason, .. } => {
                assert_eq!(wait_reason, "");
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    #[test]
    fn stop_has_no_response() {
        let adapter = CopilotAdapter;
        let input = json!({
            "cwd": "/tmp",
            "transcriptPath": "/tmp/transcript",
            "stopReason": "end_turn",
            "sessionId": "ses-copilot-3"
        });
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: COPILOT_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "".into(),
                response: None,
                worktree: None,
                agent_id: None,
                session_id: Some("ses-copilot-3".into()),
            }
        );
    }

    #[test]
    fn activity_log_from_post_tool_use() {
        let adapter = CopilotAdapter;
        let input = json!({
            "toolName": "bash",
            "toolArgs": {"command": "ls -la"},
            "toolResult": {"resultType": "success", "textResultForLlm": "file.txt\n"}
        });
        let event = adapter.parse("activity-log", &input).unwrap();
        match event {
            AgentEvent::ActivityLog {
                tool_name,
                tool_input,
                tool_response,
            } => {
                assert_eq!(tool_name, "bash");
                assert_eq!(
                    tool_input.get("command").and_then(|v| v.as_str()),
                    Some("ls -la")
                );
                assert_eq!(
                    tool_response.get("resultType").and_then(|v| v.as_str()),
                    Some("success")
                );
            }
            other => panic!("expected ActivityLog, got {other:?}"),
        }
    }

    #[test]
    fn activity_log_empty_tool_name_rejected() {
        let adapter = CopilotAdapter;
        assert!(adapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn pre_tool_use_not_supported() {
        // preToolUse is a real Copilot CLI trigger but is not yet wired to
        // an internal AgentEventKind (see HOOK_REGISTRATIONS doc comment).
        assert!(CopilotAdapter.parse("pre-tool-use", &json!({})).is_none());
    }

    #[test]
    fn permission_denied_not_supported() {
        assert!(
            CopilotAdapter
                .parse("permission-denied", &json!({}))
                .is_none()
        );
    }

    #[test]
    fn subagent_events_not_supported() {
        assert!(
            CopilotAdapter
                .parse("subagent-start", &json!({"agent_type": "X"}))
                .is_none()
        );
        assert!(
            CopilotAdapter
                .parse("subagent-stop", &json!({"agent_type": "X"}))
                .is_none()
        );
    }

    #[test]
    fn cwd_changed_not_supported() {
        assert!(CopilotAdapter.parse("cwd-changed", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        assert!(CopilotAdapter.parse("something-else", &json!({})).is_none());
    }
}
