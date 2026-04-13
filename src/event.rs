use serde_json::Value;

use crate::adapter;
use crate::tmux::{CLAUDE_AGENT, CODEX_AGENT};

/// Worktree metadata from Claude Code hook payloads.
/// Present only when the agent is running in a worktree; `None` otherwise.
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
        session_id: Option<String>,
    },
    SessionEnd,
    UserPromptSubmit {
        agent: String,
        cwd: String,
        permission_mode: String,
        prompt: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
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
        session_id: Option<String>,
    },
    Stop {
        agent: String,
        cwd: String,
        permission_mode: String,
        last_message: String,
        response: Option<String>,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    StopFailure {
        agent: String,
        cwd: String,
        permission_mode: String,
        error: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    SubagentStart {
        agent_type: String,
        agent_id: Option<String>,
    },
    SubagentStop {
        agent_type: String,
        agent_id: Option<String>,
        last_message: String,
        transcript_path: String,
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
        session_id: Option<String>,
    },
    CwdChanged {
        cwd: String,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    TaskCreated {
        task_id: String,
        task_subject: String,
    },
    TaskCompleted {
        task_id: String,
        task_subject: String,
    },
    TeammateIdle {
        teammate_name: String,
        team_name: String,
    },
    WorktreeCreate,
    WorktreeRemove {
        worktree_path: String,
    },
}

/// Discriminant of `AgentEvent`. The single compile-time-enforced source of
/// truth for the mapping between internal events and their external
/// (string) names. `HookRegistration` tables and drift tests are keyed on
/// this enum — not on bare strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentEventKind {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    Notification,
    Stop,
    StopFailure,
    PermissionDenied,
    CwdChanged,
    SubagentStart,
    SubagentStop,
    ActivityLog,
    TaskCreated,
    TaskCompleted,
    TeammateIdle,
    WorktreeCreate,
    WorktreeRemove,
}

impl AgentEventKind {
    /// Every variant, in a stable order suitable for iteration. Adding a new
    /// variant without extending this list fails the
    /// `all_contains_every_variant` test below.
    pub const ALL: &'static [Self] = &[
        Self::SessionStart,
        Self::SessionEnd,
        Self::UserPromptSubmit,
        Self::Notification,
        Self::Stop,
        Self::StopFailure,
        Self::PermissionDenied,
        Self::CwdChanged,
        Self::SubagentStart,
        Self::SubagentStop,
        Self::ActivityLog,
        Self::TaskCreated,
        Self::TaskCompleted,
        Self::TeammateIdle,
        Self::WorktreeCreate,
        Self::WorktreeRemove,
    ];

    /// Normalized external event name passed to
    /// `tmux-agent-sidebar hook <agent> <event>`. Exhaustive match — adding
    /// a variant without assigning a name is a compile error.
    pub const fn external_name(self) -> &'static str {
        match self {
            Self::SessionStart => "session-start",
            Self::SessionEnd => "session-end",
            Self::UserPromptSubmit => "user-prompt-submit",
            Self::Notification => "notification",
            Self::Stop => "stop",
            Self::StopFailure => "stop-failure",
            Self::PermissionDenied => "permission-denied",
            Self::CwdChanged => "cwd-changed",
            Self::SubagentStart => "subagent-start",
            Self::SubagentStop => "subagent-stop",
            Self::ActivityLog => "activity-log",
            Self::TaskCreated => "task-created",
            Self::TaskCompleted => "task-completed",
            Self::TeammateIdle => "teammate-idle",
            Self::WorktreeCreate => "worktree-create",
            Self::WorktreeRemove => "worktree-remove",
        }
    }

    pub fn from_external_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|k| k.external_name() == name)
    }
}

impl AgentEvent {
    /// Project an `AgentEvent` down to its `AgentEventKind` discriminant.
    pub fn kind(&self) -> AgentEventKind {
        match self {
            Self::SessionStart { .. } => AgentEventKind::SessionStart,
            Self::SessionEnd => AgentEventKind::SessionEnd,
            Self::UserPromptSubmit { .. } => AgentEventKind::UserPromptSubmit,
            Self::Notification { .. } => AgentEventKind::Notification,
            Self::Stop { .. } => AgentEventKind::Stop,
            Self::StopFailure { .. } => AgentEventKind::StopFailure,
            Self::SubagentStart { .. } => AgentEventKind::SubagentStart,
            Self::SubagentStop { .. } => AgentEventKind::SubagentStop,
            Self::ActivityLog { .. } => AgentEventKind::ActivityLog,
            Self::PermissionDenied { .. } => AgentEventKind::PermissionDenied,
            Self::CwdChanged { .. } => AgentEventKind::CwdChanged,
            Self::TaskCreated { .. } => AgentEventKind::TaskCreated,
            Self::TaskCompleted { .. } => AgentEventKind::TaskCompleted,
            Self::TeammateIdle { .. } => AgentEventKind::TeammateIdle,
            Self::WorktreeCreate => AgentEventKind::WorktreeCreate,
            Self::WorktreeRemove { .. } => AgentEventKind::WorktreeRemove,
        }
    }
}

/// Adapter that converts external agent events into internal `AgentEvent`.
pub trait EventAdapter {
    fn parse(&self, event_name: &str, input: &Value) -> Option<AgentEvent>;
}

pub fn resolve_adapter(agent_name: &str) -> Option<Box<dyn EventAdapter>> {
    match agent_name {
        CLAUDE_AGENT => Some(Box::new(adapter::claude::ClaudeAdapter)),
        CODEX_AGENT => Some(Box::new(adapter::codex::CodexAdapter)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_contains_every_variant() {
        // This match is intentionally exhaustive: adding a new variant to
        // `AgentEventKind` fails compilation here until the new variant is
        // also added to `AgentEventKind::ALL` and the length assertion below.
        for kind in AgentEventKind::ALL {
            match kind {
                AgentEventKind::SessionStart
                | AgentEventKind::SessionEnd
                | AgentEventKind::UserPromptSubmit
                | AgentEventKind::Notification
                | AgentEventKind::Stop
                | AgentEventKind::StopFailure
                | AgentEventKind::PermissionDenied
                | AgentEventKind::CwdChanged
                | AgentEventKind::SubagentStart
                | AgentEventKind::SubagentStop
                | AgentEventKind::ActivityLog
                | AgentEventKind::TaskCreated
                | AgentEventKind::TaskCompleted
                | AgentEventKind::TeammateIdle
                | AgentEventKind::WorktreeCreate
                | AgentEventKind::WorktreeRemove => {}
            }
        }
        assert_eq!(AgentEventKind::ALL.len(), 16);
    }

    #[test]
    fn external_names_are_unique() {
        let mut names: Vec<&str> = AgentEventKind::ALL
            .iter()
            .map(|k| k.external_name())
            .collect();
        names.sort();
        let len_before = names.len();
        names.dedup();
        assert_eq!(names.len(), len_before, "duplicate external_name() values");
    }

    #[test]
    fn from_external_name_round_trip() {
        for kind in AgentEventKind::ALL {
            assert_eq!(
                AgentEventKind::from_external_name(kind.external_name()),
                Some(*kind)
            );
        }
        assert_eq!(AgentEventKind::from_external_name("not-a-real-event"), None);
    }

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
            session_id: None,
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
            session_id: None,
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
            Some(AgentEvent::SessionEnd),
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
        let input = json!({"teammate_name": "reviewer", "team_name": "dev"});
        let event = adapter.parse("teammate-idle", &input).unwrap();
        match event {
            AgentEvent::TeammateIdle {
                teammate_name,
                team_name,
            } => {
                assert_eq!(teammate_name, "reviewer");
                assert_eq!(team_name, "dev");
            }
            other => panic!("expected TeammateIdle, got {:?}", other),
        }
    }

    #[test]
    fn claude_worktree_create_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let event = adapter.parse("worktree-create", &json!({})).unwrap();
        assert_eq!(event, AgentEvent::WorktreeCreate);
    }

    #[test]
    fn claude_worktree_remove_round_trip() {
        let adapter = resolve_adapter("claude").unwrap();
        let input = json!({"worktree_path": "/tmp/wt-feat"});
        let event = adapter.parse("worktree-remove", &input).unwrap();
        match event {
            AgentEvent::WorktreeRemove { worktree_path } => {
                assert_eq!(worktree_path, "/tmp/wt-feat");
            }
            other => panic!("expected WorktreeRemove, got {:?}", other),
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
