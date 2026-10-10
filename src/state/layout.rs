use super::{AppState, RepoFilter, StatusFilter};

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
    /// Click region for the bottom-panel toggle handle, drawn only while the
    /// window is compact (the last row: a `▸`/`▾` drawer handle that reveals
    /// or hides the Activity/Git panel). `None` on tall windows, where the
    /// panel is always shown and there is nothing to toggle.
    pub bottom_toggle_target: Option<ratatui::layout::Rect>,
}

pub(super) fn point_in_rect(row: u16, col: u16, rect: ratatui::layout::Rect) -> bool {
    rect.contains(ratatui::layout::Position { x: col, y: row })
}

impl AppState {
    /// Minimum rows the agent/session list must keep before the fixed bottom
    /// panel is allowed to share the viewport. Below this the layout switches
    /// to "compact": the bottom panel auto-hides so the list keeps the full
    /// height, and the user toggles it back in on demand (see the `b` key).
    pub const COMPACT_LIST_MIN: u16 = 10;

    /// Resolve, for a viewport of `height` rows, `(compact, shown)`:
    ///
    /// * `compact` — the fixed bottom panel would leave the list fewer than
    ///   [`COMPACT_LIST_MIN`] rows, i.e. `height < bottom_height + MIN`. In
    ///   compact mode the panel renders as a full-height accordion over the
    ///   list rather than a split beneath it.
    /// * `shown` — whether the bottom panel is rendered at all. A tall window
    ///   *always* shows it; a compact window auto-hides it and only reveals it
    ///   while the user has peeked it in via [`toggle_bottom_panel`]
    ///   (`bottom_override == Some(true)`). Always `false` when
    ///   `@sidebar_bottom_height` is 0 (panel disabled).
    pub fn bottom_visibility(&self, height: u16) -> (bool, bool) {
        if self.bottom_panel_height == 0 {
            return (false, false);
        }
        let compact = height
            < self
                .bottom_panel_height
                .saturating_add(Self::COMPACT_LIST_MIN);
        // A roomy window always shows the panel — the manual override only
        // governs the compact regime, where space must be shared. This means a
        // resize back to a tall window re-shows the panel regardless of any
        // earlier toggle (the override is also cleared on the crossing; see
        // [`sync_bottom_regime`]).
        let shown = if compact {
            self.bottom_override.unwrap_or(false)
        } else {
            true
        };
        (compact, shown)
    }

    /// Reset the manual override whenever the window crosses the compact/tall
    /// threshold, so height changes win over a stale toggle: growing tall
    /// re-shows the panel, shrinking short re-hides it. Called once per frame
    /// by the renderer with the freshly computed `compact` flag. The very
    /// first frame only records the regime (it must not wipe an override that
    /// was set before the first render); only a genuine crossing between two
    /// observed regimes resets the override.
    pub fn sync_bottom_regime(&mut self, compact: bool) {
        if let Some(prev) = self.last_compact
            && prev != compact
        {
            self.bottom_override = None;
        }
        self.last_compact = Some(compact);
    }

    /// Whether the bottom panel is visible at the last rendered viewport
    /// height. Keyboard handlers use this (they have no terminal handle) to
    /// avoid descending focus into a hidden Activity log.
    pub fn bottom_panel_visible(&self) -> bool {
        self.bottom_visibility(self.last_viewport_height).1
    }

    /// Flip bottom-panel visibility on a compact window: reveal the panel as a
    /// full-height accordion over the list, or hide it again to give the list
    /// the whole viewport. No-op when the panel is disabled
    /// (`@sidebar_bottom_height` = 0) or when the window is tall — a tall
    /// window always shows the panel, so there is nothing to toggle.
    pub fn toggle_bottom_panel(&mut self) {
        if self.bottom_panel_height == 0 {
            return;
        }
        let (compact, shown) = self.bottom_visibility(self.last_viewport_height);
        if !compact {
            return;
        }
        self.bottom_override = Some(!shown);
    }

    /// Route a left-click that landed on the bottom-panel toggle handle (the
    /// `▸`/`▾` drawer on the last row of a compact window). Returns `true` if
    /// the click hit the handle and the panel was toggled.
    pub fn handle_bottom_toggle_click(&mut self, row: u16, col: u16) -> bool {
        if let Some(rect) = self.layout.bottom_toggle_target
            && point_in_rect(row, col, rect)
        {
            self.toggle_bottom_panel();
            return true;
        }
        false
    }

    pub fn rebuild_row_targets(&mut self) {
        // Reset stale repo filter if the repo no longer exists, and
        // persist the reset back to tmux so fresh sidebar instances do
        // not reload the dead repo name on startup.
        if let RepoFilter::Repo(ref name) = self.global.repo_filter
            && !self.repo_groups.iter().any(|g| g.name == *name)
        {
            self.global.repo_filter = RepoFilter::All;
            self.global.save_repo_filter();
        }

        self.layout.pane_row_targets.clear();
        for group in &self.repo_groups {
            if !self.global.repo_filter.matches_group(&group.name) {
                continue;
            }
            for (pane, _) in &group.panes {
                if self.global.status_filter.matches(&pane.status) {
                    self.layout.pane_row_targets.push(RowTarget {
                        pane_id: pane.pane_id.clone(),
                    });
                }
            }
        }
        if self.global.selected_pane_row >= self.layout.pane_row_targets.len()
            && !self.layout.pane_row_targets.is_empty()
        {
            self.global.selected_pane_row = self.layout.pane_row_targets.len() - 1;
        }
    }

    /// Handle mouse scroll event, routing to agents or bottom panel based on Y position.
    pub fn handle_mouse_scroll(
        &mut self,
        row: u16,
        term_height: u16,
        bottom_panel_height: u16,
        delta: isize,
    ) {
        let bottom_start = term_height.saturating_sub(bottom_panel_height);
        if row >= bottom_start {
            self.scroll_bottom(delta);
        } else {
            self.scrolls.panes.scroll(delta);
        }
    }

    /// Handle mouse click on the filter bar (row 0).
    /// Determines which filter was clicked based on x coordinate.
    /// Debounces rapid clicks to ignore phantom mouse events from tmux
    /// pane resize/layout changes.
    pub fn handle_filter_click(&mut self, col: u16) {
        const DEBOUNCE_MS: u128 = 150;
        let now = std::time::Instant::now();
        if now
            .duration_since(self.timers.last_filter_click)
            .as_millis()
            < DEBOUNCE_MS
        {
            return;
        }
        self.timers.last_filter_click = now;

        let (all, running, background, waiting, idle, error) = self.status_counts();
        // Layout: " ∑N  ●N  ◎N  ◐N  ○N  ✕N"
        // Each filter item renders as `icon(1) + count`, so the clickable
        // width is `1 + digits(count)`.
        let mut x = 1usize; // leading space
        let items: Vec<(StatusFilter, usize)> = vec![
            (StatusFilter::All, 1 + format!("{all}").len()),
            (StatusFilter::Running, 1 + format!("{running}").len()),
            (StatusFilter::Background, 1 + format!("{background}").len()),
            (StatusFilter::Waiting, 1 + format!("{waiting}").len()),
            (StatusFilter::Idle, 1 + format!("{idle}").len()),
            (StatusFilter::Error, 1 + format!("{error}").len()),
        ];
        let col = col as usize;
        for (i, (filter, width)) in items.iter().enumerate() {
            if i > 0 {
                x += 2; // "  " separator
            }
            if col >= x && col < x + width {
                self.global.status_filter = *filter;
                self.global.save_filter();
                self.rebuild_row_targets();
                return;
            }
            x += width;
        }
    }

    /// Handle mouse click on the secondary header row (row 1).
    /// The repo filter button lives on the far right of this row.
    pub fn handle_secondary_header_click(&mut self, col: u16) {
        if self
            .notices
            .button_col
            .is_some_and(|notices_col| col == notices_col)
        {
            self.toggle_notices_popup();
            return;
        }
        if self
            .layout
            .repo_button_col
            .is_some_and(|repo_button_col| col >= repo_button_col)
        {
            self.toggle_repo_popup();
        }
    }

    /// Handle mouse click in agents panel. Maps screen row to agent row
    /// via line_to_row (adjusted for scroll offset) and activates that pane.
    /// Row 0 is the fixed filter bar, row 1+ maps to the scrollable agent list.
    pub fn handle_mouse_click(&mut self, row: u16, col: u16) {
        if self.is_notices_popup_open() {
            if let Some(area) = self.notices_popup_area()
                && point_in_rect(row, col, area)
            {
                if let Some(agent) = self.notices_copy_target_at(row, col).map(str::to_string) {
                    self.copy_notices_prompt(&agent);
                }
                return;
            }
            self.close_notices_popup();
            return;
        }
        if self.is_repo_popup_open() {
            if let Some(area) = self.repo_popup_area()
                && point_in_rect(row, col, area)
            {
                // Skip clicks on the popup chrome (top border / title row).
                // Without this guard `saturating_sub(1)` collapses a click on
                // the title row into `item_index == 0`, switching the filter
                // to the first repo the moment the user reaches for the
                // popup.
                if row > area.y {
                    let item_index = (row - area.y - 1) as usize;
                    if item_index < self.repo_names().len() {
                        self.set_repo_popup_selected(item_index);
                        self.confirm_repo_popup();
                    }
                }
                return;
            }
            self.close_repo_popup();
            return;
        }
        if self.is_spawn_input_open() {
            if let Some(area) = self.spawn_input_popup_area()
                && point_in_rect(row, col, area)
            {
                return;
            }
            self.close_spawn_input();
            return;
        }
        if self.is_remove_confirm_open() {
            if let Some(area) = self.remove_confirm_popup_area()
                && point_in_rect(row, col, area)
            {
                return;
            }
            self.close_remove_confirm();
            return;
        }

        if row == 0 {
            self.handle_filter_click(col);
            return;
        }
        if row == 1 {
            self.handle_secondary_header_click(col);
            return;
        }

        // Check the `+` spawn buttons before the pane-row fallback so a
        // click on the button doesn't also shift the pane selection.
        if let Some((repo_name, repo_root, anchor_y)) = self
            .layout
            .repo_spawn_targets
            .iter()
            .find(|t| point_in_rect(row, col, t.rect))
            .map(|t| (t.repo_name.clone(), t.repo_root.clone(), t.rect.y))
        {
            self.open_spawn_input_for_repo(repo_name, repo_root, Some(anchor_y));
            return;
        }

        // Check the red `×` remove markers next to spawn-created branches.
        if let Some(pane_id) = self
            .layout
            .spawn_remove_targets
            .iter()
            .find(|t| point_in_rect(row, col, t.rect))
            .map(|t| t.pane_id.clone())
        {
            self.open_remove_confirm_for_pane(pane_id);
            return;
        }

        let line_index = (row as usize - 2) + self.scrolls.panes.offset;
        if let Some(Some(agent_row)) = self.layout.line_to_row.get(line_index) {
            self.global.selected_pane_row = *agent_row;
            self.global.queue_cursor_save();
            self.activate_selected_pane();
        }
    }
}

#[cfg(test)]
mod visibility_tests {
    use crate::state::AppState;

    fn state_with_bottom(height: u16) -> AppState {
        let mut s = AppState::new("%99".into());
        s.bottom_panel_height = height;
        s
    }

    #[test]
    fn tall_window_shows_both_by_default() {
        let s = state_with_bottom(20);
        // 50 >= 20 + COMPACT_LIST_MIN(10) -> not compact, shown.
        assert_eq!(s.bottom_visibility(50), (false, true));
    }

    #[test]
    fn short_window_auto_hides_bottom() {
        let s = state_with_bottom(20);
        // Threshold is 20 + 10 = 30: below it is compact + hidden, at/above
        // it is roomy + shown.
        assert_eq!(s.bottom_visibility(29), (true, false));
        assert_eq!(s.bottom_visibility(30), (false, true));
    }

    #[test]
    fn zero_height_option_always_hidden() {
        let s = state_with_bottom(0);
        assert_eq!(s.bottom_visibility(100), (false, false));
    }

    #[test]
    fn toggle_reveals_on_short_but_is_noop_on_tall() {
        let mut s = state_with_bottom(20);

        // Short window: hidden by default, toggle reveals then hides again.
        s.last_viewport_height = 25;
        assert!(!s.bottom_panel_visible());
        s.toggle_bottom_panel();
        assert!(s.bottom_panel_visible());
        s.toggle_bottom_panel();
        assert!(!s.bottom_panel_visible());

        // Tall window: always shown, and the toggle can't hide it.
        s.bottom_override = None;
        s.last_viewport_height = 60;
        assert!(s.bottom_panel_visible());
        s.toggle_bottom_panel();
        assert!(s.bottom_panel_visible());
        assert_eq!(s.bottom_override, None);
    }

    #[test]
    fn crossing_the_threshold_resets_the_override() {
        let mut s = state_with_bottom(20);

        // Peek the panel in on a short window.
        s.last_viewport_height = 25;
        s.sync_bottom_regime(true);
        s.toggle_bottom_panel();
        assert!(s.bottom_panel_visible());

        // Growing to a tall window re-shows it and clears the override.
        s.sync_bottom_regime(false);
        assert_eq!(s.bottom_override, None);
        s.last_viewport_height = 60;
        assert!(s.bottom_panel_visible());

        // Shrinking back to compact re-hides it (override cleared again).
        s.sync_bottom_regime(true);
        s.last_viewport_height = 25;
        assert!(!s.bottom_panel_visible());
    }

    #[test]
    fn clicking_the_drawer_handle_toggles_the_panel() {
        let mut s = state_with_bottom(20);
        s.last_viewport_height = 14;
        s.sync_bottom_regime(true);
        // The renderer publishes the handle rect on the compact window's last row.
        s.layout.bottom_toggle_target = Some(ratatui::layout::Rect::new(0, 13, 28, 1));

        assert!(!s.bottom_panel_visible());
        assert!(s.handle_bottom_toggle_click(13, 5), "click hit the handle");
        assert!(s.bottom_panel_visible());
        assert!(s.handle_bottom_toggle_click(13, 5));
        assert!(!s.bottom_panel_visible());

        // A click anywhere off the handle row is not consumed.
        assert!(!s.handle_bottom_toggle_click(2, 5));
        assert!(!s.bottom_panel_visible());
    }

    #[test]
    fn toggle_is_noop_when_panel_disabled() {
        let mut s = state_with_bottom(0);
        s.last_viewport_height = 100;
        s.toggle_bottom_panel();
        assert_eq!(s.bottom_override, None);
        assert!(!s.bottom_panel_visible());
    }
}
