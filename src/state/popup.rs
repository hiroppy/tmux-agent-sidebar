use super::AppState;

/// Focus target inside the spawn input popup. Tab / Shift+Tab / arrow
/// keys cycle through these in order; only `Task` accepts text input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpawnField {
    #[default]
    Task,
    Agent,
    Mode,
}

impl SpawnField {
    pub fn next(self) -> Self {
        match self {
            Self::Task => Self::Agent,
            Self::Agent => Self::Mode,
            Self::Mode => Self::Task,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Task => Self::Mode,
            Self::Agent => Self::Task,
            Self::Mode => Self::Agent,
        }
    }
}

/// At-most-one popup state for the sidebar. The enum variant encodes
/// both which popup is open and its per-popup data, so the "only one
/// popup open at a time" invariant is checked by the type system.
#[derive(Debug, Clone, Default)]
pub enum PopupState {
    #[default]
    None,
    Repo {
        selected: usize,
        area: Option<ratatui::layout::Rect>,
    },
    Notices {
        area: Option<ratatui::layout::Rect>,
    },
    /// Modal text input shown when the user presses `n` (or clicks `+`)
    /// to spawn a new worktree. `target_repo` / `target_repo_root` pin
    /// the spawn target; `agent_idx` / `mode_idx` index into
    /// [`crate::worktree::AGENTS`] / [`crate::worktree::modes_for`] so
    /// arrow keys can cycle the user's agent and permission-mode picks.
    SpawnInput {
        input: String,
        target_repo: String,
        target_repo_root: String,
        agent_idx: usize,
        mode_idx: usize,
        field: SpawnField,
        /// Screen Y of the repo header row that owns the `+` button
        /// this modal was opened from. Renderer anchors the popup just
        /// below it; `None` falls back to a centered layout.
        anchor_y: Option<u16>,
        /// Inline error message rendered at the bottom of the popup
        /// so spawn failures stay visually attached to the input the
        /// user was editing. Cleared on the next edit / field change.
        error: Option<String>,
        area: Option<ratatui::layout::Rect>,
    },
    /// Confirmation prompt shown when the user presses `x` on a
    /// spawn-created pane. `pane_id` feeds `worktree::remove`; `branch`
    /// is shown in the modal title.
    RemoveConfirm {
        pane_id: String,
        branch: String,
        error: Option<String>,
        area: Option<ratatui::layout::Rect>,
    },
}

impl PopupState {
    pub fn set_repo_area(&mut self, rect: Option<ratatui::layout::Rect>) {
        if let Self::Repo { area, .. } = self {
            *area = rect;
        }
    }

    pub fn set_notices_area(&mut self, rect: Option<ratatui::layout::Rect>) {
        if let Self::Notices { area } = self {
            *area = rect;
        }
    }

    pub fn set_spawn_input_area(&mut self, rect: Option<ratatui::layout::Rect>) {
        if let Self::SpawnInput { area, .. } = self {
            *area = rect;
        }
    }

    pub fn set_remove_confirm_area(&mut self, rect: Option<ratatui::layout::Rect>) {
        if let Self::RemoveConfirm { area, .. } = self {
            *area = rect;
        }
    }
}

impl AppState {
    // ─── Repo popup ──────────────────────────────────────────────────────

    pub fn is_repo_popup_open(&self) -> bool {
        matches!(self.popup, PopupState::Repo { .. })
    }

    pub fn repo_popup_selected(&self) -> usize {
        match &self.popup {
            PopupState::Repo { selected, .. } => *selected,
            _ => 0,
        }
    }

    pub fn set_repo_popup_selected(&mut self, n: usize) {
        if let PopupState::Repo { selected, .. } = &mut self.popup {
            *selected = n;
        }
    }

    pub fn repo_popup_area(&self) -> Option<ratatui::layout::Rect> {
        match &self.popup {
            PopupState::Repo { area, .. } => *area,
            _ => None,
        }
    }

    pub fn toggle_repo_popup(&mut self) {
        if self.is_repo_popup_open() {
            self.close_repo_popup();
            return;
        }
        // Set selected to current filter position
        let names = self.repo_names();
        let selected = match &self.global.repo_filter {
            super::RepoFilter::All => 0,
            super::RepoFilter::Repo(name) => names.iter().position(|n| n == name).unwrap_or(0),
        };
        self.popup = PopupState::Repo {
            selected,
            area: None,
        };
    }

    pub fn confirm_repo_popup(&mut self) {
        let selected = self.repo_popup_selected();
        let names = self.repo_names();
        if let Some(name) = names.get(selected) {
            self.global.repo_filter = if selected == 0 {
                super::RepoFilter::All
            } else {
                super::RepoFilter::Repo(name.clone())
            };
        }
        self.popup = PopupState::None;
        self.global.save_repo_filter();
        self.rebuild_row_targets();
    }

    pub fn close_repo_popup(&mut self) {
        self.popup = PopupState::None;
    }

    // ─── Notices popup ───────────────────────────────────────────────────

    pub fn is_notices_popup_open(&self) -> bool {
        matches!(self.popup, PopupState::Notices { .. })
    }

    pub fn notices_popup_area(&self) -> Option<ratatui::layout::Rect> {
        match &self.popup {
            PopupState::Notices { area } => *area,
            _ => None,
        }
    }

    pub fn toggle_notices_popup(&mut self) {
        if self.is_notices_popup_open() {
            self.close_notices_popup();
        } else {
            self.popup = PopupState::Notices { area: None };
        }
    }

    pub fn close_notices_popup(&mut self) {
        self.popup = PopupState::None;
        self.notices.copy_targets.clear();
        self.notices.copied_at = None;
    }

    // ─── Spawn input popup (n key / + click) ─────────────────────────────

    pub fn is_spawn_input_open(&self) -> bool {
        matches!(self.popup, PopupState::SpawnInput { .. })
    }

    pub fn spawn_input_popup_area(&self) -> Option<ratatui::layout::Rect> {
        match &self.popup {
            PopupState::SpawnInput { area, .. } => *area,
            _ => None,
        }
    }

    pub fn open_spawn_input_for_repo(
        &mut self,
        repo_name: String,
        repo_root: String,
        anchor_y: Option<u16>,
    ) {
        self.popup = PopupState::SpawnInput {
            input: String::new(),
            target_repo: repo_name,
            target_repo_root: repo_root,
            agent_idx: 0,
            mode_idx: 0,
            field: SpawnField::Task,
            anchor_y,
            error: None,
            area: None,
        };
    }

    pub fn open_spawn_input_from_selection(&mut self) {
        let Some(pane) = self.selected_pane() else {
            self.set_flash("spawn: no pane selected");
            return;
        };
        let pane_id = pane.pane_id.clone();
        let Some(group) = self
            .repo_groups
            .iter()
            .find(|g| g.panes.iter().any(|(p, _)| p.pane_id == pane_id))
        else {
            self.set_flash("spawn: could not find repo group for selection");
            return;
        };
        let Some(root) = group
            .panes
            .iter()
            .find_map(|(_, git)| git.repo_root.clone())
        else {
            self.set_flash("spawn: selected pane is not in a git repo");
            return;
        };
        let name = group.name.clone();
        // Anchor the popup directly below the repo header row so it
        // matches what the mouse `+` click flow does.
        let anchor = self
            .layout
            .repo_spawn_targets
            .iter()
            .find(|t| t.repo_name == name)
            .map(|t| t.rect.y);
        self.open_spawn_input_for_repo(name, root, anchor);
    }

    pub fn close_spawn_input(&mut self) {
        if matches!(self.popup, PopupState::SpawnInput { .. }) {
            self.popup = PopupState::None;
        }
    }

    pub fn spawn_input_next_field(&mut self) {
        if let PopupState::SpawnInput { field, error, .. } = &mut self.popup {
            *field = field.next();
            *error = None;
        }
    }

    pub fn spawn_input_prev_field(&mut self) {
        if let PopupState::SpawnInput { field, error, .. } = &mut self.popup {
            *field = field.prev();
            *error = None;
        }
    }

    /// Cycle the value under the focused agent or mode field. No-op on
    /// the task input field so typing isn't interfered with.
    pub fn spawn_input_cycle(&mut self, delta: isize) {
        let PopupState::SpawnInput {
            field,
            agent_idx,
            mode_idx,
            error,
            ..
        } = &mut self.popup
        else {
            return;
        };
        match *field {
            SpawnField::Agent => {
                let len = crate::worktree::AGENTS.len() as isize;
                *agent_idx = ((*agent_idx as isize + delta).rem_euclid(len)) as usize;
                // Mode list is agent-specific.
                *mode_idx = 0;
                *error = None;
            }
            SpawnField::Mode => {
                let agent = crate::worktree::AGENTS
                    .get(*agent_idx)
                    .copied()
                    .unwrap_or("");
                let len = crate::worktree::modes_for(agent).len() as isize;
                if len > 0 {
                    *mode_idx = ((*mode_idx as isize + delta).rem_euclid(len)) as usize;
                    *error = None;
                }
            }
            SpawnField::Task => {}
        }
    }

    pub fn spawn_input_push_char(&mut self, c: char) {
        if let PopupState::SpawnInput {
            input,
            field,
            error,
            ..
        } = &mut self.popup
            && *field == SpawnField::Task
        {
            input.push(c);
            *error = None;
        }
    }

    pub fn spawn_input_pop_char(&mut self) {
        if let PopupState::SpawnInput {
            input,
            field,
            error,
            ..
        } = &mut self.popup
            && *field == SpawnField::Task
        {
            input.pop();
            *error = None;
        }
    }

    fn set_spawn_error(&mut self, msg: impl Into<String>) {
        if let PopupState::SpawnInput { error, .. } = &mut self.popup {
            *error = Some(msg.into());
        }
    }

    fn set_remove_error(&mut self, msg: impl Into<String>) {
        if let PopupState::RemoveConfirm { error, .. } = &mut self.popup {
            *error = Some(msg.into());
        }
    }

    /// Run the spawn flow against the repo stored in the popup, using
    /// the agent / mode the user picked. On success the popup closes
    /// silently (the new window appearing in the sidebar is the
    /// feedback). On failure the error is surfaced inside the popup
    /// and the modal stays open so the user can retry.
    pub fn confirm_spawn_input(&mut self) {
        let PopupState::SpawnInput {
            input,
            target_repo_root,
            agent_idx,
            mode_idx,
            ..
        } = &self.popup
        else {
            return;
        };
        let task_name = input.trim().to_string();
        if task_name.is_empty() {
            self.set_spawn_error("name is empty");
            return;
        }
        let agent = crate::worktree::AGENTS
            .get(*agent_idx)
            .copied()
            .unwrap_or(crate::worktree::DEFAULT_AGENT)
            .to_string();
        let mode = crate::worktree::modes_for(&agent)
            .get(*mode_idx)
            .copied()
            .unwrap_or(crate::worktree::DEFAULT_MODE)
            .to_string();
        let repo_root = std::path::PathBuf::from(target_repo_root.clone());

        let Some(session) = crate::tmux::pane_session_name(&self.tmux_pane) else {
            self.set_spawn_error("could not resolve tmux session");
            return;
        };

        let req = crate::worktree::SpawnRequest {
            repo_root,
            task_name,
            session,
            agent,
            mode,
        };
        match crate::worktree::spawn(&req) {
            Ok(_) => self.popup = PopupState::None,
            Err(e) => self.set_spawn_error(e),
        }
    }

    // ─── Remove confirm popup (x key) ────────────────────────────────────

    pub fn is_remove_confirm_open(&self) -> bool {
        matches!(self.popup, PopupState::RemoveConfirm { .. })
    }

    pub fn remove_confirm_popup_area(&self) -> Option<ratatui::layout::Rect> {
        match &self.popup {
            PopupState::RemoveConfirm { area, .. } => *area,
            _ => None,
        }
    }

    pub fn close_remove_confirm(&mut self) {
        if matches!(self.popup, PopupState::RemoveConfirm { .. }) {
            self.popup = PopupState::None;
        }
    }

    /// Open the remove confirmation popup for the currently selected pane,
    /// but only if it was created by the sidebar's spawn flow. Otherwise
    /// flashes an error so the user knows nothing happened.
    pub fn open_remove_confirm(&mut self) {
        let Some(pane) = self.selected_pane() else {
            self.set_flash("remove: no pane selected");
            return;
        };
        self.open_remove_confirm_for_pane(pane.pane_id.clone());
    }

    pub fn open_remove_confirm_for_pane(&mut self, pane_id: String) {
        let markers = crate::worktree::read_spawn_markers(&pane_id);
        if !markers.is_spawned() {
            self.set_flash("remove: selected pane was not spawned by sidebar");
            return;
        }
        let branch = std::path::Path::new(&markers.worktree_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.popup = PopupState::RemoveConfirm {
            pane_id,
            branch,
            error: None,
            area: None,
        };
    }

    /// Run the remove flow on the pane stored in the confirmation popup.
    /// Success silently closes the popup; failures are surfaced inside
    /// the popup so the user can retry.
    pub fn confirm_remove(&mut self, mode: crate::worktree::RemoveMode) {
        let pane_id = match &self.popup {
            PopupState::RemoveConfirm { pane_id, .. } => pane_id.clone(),
            _ => return,
        };
        match crate::worktree::remove(&pane_id, mode) {
            Ok(_) => self.popup = PopupState::None,
            Err(e) => self.set_remove_error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{NoticesCopyTarget, RepoFilter};
    use super::*;
    use crate::group::{PaneGitInfo, RepoGroup};
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

    // ─── SpawnField cycle ────────────────────────────────────────────

    #[test]
    fn spawn_field_next_and_prev_cycle() {
        assert_eq!(SpawnField::Task.next(), SpawnField::Agent);
        assert_eq!(SpawnField::Agent.next(), SpawnField::Mode);
        assert_eq!(SpawnField::Mode.next(), SpawnField::Task);
        assert_eq!(SpawnField::Task.prev(), SpawnField::Mode);
        assert_eq!(SpawnField::Agent.prev(), SpawnField::Task);
        assert_eq!(SpawnField::Mode.prev(), SpawnField::Agent);
    }

    // ─── PopupState::set_*_area ──────────────────────────────────────

    #[test]
    fn set_area_updates_only_matching_variant() {
        let mut popup = PopupState::Repo {
            selected: 0,
            area: None,
        };
        let rect = ratatui::layout::Rect::new(1, 2, 3, 4);
        popup.set_repo_area(Some(rect));
        popup.set_notices_area(Some(rect));
        popup.set_spawn_input_area(Some(rect));
        popup.set_remove_confirm_area(Some(rect));
        match popup {
            PopupState::Repo { area, .. } => assert_eq!(area, Some(rect)),
            _ => panic!("variant must remain Repo"),
        }
    }

    // ─── Repo popup ──────────────────────────────────────────────────

    #[test]
    fn toggle_repo_popup_sets_selected_to_current() {
        let mut state = AppState::new("%99".into());
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

        state.toggle_repo_popup();
        assert!(state.is_repo_popup_open());
        assert_eq!(state.repo_popup_selected(), 0);

        state.close_repo_popup();
        state.global.repo_filter = RepoFilter::Repo("beta".into());
        state.toggle_repo_popup();
        assert_eq!(state.repo_popup_selected(), 2);
    }

    #[test]
    fn confirm_repo_popup_sets_filter() {
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
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.popup = PopupState::Repo {
            selected: 2,
            area: None,
        };
        state.confirm_repo_popup();

        assert_eq!(state.global.repo_filter, RepoFilter::Repo("beta".into()));
        assert!(!state.is_repo_popup_open());
        assert_eq!(state.layout.pane_row_targets.len(), 1);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%2");
    }

    #[test]
    fn confirm_repo_popup_all_resets_filter() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "app".into(),
            has_focus: true,
            panes: vec![(test_pane("%1"), PaneGitInfo::default())],
        }];
        state.global.repo_filter = RepoFilter::Repo("app".into());
        state.popup = PopupState::Repo {
            selected: 0,
            area: None,
        };
        state.confirm_repo_popup();

        assert_eq!(state.global.repo_filter, RepoFilter::All);
    }

    #[test]
    fn close_repo_popup_resets_popup_state() {
        let mut state = AppState::new("%99".into());
        state.popup = PopupState::Repo {
            selected: 5,
            area: Some(ratatui::layout::Rect::new(0, 0, 10, 5)),
        };
        assert!(state.is_repo_popup_open());
        state.close_repo_popup();
        assert!(matches!(state.popup, PopupState::None));
        assert!(state.repo_popup_area().is_none());
    }

    #[test]
    fn toggle_repo_popup_twice_closes() {
        let mut state = AppState::new("%99".into());
        state.toggle_repo_popup();
        assert!(state.is_repo_popup_open());
        state.toggle_repo_popup();
        assert!(!state.is_repo_popup_open());
    }

    #[test]
    fn set_repo_popup_selected_noop_when_other_variant() {
        let mut state = AppState::new("%99".into());
        state.popup = PopupState::Notices { area: None };
        state.set_repo_popup_selected(9);
        // Repo accessor returns default when not in Repo variant.
        assert_eq!(state.repo_popup_selected(), 0);
    }

    // ─── Notices popup ───────────────────────────────────────────────

    #[test]
    fn toggle_notices_popup_opens_then_closes() {
        let mut state = AppState::new("%99".into());
        state.toggle_notices_popup();
        assert!(state.is_notices_popup_open());
        state.toggle_notices_popup();
        assert!(!state.is_notices_popup_open());
    }

    #[test]
    fn close_notices_popup_clears_copy_state() {
        let mut state = AppState::new("%99".into());
        state.popup = PopupState::Notices { area: None };
        state.notices.copy_targets = vec![NoticesCopyTarget {
            area: ratatui::layout::Rect::new(0, 0, 5, 1),
            agent: "claude".into(),
        }];
        state.notices.copied_at = Some(("claude".into(), std::time::Instant::now()));

        state.close_notices_popup();

        assert!(!state.is_notices_popup_open());
        assert!(state.notices.copy_targets.is_empty());
        assert!(state.notices.copied_at.is_none());
    }

    #[test]
    fn notices_popup_area_none_when_closed() {
        let state = AppState::new("%99".into());
        assert!(state.notices_popup_area().is_none());
    }

    // ─── Spawn input popup ───────────────────────────────────────────

    #[test]
    fn open_spawn_input_for_repo_initializes_fields() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), Some(7));
        assert!(state.is_spawn_input_open());
        if let PopupState::SpawnInput {
            input,
            target_repo,
            target_repo_root,
            agent_idx,
            mode_idx,
            field,
            anchor_y,
            error,
            area,
        } = &state.popup
        {
            assert!(input.is_empty());
            assert_eq!(target_repo, "alpha");
            assert_eq!(target_repo_root, "/tmp/alpha");
            assert_eq!(*agent_idx, 0);
            assert_eq!(*mode_idx, 0);
            assert_eq!(*field, SpawnField::Task);
            assert_eq!(*anchor_y, Some(7));
            assert!(error.is_none());
            assert!(area.is_none());
        } else {
            panic!("expected SpawnInput, got {:?}", state.popup);
        }
    }

    #[test]
    fn close_spawn_input_noop_when_not_open() {
        let mut state = AppState::new("%99".into());
        state.popup = PopupState::Notices { area: None };
        state.close_spawn_input();
        // The other popup must not be touched.
        assert!(matches!(state.popup, PopupState::Notices { .. }));
    }

    #[test]
    fn spawn_input_next_prev_cycle_field() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), None);
        state.spawn_input_next_field();
        assert!(matches!(
            state.popup,
            PopupState::SpawnInput {
                field: SpawnField::Agent,
                ..
            }
        ));
        state.spawn_input_next_field();
        assert!(matches!(
            state.popup,
            PopupState::SpawnInput {
                field: SpawnField::Mode,
                ..
            }
        ));
        state.spawn_input_prev_field();
        assert!(matches!(
            state.popup,
            PopupState::SpawnInput {
                field: SpawnField::Agent,
                ..
            }
        ));
    }

    #[test]
    fn spawn_input_field_change_clears_error() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), None);
        if let PopupState::SpawnInput { error, .. } = &mut state.popup {
            *error = Some("boom".into());
        }
        state.spawn_input_next_field();
        if let PopupState::SpawnInput { error, .. } = &state.popup {
            assert!(error.is_none());
        }
    }

    #[test]
    fn spawn_input_cycle_agent_wraps_and_resets_mode() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), None);
        // Switch field to Agent, bump mode_idx artificially, then cycle to
        // verify mode_idx resets to 0 on agent change.
        if let PopupState::SpawnInput {
            field, mode_idx, ..
        } = &mut state.popup
        {
            *field = SpawnField::Agent;
            *mode_idx = 1;
        }
        state.spawn_input_cycle(1);
        if let PopupState::SpawnInput {
            agent_idx,
            mode_idx,
            ..
        } = &state.popup
        {
            assert_eq!(*agent_idx, 1 % crate::worktree::AGENTS.len());
            assert_eq!(*mode_idx, 0);
        }
    }

    #[test]
    fn spawn_input_cycle_task_field_is_noop() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), None);
        state.spawn_input_cycle(1);
        if let PopupState::SpawnInput {
            agent_idx,
            mode_idx,
            ..
        } = &state.popup
        {
            assert_eq!(*agent_idx, 0);
            assert_eq!(*mode_idx, 0);
        }
    }

    #[test]
    fn spawn_input_push_pop_char_only_on_task_field() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), None);

        state.spawn_input_push_char('h');
        state.spawn_input_push_char('i');
        if let PopupState::SpawnInput { input, .. } = &state.popup {
            assert_eq!(input, "hi");
        }

        // Switch field — push/pop must have no effect on input text.
        state.spawn_input_next_field();
        state.spawn_input_push_char('x');
        state.spawn_input_pop_char();
        if let PopupState::SpawnInput { input, .. } = &state.popup {
            assert_eq!(input, "hi");
        }
    }

    #[test]
    fn confirm_spawn_input_empty_sets_error() {
        let mut state = AppState::new("%99".into());
        state.open_spawn_input_for_repo("alpha".into(), "/tmp/alpha".into(), None);
        state.confirm_spawn_input();
        if let PopupState::SpawnInput { error, .. } = &state.popup {
            assert_eq!(error.as_deref(), Some("name is empty"));
        } else {
            panic!("popup must stay open on error");
        }
    }

    // ─── Remove confirm popup ────────────────────────────────────────

    #[test]
    fn remove_confirm_accessors_and_close() {
        let mut state = AppState::new("%99".into());
        state.popup = PopupState::RemoveConfirm {
            pane_id: "%1".into(),
            branch: "feature/x".into(),
            error: None,
            area: Some(ratatui::layout::Rect::new(0, 0, 20, 5)),
        };
        assert!(state.is_remove_confirm_open());
        assert_eq!(
            state.remove_confirm_popup_area(),
            Some(ratatui::layout::Rect::new(0, 0, 20, 5))
        );
        state.close_remove_confirm();
        assert!(!state.is_remove_confirm_open());
        assert!(state.remove_confirm_popup_area().is_none());
    }

    #[test]
    fn confirm_remove_noop_when_popup_absent() {
        // When no RemoveConfirm popup is open, `confirm_remove` must early-return
        // without touching state. We verify nothing blew up and the popup
        // variant is preserved.
        let mut state = AppState::new("%99".into());
        state.popup = PopupState::Notices { area: None };
        state.confirm_remove(crate::worktree::RemoveMode::WindowOnly);
        assert!(matches!(state.popup, PopupState::Notices { .. }));
    }

    #[test]
    fn open_remove_confirm_without_selection_flashes() {
        let mut state = AppState::new("%99".into());
        state.open_remove_confirm();
        assert!(state.take_flash().is_some());
        assert!(!state.is_remove_confirm_open());
    }
}
