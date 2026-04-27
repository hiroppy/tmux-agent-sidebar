//! Mascot animation state transitions driven by the per-spinner tick.
//! Holds the `tick_mascot` state machine (Idle → WalkRight → Working →
//! WalkLeft → Idle) plus the LCG-based reseed helpers that stagger the
//! idle-bob / walk-bounce / working-paper sub-animations so they don't
//! all fire in lockstep.

use super::AppState;

pub(crate) fn reseed_mascot_idle_motion(state: &mut AppState) {
    const A: usize = 1664525;
    const C: usize = 1013904223;
    state.mascot_idle_seed = state.mascot_idle_seed.wrapping_mul(A).wrapping_add(C);
    let interval = crate::ui::mascot::BOB_INTERVAL;
    let first_window = (interval / 3).max(1);
    let second_window = (interval / 3).max(1);
    state.mascot_idle_jump_tick = 3 + (state.mascot_idle_seed % first_window);
    state.mascot_idle_blink_tick = (interval / 2) + ((state.mascot_idle_seed / 7) % second_window);
    if state.mascot_idle_blink_tick >= interval {
        state.mascot_idle_blink_tick = interval.saturating_sub(1);
    }
    if state.mascot_idle_blink_tick == state.mascot_idle_jump_tick {
        state.mascot_idle_blink_tick = (state.mascot_idle_blink_tick + 1) % interval;
    }
    if state.mascot_idle_blink_tick == state.mascot_idle_jump_tick {
        state.mascot_idle_blink_tick = (state.mascot_idle_blink_tick + 2) % interval;
    }
    state.mascot_idle_wave_enabled = (state.mascot_idle_seed & 3) == 0;
    state.mascot_idle_wave_tick = if state.mascot_idle_wave_enabled {
        16 + ((state.mascot_idle_seed / 11) % 4)
    } else {
        0
    };
}

pub(crate) fn reseed_mascot_working_paper_motion(state: &mut AppState) {
    const A: usize = 1103515245;
    const C: usize = 12345;
    state.mascot_working_paper_seed = state
        .mascot_working_paper_seed
        .wrapping_mul(A)
        .wrapping_add(C);
    let delay = 10 + (state.mascot_working_paper_seed % 18);
    state.mascot_working_paper_next_lift_tick = state.mascot_working_paper_timer + delay;
}

pub(crate) fn reseed_mascot_walk_bounce(state: &mut AppState) {
    const A: usize = 1664525;
    const C: usize = 1013904223;
    state.mascot_walk_seed = state.mascot_walk_seed.wrapping_mul(A).wrapping_add(C);
    let delay = 3 + (state.mascot_walk_seed % 5);
    state.mascot_walk_bounce_next_tick = state.mascot_walk_tick + delay;
}

impl AppState {
    /// Count the number of running agents across all repo groups.
    pub fn running_count(&self) -> usize {
        self.repo_groups
            .iter()
            .flat_map(|g| &g.panes)
            .filter(|(p, _)| p.status == crate::tmux::PaneStatus::Running)
            .count()
    }

    /// Advance mascot animation state. Called every spinner tick (200ms).
    pub fn tick_mascot(&mut self, panel_width: u16) {
        let running_count = self
            .repo_groups
            .iter()
            .flat_map(|g| &g.panes)
            .filter(|(p, _)| p.status == crate::tmux::PaneStatus::Running)
            .count();

        // Mascot stops so the seated sprite leaves one column before the desk.
        let working_width = crate::ui::mascot::CHAIR_WIDTH + 3;
        let stop_x = panel_width.saturating_sub(
            crate::ui::mascot::DESK_OFFSET + crate::ui::mascot::DESK_WIDTH + working_width,
        );

        fn walk_step(distance: u16) -> u16 {
            if distance > 8 { 2 } else { 1 }
        }

        match self.mascot_state {
            crate::ui::mascot::MascotState::Idle => {
                if running_count > 0 {
                    self.mascot_state = crate::ui::mascot::MascotState::WalkRight;
                    self.mascot_frame = 0;
                    self.mascot_working_frame_tick = 0;
                    self.mascot_walk_tick = 0;
                    self.mascot_walk_seed = 1;
                    self.mascot_walk_bounce_next_tick = 0;
                    self.mascot_walk_bounce_lift_until = 0;
                    self.mascot_x = self.mascot_x.saturating_add(1);
                } else {
                    self.mascot_bob_timer =
                        (self.mascot_bob_timer + 1) % crate::ui::mascot::BOB_INTERVAL;
                    if self.mascot_bob_timer == 0 {
                        reseed_mascot_idle_motion(self);
                    }
                }
            }
            crate::ui::mascot::MascotState::WalkRight => {
                if running_count == 0 {
                    self.mascot_state = crate::ui::mascot::MascotState::WalkLeft;
                    self.mascot_frame = 0;
                    self.mascot_working_frame_tick = 0;
                    self.mascot_walk_bounce_next_tick = 0;
                    self.mascot_walk_bounce_lift_until = 0;
                    return;
                }
                let remaining = stop_x.saturating_sub(self.mascot_x);
                let step = walk_step(remaining);
                self.mascot_x = self.mascot_x.saturating_add(step);
                self.mascot_walk_tick = self.mascot_walk_tick.saturating_add(1);
                if self.mascot_walk_bounce_next_tick == 0 {
                    reseed_mascot_walk_bounce(self);
                }
                if self.mascot_walk_tick >= self.mascot_walk_bounce_next_tick {
                    self.mascot_walk_bounce_lift_until = self.mascot_walk_tick + 2;
                    reseed_mascot_walk_bounce(self);
                }
                self.mascot_frame = match self.mascot_frame {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                };
                if self.mascot_x >= stop_x {
                    self.mascot_x = stop_x;
                    self.mascot_state = crate::ui::mascot::MascotState::Working;
                    self.mascot_frame = 0;
                    self.mascot_working_frame_tick = 0;
                    self.mascot_walk_tick = 0;
                    self.mascot_walk_seed = 1;
                    self.mascot_walk_bounce_next_tick = 0;
                    self.mascot_walk_bounce_lift_until = 0;
                    self.mascot_working_paper_timer = 0;
                    reseed_mascot_working_paper_motion(self);
                }
            }
            crate::ui::mascot::MascotState::Working => {
                if self.mascot_working_paper_next_lift_tick == 0 {
                    reseed_mascot_working_paper_motion(self);
                }
                self.mascot_working_paper_timer = self.mascot_working_paper_timer.saturating_add(1);
                if self.mascot_working_paper_timer >= self.mascot_working_paper_next_lift_tick {
                    self.mascot_working_paper_lift_until = self.mascot_working_paper_timer + 2;
                    self.mascot_working_paper_x_offset =
                        (self.mascot_working_paper_seed & 1) as u16;
                    reseed_mascot_working_paper_motion(self);
                }
                self.mascot_working_frame_tick = self.mascot_working_frame_tick.saturating_add(1);
                if self.mascot_working_frame_tick.is_multiple_of(2) {
                    self.mascot_frame = match self.mascot_frame {
                        1 => 2,
                        2 => 3,
                        _ => 1,
                    };
                }
                if running_count == 0 {
                    self.mascot_state = crate::ui::mascot::MascotState::WalkLeft;
                    self.mascot_frame = 0;
                    self.mascot_working_frame_tick = 0;
                    self.mascot_walk_tick = 0;
                    self.mascot_walk_seed = 1;
                    self.mascot_walk_bounce_next_tick = 0;
                    self.mascot_walk_bounce_lift_until = 0;
                    self.mascot_working_paper_timer = 0;
                    self.mascot_working_paper_next_lift_tick = 0;
                    self.mascot_working_paper_lift_until = 0;
                    self.mascot_working_paper_x_offset = 0;
                    self.mascot_working_paper_seed = 1;
                }
            }
            crate::ui::mascot::MascotState::WalkLeft => {
                if running_count > 0 {
                    self.mascot_state = crate::ui::mascot::MascotState::WalkRight;
                    self.mascot_frame = 0;
                    self.mascot_working_frame_tick = 0;
                    self.mascot_walk_bounce_next_tick = 0;
                    self.mascot_walk_bounce_lift_until = 0;
                    return;
                }
                let remaining = self
                    .mascot_x
                    .saturating_sub(crate::ui::mascot::MASCOT_HOME_X);
                let step = walk_step(remaining);
                self.mascot_x = self.mascot_x.saturating_sub(step);
                self.mascot_walk_tick = self.mascot_walk_tick.saturating_add(1);
                if self.mascot_walk_bounce_next_tick == 0 {
                    reseed_mascot_walk_bounce(self);
                }
                if self.mascot_walk_tick >= self.mascot_walk_bounce_next_tick {
                    self.mascot_walk_bounce_lift_until = self.mascot_walk_tick + 2;
                    reseed_mascot_walk_bounce(self);
                }
                self.mascot_frame = match self.mascot_frame {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                };
                if self.mascot_x <= crate::ui::mascot::MASCOT_HOME_X {
                    self.mascot_x = crate::ui::mascot::MASCOT_HOME_X;
                    self.mascot_state = crate::ui::mascot::MascotState::Idle;
                    self.mascot_frame = 0;
                    self.mascot_working_frame_tick = 0;
                    self.mascot_walk_tick = 0;
                    self.mascot_walk_seed = 1;
                    self.mascot_walk_bounce_next_tick = 0;
                    self.mascot_walk_bounce_lift_until = 0;
                    self.mascot_bob_timer = 0;
                    self.mascot_working_paper_timer = 0;
                    self.mascot_working_paper_next_lift_tick = 0;
                    self.mascot_working_paper_lift_until = 0;
                    self.mascot_working_paper_x_offset = 0;
                    reseed_mascot_idle_motion(self);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::{PaneGitInfo, RepoGroup};
    use crate::tmux::{AgentType, PaneInfo, PaneStatus, PermissionMode, WorktreeMetadata};

    fn test_pane(id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status: PaneStatus::Idle,
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
            bg_shell_cmd: None,
        }
    }

    #[test]
    fn mascot_state_defaults() {
        let state = AppState::new("%0".into());
        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::Idle
        ));
        assert_eq!(state.mascot_x, crate::ui::mascot::MASCOT_HOME_X);
        assert_eq!(state.mascot_frame, 0);
        assert_eq!(state.mascot_working_frame_tick, 0);
        assert_eq!(state.mascot_bob_timer, 0);
        assert_eq!(state.mascot_walk_bounce_next_tick, 0);
        assert_eq!(state.mascot_walk_bounce_lift_until, 0);
        assert_eq!(state.mascot_working_paper_timer, 0);
        assert_eq!(state.mascot_working_paper_next_lift_tick, 0);
        assert_eq!(state.mascot_working_paper_lift_until, 0);
        assert_eq!(state.mascot_working_paper_x_offset, 0);
        assert!(state.mascot_idle_jump_tick < crate::ui::mascot::BOB_INTERVAL);
        assert!(state.mascot_idle_blink_tick < crate::ui::mascot::BOB_INTERVAL);
        assert_ne!(state.mascot_idle_jump_tick, state.mascot_idle_blink_tick);
        assert!(state.mascot_idle_wave_tick < crate::ui::mascot::BOB_INTERVAL);
        if state.mascot_idle_wave_enabled {
            assert!((16..=19).contains(&state.mascot_idle_wave_tick));
        } else {
            assert_eq!(state.mascot_idle_wave_tick, 0);
        }
    }

    #[test]
    fn tick_mascot_idle_to_walk_right_on_running() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.tick_mascot(60);
        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::WalkRight
        ));
        assert!(state.mascot_x > crate::ui::mascot::MASCOT_HOME_X);
    }

    #[test]
    fn tick_mascot_walk_right_to_working_at_desk() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let panel_width = 60u16;
        let working_width = crate::ui::mascot::CHAIR_WIDTH + 3;
        let stop_x = panel_width.saturating_sub(
            crate::ui::mascot::DESK_OFFSET + crate::ui::mascot::DESK_WIDTH + working_width,
        );
        state.mascot_state = crate::ui::mascot::MascotState::WalkRight;
        state.mascot_x = stop_x - 1;
        state.tick_mascot(panel_width);
        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::Working
        ));
    }

    #[test]
    fn tick_mascot_walk_right_schedules_bounce() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.mascot_state = crate::ui::mascot::MascotState::WalkRight;
        state.mascot_x = 20;

        state.tick_mascot(60);

        assert!(state.mascot_walk_bounce_next_tick > 0);
        assert!(state.mascot_walk_bounce_next_tick > state.mascot_walk_tick);
    }

    #[test]
    fn tick_mascot_walk_right_returns_to_walk_left_when_running_stops() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.mascot_state = crate::ui::mascot::MascotState::WalkRight;
        state.mascot_x = 20;

        state.tick_mascot(60);

        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::WalkLeft
        ));
        assert_eq!(state.mascot_frame, 0);
        assert_eq!(state.mascot_x, 20);
    }

    #[test]
    fn tick_mascot_working_to_walk_left_when_no_running() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.mascot_state = crate::ui::mascot::MascotState::Working;
        state.mascot_x = 40;
        state.tick_mascot(60);
        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::WalkLeft
        ));
    }

    #[test]
    fn tick_mascot_working_holds_hand_frame_for_two_ticks() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.mascot_state = crate::ui::mascot::MascotState::Working;
        state.mascot_x = 40;
        state.mascot_frame = 1;

        state.tick_mascot(60);
        assert_eq!(state.mascot_frame, 1);

        state.tick_mascot(60);
        assert_eq!(state.mascot_frame, 2);
    }

    #[test]
    fn tick_mascot_walk_left_returns_to_walk_right_when_running_resumes() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.mascot_state = crate::ui::mascot::MascotState::WalkLeft;
        state.mascot_x = 20;

        state.tick_mascot(60);

        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::WalkRight
        ));
        assert_eq!(state.mascot_frame, 0);
        assert_eq!(state.mascot_x, 20);
    }

    #[test]
    fn tick_mascot_walk_left_to_idle_at_home() {
        let mut state = AppState::new("%0".into());
        state.mascot_state = crate::ui::mascot::MascotState::WalkLeft;
        state.mascot_x = crate::ui::mascot::MASCOT_HOME_X + 1;
        state.tick_mascot(60);
        assert_eq!(state.mascot_x, crate::ui::mascot::MASCOT_HOME_X);
        state.tick_mascot(60);
        assert!(matches!(
            state.mascot_state,
            crate::ui::mascot::MascotState::Idle
        ));
    }

    #[test]
    fn tick_mascot_idle_bob() {
        let mut state = AppState::new("%0".into());
        for _ in 0..crate::ui::mascot::BOB_INTERVAL {
            state.tick_mascot(60);
        }
        assert_eq!(state.mascot_bob_timer, 0);
    }
}
