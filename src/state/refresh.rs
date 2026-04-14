use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::activity::{self, TaskProgress};
use crate::tmux::{self, PaneStatus, SessionInfo};

use super::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskProgressDecision {
    Clear,
    Show,
    Dismiss { total: usize },
    Skip,
}

/// A per-pane task-progress update computed in the first pass of
/// `refresh_task_progress`, applied back to `pane_states` in the second pass.
struct PaneTaskUpdate {
    pane_id: String,
    progress: Option<TaskProgress>,
    dismissed_total: Option<usize>,
    inactive_since: Option<u64>,
}

pub(crate) fn classify_task_progress(
    progress: &TaskProgress,
    dismissed_total: Option<usize>,
) -> TaskProgressDecision {
    if progress.is_empty() {
        return TaskProgressDecision::Clear;
    }
    if progress.all_completed() {
        if dismissed_total == Some(progress.total()) {
            TaskProgressDecision::Skip
        } else {
            TaskProgressDecision::Dismiss {
                total: progress.total(),
            }
        }
    } else {
        TaskProgressDecision::Show
    }
}

impl AppState {
    pub(crate) fn refresh_now(&mut self) {
        self.now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }

    pub(crate) fn apply_session_snapshot(
        &mut self,
        sidebar_focused: bool,
        sessions: Vec<SessionInfo>,
    ) {
        self.sidebar_focused = sidebar_focused;
        self.sessions = sessions;
        self.repo_groups = crate::group::group_panes_by_repo(&self.sessions);
        self.prune_pane_states_to_current_panes();
        self.rebuild_row_targets();
        self.find_focused_pane();
    }

    fn clear_dead_agent_metadata(pane_id: &str) {
        for key in &[
            "@pane_agent",
            "@pane_status",
            "@pane_attention",
            "@pane_prompt",
            "@pane_prompt_source",
            "@pane_subagents",
            "@pane_cwd",
            "@pane_permission_mode",
            "@pane_worktree_name",
            "@pane_worktree_branch",
            "@pane_started_at",
            "@pane_wait_reason",
            "@pane_session_id",
        ] {
            tmux::unset_pane_option(pane_id, key);
        }

        let _ = std::fs::remove_file(activity::log_file_path(pane_id));
    }

    fn filter_sessions_to_live_agent_panes(
        sessions: Vec<SessionInfo>,
        live_agent_panes: &HashSet<String>,
    ) -> Vec<SessionInfo> {
        let mut out = Vec::new();
        for mut session in sessions {
            let mut windows = Vec::new();
            for mut window in session.windows {
                window
                    .panes
                    .retain(|pane| live_agent_panes.contains(&pane.pane_id));
                if !window.panes.is_empty() {
                    windows.push(window);
                }
            }
            if !windows.is_empty() {
                session.windows = windows;
                out.push(session);
            }
        }
        out
    }

    fn refresh_activity_data(&mut self) {
        self.refresh_activity_log();
        self.refresh_task_progress();
        self.auto_switch_tab();
    }

    /// Fast refresh: tmux state + activity log (called every 1s).
    /// Returns whether the sidebar's window is the active tmux window.
    pub fn refresh(&mut self) -> bool {
        self.refresh_now();
        let (focused, window_active, _, _) = tmux::get_sidebar_pane_info(&self.tmux_pane);
        let sessions = tmux::query_sessions();
        if let Some(process_snapshot) = self.refresh_port_data(&sessions) {
            let sessions = Self::filter_sessions_to_live_agent_panes(
                sessions,
                &process_snapshot.live_agent_panes,
            );
            self.apply_session_snapshot(focused, sessions);
        } else {
            self.apply_session_snapshot(focused, sessions);
        }
        self.refresh_session_names();
        self.refresh_activity_data();
        window_active
    }

    /// Periodically scan `~/.claude/sessions/*.json` to resolve session names.
    /// Populates `pane.session_name` for panes that have a matching `session_id`.
    fn refresh_session_names(&mut self) {
        const SESSION_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

        if self.last_session_refresh.elapsed() >= SESSION_REFRESH_INTERVAL {
            self.session_names = crate::session::scan_session_names();
            self.last_session_refresh = std::time::Instant::now();
        }

        for group in &mut self.repo_groups {
            for (pane, _) in &mut group.panes {
                if let Some(sid) = &pane.session_id
                    && let Some(name) = self.session_names.get(sid)
                {
                    pane.session_name.clone_from(name);
                } else {
                    pane.session_name.clear();
                }
            }
        }
    }

    pub(crate) fn refresh_port_data(
        &mut self,
        sessions: &[SessionInfo],
    ) -> Option<crate::port::PaneProcessSnapshot> {
        const PORT_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

        if !self.port_scan_initialized || self.last_port_refresh.elapsed() >= PORT_REFRESH_INTERVAL
        {
            let scanned = crate::port::scan_session_process_snapshot(sessions)?;
            let mut active_ids: HashSet<String> = HashSet::new();
            let mut updates: Vec<(String, Vec<u16>, Option<String>)> = Vec::new();
            let mut dead_panes: Vec<String> = Vec::new();
            for session in sessions {
                for window in &session.windows {
                    for pane in &window.panes {
                        active_ids.insert(pane.pane_id.clone());
                        if !scanned.live_agent_panes.contains(&pane.pane_id) {
                            dead_panes.push(pane.pane_id.clone());
                        }
                        updates.push((
                            pane.pane_id.clone(),
                            scanned
                                .ports_by_pane
                                .get(&pane.pane_id)
                                .cloned()
                                .unwrap_or_default(),
                            scanned.command_by_pane.get(&pane.pane_id).cloned(),
                        ));
                    }
                }
            }
            for (pane_id, ports, command) in updates {
                let pane_state = self.pane_state_mut(&pane_id);
                pane_state.ports = ports;
                pane_state.command = command;
            }
            for pane_id in dead_panes {
                Self::clear_dead_agent_metadata(&pane_id);
                self.clear_pane_state(&pane_id);
            }
            self.pane_states
                .retain(|pane_id, _| active_ids.contains(pane_id));
            self.port_scan_initialized = true;
            self.last_port_refresh = std::time::Instant::now();
            return Some(scanned);
        }

        None
    }

    pub(crate) fn refresh_task_progress(&mut self) {
        let mut active_pane_ids: HashSet<String> = HashSet::new();
        let mut updates: Vec<PaneTaskUpdate> = Vec::new();
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                active_pane_ids.insert(pane.pane_id.clone());
                // Read all entries for task progress (not limited to display max)
                // so that TaskCreate entries aren't lost when subagents flood the log
                let entries = activity::read_activity_log(&pane.pane_id, 0);
                let progress = activity::parse_task_progress(&entries);
                // Debounce inactive→dismiss transition to avoid flicker.
                //
                // The agent status can briefly drop to idle during normal operation
                // (e.g. when Claude Code processes a system prompt or between tool
                // calls). Without a grace period, the 1-second refresh cycle can
                // catch that transient idle state and immediately hide the task
                // progress bar, causing a visible flicker.
                //
                // We track when each pane first appeared inactive and only dismiss
                // after INACTIVE_GRACE_SECS have elapsed. If the agent returns to
                // Running/Waiting within that window, the timer is reset.
                const INACTIVE_GRACE_SECS: u64 = 3;

                let agent_inactive =
                    !matches!(pane.status, PaneStatus::Running | PaneStatus::Waiting);

                let prior_state = self.pane_state(&pane.pane_id).cloned().unwrap_or_default();
                let next_inactive_since = if agent_inactive {
                    Some(prior_state.inactive_since.unwrap_or(self.now))
                } else {
                    None
                };
                let grace_expired = next_inactive_since
                    .is_some_and(|since| self.now.saturating_sub(since) >= INACTIVE_GRACE_SECS);

                let decision = if grace_expired && !progress.is_empty() && !progress.all_completed()
                {
                    TaskProgressDecision::Dismiss {
                        total: progress.total(),
                    }
                } else {
                    classify_task_progress(&progress, prior_state.task_dismissed_total)
                };
                let next_progress = match decision {
                    TaskProgressDecision::Clear => None,
                    TaskProgressDecision::Show => Some(progress),
                    TaskProgressDecision::Dismiss { .. } => None,
                    TaskProgressDecision::Skip => prior_state.task_progress.clone(),
                };
                let next_dismissed_total = match decision {
                    TaskProgressDecision::Clear | TaskProgressDecision::Show => None,
                    TaskProgressDecision::Dismiss { total } => Some(total),
                    TaskProgressDecision::Skip => prior_state.task_dismissed_total,
                };
                updates.push(PaneTaskUpdate {
                    pane_id: pane.pane_id.clone(),
                    progress: next_progress,
                    dismissed_total: next_dismissed_total,
                    inactive_since: next_inactive_since,
                });
            }
        }
        for update in updates {
            let pane_state = self.pane_state_mut(&update.pane_id);
            pane_state.inactive_since = update.inactive_since;
            pane_state.task_dismissed_total = update.dismissed_total;
            pane_state.task_progress = update.progress;
        }
        self.pane_states
            .retain(|id, _| active_pane_ids.contains(id));
    }

    pub(crate) fn refresh_activity_log(&mut self) {
        if let Some(ref pane_id) = self.focused_pane_id {
            self.activity_entries = activity::read_activity_log(pane_id, self.activity_max_entries);
        } else {
            self.activity_entries.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::{AgentType, PaneInfo, PaneStatus, PermissionMode, SessionInfo, WindowInfo};

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
            worktree_name: String::new(),
            worktree_branch: String::new(),
            session_id: None,
            session_name: String::new(),
        }
    }

    fn test_session(panes: Vec<PaneInfo>) -> Vec<SessionInfo> {
        vec![SessionInfo {
            session_name: "main".into(),
            windows: vec![WindowInfo {
                window_id: "@0".into(),
                window_name: "test".into(),
                window_active: true,
                auto_rename: false,
                panes,
            }],
        }]
    }

    #[test]
    fn filter_sessions_to_live_agent_panes_removes_dead_panes() {
        let sessions = test_session(vec![test_pane("%1"), test_pane("%2")]);
        let live = HashSet::from(["%2".to_string()]);

        let filtered = AppState::filter_sessions_to_live_agent_panes(sessions, &live);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].windows.len(), 1);
        assert_eq!(filtered[0].windows[0].panes.len(), 1);
        assert_eq!(filtered[0].windows[0].panes[0].pane_id, "%2");
    }

    #[test]
    fn filter_sessions_to_live_agent_panes_drops_empty_sessions() {
        let sessions = test_session(vec![test_pane("%1")]);
        let live = HashSet::new();

        let filtered = AppState::filter_sessions_to_live_agent_panes(sessions, &live);

        assert!(filtered.is_empty());
    }
}
