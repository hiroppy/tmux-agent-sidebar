use serde_json::Value;

use crate::adapter;

/// Worktree metadata from Claude Code hook payloads.
/// All fields are empty strings when the agent is not running in a worktree.
#[derive(Debug, Clone, PartialEq)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub original_repo_dir: String,
}

/// Internal event representation. All fields are pre-extracted by the adapter.
/// The core handler never reads raw JSON or checks agent names.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    SessionStart {
        agent: String,
        cwd: String,
        permission_mode: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
    SessionEnd,
    UserPromptSubmit {
        agent: String,
        cwd: String,
        permission_mode: String,
        prompt: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
    Notification {
        agent: String,
        cwd: String,
        permission_mode: String,
        wait_reason: String,
        /// When true, only refresh pane metadata without changing status/attention.
        /// Used for events like idle_prompt that carry metadata but should not
        /// trigger a visible status change.
        meta_only: bool,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
    Stop {
        agent: String,
        cwd: String,
        permission_mode: String,
        last_message: String,
        response: Option<String>,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
    StopFailure {
        agent: String,
        cwd: String,
        permission_mode: String,
        error: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
    SubagentStart {
        agent_type: String,
    },
    SubagentStop {
        agent_type: String,
    },
    ActivityLog {
        tool_name: String,
        tool_input: Value,
        tool_response: Value,
    },
    PermissionDenied {
        agent: String,
        cwd: String,
        permission_mode: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
    CwdChanged {
        cwd: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
    },
}

/// Adapter that converts external agent events into internal `AgentEvent`.
pub trait EventAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent>;
}

pub fn resolve_adapter(agent_name: &str) -> Option<Box<dyn EventAdapter>> {
    match agent_name {
        "claude" => Some(Box::new(adapter::claude::ClaudeAdapter)),
        "codex" => Some(Box::new(adapter::codex::CodexAdapter)),
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
        assert_eq!(event, Some(AgentEvent::SessionEnd));
    }

    #[test]
    fn resolve_codex() {
        let adapter = resolve_adapter("codex");
        assert!(adapter.is_some());
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
        assert!(
            adapter
                .parse("activity-log", &json!({"tool_name": "Read"}))
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
    fn worktree_info_default_is_none() {
        let event = AgentEvent::SessionStart {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            worktree: None,
            agent_id: None,
        };
        match event {
            AgentEvent::SessionStart {
                worktree, agent_id, ..
            } => {
                assert!(worktree.is_none());
                assert!(agent_id.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn worktree_info_with_values() {
        let wt = WorktreeInfo {
            name: "feat-branch".into(),
            path: "/tmp/wt".into(),
            branch: "feat".into(),
            original_repo_dir: "/home/user/repo".into(),
        };
        let event = AgentEvent::SessionStart {
            agent: "claude".into(),
            cwd: "/tmp/wt".into(),
            permission_mode: "default".into(),
            worktree: Some(wt.clone()),
            agent_id: Some("abc-123".into()),
        };
        match event {
            AgentEvent::SessionStart {
                worktree, agent_id, ..
            } => {
                let wt = worktree.unwrap();
                assert_eq!(wt.original_repo_dir, "/home/user/repo");
                assert_eq!(agent_id.unwrap(), "abc-123");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn both_adapters_handle_session_lifecycle() {
        for agent_name in &["claude", "codex"] {
            let adapter = resolve_adapter(agent_name).unwrap();
            assert!(
                adapter.parse("session-start", &json!({})).is_some(),
                "{agent_name} should handle session-start"
            );
            assert_eq!(
                adapter.parse("session-end", &json!({})),
                Some(AgentEvent::SessionEnd),
                "{agent_name} should handle session-end"
            );
        }
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
