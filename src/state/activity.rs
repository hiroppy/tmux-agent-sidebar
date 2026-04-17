use std::time::SystemTime;

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
}
