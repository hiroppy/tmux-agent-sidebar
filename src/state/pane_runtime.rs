use std::collections::{HashMap, HashSet};

use crate::activity::TaskProgress;
use crate::state::BottomTab;

/// Per-pane runtime state that should vanish together with the pane.
#[derive(Debug, Clone, Default)]
pub struct PaneRuntimeState {
    pub ports: Vec<u16>,
    pub command: Option<String>,
    pub task_progress: Option<TaskProgress>,
    pub task_dismissed_total: Option<usize>,
    pub inactive_since: Option<u64>,
    /// Last bottom tab the user selected while this pane was focused.
    /// `None` until the user changes tabs at least once. Cleaned up
    /// automatically by `prune_pane_states_to_current_panes` when the
    /// pane disappears, so a relaunched pane starts fresh.
    pub tab_pref: Option<BottomTab>,
    /// Last observed mtime of this pane's `/tmp/tmux-agent-activity*.log`.
    /// Used by `refresh_task_progress` to skip the (potentially expensive)
    /// re-parse when the log has not been touched since the previous tick.
    pub task_progress_log_mtime: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone)]
pub struct PaneRuntimeMap {
    pub map: HashMap<String, PaneRuntimeState>,
    /// Agent pane IDs that have already been seen.
    pub seen: HashSet<String>,
}

impl PaneRuntimeMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            seen: HashSet::new(),
        }
    }

    pub fn get(&self, pane_id: &str) -> Option<&PaneRuntimeState> {
        self.map.get(pane_id)
    }

    pub fn get_mut(&mut self, pane_id: &str) -> Option<&mut PaneRuntimeState> {
        self.map.get_mut(pane_id)
    }

    pub fn entry_mut(&mut self, pane_id: &str) -> &mut PaneRuntimeState {
        self.map.entry(pane_id.to_string()).or_default()
    }

    pub fn contains_key(&self, pane_id: &str) -> bool {
        self.map.contains_key(pane_id)
    }

    pub fn remove(&mut self, pane_id: &str) -> Option<PaneRuntimeState> {
        self.map.remove(pane_id)
    }
}

impl Default for PaneRuntimeMap {
    fn default() -> Self {
        Self::new()
    }
}
