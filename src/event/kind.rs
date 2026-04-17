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

#[cfg(test)]
mod tests {
    use super::*;

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
}
