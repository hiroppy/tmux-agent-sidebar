use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

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
        self.rebuild_row_targets();
        self.find_focused_pane();
    }

    fn refresh_activity_data(&mut self) {
        self.refresh_activity_log();
        self.refresh_task_progress();
        self.auto_switch_tab();
    }

    /// Fast refresh: tmux state + activity log (called every 1s)
    pub fn refresh(&mut self) {
        self.refresh_now();
        let (focused, _, _) = tmux::get_sidebar_pane_info(&self.tmux_pane);
        self.apply_session_snapshot(focused, tmux::query_sessions());
        self.refresh_activity_data();
    }

    pub(crate) fn refresh_task_progress(&mut self) {
        self.pane_task_progress.clear();
        let mut active_pane_ids: HashSet<&str> = HashSet::new();
        for group in &self.repo_groups {
            for (pane, _) in &group.panes {
                active_pane_ids.insert(&pane.pane_id);
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

                // Update the per-pane inactive timer:
                // - Agent active → clear the timer
                // - Agent inactive and no timer yet → start the timer
                // - Agent inactive and timer exists → leave it (accumulate elapsed time)
                if agent_inactive {
                    self.pane_inactive_since
                        .entry(pane.pane_id.clone())
                        .or_insert(self.now);
                } else {
                    self.pane_inactive_since.remove(&pane.pane_id);
                }

                // Only treat the agent as truly inactive once the grace period
                // has passed, so momentary status flickers are ignored.
                let grace_expired = self
                    .pane_inactive_since
                    .get(&pane.pane_id)
                    .map_or(false, |&since| {
                        self.now.saturating_sub(since) >= INACTIVE_GRACE_SECS
                    });

                let decision = if grace_expired && !progress.is_empty() && !progress.all_completed()
                {
                    TaskProgressDecision::Dismiss {
                        total: progress.total(),
                    }
                } else {
                    classify_task_progress(
                        &progress,
                        self.pane_task_dismissed.get(&pane.pane_id).copied(),
                    )
                };
                match decision {
                    TaskProgressDecision::Clear => {
                        self.pane_task_dismissed.remove(&pane.pane_id);
                    }
                    TaskProgressDecision::Show => {
                        self.pane_task_dismissed.remove(&pane.pane_id);
                        self.pane_task_progress
                            .insert(pane.pane_id.clone(), progress);
                    }
                    TaskProgressDecision::Dismiss { total } => {
                        self.pane_task_dismissed.insert(pane.pane_id.clone(), total);
                    }
                    TaskProgressDecision::Skip => {}
                }
            }
        }
        self.pane_task_dismissed
            .retain(|id, _| active_pane_ids.contains(id.as_str()));
        // Clean up inactive timers for panes that no longer exist
        self.pane_inactive_since
            .retain(|id, _| active_pane_ids.contains(id.as_str()));
    }

    pub(crate) fn refresh_activity_log(&mut self) {
        if let Some(ref pane_id) = self.focused_pane_id {
            self.activity_entries = activity::read_activity_log(pane_id, self.activity_max_entries);
        } else {
            self.activity_entries.clear();
        }
    }
}
