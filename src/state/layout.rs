#[derive(Debug, Clone)]
pub struct RowTarget {
    pub pane_id: String,
}

/// Click target for the `+` button rendered at the right edge of each
/// repo-group header in the agents panel. Clicking it opens the spawn
/// modal prefilled for that repo.
#[derive(Debug, Clone)]
pub struct RepoSpawnTarget {
    pub rect: ratatui::layout::Rect,
    pub repo_name: String,
    pub repo_root: String,
}

/// Click target for the red `×` rendered next to the branch of a
/// sidebar-spawned pane. Clicking it opens the close-pane confirmation
/// for that specific pane.
#[derive(Debug, Clone)]
pub struct SpawnRemoveTarget {
    pub rect: ratatui::layout::Rect,
    pub pane_id: String,
}

/// Screen-positioned hyperlink overlay for OSC 8 terminal hyperlinks.
#[derive(Debug, Clone)]
pub struct HyperlinkOverlay {
    pub x: u16,
    pub y: u16,
    pub text: String,
    pub url: String,
}

/// Ephemeral render output cached for click hit-testing.
///
/// Every field here is **rewritten on every frame** by the UI layer and
/// only read by event handlers (mouse/keyboard) before the next render.
/// Bundling them under `state.layout` makes the "frame-scoped vs
/// persistent state" boundary visible at a glance, since the rest of
/// `AppState` only holds data that survives across frames.
#[derive(Debug, Clone, Default)]
pub struct FrameLayout {
    /// Filtered pane list, in the order the UI rendered them. Index
    /// matches `GlobalState::selected_pane_row`.
    pub pane_row_targets: Vec<RowTarget>,
    /// Maps each rendered text line in the agents panel back to a row in
    /// `pane_row_targets`. `None` for header/blank lines that should not
    /// route clicks to a pane.
    pub line_to_row: Vec<Option<usize>>,
    /// X column of the repo filter button in the secondary header. `None`
    /// when the button is hidden. Used for click hit-testing.
    pub repo_button_col: Option<u16>,
    /// Click regions for the `[+]` spawn button rendered at the right
    /// edge of each repo-group header. One entry per visible repo group.
    pub repo_spawn_targets: Vec<RepoSpawnTarget>,
    /// Click regions for the red `×` remove marker rendered next to the
    /// branch of each sidebar-spawned pane. One entry per visible row.
    pub spawn_remove_targets: Vec<SpawnRemoveTarget>,
    /// OSC 8 hyperlink overlays the main loop writes after each frame so
    /// terminals can recognise PR numbers as clickable links.
    pub hyperlink_overlays: Vec<HyperlinkOverlay>,
}
