use std::time::Instant;

use crate::ui::colors::ColorTheme;
use crate::ui::icons::StatusIcons;

mod activity;
mod filter;
mod focus;
mod global;
mod layout;
mod notices;
mod pane_runtime;
mod popup;
mod refresh;
mod scroll;
mod session;
mod tab;
mod timers;

pub use activity::ActivityState;
pub use filter::{RepoFilter, StatusFilter};
pub use focus::{Focus, FocusState};
pub use global::GlobalState;
pub use layout::{FrameLayout, HyperlinkOverlay, RepoSpawnTarget, RowTarget, SpawnRemoveTarget};
pub(crate) use notices::debug_forced_display;
pub use notices::{ClaudePluginNotice, NoticesCopyTarget, NoticesMissingHookGroup, NoticesState};
pub use pane_runtime::{PaneRuntimeMap, PaneRuntimeState};
pub use popup::{PopupState, SpawnField};
#[cfg(test)]
pub(crate) use refresh::{TaskProgressDecision, classify_task_progress};
pub use scroll::{ScrollState, ScrollStates};
pub use session::SessionNamesState;
pub use timers::RefreshTimers;

#[derive(Debug, Clone, PartialEq)]
pub enum BottomTab {
    Activity,
    GitStatus,
}

pub struct AppState {
    pub now: u64,
    pub repo_groups: Vec<crate::group::RepoGroup>,
    /// Sidebar focus + pane focus tracking (sidebar_focused, focus,
    /// focused_pane_id, prev_focused_pane_id).
    pub focus_state: FocusState,
    /// Transient one-line status banner (message + expiry) for spawn /
    /// remove feedback. Cleared by `take_flash` once the deadline passes.
    pub flash: Option<(String, Instant)>,
    pub spinner_frame: usize,
    /// Frame-scoped render output (pane_row_targets, line_to_row,
    /// repo_button_col, hyperlink_overlays). Rewritten every frame by
    /// the UI layer; consumed by mouse/keyboard handlers before the
    /// next render.
    pub layout: FrameLayout,
    pub activity: ActivityState,
    pub tmux_pane: String,
    /// Scroll offsets for the agents list and git tab. Activity tab
    /// scroll lives in [`ActivityState::scroll`].
    pub scrolls: ScrollStates,
    pub theme: ColorTheme,
    pub icons: StatusIcons,
    pub bottom_tab: BottomTab,
    pub git: crate::git::GitData,
    pub pane_states: PaneRuntimeMap,
    /// Periodic-refresh clocks (port scan, session-name scan, filter
    /// debounce, port-scan first-run flag).
    pub timers: RefreshTimers,
    /// Current popup state. At most one popup is open at a time; the enum
    /// variant encodes both which popup is open and its per-popup data.
    pub popup: PopupState,
    /// All fields related to the ⓘ notices button and its popup — the button
    /// click region, cached hook/plugin diagnostics, per-agent copy targets,
    /// and the transient "copied" feedback label.
    pub notices: NoticesState,
    /// Pending OSC 52 clipboard payload. The main loop flushes this to
    /// stdout after the next frame so tmux (with `set-clipboard on`) can
    /// forward it to the upstream terminal's clipboard — covering the
    /// SSH case where `arboard` would only reach the remote machine.
    pub pending_osc52_copy: Option<String>,
    /// Update notice shown when a newer GitHub release is available.
    pub version_notice: Option<crate::version::UpdateNotice>,
    /// Shared state across sidebar instances, persisted to tmux global variables.
    pub global: GlobalState,
    /// Height of the bottom panel in lines. Loaded once at startup from
    /// the `@sidebar_bottom_height` tmux option. A value of 0 hides the panel.
    pub bottom_panel_height: u16,
    /// Maps session_id → session name, refreshed periodically from
    /// `~/.claude/sessions/*.json` files. The `dirty` flag is `true` when
    /// the map has changed since the last `refresh_session_names`
    /// application. Set by the main loop after receiving a fresh map from
    /// `session_poll_loop`, cleared by `refresh_session_names` once the
    /// map has been propagated to every pane. Avoids re-walking every
    /// pane each tick when the map is unchanged (the polling thread only
    /// updates it every 10s).
    pub sessions: SessionNamesState,
}

impl AppState {
    pub fn new(tmux_pane: String) -> Self {
        Self {
            now: 0,
            repo_groups: vec![],
            focus_state: FocusState::new(),
            flash: None,
            spinner_frame: 0,
            layout: FrameLayout::default(),
            activity: ActivityState::new(),
            tmux_pane,
            scrolls: ScrollStates::default(),
            theme: ColorTheme::default(),
            icons: StatusIcons::default(),
            bottom_tab: BottomTab::Activity,
            git: crate::git::GitData::default(),
            pane_states: PaneRuntimeMap::new(),
            timers: RefreshTimers::default(),
            popup: PopupState::None,
            notices: NoticesState::default(),
            pending_osc52_copy: None,
            version_notice: None,
            global: GlobalState::new(),
            bottom_panel_height: crate::ui::BOTTOM_PANEL_HEIGHT,
            sessions: SessionNamesState::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{TaskProgress, TaskStatus};
    use crate::group::{PaneGitInfo, RepoGroup};
    use crate::tmux::{AgentType, PaneInfo, PaneStatus, PermissionMode, WorktreeMetadata};
    use std::fs;

    /// Reset filter click debounce so the next `handle_filter_click` is not ignored.
    fn reset_filter_debounce(state: &mut AppState) {
        state.timers.last_filter_click =
            std::time::Instant::now() - std::time::Duration::from_millis(200);
    }

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
            worktree: WorktreeMetadata::default(),
            session_id: None,
            session_name: String::new(),
            sidebar_spawned: false,
        }
    }

    fn write_activity_log(pane_id: &str, contents: &str) -> String {
        let path = crate::activity::log_file_path(pane_id);
        fs::write(&path, contents).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn rebuild_row_targets_from_repo_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![
                    (test_pane("%1"), PaneGitInfo::default()),
                    (test_pane("%2"), PaneGitInfo::default()),
                ],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%3"), PaneGitInfo::default())],
            },
        ];
        state.rebuild_row_targets();

        assert_eq!(state.layout.pane_row_targets.len(), 3);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%1");
        assert_eq!(state.layout.pane_row_targets[1].pane_id, "%2");
        assert_eq!(state.layout.pane_row_targets[2].pane_id, "%3");
    }

    #[test]
    fn selection_crosses_repo_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%5"), PaneGitInfo::default())],
            },
        ];
        state.rebuild_row_targets();

        // Start at first group
        assert_eq!(state.global.selected_pane_row, 0);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%1");

        // Move to second group
        assert!(state.move_pane_selection(1));
        assert_eq!(state.global.selected_pane_row, 1);
        assert_eq!(state.layout.pane_row_targets[1].pane_id, "%5");
    }

    #[test]
    fn task_progress_hides_when_all_completed() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%100".to_string();

        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane("%100"), PaneGitInfo::default())],
        }];

        let log_path = crate::activity::log_file_path(&pane_id);
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskUpdate|completed #1\n10:03|TaskUpdate|completed #2\n",
        ).unwrap();

        state.refresh_task_progress();

        // All completed → hidden immediately
        assert!(state.pane_task_progress(&pane_id).is_none());
        // Dismissed count should be recorded
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(2));

        // Calling refresh again should still be hidden (no flicker)
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn task_progress_reshows_when_new_tasks_added() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%101".to_string();

        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane("%101"), PaneGitInfo::default())],
        }];

        // First: 1 task, completed → dismissed
        let log_path = crate::activity::log_file_path(&pane_id);
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        )
        .unwrap();
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());

        // Now add a new in-progress task → should re-show
        fs::write(
            &log_path,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n10:02|TaskCreate|#2 B\n10:03|TaskUpdate|in_progress #2\n",
        ).unwrap();
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn classify_task_progress_empty_clears() {
        let progress = TaskProgress { tasks: vec![] };
        assert_eq!(
            classify_task_progress(&progress, None),
            TaskProgressDecision::Clear
        );
    }

    #[test]
    fn classify_task_progress_in_progress_shows() {
        let progress = TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::InProgress),
            ],
        };
        assert_eq!(
            classify_task_progress(&progress, None),
            TaskProgressDecision::Show
        );
    }

    #[test]
    fn classify_task_progress_completed_dismisses_once() {
        let progress = TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::Completed),
            ],
        };
        assert_eq!(
            classify_task_progress(&progress, None),
            TaskProgressDecision::Dismiss { total: 2 }
        );
        assert_eq!(
            classify_task_progress(&progress, Some(2)),
            TaskProgressDecision::Skip
        );
    }

    #[test]
    fn classify_task_progress_completed_with_different_dismissal_dismisses_again() {
        let progress = TaskProgress {
            tasks: vec![
                ("A".into(), TaskStatus::Completed),
                ("B".into(), TaskStatus::Completed),
            ],
        };
        assert_eq!(
            classify_task_progress(&progress, Some(1)),
            TaskProgressDecision::Dismiss { total: 2 }
        );
    }

    #[test]
    fn refresh_now_updates_current_time() {
        let mut state = AppState::new("%99".into());
        state.refresh_now();
        assert!(state.now > 0);
    }

    #[test]
    fn refresh_activity_log_reads_focused_pane() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%201";
        let log_path = crate::activity::log_file_path(pane_id);
        fs::write(&log_path, "10:00|Read|old\n10:01|Edit|new\n").unwrap();
        state.focus_state.focused_pane_id = Some(pane_id.into());
        state.activity.max_entries = 50;

        state.refresh_activity_log();

        assert_eq!(state.activity.entries.len(), 2);
        assert_eq!(state.activity.entries[0].tool, "Edit");
        assert_eq!(state.activity.entries[0].label, "new");
        assert_eq!(state.activity.entries[1].tool, "Read");

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_activity_log_clears_without_focus() {
        let mut state = AppState::new("%99".into());
        state.activity.entries = vec![crate::activity::ActivityEntry {
            timestamp: "10:00".into(),
            tool: "Read".into(),
            label: "keep?".into(),
        }];

        state.focus_state.focused_pane_id = None;
        state.refresh_activity_log();

        assert!(state.activity.entries.is_empty());
    }

    #[test]
    fn refresh_task_progress_clears_empty_logs_and_dismissal() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%202".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        state.set_pane_task_progress(
            &pane_id,
            Some(TaskProgress {
                tasks: vec![("stale".into(), TaskStatus::InProgress)],
            }),
        );
        state.set_pane_task_dismissed_total(&pane_id, Some(1));

        state.refresh_task_progress();

        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), None);
    }

    #[test]
    fn refresh_task_progress_shows_in_progress_and_clears_dismissal() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%203".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        state.set_pane_task_dismissed_total(&pane_id, Some(1));
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        state.refresh_task_progress();

        assert_eq!(state.pane_task_dismissed_total(&pane_id), None);
        assert_eq!(
            state.pane_task_progress(&pane_id).map(|p| p.total()),
            Some(1)
        );
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_records_completed_dismissal() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%204".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        );

        state.refresh_task_progress();

        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_skips_already_dismissed_completed_tasks() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%205".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        );

        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        assert!(state.pane_task_progress(&pane_id).is_none());

        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        assert!(state.pane_task_progress(&pane_id).is_none());
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_drops_dismissals_for_inactive_panes() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%206".to_string();
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|completed #1\n",
        );
        state.refresh_task_progress();
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));

        // Pane removed — both dismissed and inactive_since should be cleaned up
        // by `prune_pane_states_to_current_panes`, which `refresh()` runs via
        // `apply_session_snapshot` immediately before `refresh_task_progress`.
        state.repo_groups.clear();
        state.set_pane_inactive_since(&pane_id, Some(100));
        state.prune_pane_states_to_current_panes();
        state.refresh_task_progress();

        assert!(state.pane_state(&pane_id).is_none());
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn pane_runtime_state_accessors_round_trip() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%213";

        state.set_pane_ports(pane_id, vec![3000, 5173]);
        state.set_pane_command(pane_id, Some("npm run dev".into()));
        state.set_pane_task_progress(
            pane_id,
            Some(TaskProgress {
                tasks: vec![("A".into(), TaskStatus::InProgress)],
            }),
        );
        state.set_pane_task_dismissed_total(pane_id, Some(4));
        state.set_pane_inactive_since(pane_id, Some(123));

        assert_eq!(state.pane_ports(pane_id), Some(&[3000, 5173][..]));
        assert_eq!(state.pane_command(pane_id), Some("npm run dev"));
        assert_eq!(
            state.pane_task_progress(pane_id).map(|p| p.total()),
            Some(1)
        );
        assert_eq!(state.pane_task_dismissed_total(pane_id), Some(4));
        assert_eq!(state.pane_inactive_since(pane_id), Some(123));

        state.clear_pane_state(pane_id);
        assert!(state.pane_state(pane_id).is_none());
    }

    #[test]
    fn prune_pane_states_to_current_panes_drops_stale_entries() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane("%1"), PaneGitInfo::default())],
        }];
        state.set_pane_ports("%1", vec![3000]);
        state.set_pane_command("%1", Some("npm run dev".into()));
        state.set_pane_ports("%2", vec![5173]);
        state.set_pane_task_dismissed_total("%2", Some(2));

        state.prune_pane_states_to_current_panes();

        assert_eq!(state.pane_ports("%1"), Some(&[3000][..]));
        assert_eq!(state.pane_command("%1"), Some("npm run dev"));
        assert!(state.pane_state("%2").is_none());
    }

    #[test]
    fn refresh_task_progress_dismisses_incomplete_tasks_when_agent_idle() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%207".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        // 5 out of 6 tasks completed — agent is idle so it won't update further
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskCreate|#3 C\n10:03|TaskCreate|#4 D\n10:04|TaskCreate|#5 E\n10:05|TaskCreate|#6 F\n10:06|TaskUpdate|completed #1\n10:07|TaskUpdate|completed #2\n10:08|TaskUpdate|completed #3\n10:09|TaskUpdate|completed #4\n10:10|TaskUpdate|completed #5\n",
        );

        // First refresh: grace period starts, tasks still shown (not dismissed yet)
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_some());

        // After grace period (3s): should be dismissed
        state.now = 104;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(6));
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_shows_incomplete_tasks_when_agent_running() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%208".to_string();
        // test_pane defaults to PaneStatus::Running
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(test_pane(&pane_id), PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskUpdate|completed #1\n10:03|TaskUpdate|in_progress #2\n",
        );

        state.refresh_task_progress();

        // Agent is running, so incomplete tasks should still be shown
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert_eq!(
            state.pane_task_progress(&pane_id).map(|p| p.total()),
            Some(2)
        );
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_dismisses_incomplete_tasks_when_agent_error() {
        let mut state = AppState::new("%99".into());
        let pane_id = "%209".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Error;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        // First refresh: grace period starts, tasks still shown
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        // After grace period: agent errored out — dismiss incomplete tasks
        state.now = 104;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));
        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_debounce_resets_when_agent_resumes() {
        // Simulates brief idle flicker: agent goes idle then returns to running
        // before the grace period expires — tasks should remain visible.
        let mut state = AppState::new("%99".into());
        let pane_id = "%210".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskCreate|#2 B\n10:02|TaskUpdate|completed #1\n",
        );

        // Agent is idle — grace timer starts, tasks still shown
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_some());

        // Agent returns to running before grace expires — timer resets
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.now = 102;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_none());

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_debounce_exact_boundary() {
        // Grace period is 3 seconds. At exactly 3s the condition is >=,
        // so it should dismiss.
        let mut state = AppState::new("%99".into());
        let pane_id = "%211".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        // t=100: grace timer starts
        state.now = 100;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        // t=102 (2s elapsed): still within grace period — tasks shown
        state.now = 102;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_some());

        // t=103 (exactly 3s): grace expired (>= 3) — dismissed
        state.now = 103;
        state.refresh_task_progress();
        assert!(state.pane_task_progress(&pane_id).is_none());
        assert_eq!(state.pane_task_dismissed_total(&pane_id), Some(1));

        fs::remove_file(&log_path).ok();
    }

    #[test]
    fn refresh_task_progress_waiting_does_not_start_debounce() {
        // Waiting is an active state — inactive timer should not be set.
        let mut state = AppState::new("%99".into());
        let pane_id = "%212".to_string();
        let mut pane = test_pane(&pane_id);
        pane.status = PaneStatus::Waiting;
        state.repo_groups = vec![RepoGroup {
            name: "test".into(),
            has_focus: true,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let log_path = write_activity_log(
            &pane_id,
            "10:00|TaskCreate|#1 A\n10:01|TaskUpdate|in_progress #1\n",
        );

        state.now = 100;
        state.refresh_task_progress();

        // Tasks shown and no inactive timer started
        assert!(state.pane_task_progress(&pane_id).is_some());
        assert!(state.pane_inactive_since(&pane_id).is_none());

        fs::remove_file(&log_path).ok();
    }

    // ─── ScrollState unit tests ─────────────────────────────────────

    #[test]
    fn scroll_state_clamps_to_max() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 4,
        };
        s.scroll(100);
        assert_eq!(s.offset, 6); // max = 10 - 4
    }

    #[test]
    fn scroll_state_clamps_to_zero() {
        let mut s = ScrollState {
            offset: 3,
            total_lines: 10,
            visible_height: 4,
        };
        s.scroll(-100);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_noop_when_content_fits() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 3,
            visible_height: 5,
        };
        s.scroll(1);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_exact_fit_no_scroll() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 5,
            visible_height: 5,
        };
        s.scroll(1);
        assert_eq!(s.offset, 0);
    }

    #[test]
    fn scroll_state_incremental() {
        let mut s = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 4,
        };
        s.scroll(1);
        assert_eq!(s.offset, 1);
        s.scroll(2);
        assert_eq!(s.offset, 3);
        s.scroll(-1);
        assert_eq!(s.offset, 2);
    }

    // ─── apply_git_data tests ───────────────────────────────────────

    #[test]
    fn apply_git_data_copies_all_fields() {
        let mut state = AppState::new("%99".into());
        let data = crate::git::GitData {
            diff_stat: Some((10, 5)),
            branch: "feature/test".into(),
            ahead_behind: Some((2, 1)),
            staged_files: vec![crate::git::GitFileEntry {
                status: 'M',
                name: "lib.rs".into(),
                additions: 10,
                deletions: 5,
                path: String::new(),
            }],
            unstaged_files: vec![],
            untracked_files: vec!["new.rs".into()],
            remote_url: "https://github.com/user/repo".into(),
            pr_number: Some("42".into()),
        };

        state.apply_git_data(data);

        assert_eq!(state.git.diff_stat, Some((10, 5)));
        assert_eq!(state.git.branch, "feature/test");
        assert_eq!(state.git.ahead_behind, Some((2, 1)));
        assert_eq!(state.git.staged_files.len(), 1);
        assert_eq!(state.git.staged_files[0].status, 'M');
        assert!(state.git.unstaged_files.is_empty());
        assert_eq!(state.git.untracked_files, vec!["new.rs"]);
        assert_eq!(state.git.changed_file_count(), 2);
        assert_eq!(state.git.remote_url, "https://github.com/user/repo");
        assert_eq!(state.git.pr_number, Some("42".into()));
    }

    #[test]
    fn apply_git_data_with_defaults() {
        let mut state = AppState::new("%99".into());
        // Pre-fill some state
        state.git.branch = "old-branch".into();
        state.git.pr_number = Some("99".into());

        // Apply empty git data
        state.apply_git_data(crate::git::GitData::default());

        assert_eq!(state.git.diff_stat, None);
        assert!(state.git.branch.is_empty());
        assert_eq!(state.git.ahead_behind, None);
        assert!(state.git.staged_files.is_empty());
        assert!(state.git.unstaged_files.is_empty());
        assert!(state.git.untracked_files.is_empty());
        assert_eq!(state.git.changed_file_count(), 0);
        assert!(state.git.remote_url.is_empty());
        assert_eq!(state.git.pr_number, None);
    }

    #[test]
    fn apply_session_snapshot_rebuilds_derived_state() {
        let mut state = AppState::new("%99".into());
        state.global.selected_pane_row = 3;

        let pane = test_pane("%1");
        let sessions = vec![crate::tmux::SessionInfo {
            session_name: "main".into(),
            windows: vec![crate::tmux::WindowInfo {
                window_id: "@0".into(),
                window_name: "project".into(),
                window_active: true,
                auto_rename: false,
                panes: vec![pane],
            }],
        }];

        state.apply_session_snapshot(true, sessions);

        assert!(state.focus_state.sidebar_focused);
        assert_eq!(state.repo_groups.len(), 1);
        assert_eq!(state.layout.pane_row_targets.len(), 1);
        assert_eq!(state.global.selected_pane_row, 0);
        // focused_pane_id is set by find_focused_pane() which queries tmux
        // directly, so we don't assert it here (tmux not available in tests).
    }

    // ─── auto_switch_tab tests are in state/tab.rs ────────────────

    // ─── next_bottom_tab / scroll_bottom tests ──────────────────────

    #[test]
    fn next_bottom_tab_toggles() {
        let mut state = AppState::new("%99".into());
        assert_eq!(state.bottom_tab, BottomTab::Activity);
        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::GitStatus);
        state.next_bottom_tab();
        assert_eq!(state.bottom_tab, BottomTab::Activity);
    }

    #[test]
    fn scroll_bottom_dispatches_to_activity() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::Activity;
        state.activity.scroll = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 3,
        };

        state.scroll_bottom(2);
        assert_eq!(state.activity.scroll.offset, 2);
        assert_eq!(state.scrolls.git.offset, 0);
    }

    #[test]
    fn scroll_bottom_dispatches_to_git() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::GitStatus;
        state.scrolls.git = ScrollState {
            offset: 0,
            total_lines: 10,
            visible_height: 3,
        };

        state.scroll_bottom(2);
        assert_eq!(state.scrolls.git.offset, 2);
        assert_eq!(state.activity.scroll.offset, 0);
    }

    // ─── handle_mouse_scroll tests ────────────────────────────────────

    #[test]
    fn mouse_scroll_in_bottom_panel_scrolls_activity() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::Activity;
        state.activity.scroll = ScrollState {
            offset: 0,
            total_lines: 30,
            visible_height: 10,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at row 35 → in bottom panel
        state.handle_mouse_scroll(35, 50, 20, 3);
        assert_eq!(state.activity.scroll.offset, 3);
        assert_eq!(state.scrolls.panes.offset, 0);
    }

    #[test]
    fn mouse_scroll_in_agents_panel_scrolls_agents() {
        let mut state = AppState::new("%99".into());
        state.scrolls.panes = ScrollState {
            offset: 0,
            total_lines: 40,
            visible_height: 20,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at row 10 → in agents panel
        state.handle_mouse_scroll(10, 50, 20, 3);
        assert_eq!(state.scrolls.panes.offset, 3);
        assert_eq!(state.activity.scroll.offset, 0);
    }

    #[test]
    fn mouse_scroll_up_in_agents_panel() {
        let mut state = AppState::new("%99".into());
        state.scrolls.panes = ScrollState {
            offset: 5,
            total_lines: 40,
            visible_height: 20,
        };
        state.handle_mouse_scroll(10, 50, 20, -3);
        assert_eq!(state.scrolls.panes.offset, 2);
    }

    #[test]
    fn mouse_scroll_at_boundary_row_goes_to_bottom() {
        let mut state = AppState::new("%99".into());
        state.bottom_tab = BottomTab::GitStatus;
        state.scrolls.git = ScrollState {
            offset: 0,
            total_lines: 20,
            visible_height: 10,
        };
        // term_height=50, bottom_panel=20 → bottom starts at row 30
        // mouse at exactly row 30 → in bottom panel
        state.handle_mouse_scroll(30, 50, 20, 3);
        assert_eq!(state.scrolls.git.offset, 3);
        assert_eq!(state.scrolls.panes.offset, 0);
    }

    #[test]
    fn mouse_scroll_just_above_boundary_goes_to_agents() {
        let mut state = AppState::new("%99".into());
        state.scrolls.panes = ScrollState {
            offset: 0,
            total_lines: 40,
            visible_height: 20,
        };
        // row 29, just above bottom_start=30
        state.handle_mouse_scroll(29, 50, 20, 3);
        assert_eq!(state.scrolls.panes.offset, 3);
        assert_eq!(state.activity.scroll.offset, 0);
    }

    // ─── move_pane_selection edge cases ─────────────────────────────

    #[test]
    fn move_pane_selection_returns_false_when_empty() {
        let mut state = AppState::new("%99".into());
        assert!(!state.move_pane_selection(1));
        assert!(!state.move_pane_selection(-1));
    }

    #[test]
    fn move_pane_selection_boundary_returns() {
        let mut state = AppState::new("%99".into());
        state.layout.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
            RowTarget {
                pane_id: "%3".into(),
            },
        ];
        state.global.selected_pane_row = 0;

        assert!(!state.move_pane_selection(-1), "can't go below 0");
        assert!(state.move_pane_selection(1));
        assert!(state.move_pane_selection(1));
        assert_eq!(state.global.selected_pane_row, 2);
        assert!(!state.move_pane_selection(1), "can't go past end");
    }

    // ─── rebuild_row_targets clamp tests ────────────────────────────

    #[test]
    fn rebuild_row_targets_clamps_selection_when_shrinks() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (test_pane("%1"), PaneGitInfo::default()),
                (test_pane("%2"), PaneGitInfo::default()),
                (test_pane("%3"), PaneGitInfo::default()),
            ],
        }];
        state.global.selected_pane_row = 2;
        state.rebuild_row_targets();
        assert_eq!(state.global.selected_pane_row, 2);

        // Shrink to 1 pane
        state.repo_groups[0].panes = vec![(test_pane("%1"), PaneGitInfo::default())];
        state.rebuild_row_targets();
        assert_eq!(
            state.global.selected_pane_row, 0,
            "should clamp to last valid index"
        );
    }

    #[test]
    fn rebuild_row_targets_empty_groups() {
        let mut state = AppState::new("%99".into());
        state.global.selected_pane_row = 5;
        state.repo_groups = vec![];
        state.rebuild_row_targets();
        assert!(state.layout.pane_row_targets.is_empty());
        // selected_pane_row stays as-is when targets empty (no clamp needed)
        assert_eq!(state.global.selected_pane_row, 5);
    }

    #[test]
    fn rebuild_row_targets_respects_filter() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Idle;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Running;

        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
            ],
        }];

        // All filter: all 3 panes
        state.global.status_filter = StatusFilter::All;
        state.rebuild_row_targets();
        assert_eq!(state.layout.pane_row_targets.len(), 3);

        // Running filter: only 2 panes
        state.global.status_filter = StatusFilter::Running;
        state.rebuild_row_targets();
        assert_eq!(state.layout.pane_row_targets.len(), 2);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%1");
        assert_eq!(state.layout.pane_row_targets[1].pane_id, "%3");

        // Idle filter: only 1 pane
        state.global.status_filter = StatusFilter::Idle;
        state.rebuild_row_targets();
        assert_eq!(state.layout.pane_row_targets.len(), 1);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%2");

        // Error filter: no panes
        state.global.status_filter = StatusFilter::Error;
        state.rebuild_row_targets();
        assert!(state.layout.pane_row_targets.is_empty());
    }

    #[test]
    fn rebuild_row_targets_clamps_cursor_on_filter_change() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Idle;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Idle;

        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
            ],
        }];

        // Select last agent in All view
        state.global.status_filter = StatusFilter::All;
        state.rebuild_row_targets();
        state.global.selected_pane_row = 2;

        // Switch to Running filter (only 1 pane) — cursor should clamp
        state.global.status_filter = StatusFilter::Running;
        state.rebuild_row_targets();
        assert_eq!(state.global.selected_pane_row, 0);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%1");
    }

    // ─── handle_mouse_click tests ────────────────────────────────────

    #[test]
    fn mouse_click_selects_agent_row() {
        let mut state = AppState::new("%99".into());
        state.layout.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        // line_to_row: line 0 = group header (None), line 1 = agent 0, line 2 = agent 1
        state.layout.line_to_row = vec![None, Some(0), Some(1)];
        state.scrolls.panes.offset = 0;

        // row 0 = filter bar, row 1 = secondary header, row 2+ = agent list rows
        state.handle_mouse_click(3, 5); // row 3 → line_index = (3-2) = 1 → agent row 0
        assert_eq!(state.global.selected_pane_row, 0);

        state.handle_mouse_click(4, 5); // row 4 → line_index = (4-2) = 2 → agent row 1
        assert_eq!(state.global.selected_pane_row, 1);
    }

    #[test]
    fn mouse_click_on_filter_bar_changes_filter() {
        let mut state = AppState::new("%99".into());
        state.layout.pane_row_targets = vec![RowTarget {
            pane_id: "%1".into(),
        }];
        state.layout.line_to_row = vec![None, Some(0)];
        state.global.selected_pane_row = 0;
        state.global.status_filter = StatusFilter::All;

        // Click on "All" (x=1..3) should keep All
        reset_filter_debounce(&mut state);
        state.handle_mouse_click(0, 1);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        // Click on Running icon area (x=6..) should switch to Running
        reset_filter_debounce(&mut state);
        state.handle_mouse_click(0, 6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // agent selection unchanged
        assert_eq!(state.global.selected_pane_row, 0);
    }

    #[test]
    fn mouse_click_on_secondary_header_toggles_repo_popup() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.layout.repo_button_col = Some(20);

        state.handle_mouse_click(1, 19);
        assert!(!state.is_repo_popup_open());

        state.handle_mouse_click(1, 20);
        assert!(state.is_repo_popup_open());
    }

    #[test]
    fn mouse_click_on_repo_popup_title_row_does_not_confirm() {
        // Regression: clicking the popup's top border/title row used
        // to collapse to `item_index == 0` via `saturating_sub(1)` and
        // immediately switch the filter to the first repo.
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.global.repo_filter = RepoFilter::Repo("beta".into());
        state.popup = PopupState::Repo {
            selected: 2,
            area: Some(ratatui::layout::Rect::new(0, 3, 20, 5)),
        };

        // Click the top border row (row == area.y = 3).
        state.handle_mouse_click(3, 5);

        assert!(
            state.is_repo_popup_open(),
            "title-row click must keep the popup open"
        );
        assert_eq!(
            state.global.repo_filter,
            RepoFilter::Repo("beta".into()),
            "title-row click must not confirm a selection"
        );
    }

    #[test]
    fn mouse_click_on_repo_popup_item_row_confirms_selection() {
        // Companion to the regression test above: clicks on the item
        // rows (area.y + 1, area.y + 2, …) should still confirm.
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.popup = PopupState::Repo {
            selected: 0,
            area: Some(ratatui::layout::Rect::new(0, 3, 20, 5)),
        };

        // Click row area.y + 1 (first list entry = "All").
        state.handle_mouse_click(4, 5);

        assert!(!state.is_repo_popup_open());
        assert_eq!(state.global.repo_filter, RepoFilter::All);
    }

    #[test]
    fn mouse_click_with_scroll_offset() {
        let mut state = AppState::new("%99".into());
        state.layout.pane_row_targets = vec![
            RowTarget {
                pane_id: "%1".into(),
            },
            RowTarget {
                pane_id: "%2".into(),
            },
        ];
        // 5 lines total, scrolled down by 2
        state.layout.line_to_row = vec![None, Some(0), Some(0), None, Some(1)];
        state.scrolls.panes.offset = 2;

        // row 4 → line_index = (4-2) + 2 = 4 → agent row 1
        state.handle_mouse_click(4, 5);
        assert_eq!(state.global.selected_pane_row, 1);
    }

    #[test]
    fn mouse_click_out_of_bounds() {
        let mut state = AppState::new("%99".into());
        state.layout.pane_row_targets = vec![RowTarget {
            pane_id: "%1".into(),
        }];
        state.layout.line_to_row = vec![None, Some(0)];
        state.global.selected_pane_row = 0;

        state.handle_mouse_click(50, 5); // way beyond line_to_row
        assert_eq!(state.global.selected_pane_row, 0); // unchanged
    }

    // ─── StatusFilter tests live in state/filter.rs ──────────────────

    // ─── status_counts tests ─────────────────────────────────────────

    #[test]
    fn status_counts_empty() {
        let state = AppState::new("%99".into());
        assert_eq!(state.status_counts(), (0, 0, 0, 0, 0));
    }

    #[test]
    fn status_counts_mixed() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Running;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Idle;
        let mut p4 = test_pane("%4");
        p4.status = PaneStatus::Waiting;
        let mut p5 = test_pane("%5");
        p5.status = PaneStatus::Error;

        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
                (p4, PaneGitInfo::default()),
                (p5, PaneGitInfo::default()),
            ],
        }];
        // (all, running, waiting, idle, error)
        assert_eq!(state.status_counts(), (5, 2, 1, 1, 1));
    }

    // ─── handle_filter_click tests ───────────────────────────────────

    #[test]
    fn filter_click_all_positions() {
        let mut state = AppState::new("%99".into());
        // With 0 agents, counts are all 0, so layout: " All  ●0  ◐0  ○0  ✕0"
        //                                              0123456789...

        // "All" at x=1..3
        state.global.status_filter = StatusFilter::Running;
        reset_filter_debounce(&mut state);
        state.handle_filter_click(1);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        reset_filter_debounce(&mut state);
        state.handle_filter_click(3);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        // "●0" at x=6..7
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // "◐0" at x=10..11
        reset_filter_debounce(&mut state);
        state.handle_filter_click(10);
        assert_eq!(state.global.status_filter, StatusFilter::Waiting);

        // "○0" at x=14..15
        reset_filter_debounce(&mut state);
        state.handle_filter_click(14);
        assert_eq!(state.global.status_filter, StatusFilter::Idle);

        // "✕0" at x=18..19
        reset_filter_debounce(&mut state);
        state.handle_filter_click(18);
        assert_eq!(state.global.status_filter, StatusFilter::Error);
    }

    #[test]
    fn filter_click_gap_does_nothing() {
        let mut state = AppState::new("%99".into());
        state.global.status_filter = StatusFilter::All;

        // x=0 is leading space, x=4 and x=5 are separator
        state.handle_filter_click(0);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        state.handle_filter_click(4);
        assert_eq!(state.global.status_filter, StatusFilter::All);

        state.handle_filter_click(5);
        assert_eq!(state.global.status_filter, StatusFilter::All);
    }

    #[test]
    fn filter_click_debounce_ignores_rapid_clicks() {
        let mut state = AppState::new("%99".into());
        state.global.status_filter = StatusFilter::All;

        // First click within debounce window should be ignored
        // (AppState::new sets last_filter_click to now)
        state.handle_filter_click(6); // would be Running
        assert_eq!(state.global.status_filter, StatusFilter::All); // unchanged due to debounce

        // After resetting debounce, click should work
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // Immediate second click should be debounced
        state.handle_filter_click(1); // would be All
        assert_eq!(state.global.status_filter, StatusFilter::Running); // unchanged
    }

    #[test]
    fn filter_click_with_large_counts() {
        let mut state = AppState::new("%99".into());
        // Add 10 running agents to shift positions
        let panes: Vec<_> = (0..10)
            .map(|i| {
                let mut p = test_pane(&format!("%{i}"));
                p.status = PaneStatus::Running;
                (p, PaneGitInfo::default())
            })
            .collect();
        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes,
        }];
        // Layout: " All  ●10  ◐0  ○0  ✕0"
        //          0123456789...
        // "●10" at x=6..8 (icon + "10")
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);
        reset_filter_debounce(&mut state);
        state.handle_filter_click(8);
        assert_eq!(state.global.status_filter, StatusFilter::Running);

        // "◐0" shifts to x=11..12
        reset_filter_debounce(&mut state);
        state.handle_filter_click(11);
        assert_eq!(state.global.status_filter, StatusFilter::Waiting);
    }

    #[test]
    fn filter_click_rebuilds_row_targets() {
        let mut state = AppState::new("%99".into());
        let mut p1 = test_pane("%1");
        p1.status = PaneStatus::Running;
        let mut p2 = test_pane("%2");
        p2.status = PaneStatus::Idle;
        let mut p3 = test_pane("%3");
        p3.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (p1, PaneGitInfo::default()),
                (p2, PaneGitInfo::default()),
                (p3, PaneGitInfo::default()),
            ],
        }];
        state.global.status_filter = StatusFilter::All;
        state.rebuild_row_targets();
        assert_eq!(state.layout.pane_row_targets.len(), 3);

        // Click Running filter — row_targets should update immediately
        // Layout: " All  ●2  ◐0  ○1  ✕0" → Running at x=6
        reset_filter_debounce(&mut state);
        state.handle_filter_click(6);
        assert_eq!(state.global.status_filter, StatusFilter::Running);
        assert_eq!(state.layout.pane_row_targets.len(), 2);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%1");
        assert_eq!(state.layout.pane_row_targets[1].pane_id, "%3");

        // Click Idle filter — row_targets should update again
        // Layout: " All  ●2  ◐0  ○1  ✕0" → Idle at x=14
        reset_filter_debounce(&mut state);
        state.handle_filter_click(14);
        assert_eq!(state.global.status_filter, StatusFilter::Idle);
        assert_eq!(state.layout.pane_row_targets.len(), 1);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%2");
    }

    // ─── StatusFilter / RepoFilter pure tests live in state/filter.rs ─

    #[test]
    fn repo_filter_all_shows_all_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.global.repo_filter = RepoFilter::All;
        state.rebuild_row_targets();

        assert_eq!(state.layout.pane_row_targets.len(), 2);
    }

    #[test]
    fn repo_filter_specific_repo() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "dotfiles".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())],
            },
            RepoGroup {
                name: "app".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())],
            },
        ];
        state.global.repo_filter = RepoFilter::Repo("app".into());
        state.rebuild_row_targets();

        assert_eq!(state.layout.pane_row_targets.len(), 1);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%2");
    }

    #[test]
    fn repo_filter_combined_with_status() {
        let mut state = AppState::new("%99".into());
        let mut idle_pane = test_pane("%3");
        idle_pane.status = PaneStatus::Idle;
        state.repo_groups = vec![
            RepoGroup {
                name: "app".into(),
                has_focus: true,
                panes: vec![
                    (test_pane("%1"), PaneGitInfo::default()), // Running
                    (idle_pane, PaneGitInfo::default()),       // Idle
                ],
            },
            RepoGroup {
                name: "lib".into(),
                has_focus: false,
                panes: vec![(test_pane("%2"), PaneGitInfo::default())], // Running
            },
        ];
        state.global.repo_filter = RepoFilter::Repo("app".into());
        state.global.status_filter = StatusFilter::Running;
        state.rebuild_row_targets();

        // Only Running panes in "app" group
        assert_eq!(state.layout.pane_row_targets.len(), 1);
        assert_eq!(state.layout.pane_row_targets[0].pane_id, "%1");
    }

    #[test]
    fn repo_filter_stale_name_resets() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![RepoGroup {
            name: "app".into(),
            has_focus: true,
            panes: vec![(test_pane("%1"), PaneGitInfo::default())],
        }];
        state.global.repo_filter = RepoFilter::Repo("deleted-repo".into());
        state.rebuild_row_targets();

        assert_eq!(state.global.repo_filter, RepoFilter::All);
        assert_eq!(state.layout.pane_row_targets.len(), 1);
    }

    #[test]
    fn repo_names_returns_all_plus_groups() {
        let mut state = AppState::new("%99".into());
        state.repo_groups = vec![
            RepoGroup {
                name: "alpha".into(),
                has_focus: true,
                panes: vec![],
            },
            RepoGroup {
                name: "beta".into(),
                has_focus: false,
                panes: vec![],
            },
        ];
        assert_eq!(state.repo_names(), vec!["All", "alpha", "beta"]);
    }

    #[test]
    fn status_counts_respects_repo_filter() {
        let mut state = AppState::new("%99".into());
        let mut idle_pane = test_pane("%2");
        idle_pane.status = PaneStatus::Idle;
        state.repo_groups = vec![
            RepoGroup {
                name: "app".into(),
                has_focus: true,
                panes: vec![(test_pane("%1"), PaneGitInfo::default())], // Running
            },
            RepoGroup {
                name: "lib".into(),
                has_focus: false,
                panes: vec![(idle_pane, PaneGitInfo::default())], // Idle
            },
        ];

        // All repos: 2 total
        state.global.repo_filter = RepoFilter::All;
        let (all, running, _, idle, _) = state.status_counts();
        assert_eq!(all, 2);
        assert_eq!(running, 1);
        assert_eq!(idle, 1);

        // Filter to "app" only: 1 Running
        state.global.repo_filter = RepoFilter::Repo("app".into());
        let (all, running, _, idle, _) = state.status_counts();
        assert_eq!(all, 1);
        assert_eq!(running, 1);
        assert_eq!(idle, 0);
    }
}
