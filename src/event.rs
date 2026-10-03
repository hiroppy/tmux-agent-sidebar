mod adapter;
mod kind;

pub use adapter::{EventAdapter, resolve_adapter};
pub use kind::AgentEventKind;

use serde_json::Value;

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
        source: String,
        /// Whether the adapter guarantees this belongs to the pane's host
        /// session rather than a child sharing the same `$TMUX_PANE`.
        top_level: bool,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    SessionEnd {
        agent: String,
        session_id: Option<String>,
        /// Whether teardown must match the pane's currently tracked host session.
        requires_existing_session: bool,
        end_reason: String,
        /// Whether the adapter guarantees this belongs to the pane's host
        /// session rather than a child sharing the same `$TMUX_PANE`.
        top_level: bool,
    },
    UserPromptSubmit {
        agent: String,
        cwd: String,
        permission_mode: String,
        prompt: String,
        /// Adapter-normalized classification for harness-injected prompt text.
        prompt_is_system_message: bool,
        /// Whether this prompt must match an already-tracked host session.
        requires_existing_session: bool,
        prompt_id: Option<String>,
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
        /// Whether this event must match an already-tracked pane session
        /// before it may apply metadata or attention state.
        requires_existing_session: bool,
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
        prompt_id: Option<String>,
        /// Whether this stop must match the pane's tracked host session.
        requires_existing_session: bool,
        /// Whether tracked children may continue after this parent turn ends.
        children_may_outlive_turn: bool,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    /// A turn ended without a successful completion notification. Used for
    /// cancellation and idle-backstop events that should only settle status.
    TurnSettled {
        agent: String,
        cwd: String,
        permission_mode: String,
        prompt_id: Option<String>,
        /// Whether settlement must match the pane's tracked host session.
        requires_existing_session: bool,
        /// Whether tracked children may continue after this parent turn ends.
        children_may_outlive_turn: bool,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    StopFailure {
        agent: String,
        cwd: String,
        permission_mode: String,
        error: String,
        prompt_id: Option<String>,
        /// Whether this failure must match the pane's tracked host session.
        requires_existing_session: bool,
        worktree: Option<WorktreeInfo>,
        agent_id: Option<String>,
        session_id: Option<String>,
    },
    SubagentStart {
        agent: String,
        session_id: Option<String>,
        /// Whether the child may be registered only while its host is tracked.
        requires_existing_session: bool,
        agent_type: String,
        agent_id: Option<String>,
        /// Optional human-readable identity supplied separately from the type.
        display_name: Option<String>,
        /// Whether a child registering after the parent turn already settled
        /// revives the pane's background lifecycle. False for agents whose
        /// children die with the turn — there a late start is stale.
        children_may_outlive_turn: bool,
    },
    SubagentStop {
        agent_type: String,
        agent_id: Option<String>,
        last_message: String,
        transcript_path: String,
        /// Whether this child may be the final work keeping a settled parent active.
        children_may_outlive_turn: bool,
    },
    ActivityLog {
        agent: String,
        session_id: Option<String>,
        /// Whether this event must belong to the tracked host or one of its
        /// currently tracked child sessions before activity may be recorded.
        requires_existing_session: bool,
        tool_name: String,
        tool_input: Value,
        tool_response: Value,
    },
    PermissionDenied {
        agent: String,
        cwd: String,
        permission_mode: String,
        /// Whether this event must belong to the tracked host or one of its
        /// currently tracked child sessions before attention may be raised.
        requires_existing_session: bool,
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
        idle_reason: String,
    },
    WorktreeCreate,
    WorktreeRemove {
        worktree_path: String,
    },
}

impl AgentEvent {
    /// Project an `AgentEvent` down to its `AgentEventKind` discriminant.
    pub fn kind(&self) -> AgentEventKind {
        match self {
            Self::SessionStart { .. } => AgentEventKind::SessionStart,
            Self::SessionEnd { .. } => AgentEventKind::SessionEnd,
            Self::UserPromptSubmit { .. } => AgentEventKind::UserPromptSubmit,
            Self::Notification { .. } => AgentEventKind::Notification,
            Self::Stop { .. } => AgentEventKind::Stop,
            Self::TurnSettled { .. } => AgentEventKind::TurnSettled,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_info_default_is_none() {
        let event = AgentEvent::SessionStart {
            agent: "claude".into(),
            cwd: "/tmp".into(),
            permission_mode: "default".into(),
            source: String::new(),
            top_level: false,
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
            source: String::new(),
            top_level: false,
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
}
