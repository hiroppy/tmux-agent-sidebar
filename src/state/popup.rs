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
