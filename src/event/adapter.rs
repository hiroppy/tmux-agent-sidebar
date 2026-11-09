use serde_json::Value;

use super::AgentEvent;
use crate::adapter;
use crate::tmux::{CLAUDE_AGENT, CODEX_AGENT, GROK_AGENT, OPENCODE_AGENT};

/// Adapter that converts external agent events into internal `AgentEvent`.
pub trait EventAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent>;
}

pub fn resolve_adapter(agent_name: &str) -> Option<Box<dyn EventAdapter>> {
    match agent_name {
        CLAUDE_AGENT => Some(Box::new(adapter::claude::ClaudeAdapter)),
        CODEX_AGENT => Some(Box::new(adapter::codex::CodexAdapter)),
        GROK_AGENT => Some(Box::new(adapter::grok::GrokAdapter)),
        OPENCODE_AGENT => Some(Box::new(adapter::opencode::OpenCodeAdapter)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_claude() {
        let adapter = resolve_adapter("claude");
        assert!(adapter.is_some());
        let event = adapter.unwrap().parse("session-end", &json!({}));
        assert_eq!(
            event,
            Some(AgentEvent::SessionEnd {
                agent: "claude".into(),
                session_id: None,
                requires_existing_session: false,
                end_reason: "".into(),
                top_level: false,
            })
        );
    }

    #[test]
    fn resolve_codex() {
        let adapter = resolve_adapter("codex");
        assert!(adapter.is_some());
    }

    #[test]
    fn resolve_opencode() {
        let adapter = resolve_adapter("opencode");
        assert!(adapter.is_some());
    }

    #[test]
    fn resolve_grok_parses_camel_case_session_start() {
        let adapter = resolve_adapter("grok").expect("Grok adapter should resolve");
        let event = adapter
            .parse(
                "session-start",
                &json!({
                    "cwd": "/tmp/project",
                    "permissionMode": "auto",
                    "sessionId": "grok-session-1",
                    "source": "resume"
                }),
            )
            .expect("Grok SessionStart should parse");

        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: "grok".into(),
                cwd: "/tmp/project".into(),
                permission_mode: "auto".into(),
                source: "resume".into(),
                top_level: true,
                worktree: None,
                agent_id: None,
                session_id: Some("grok-session-1".into()),
            }
        );
    }

    #[test]
    fn resolve_grok_normalizes_native_tool_payload() {
        let adapter = resolve_adapter("grok").expect("Grok adapter should resolve");
        let event = adapter
            .parse(
                "activity-log",
                &json!({
                    "toolName": "read_file",
                    "toolInput": {"path": "/tmp/project/src/main.rs"},
                    "toolResult": {"content": "fn main() {}"}
                }),
            )
            .expect("Grok PostToolUse should parse");

        assert_eq!(
            event,
            AgentEvent::ActivityLog {
                agent: "grok".into(),
                session_id: None,
                requires_existing_session: true,
                tool_name: "Read".into(),
                tool_input: json!({
                    "path": "/tmp/project/src/main.rs",
                    "file_path": "/tmp/project/src/main.rs"
                }),
                tool_response: json!({"content": "fn main() {}"}),
            }
        );
    }

    #[test]
    fn resolve_unknown_returns_none() {
        assert!(resolve_adapter("gemini").is_none());
        assert!(resolve_adapter("").is_none());
    }

    // ─── integration: resolve + parse produce correct agent names ─────

    #[test]
    fn claude_adapter_sets_agent_claude() {
        let adapter = resolve_adapter("claude").unwrap();
        let event = adapter
            .parse("user-prompt-submit", &json!({"prompt": "hi"}))
            .unwrap();
        match event {
            AgentEvent::UserPromptSubmit { agent, .. } => assert_eq!(agent, "claude"),
            other => panic!("expected UserPromptSubmit, got {:?}", other),
        }
    }

    #[test]
    fn codex_adapter_sets_agent_codex() {
        let adapter = resolve_adapter("codex").unwrap();
        let event = adapter
            .parse("user-prompt-submit", &json!({"prompt": "hi"}))
            .unwrap();
        match event {
            AgentEvent::UserPromptSubmit { agent, .. } => assert_eq!(agent, "codex"),
            other => panic!("expected UserPromptSubmit, got {:?}", other),
        }
    }

    #[test]
    fn opencode_adapter_sets_agent_opencode() {
        let adapter = resolve_adapter("opencode").unwrap();
        let event = adapter
            .parse("user-prompt-submit", &json!({"prompt": "hi"}))
            .unwrap();
        match event {
            AgentEvent::UserPromptSubmit { agent, .. } => assert_eq!(agent, "opencode"),
            other => panic!("expected UserPromptSubmit, got {:?}", other),
        }
    }

    #[test]
    fn adapters_normalize_prompt_system_classification() {
        for (agent_name, expected) in [
            ("claude", true),
            ("codex", true),
            ("opencode", true),
            ("grok", false),
        ] {
            let event = resolve_adapter(agent_name)
                .unwrap()
                .parse(
                    "user-prompt-submit",
                    &json!({"prompt": "explain <system-reminder>this tag</system-reminder>"}),
                )
                .unwrap();
            assert!(matches!(
                event,
                AgentEvent::UserPromptSubmit {
                    prompt_is_system_message,
                    ..
                } if prompt_is_system_message == expected
            ));
        }
    }

    #[test]
    fn claude_stop_has_no_response() {
        let adapter = resolve_adapter("claude").unwrap();
        let event = adapter.parse("stop", &json!({})).unwrap();
        match event {
            AgentEvent::Stop { response, .. } => assert!(response.is_none()),
            other => panic!("expected Stop, got {:?}", other),
        }
    }

    #[test]
    fn codex_stop_has_continue_response() {
        let adapter = resolve_adapter("codex").unwrap();
        let event = adapter.parse("stop", &json!({})).unwrap();
        match event {
            AgentEvent::Stop { response, .. } => {
                assert_eq!(response, Some("{\"continue\":true}".into()));
            }
            other => panic!("expected Stop, got {:?}", other),
        }
    }

    #[test]
    fn adapters_normalize_child_turn_lifetime_policy() {
        for (agent_name, expected) in [
            ("claude", false),
            ("codex", false),
            ("opencode", false),
            ("grok", true),
        ] {
            let event = resolve_adapter(agent_name)
                .unwrap()
                .parse("stop", &json!({}))
                .unwrap();
            assert!(
                matches!(
                    event,
                    AgentEvent::Stop {
                        children_may_outlive_turn,
                        ..
                    } if children_may_outlive_turn == expected
                ),
                "unexpected child-turn lifetime policy for {agent_name}: {event:?}"
            );
        }
    }

    #[test]
    fn adapters_normalize_subagent_start_child_turn_lifetime_policy() {
        // A child registering after its parent settled may only revive the
        // background lifecycle for agents whose children outlive the turn,
        // so SubagentStart must carry the same policy its Stop arm declares.
        // Only Claude and Grok construct the variant; Codex returns None for
        // `subagent-start` (pinned by `codex_ignores_claude_only_events`).
        for (agent_name, payload, expected) in [
            ("claude", json!({"agent_type": "Explore"}), false),
            (
                "grok",
                json!({"subagentType": "explore", "subagentId": "sub-1"}),
                true,
            ),
        ] {
            let event = resolve_adapter(agent_name)
                .unwrap()
                .parse("subagent-start", &payload)
                .unwrap();
            assert!(
                matches!(
                    event,
                    AgentEvent::SubagentStart {
                        children_may_outlive_turn,
                        ..
                    } if children_may_outlive_turn == expected
                ),
                "unexpected child-turn lifetime policy for {agent_name}: {event:?}"
            );
        }
    }

    #[test]
    fn codex_ignores_claude_only_events() {
        let adapter = resolve_adapter("codex").unwrap();
        assert!(adapter.parse("notification", &json!({})).is_none());
        assert!(adapter.parse("stop-failure", &json!({})).is_none());
        assert!(
            adapter
                .parse("subagent-start", &json!({"agent_type": "X"}))
                .is_none()
        );
        assert!(
            adapter
                .parse("subagent-stop", &json!({"agent_type": "X"}))
                .is_none()
        );
    }

    #[test]
    fn claude_idle_prompt_returns_meta_only_notification() {
        let adapter = resolve_adapter("claude").unwrap();
        let input =
            json!({"cwd": "/tmp", "permission_mode": "auto", "notification_type": "idle_prompt"});
        let event = adapter.parse("notification", &input).unwrap();
        match event {
            AgentEvent::Notification {
                meta_only,
                wait_reason,
                agent,
                cwd,
                permission_mode,
                ..
            } => {
                assert!(meta_only, "idle_prompt should be meta_only");
                assert_eq!(wait_reason, "idle_prompt");
                assert_eq!(agent, "claude");
                assert_eq!(cwd, "/tmp");
                assert_eq!(permission_mode, "auto");
            }
            other => panic!("expected Notification, got {:?}", other),
        }
    }

    #[test]
    fn claude_normal_notification_is_not_meta_only() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({"notification_type": "permission"});
        let event = adapter.parse("notification", &input).unwrap();
        match event {
            AgentEvent::Notification { meta_only, .. } => {
                assert!(!meta_only, "normal notification should not be meta_only");
            }
            other => panic!("expected Notification, got {:?}", other),
        }
    }

    #[test]
    fn both_adapters_handle_session_start() {
        for agent_name in &["claude", "codex"] {
            let adapter = resolve_adapter(agent_name).unwrap();
            assert!(
                adapter.parse("session-start", &json!({})).is_some(),
                "{agent_name} should handle session-start"
            );
        }
        // Codex does not fire SessionEnd, so only Claude handles it.
        let claude = resolve_adapter("claude").unwrap();
        assert_eq!(
            claude.parse("session-end", &json!({})),
            Some(AgentEvent::SessionEnd {
                agent: "claude".into(),
                session_id: None,
                requires_existing_session: false,
                end_reason: "".into(),
                top_level: false,
            }),
        );
        assert!(
            resolve_adapter("codex")
                .unwrap()
                .parse("session-end", &json!({}))
                .is_none()
        );
    }

    #[test]
    fn claude_permission_denied_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({
            "cwd": "/tmp",
            "permission_mode": "auto",
            "tool_name": "Bash",
            "agent_id": "sub-1"
        });
        let event = adapter.parse("permission-denied", &input).unwrap();
        match event {
            AgentEvent::PermissionDenied {
                agent,
                permission_mode,
                agent_id,
                ..
            } => {
                assert_eq!(agent, "claude");
                assert_eq!(permission_mode, "auto");
                assert_eq!(agent_id, Some("sub-1".into()));
            }
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn claude_cwd_changed_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({"cwd": "/new/dir"});
        let event = adapter.parse("cwd-changed", &input).unwrap();
        match event {
            AgentEvent::CwdChanged {
                cwd,
                worktree,
                agent_id,
                ..
            } => {
                assert_eq!(cwd, "/new/dir");
                assert!(worktree.is_none());
                assert!(agent_id.is_none());
            }
            other => panic!("expected CwdChanged, got {:?}", other),
        }
    }

    #[test]
    fn codex_ignores_new_events() {
        let adapter = resolve_adapter("codex").unwrap();
        assert!(adapter.parse("permission-denied", &json!({})).is_none());
        assert!(adapter.parse("cwd-changed", &json!({})).is_none());
        assert!(adapter.parse("task-created", &json!({})).is_none());
        assert!(adapter.parse("task-completed", &json!({})).is_none());
        assert!(adapter.parse("teammate-idle", &json!({})).is_none());
        assert!(adapter.parse("worktree-create", &json!({})).is_none());
        assert!(adapter.parse("worktree-remove", &json!({})).is_none());
    }

    #[test]
    fn claude_task_created_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({"task_id": "7", "task_subject": "Deploy fix"});
        let event = adapter.parse("task-created", &input).unwrap();
        match event {
            AgentEvent::TaskCreated {
                task_id,
                task_subject,
            } => {
                assert_eq!(task_id, "7");
                assert_eq!(task_subject, "Deploy fix");
            }
            other => panic!("expected TaskCreated, got {:?}", other),
        }
    }

    #[test]
    fn claude_task_completed_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({"task_id": "7", "task_subject": "Deploy fix"});
        let event = adapter.parse("task-completed", &input).unwrap();
        match event {
            AgentEvent::TaskCompleted {
                task_id,
                task_subject,
            } => {
                assert_eq!(task_id, "7");
                assert_eq!(task_subject, "Deploy fix");
            }
            other => panic!("expected TaskCompleted, got {:?}", other),
        }
    }

    #[test]
    fn claude_teammate_idle_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({
            "teammate_name": "reviewer",
            "team_name": "dev",
            "idle_reason": "tokens_exhausted"
        });
        let event = adapter.parse("teammate-idle", &input).unwrap();
        match event {
            AgentEvent::TeammateIdle {
                teammate_name,
                team_name,
                idle_reason,
            } => {
                assert_eq!(teammate_name, "reviewer");
                assert_eq!(team_name, "dev");
                assert_eq!(idle_reason, "tokens_exhausted");
            }
            other => panic!("expected TeammateIdle, got {:?}", other),
        }
    }

    #[test]
    fn codex_rejects_new_events_with_full_payloads() {
        let adapter = resolve_adapter("codex").unwrap();
        // Codex should ignore all new lifecycle events even with realistic payloads
        assert!(
            adapter
                .parse(
                    "task-created",
                    &json!({"task_id": "1", "task_subject": "Deploy"})
                )
                .is_none()
        );
        assert!(
            adapter
                .parse(
                    "task-completed",
                    &json!({"task_id": "1", "task_subject": "Deploy"})
                )
                .is_none()
        );
        assert!(
            adapter
                .parse(
                    "teammate-idle",
                    &json!({"teammate_name": "reviewer", "team_name": "dev"})
                )
                .is_none()
        );
        assert!(
            adapter
                .parse("worktree-remove", &json!({"worktree_path": "/tmp/wt"}))
                .is_none()
        );
    }

    #[test]
    fn claude_stop_failure_upstream_fields_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({
            "cwd": "/tmp",
            "permission_mode": "auto",
            "error_type": "billing_error",
            "error_message": "Quota exceeded"
        });
        let event = adapter.parse("stop-failure", &input).unwrap();
        match event {
            AgentEvent::StopFailure {
                error,
                permission_mode,
                ..
            } => {
                assert_eq!(error, "billing_error");
                assert_eq!(permission_mode, "auto");
            }
            other => panic!("expected StopFailure, got {:?}", other),
        }
    }

    #[test]
    fn claude_stop_with_worktree() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({
            "cwd": "/tmp/wt",
            "permission_mode": "auto",
            "worktree": {
                "name": "wt",
                "path": "/tmp/wt",
                "branch": "feat",
                "originalRepoDir": "/home/user/repo"
            }
        });
        let event = adapter.parse("stop", &input).unwrap();
        match event {
            AgentEvent::Stop { worktree, .. } => {
                let wt = worktree.unwrap();
                assert_eq!(wt.original_repo_dir, "/home/user/repo");
            }
            other => panic!("expected Stop, got {:?}", other),
        }
    }
}
