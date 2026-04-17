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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let map = PaneRuntimeMap::new();
        assert!(map.map.is_empty());
        assert!(map.seen.is_empty());
    }

    #[test]
    fn default_delegates_to_new() {
        let map = PaneRuntimeMap::default();
        assert!(map.map.is_empty());
        assert!(map.seen.is_empty());
    }

    #[test]
    fn entry_mut_creates_default_on_miss() {
        let mut map = PaneRuntimeMap::new();
        let state = map.entry_mut("pane-1");
        assert!(state.ports.is_empty());
        assert!(state.command.is_none());
        assert!(state.task_progress.is_none());
        assert!(state.task_dismissed_total.is_none());
        assert!(state.inactive_since.is_none());
        assert!(state.tab_pref.is_none());
        assert!(state.task_progress_log_mtime.is_none());
    }

    #[test]
    fn entry_mut_returns_existing_entry() {
        let mut map = PaneRuntimeMap::new();
        map.entry_mut("pane-1").ports = vec![8080];
        let state = map.entry_mut("pane-1");
        assert_eq!(state.ports, vec![8080]);
    }

    #[test]
    fn get_returns_none_before_entry() {
        let map = PaneRuntimeMap::new();
        assert!(map.get("pane-1").is_none());
    }

    #[test]
    fn get_returns_some_after_insertion() {
        let mut map = PaneRuntimeMap::new();
        map.entry_mut("pane-1").ports = vec![3000];
        let state = map.get("pane-1").unwrap();
        assert_eq!(state.ports, vec![3000]);
    }

    #[test]
    fn get_mut_returns_some_after_insertion() {
        let mut map = PaneRuntimeMap::new();
        map.entry_mut("pane-1");
        let state = map.get_mut("pane-1").unwrap();
        state.command = Some("cargo run".into());
        assert_eq!(
            map.get("pane-1").unwrap().command.as_deref(),
            Some("cargo run")
        );
    }

    #[test]
    fn get_mut_returns_none_before_insertion() {
        let mut map = PaneRuntimeMap::new();
        assert!(map.get_mut("pane-x").is_none());
    }

    #[test]
    fn contains_key_reflects_insertion() {
        let mut map = PaneRuntimeMap::new();
        assert!(!map.contains_key("pane-1"));
        map.entry_mut("pane-1");
        assert!(map.contains_key("pane-1"));
    }

    #[test]
    fn remove_returns_the_prior_value() {
        let mut map = PaneRuntimeMap::new();
        map.entry_mut("pane-1").ports = vec![8080];
        let removed = map.remove("pane-1").unwrap();
        assert_eq!(removed.ports, vec![8080]);
        assert!(map.get("pane-1").is_none());
        assert!(!map.contains_key("pane-1"));
    }

    #[test]
    fn remove_missing_returns_none() {
        let mut map = PaneRuntimeMap::new();
        assert!(map.remove("nope").is_none());
    }
}
