use super::AppState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusFilter {
    All,
    Running,
    Background,
    Waiting,
    Idle,
    Error,
}

impl StatusFilter {
    pub const VARIANTS: [StatusFilter; 6] = [
        StatusFilter::All,
        StatusFilter::Running,
        StatusFilter::Background,
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
            Self::Background => "background",
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
            "background" => Self::Background,
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
            StatusFilter::Background => *status == crate::tmux::PaneStatus::Background,
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

impl AppState {
    /// Count agents per status across all repo groups.
    pub fn status_counts(&self) -> (usize, usize, usize, usize, usize, usize) {
        let (mut running, mut background, mut waiting, mut idle, mut error) = (0, 0, 0, 0, 0);
        for group in &self.repo_groups {
            if !self.global.repo_filter.matches_group(&group.name) {
                continue;
            }
            for (pane, _) in &group.panes {
                match pane.status {
                    crate::tmux::PaneStatus::Running => running += 1,
                    crate::tmux::PaneStatus::Background => background += 1,
                    crate::tmux::PaneStatus::Waiting => waiting += 1,
                    crate::tmux::PaneStatus::Idle => idle += 1,
                    crate::tmux::PaneStatus::Error => error += 1,
                    crate::tmux::PaneStatus::Unknown => {}
                }
            }
        }
        let all = running + background + waiting + idle + error;
        (all, running, background, waiting, idle, error)
    }

    /// Return list of repo names for the popup: ["All", repo1, repo2, ...]
    pub fn repo_names(&self) -> Vec<String> {
        let mut names = vec!["All".to_string()];
        for group in &self.repo_groups {
            names.push(group.name.clone());
        }
        names
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
        assert_eq!(StatusFilter::Running.next(), StatusFilter::Background);
        assert_eq!(StatusFilter::Background.next(), StatusFilter::Waiting);
        assert_eq!(StatusFilter::Waiting.next(), StatusFilter::Idle);
        assert_eq!(StatusFilter::Idle.next(), StatusFilter::Error);
        assert_eq!(StatusFilter::Error.next(), StatusFilter::All);
    }

    #[test]
    fn status_filter_prev_cycles() {
        assert_eq!(StatusFilter::All.prev(), StatusFilter::Error);
        assert_eq!(StatusFilter::Error.prev(), StatusFilter::Idle);
        assert_eq!(StatusFilter::Idle.prev(), StatusFilter::Waiting);
        assert_eq!(StatusFilter::Waiting.prev(), StatusFilter::Background);
        assert_eq!(StatusFilter::Background.prev(), StatusFilter::Running);
        assert_eq!(StatusFilter::Running.prev(), StatusFilter::All);
    }

    #[test]
    fn status_filter_matches_status() {
        assert!(StatusFilter::All.matches(&PaneStatus::Running));
        assert!(StatusFilter::All.matches(&PaneStatus::Background));
        assert!(StatusFilter::All.matches(&PaneStatus::Idle));
        assert!(StatusFilter::All.matches(&PaneStatus::Waiting));
        assert!(StatusFilter::All.matches(&PaneStatus::Error));

        assert!(StatusFilter::Running.matches(&PaneStatus::Running));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Background));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Idle));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Waiting));
        assert!(!StatusFilter::Running.matches(&PaneStatus::Error));

        assert!(StatusFilter::Background.matches(&PaneStatus::Background));
        assert!(!StatusFilter::Background.matches(&PaneStatus::Running));

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
        assert_eq!(StatusFilter::Background.as_str(), "background");
        assert_eq!(StatusFilter::Waiting.as_str(), "waiting");
        assert_eq!(StatusFilter::Idle.as_str(), "idle");
        assert_eq!(StatusFilter::Error.as_str(), "error");
    }

    #[test]
    fn status_filter_from_str_all_variants() {
        assert_eq!(StatusFilter::from_label("all"), StatusFilter::All);
        assert_eq!(StatusFilter::from_label("running"), StatusFilter::Running);
        assert_eq!(
            StatusFilter::from_label("background"),
            StatusFilter::Background
        );
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

    // ─── AppState status_counts / repo_names ─────────────────────────

    use crate::group::{PaneGitInfo, RepoGroup};
    use crate::tmux::{AgentType, PaneInfo, PermissionMode, WorktreeMetadata};

    fn test_pane(id: &str, status: PaneStatus) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status,
            attention: false,
            agent: AgentType::Claude,
            path: "/tmp".into(),
            current_command: String::new(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree: WorktreeMetadata::default(),
            session_id: None,
            session_name: String::new(),
            sidebar_spawned: false,
            bg_shell_cmd: None,
        }
    }

    #[test]
    fn status_counts_on_empty_state_is_all_zeroes() {
        let state = AppState::new("%99".into());
        assert_eq!(state.status_counts(), (0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn status_counts_sums_across_repo_groups_and_filters() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "app".into(),
                has_focus: true,
                panes: vec![
                    (test_pane("%1", PaneStatus::Running), PaneGitInfo::default()),
                    (test_pane("%2", PaneStatus::Idle), PaneGitInfo::default()),
                    (
                        test_pane("%4", PaneStatus::Background),
                        PaneGitInfo::default(),
                    ),
                ],
            },
            RepoGroup {
                name: "lib".into(),
                has_focus: false,
                panes: vec![(test_pane("%3", PaneStatus::Waiting), PaneGitInfo::default())],
            },
        ];

        // All repos: 4 total
        let (all, r, b, w, i, e) = state.status_counts();
        assert_eq!((all, r, b, w, i, e), (4, 1, 1, 1, 1, 0));

        // Restrict to "app"
        state.global.repo_filter = RepoFilter::Repo("app".into());
        let (all, r, b, w, i, e) = state.status_counts();
        assert_eq!((all, r, b, w, i, e), (3, 1, 1, 0, 1, 0));
    }

    #[test]
    fn repo_names_leads_with_all_sentinel() {
        let mut state = AppState::new("%99".into());
        assert_eq!(state.repo_names(), vec!["All"]);
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![],
            },
        ];
        assert_eq!(state.repo_names(), vec!["All", "alpha", "beta"]);
    }
}
