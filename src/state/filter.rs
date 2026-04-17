#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusFilter {
    All,
    Running,
    Waiting,
    Idle,
    Error,
}

impl StatusFilter {
    pub const VARIANTS: [StatusFilter; 5] = [
        StatusFilter::All,
        StatusFilter::Running,
        StatusFilter::Waiting,
        StatusFilter::Idle,
        StatusFilter::Error,
    ];

    pub fn next(self) -> Self {
        let idx = StatusFilter::VARIANTS
            .iter()
            .position(|v| *v == self)
            .unwrap_or(0);
        StatusFilter::VARIANTS[(idx + 1) % StatusFilter::VARIANTS.len()]
    }

    pub fn prev(self) -> Self {
        let idx = StatusFilter::VARIANTS
            .iter()
            .position(|v| *v == self)
            .unwrap_or(0);
        StatusFilter::VARIANTS
            [(idx + StatusFilter::VARIANTS.len() - 1) % StatusFilter::VARIANTS.len()]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Idle => "idle",
            Self::Error => "error",
        }
    }

    /// Parse a tmux-option label into a `StatusFilter`. Unknown values
    /// fall back to `All`.
    pub fn from_label(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "waiting" => Self::Waiting,
            "idle" => Self::Idle,
            "error" => Self::Error,
            _ => Self::All,
        }
    }

    pub fn matches(self, status: &crate::tmux::PaneStatus) -> bool {
        match self {
            StatusFilter::All => true,
            StatusFilter::Running => *status == crate::tmux::PaneStatus::Running,
            StatusFilter::Waiting => *status == crate::tmux::PaneStatus::Waiting,
            StatusFilter::Idle => *status == crate::tmux::PaneStatus::Idle,
            StatusFilter::Error => *status == crate::tmux::PaneStatus::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RepoFilter {
    All,
    Repo(String),
}

impl RepoFilter {
    pub fn as_str(&self) -> &str {
        match self {
            Self::All => "all",
            Self::Repo(name) => name.as_str(),
        }
    }

    /// Parse a tmux-option label into a `RepoFilter`. `""` and `"all"`
    /// map to `All`; any other value is stored as `Repo(name)`.
    pub fn from_label(s: &str) -> Self {
        match s {
            "all" | "" => Self::All,
            name => Self::Repo(name.to_string()),
        }
    }

    pub fn matches_group(&self, group_name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Repo(name) => name == group_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::PaneStatus;

    // ─── StatusFilter tests ───────────────────────────────────────────

    #[test]
    fn status_filter_next_cycles() {
        assert_eq!(StatusFilter::All.next(), StatusFilter::Running);
        assert_eq!(StatusFilter::Running.next(), StatusFilter::Waiting);
        assert_eq!(StatusFilter::Waiting.next(), StatusFilter::Idle);
        assert_eq!(StatusFilter::Idle.next(), StatusFilter::Error);
        assert_eq!(StatusFilter::Error.next(), StatusFilter::All);
    }

    #[test]
    fn status_filter_prev_cycles() {
        assert_eq!(StatusFilter::All.prev(), StatusFilter::Error);
        assert_eq!(StatusFilter::Error.prev(), StatusFilter::Idle);
        assert_eq!(StatusFilter::Idle.prev(), StatusFilter::Waiting);
        assert_eq!(StatusFilter::Waiting.prev(), StatusFilter::Running);
        assert_eq!(StatusFilter::Running.prev(), StatusFilter::All);
    }

    #[test]
    fn status_filter_matches_status() {
        assert!(StatusFilter::All.matches(&PaneStatus::Running));
        assert!(StatusFilter::All.matches(&PaneStatus::Idle));
        assert!(StatusFilter::All.matches(&PaneStatus::Waiting));
        assert!(StatusFilter::All.matches(&PaneStatus::Error));

        assert!(StatusFilter::Running.matches(&PaneStatus::Running));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Idle));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Waiting));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Error));

        assert!(StatusFilter::Waiting.matches(&PaneStatus::Waiting));
        assert!(!StatusFilter::Waiting.matches(&PaneStatus::Running));

        assert!(StatusFilter::Idle.matches(&PaneStatus::Idle));
        assert!(!StatusFilter::Idle.matches(&PaneStatus::Running));

        assert!(StatusFilter::Error.matches(&PaneStatus::Error));
        assert!(!StatusFilter::Error.matches(&PaneStatus::Idle));
    }

    // ─── StatusFilter as_str / from_str tests ─────────────────────────

    #[test]
    fn status_filter_as_str_all_variants() {
        assert_eq!(StatusFilter::All.as_str(), "all");
        assert_eq!(StatusFilter::Running.as_str(), "running");
        assert_eq!(StatusFilter::Waiting.as_str(), "waiting");
        assert_eq!(StatusFilter::Idle.as_str(), "idle");
        assert_eq!(StatusFilter::Error.as_str(), "error");
    }

    #[test]
    fn status_filter_from_str_all_variants() {
        assert_eq!(StatusFilter::from_label("all"), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("running"), StatusFilter::Running);
        assert_eq!(StatusFilter::from_label("waiting"), StatusFilter::Waiting);
        assert_eq!(StatusFilter::from_label("idle"), StatusFilter::Idle);
        assert_eq!(StatusFilter::from_label("error"), StatusFilter::Error);
    }

    #[test]
    fn status_filter_from_str_unknown_defaults_to_all() {
        assert_eq!(StatusFilter::from_label(""), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("unknown"), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("Running"), StatusFilter::All); // case-sensitive
    }

    #[test]
    fn status_filter_roundtrip() {
        for filter in StatusFilter::VARIANTS {
            assert_eq!(StatusFilter::from_label(filter.as_str()), filter);
        }
    }

    // ─── RepoFilter tests ─────────────────────────────────────

    #[test]
    fn repo_filter_persistence_roundtrip() {
        assert_eq!(RepoFilter::from_label("all"), RepoFilter::All);
        assert_eq!(RepoFilter::from_label(""), RepoFilter::All);
        assert_eq!(
            RepoFilter::from_label("my-app"),
            RepoFilter::Repo("my-app".into())
        );
        assert_eq!(RepoFilter::All.as_str(), "all");
        assert_eq!(RepoFilter::Repo("my-app".into()).as_str(), "my-app");
    }

    #[test]
    fn repo_filter_matches_group() {
        assert!(RepoFilter::All.matches_group("anything"));
        assert!(RepoFilter::Repo("app".into()).matches_group("app"));
        assert!(!RepoFilter::Repo("app".into()).matches_group("other"));
    }
}
