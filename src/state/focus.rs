use super::AppState;
use crate::tmux;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Focus {
    Filter,
    #[default]
    Panes,
    ActivityLog,
}

#[derive(Debug, Clone)]
pub struct FocusState {
    pub sidebar_focused: bool,
    pub focus: Focus,
    pub focused_pane_id: Option<String>,
    pub prev_focused_pane_id: Option<String>,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            sidebar_focused: false,
            focus: Focus::Panes,
            focused_pane_id: None,
            prev_focused_pane_id: None,
        }
    }
}

impl Default for FocusState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn pane_by_id(&self, pane_id: &str) -> Option<&tmux::PaneInfo> {
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                if pane.pane_id == pane_id {
                    return Some(pane);
                }
            }
        }
        None
    }

    pub fn selected_pane(&self) -> Option<&tmux::PaneInfo> {
        let target = self
            .layout
            .pane_row_targets
            .get(self.global.selected_pane_row)?;
        self.pane_by_id(&target.pane_id)
    }

    pub fn find_focused_pane(&mut self) {
        // Query tmux directly for the active pane, not through `repo_groups`
        // which only contains agent panes. This allows activity/git info to
        // be displayed even when the focused pane has no agent running.
        // When the sidebar has focus, find_active_pane returns None — preserve
        // the previously focused pane so bottom panel data stays stable.
        if let Some((id, _)) = tmux::find_active_pane(&self.tmux_pane) {
            self.focus_state.focused_pane_id = Some(id);
        }
    }

    /// Move agent selection. Returns true if moved, false if at boundary.
    pub fn move_pane_selection(&mut self, delta: isize) -> bool {
        if self.layout.pane_row_targets.is_empty() {
            return false;
        }
        let len = self.layout.pane_row_targets.len() as isize;
        let next = self.global.selected_pane_row as isize + delta;
        if next >= 0 && next < len {
            self.global.selected_pane_row = next as usize;
            true
        } else {
            false
        }
    }

    pub fn activate_selected_pane(&mut self) {
        if let Some(target_pane_id) = self
            .layout
            .pane_row_targets
            .get(self.global.selected_pane_row)
            .map(|target| target.pane_id.clone())
        {
            // Update the sidebar immediately so the active marker and
            // repo header highlight move without waiting for the next
            // periodic tmux refresh.
            self.focus_state.focused_pane_id = Some(target_pane_id.clone());
            tmux::select_pane(&target_pane_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_default_is_panes() {
        assert_eq!(Focus::default(), Focus::Panes);
    }

    #[test]
    fn focus_state_new_has_expected_initial_values() {
        let state = FocusState::new();
        assert!(!state.sidebar_focused);
        assert_eq!(state.focus, Focus::Panes);
        assert!(state.focused_pane_id.is_none());
        assert!(state.prev_focused_pane_id.is_none());
    }

    #[test]
    fn focus_state_default_delegates_to_new() {
        let state = FocusState::default();
        assert!(!state.sidebar_focused);
        assert_eq!(state.focus, Focus::Panes);
        assert!(state.focused_pane_id.is_none());
        assert!(state.prev_focused_pane_id.is_none());
    }

    // ─── AppState focus accessors ────────────────────────────────────

    use crate::group::{PaneGitInfo, RepoGroup};
    use crate::state::layout::RowTarget;
    use crate::tmux::{AgentType, PaneInfo, PaneStatus, PermissionMode, WorktreeMetadata};

    fn test_pane(id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status: PaneStatus::Running,
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
        }
    }

    #[test]
    fn pane_by_id_searches_all_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![(test_pane("%42"), PaneGitInfo::default())],
            },
        ];

        assert!(state.pane_by_id("%1").is_some());
        assert!(state.pane_by_id("%42").is_some());
        assert!(state.pane_by_id("%does-not-exist").is_none());
    }

    #[test]
    fn selected_pane_tracks_pane_row_targets() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "alpha".into(),
            has_focus: true,
            panes: vec![
                (test_pane("%1"), PaneGitInfo::default()),
                (test_pane("%2"), PaneGitInfo::default()),
            ],
        }];
        state.layout.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];

        state.global.selected_pane_row = 0;
        assert_eq!(
            state.selected_pane().map(|p| p.pane_id.as_str()),
            Some("%1")
        );
        state.global.selected_pane_row = 1;
        assert_eq!(
            state.selected_pane().map(|p| p.pane_id.as_str()),
            Some("%2")
        );
        // Out-of-range cursor → None (not a panic).
        state.global.selected_pane_row = 99;
        assert!(state.selected_pane().is_none());
    }

    #[test]
    fn move_pane_selection_stays_within_bounds() {
        let mut state = AppState::new("%99".into());
        state.layout.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        state.global.selected_pane_row = 0;
        assert!(!state.move_pane_selection(-1));
        assert!(state.move_pane_selection(1));
        assert_eq!(state.global.selected_pane_row, 1);
        assert!(!state.move_pane_selection(5));
    }
}
