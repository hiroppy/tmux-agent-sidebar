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
