use std::collections::{HashMap, HashSet};

use super::AppState;
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

impl AppState {
    pub fn pane_state_mut(&mut self, pane_id: &str) -> &mut PaneRuntimeState {
        self.pane_states.entry_mut(pane_id)
    }

    pub fn pane_state(&self, pane_id: &str) -> Option<&PaneRuntimeState> {
        self.pane_states.get(pane_id)
    }

    pub fn set_pane_ports(&mut self, pane_id: &str, ports: Vec<u16>) {
        self.pane_state_mut(pane_id).ports = ports;
    }

    pub fn pane_ports(&self, pane_id: &str) -> Option<&[u16]> {
        self.pane_state(pane_id).map(|s| s.ports.as_slice())
    }

    pub fn set_pane_command(&mut self, pane_id: &str, command: Option<String>) {
        self.pane_state_mut(pane_id).command = command;
    }

    pub fn pane_command(&self, pane_id: &str) -> Option<&str> {
        self.pane_state(pane_id).and_then(|s| s.command.as_deref())
    }

    pub fn set_pane_task_progress(&mut self, pane_id: &str, progress: Option<TaskProgress>) {
        self.pane_state_mut(pane_id).task_progress = progress;
    }

    pub fn pane_task_progress(&self, pane_id: &str) -> Option<&TaskProgress> {
        self.pane_state(pane_id)
            .and_then(|s| s.task_progress.as_ref())
    }

    pub fn set_pane_task_dismissed_total(&mut self, pane_id: &str, total: Option<usize>) {
        self.pane_state_mut(pane_id).task_dismissed_total = total;
    }

    pub fn pane_task_dismissed_total(&self, pane_id: &str) -> Option<usize> {
        self.pane_state(pane_id)
            .and_then(|s| s.task_dismissed_total)
    }

    pub fn set_pane_inactive_since(&mut self, pane_id: &str, since: Option<u64>) {
        self.pane_state_mut(pane_id).inactive_since = since;
    }

    pub fn pane_inactive_since(&self, pane_id: &str) -> Option<u64> {
        self.pane_state(pane_id).and_then(|s| s.inactive_since)
    }

    pub fn clear_pane_state(&mut self, pane_id: &str) {
        self.pane_states.remove(pane_id);
    }

    pub fn prune_pane_states_to_current_panes(&mut self) {
        let mut active_ids = HashSet::new();
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                active_ids.insert(pane.pane_id.clone());
            }
        }
        self.pane_states
            .map
            .retain(|pane_id, _| active_ids.contains(pane_id));
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

    // ─── AppState accessors ──────────────────────────────────────────

    use crate::activity::TaskStatus;

    #[test]
    fn app_state_pane_accessors_round_trip_through_runtime_map() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%42";

        state.set_pane_ports(pane_id, vec![3000]);
        state.set_pane_command(pane_id, Some("pnpm dev".into()));
        state.set_pane_task_progress(
            pane_id,
            Some(TaskProgress {
                tasks: vec![("t".into(), TaskStatus::InProgress)],
            }),
        );
        state.set_pane_task_dismissed_total(pane_id, Some(7));
        state.set_pane_inactive_since(pane_id, Some(42));

        assert_eq!(state.pane_ports(pane_id), Some(&[3000][..]));
        assert_eq!(state.pane_command(pane_id), Some("pnpm dev"));
        assert_eq!(
            state.pane_task_progress(pane_id).map(|p| p.total()),
            Some(1)
        );
        assert_eq!(state.pane_task_dismissed_total(pane_id), Some(7));
        assert_eq!(state.pane_inactive_since(pane_id), Some(42));

        state.clear_pane_state(pane_id);
        assert!(state.pane_state(pane_id).is_none());
    }

    #[test]
    fn pane_state_mut_creates_entry_on_first_access() {
        let mut state = AppState::new("%99".into());
        assert!(state.pane_state("%1").is_none());
        state.pane_state_mut("%1").inactive_since = Some(5);
        assert_eq!(state.pane_inactive_since("%1"), Some(5));
    }

    #[test]
    fn setters_with_none_clear_previous_values() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%1";
        state.set_pane_command(pane_id, Some("old".into()));
        state.set_pane_command(pane_id, None);
        assert!(state.pane_command(pane_id).is_none());

        state.set_pane_task_dismissed_total(pane_id, Some(3));
        state.set_pane_task_dismissed_total(pane_id, None);
        assert!(state.pane_task_dismissed_total(pane_id).is_none());

        state.set_pane_inactive_since(pane_id, Some(10));
        state.set_pane_inactive_since(pane_id, None);
        assert!(state.pane_inactive_since(pane_id).is_none());
    }
}
