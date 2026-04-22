use std::time::{Instant, SystemTime};

use super::AppState;
use crate::activity::ActivityEntry;
use crate::state::ScrollState;

#[derive(Debug, Clone)]
pub struct ActivityState {
    pub entries: Vec<ActivityEntry>,
    pub scroll: ScrollState,
    pub max_entries: usize,
    /// `(focused_pane_id, mtime)` of the activity log most recently
    /// rendered into `entries`. `refresh_activity_log` skips re-reading
    /// the log when neither field has changed.
    pub log_cache: Option<(String, SystemTime)>,
}

impl ActivityState {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: ScrollState::default(),
            max_entries: 50,
            log_cache: None,
        }
    }
}

impl Default for ActivityState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    // ─── Flash banner ────────────────────────────────────────────────

    pub fn set_flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((
            msg.into(),
            Instant::now() + std::time::Duration::from_secs(4),
        ));
    }

    /// Return the current flash text if still valid, clearing it once the
    /// deadline passes. Called by the UI once per frame.
    pub fn take_flash(&mut self) -> Option<String> {
        match &self.flash {
            Some((text, exp)) if Instant::now() < *exp => Some(text.clone()),
            Some(_) => {
                self.flash = None;
                None
            }
            None => None,
        }
    }

    pub fn apply_git_data(&mut self, data: crate::git::GitData) {
        self.git = data;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_expected_defaults() {
        let state = ActivityState::new();
        assert!(state.entries.is_empty());
        assert_eq!(state.max_entries, 50);
        assert_eq!(state.scroll.offset, 0);
        assert_eq!(state.scroll.total_lines, 0);
        assert_eq!(state.scroll.visible_height, 0);
        assert!(state.log_cache.is_none());
    }

    #[test]
    fn default_delegates_to_new() {
        let default_state = ActivityState::default();
        let new_state = ActivityState::new();
        assert_eq!(default_state.entries.len(), new_state.entries.len());
        assert_eq!(default_state.max_entries, new_state.max_entries);
        assert_eq!(default_state.scroll.offset, new_state.scroll.offset);
        assert!(default_state.log_cache.is_none());
    }

    // ─── Flash banner / apply_git_data ───────────────────────────────

    #[test]
    fn set_flash_stores_message_with_future_expiry() {
        let mut state = AppState::new("%99".into());
        state.set_flash("hello");
        let (msg, exp) = state.flash.as_ref().expect("flash must be set");
        assert_eq!(msg, "hello");
        assert!(*exp > Instant::now());
    }

    #[test]
    fn take_flash_returns_message_then_noop_on_expiry() {
        let mut state = AppState::new("%99".into());
        state.set_flash("msg");
        assert_eq!(state.take_flash().as_deref(), Some("msg"));
        // Force-expire by rewinding the deadline into the past.
        state.flash = Some((
            "stale".into(),
            Instant::now() - std::time::Duration::from_secs(1),
        ));
        assert!(state.take_flash().is_none());
        assert!(state.flash.is_none());
    }

    #[test]
    fn take_flash_none_when_unset() {
        let mut state = AppState::new("%99".into());
        assert!(state.take_flash().is_none());
    }

    #[test]
    fn apply_git_data_overwrites_previous_state() {
        let mut state = AppState::new("%99".into());
        state.git.branch = "old".into();
        state.apply_git_data(crate::git::GitData {
            branch: "new".into(),
            diff_stat: Some((3, 1)),
            ..Default::default()
        });
        assert_eq!(state.git.branch, "new");
        assert_eq!(state.git.diff_stat, Some((3, 1)));
    }
}
