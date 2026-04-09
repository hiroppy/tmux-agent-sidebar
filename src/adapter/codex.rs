use crate::event::{AgentEvent, EventAdapter};
use crate::tmux::CODEX_AGENT;
use serde_json::Value;

use super::json_str;

pub struct CodexAdapter;

impl EventAdapter for CodexAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent> {
        match event_name {
            "session-start" => Some(AgentEvent::SessionStart {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                worktree: None,
                agent_id: None,
            }),
            "session-end" => Some(AgentEvent::SessionEnd),
            "user-prompt-submit" => Some(AgentEvent::UserPromptSubmit {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                prompt: json_str(input, "prompt").into(),
                worktree: None,
                agent_id: None,
            }),
            "stop" => Some(AgentEvent::Stop {
                agent: CODEX_AGENT.into(),
                cwd: json_str(input, "cwd").into(),
                permission_mode: json_str(input, "permission_mode").into(),
                last_message: json_str(input, "last_assistant_message").into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
            }),
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
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/home/user"});
        let event = adapter.parse("session-start", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: CODEX_AGENT.into(),
                cwd: "/home/user".into(),
                permission_mode: "".into(),
                worktree: None,
                agent_id: None,
            }
        );
    }

    #[test]
    fn session_end() {
        let adapter = CodexAdapter;
        assert_eq!(
            adapter.parse("session-end", &json!({})).unwrap(),
            AgentEvent::SessionEnd
        );
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "prompt": "hello"});
        let event = adapter.parse("user-prompt-submit", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::UserPromptSubmit {
                agent: CODEX_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                prompt: "hello".into(),
                worktree: None,
                agent_id: None,
            }
        );
    }

    #[test]
    fn stop_has_continue_response() {
        let adapter = CodexAdapter;
        let input = json!({"cwd": "/tmp", "last_assistant_message": "done"});
        let event = adapter.parse("stop", &input).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: CODEX_AGENT.into(),
                cwd: "/tmp".into(),
                permission_mode: "".into(),
                last_message: "done".into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
            }
        );
    }

    #[test]
    fn notification_not_supported() {
        assert!(CodexAdapter.parse("notification", &json!({})).is_none());
    }

    #[test]
    fn stop_failure_not_supported() {
        assert!(CodexAdapter.parse("stop-failure", &json!({})).is_none());
    }

    #[test]
    fn subagent_start_not_supported() {
        assert!(CodexAdapter.parse("subagent-start", &json!({})).is_none());
    }

    #[test]
    fn activity_log_not_supported() {
        assert!(CodexAdapter.parse("activity-log", &json!({})).is_none());
    }

    #[test]
    fn unknown_event_ignored() {
        assert!(CodexAdapter.parse("something-else", &json!({})).is_none());
    }

    #[test]
    fn stop_empty_fields() {
        let adapter = CodexAdapter;
        let event = adapter.parse("stop", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::Stop {
                agent: "codex".into(),
                cwd: "".into(),
                permission_mode: "".into(),
                last_message: "".into(),
                response: Some("{\"continue\":true}".into()),
                worktree: None,
                agent_id: None,
            }
        );
    }

    #[test]
    fn subagent_stop_not_supported() {
        assert!(CodexAdapter.parse("subagent-stop", &json!({})).is_none());
    }

    #[test]
    fn permission_denied_not_supported() {
        assert!(
            CodexAdapter
                .parse("permission-denied", &json!({}))
                .is_none()
        );
    }

    #[test]
    fn cwd_changed_not_supported() {
        assert!(CodexAdapter.parse("cwd-changed", &json!({})).is_none());
    }

    #[test]
    fn session_start_has_no_worktree() {
        let event = CodexAdapter
            .parse("session-start", &json!({"cwd": "/tmp"}))
            .unwrap();
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
    fn task_created_not_supported() {
        assert!(CodexAdapter.parse("task-created", &json!({})).is_none());
    }

    #[test]
    fn task_completed_not_supported() {
        assert!(CodexAdapter.parse("task-completed", &json!({})).is_none());
    }

    #[test]
    fn teammate_idle_not_supported() {
        assert!(CodexAdapter.parse("teammate-idle", &json!({})).is_none());
    }

    #[test]
    fn worktree_create_not_supported() {
        assert!(CodexAdapter.parse("worktree-create", &json!({})).is_none());
    }

    #[test]
    fn worktree_remove_not_supported() {
        assert!(CodexAdapter.parse("worktree-remove", &json!({})).is_none());
    }

    #[test]
    fn session_start_missing_fields_default_to_empty() {
        let adapter = CodexAdapter;
        let event = adapter.parse("session-start", &json!({})).unwrap();
        assert_eq!(
            event,
            AgentEvent::SessionStart {
                agent: "codex".into(),
                cwd: "".into(),
                permission_mode: "".into(),
                worktree: None,
                agent_id: None,
            }
        );
    }
}
