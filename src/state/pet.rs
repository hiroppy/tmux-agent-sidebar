//! Pet animation state transitions driven by the per-spinner tick.
//! Holds the `tick_pet` state machine (Idle → WalkRight → Working →
//! WalkLeft → Idle) plus the LCG-based reseed helpers that stagger the
//! idle-bob / walk-bounce / working-paper sub-animations so they don't
//! all fire in lockstep.

use super::AppState;

pub(crate) fn reseed_pet_idle_motion(state: &mut AppState) {
    const A: usize = 1664525;
    const C: usize = 1013904223;
    state.pet_idle_seed = state.pet_idle_seed.wrapping_mul(A).wrapping_add(C);
    let interval = crate::ui::pet::BOB_INTERVAL;
    let first_window = (interval / 3).max(1);
    let second_window = (interval / 3).max(1);
    state.pet_idle_jump_tick = 3 + (state.pet_idle_seed % first_window);
    state.pet_idle_blink_tick = (interval / 2) + ((state.pet_idle_seed / 7) % second_window);
    if state.pet_idle_blink_tick >= interval {
        state.pet_idle_blink_tick = interval.saturating_sub(1);
    }
    if state.pet_idle_blink_tick == state.pet_idle_jump_tick {
        state.pet_idle_blink_tick = (state.pet_idle_blink_tick + 1) % interval;
    }
    if state.pet_idle_blink_tick == state.pet_idle_jump_tick {
        state.pet_idle_blink_tick = (state.pet_idle_blink_tick + 2) % interval;
    }
    state.pet_idle_wave_enabled = (state.pet_idle_seed & 3) == 0;
    state.pet_idle_wave_tick = if state.pet_idle_wave_enabled {
        16 + ((state.pet_idle_seed / 11) % 4)
    } else {
        0
    };
}

pub(crate) fn reseed_pet_working_paper_motion(state: &mut AppState) {
    const A: usize = 1103515245;
    const C: usize = 12345;
    state.pet_working_paper_seed = state.pet_working_paper_seed.wrapping_mul(A).wrapping_add(C);
    let delay = 10 + (state.pet_working_paper_seed % 18);
    state.pet_working_paper_next_lift_tick = state.pet_working_paper_timer + delay;
}

pub(crate) fn reseed_pet_walk_bounce(state: &mut AppState) {
    const A: usize = 1664525;
    const C: usize = 1013904223;
    state.pet_walk_seed = state.pet_walk_seed.wrapping_mul(A).wrapping_add(C);
    let delay = 3 + (state.pet_walk_seed % 5);
    state.pet_walk_bounce_next_tick = state.pet_walk_tick + delay;
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

    /// Advance pet animation state. Called every spinner tick (200ms).
    pub fn tick_pet(&mut self, panel_width: u16) {
        let running_count = self.running_count();

        // Pet stops so the seated sprite leaves one column before the desk.
        let working_width = crate::ui::pet::CHAIR_WIDTH + 3;
        let stop_x = panel_width.saturating_sub(
            crate::ui::pet::DESK_OFFSET + crate::ui::pet::DESK_WIDTH + working_width,
        );

        fn walk_step(distance: u16) -> u16 {
            if distance > 8 { 2 } else { 1 }
        }

        match self.pet_state {
            crate::ui::pet::PetState::Idle => {
                if running_count > 0 {
                    self.pet_state = crate::ui::pet::PetState::WalkRight;
                    self.pet_frame = 0;
                    self.pet_working_frame_tick = 0;
                    self.pet_walk_tick = 0;
                    self.pet_walk_seed = 1;
                    self.pet_walk_bounce_next_tick = 0;
                    self.pet_walk_bounce_lift_until = 0;
                    self.pet_x = self.pet_x.saturating_add(1);
                } else {
                    self.pet_bob_timer = (self.pet_bob_timer + 1) % crate::ui::pet::BOB_INTERVAL;
                    if self.pet_bob_timer == 0 {
                        reseed_pet_idle_motion(self);
                    }
                }
            }
            crate::ui::pet::PetState::WalkRight => {
                if running_count == 0 {
                    self.pet_state = crate::ui::pet::PetState::WalkLeft;
                    self.pet_frame = 0;
                    self.pet_working_frame_tick = 0;
                    self.pet_walk_bounce_next_tick = 0;
                    self.pet_walk_bounce_lift_until = 0;
                    return;
                }
                let remaining = stop_x.saturating_sub(self.pet_x);
                let step = walk_step(remaining);
                self.pet_x = self.pet_x.saturating_add(step);
                self.pet_walk_tick = self.pet_walk_tick.saturating_add(1);
                if self.pet_walk_bounce_next_tick == 0 {
                    reseed_pet_walk_bounce(self);
                }
                if self.pet_walk_tick >= self.pet_walk_bounce_next_tick {
                    self.pet_walk_bounce_lift_until = self.pet_walk_tick + 2;
                    reseed_pet_walk_bounce(self);
                }
                self.pet_frame = match self.pet_frame {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                };
                if self.pet_x >= stop_x {
                    self.pet_x = stop_x;
                    self.pet_state = crate::ui::pet::PetState::Working;
                    self.pet_frame = 0;
                    self.pet_working_frame_tick = 0;
                    self.pet_walk_tick = 0;
                    self.pet_walk_seed = 1;
                    self.pet_walk_bounce_next_tick = 0;
                    self.pet_walk_bounce_lift_until = 0;
                    self.pet_working_paper_timer = 0;
                    reseed_pet_working_paper_motion(self);
                }
            }
            crate::ui::pet::PetState::Working => {
                if self.pet_working_paper_next_lift_tick == 0 {
                    reseed_pet_working_paper_motion(self);
                }
                self.pet_working_paper_timer = self.pet_working_paper_timer.saturating_add(1);
                if self.pet_working_paper_timer >= self.pet_working_paper_next_lift_tick {
                    self.pet_working_paper_lift_until = self.pet_working_paper_timer + 2;
                    self.pet_working_paper_x_offset = (self.pet_working_paper_seed & 1) as u16;
                    reseed_pet_working_paper_motion(self);
                }
                self.pet_working_frame_tick = self.pet_working_frame_tick.saturating_add(1);
                if self.pet_working_frame_tick.is_multiple_of(2) {
                    self.pet_frame = match self.pet_frame {
                        1 => 2,
                        2 => 3,
                        _ => 1,
                    };
                }
                if running_count == 0 {
                    self.pet_state = crate::ui::pet::PetState::WalkLeft;
                    self.pet_frame = 0;
                    self.pet_working_frame_tick = 0;
                    self.pet_walk_tick = 0;
                    self.pet_walk_seed = 1;
                    self.pet_walk_bounce_next_tick = 0;
                    self.pet_walk_bounce_lift_until = 0;
                    self.pet_working_paper_timer = 0;
                    self.pet_working_paper_next_lift_tick = 0;
                    self.pet_working_paper_lift_until = 0;
                    self.pet_working_paper_x_offset = 0;
                    self.pet_working_paper_seed = 1;
                }
            }
            crate::ui::pet::PetState::WalkLeft => {
                if running_count > 0 {
                    self.pet_state = crate::ui::pet::PetState::WalkRight;
                    self.pet_frame = 0;
                    self.pet_working_frame_tick = 0;
                    self.pet_walk_bounce_next_tick = 0;
                    self.pet_walk_bounce_lift_until = 0;
                    return;
                }
                let remaining = self.pet_x.saturating_sub(crate::ui::pet::PET_HOME_X);
                let step = walk_step(remaining);
                self.pet_x = self.pet_x.saturating_sub(step);
                self.pet_walk_tick = self.pet_walk_tick.saturating_add(1);
                if self.pet_walk_bounce_next_tick == 0 {
                    reseed_pet_walk_bounce(self);
                }
                if self.pet_walk_tick >= self.pet_walk_bounce_next_tick {
                    self.pet_walk_bounce_lift_until = self.pet_walk_tick + 2;
                    reseed_pet_walk_bounce(self);
                }
                self.pet_frame = match self.pet_frame {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                };
                if self.pet_x <= crate::ui::pet::PET_HOME_X {
                    self.pet_x = crate::ui::pet::PET_HOME_X;
                    self.pet_state = crate::ui::pet::PetState::Idle;
                    self.pet_frame = 0;
                    self.pet_working_frame_tick = 0;
                    self.pet_walk_tick = 0;
                    self.pet_walk_seed = 1;
                    self.pet_walk_bounce_next_tick = 0;
                    self.pet_walk_bounce_lift_until = 0;
                    self.pet_bob_timer = 0;
                    self.pet_working_paper_timer = 0;
                    self.pet_working_paper_next_lift_tick = 0;
                    self.pet_working_paper_lift_until = 0;
                    self.pet_working_paper_x_offset = 0;
                    reseed_pet_idle_motion(self);
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
    fn pet_state_defaults() {
        let state = AppState::new("%0".into());
        assert!(matches!(state.pet_state, crate::ui::pet::PetState::Idle));
        assert_eq!(state.pet_x, crate::ui::pet::PET_HOME_X);
        assert_eq!(state.pet_frame, 0);
        assert_eq!(state.pet_working_frame_tick, 0);
        assert_eq!(state.pet_bob_timer, 0);
        assert_eq!(state.pet_walk_bounce_next_tick, 0);
        assert_eq!(state.pet_walk_bounce_lift_until, 0);
        assert_eq!(state.pet_working_paper_timer, 0);
        assert_eq!(state.pet_working_paper_next_lift_tick, 0);
        assert_eq!(state.pet_working_paper_lift_until, 0);
        assert_eq!(state.pet_working_paper_x_offset, 0);
        assert!(state.pet_idle_jump_tick < crate::ui::pet::BOB_INTERVAL);
        assert!(state.pet_idle_blink_tick < crate::ui::pet::BOB_INTERVAL);
        assert_ne!(state.pet_idle_jump_tick, state.pet_idle_blink_tick);
        assert!(state.pet_idle_wave_tick < crate::ui::pet::BOB_INTERVAL);
        if state.pet_idle_wave_enabled {
            assert!((16..=19).contains(&state.pet_idle_wave_tick));
        } else {
            assert_eq!(state.pet_idle_wave_tick, 0);
        }
    }

    #[test]
    fn tick_pet_idle_to_walk_right_on_running() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.tick_pet(60);
        assert!(matches!(
            state.pet_state,
            crate::ui::pet::PetState::WalkRight
        ));
        assert!(state.pet_x > crate::ui::pet::PET_HOME_X);
    }

    #[test]
    fn tick_pet_walk_right_to_working_at_desk() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        let panel_width = 60u16;
        let working_width = crate::ui::pet::CHAIR_WIDTH + 3;
        let stop_x = panel_width.saturating_sub(
            crate::ui::pet::DESK_OFFSET + crate::ui::pet::DESK_WIDTH + working_width,
        );
        state.pet_state = crate::ui::pet::PetState::WalkRight;
        state.pet_x = stop_x - 1;
        state.tick_pet(panel_width);
        assert!(matches!(state.pet_state, crate::ui::pet::PetState::Working));
    }

    #[test]
    fn tick_pet_walk_right_schedules_bounce() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.pet_state = crate::ui::pet::PetState::WalkRight;
        state.pet_x = 20;

        state.tick_pet(60);

        assert!(state.pet_walk_bounce_next_tick > 0);
        assert!(state.pet_walk_bounce_next_tick > state.pet_walk_tick);
    }

    #[test]
    fn tick_pet_walk_right_returns_to_walk_left_when_running_stops() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.pet_state = crate::ui::pet::PetState::WalkRight;
        state.pet_x = 20;

        state.tick_pet(60);

        assert!(matches!(
            state.pet_state,
            crate::ui::pet::PetState::WalkLeft
        ));
        assert_eq!(state.pet_frame, 0);
        assert_eq!(state.pet_x, 20);
    }

    #[test]
    fn tick_pet_working_to_walk_left_when_no_running() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Idle;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.pet_state = crate::ui::pet::PetState::Working;
        state.pet_x = 40;
        state.tick_pet(60);
        assert!(matches!(
            state.pet_state,
            crate::ui::pet::PetState::WalkLeft
        ));
    }

    #[test]
    fn tick_pet_working_holds_hand_frame_for_two_ticks() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.pet_state = crate::ui::pet::PetState::Working;
        state.pet_x = 40;
        state.pet_frame = 1;

        state.tick_pet(60);
        assert_eq!(state.pet_frame, 1);

        state.tick_pet(60);
        assert_eq!(state.pet_frame, 2);
    }

    #[test]
    fn tick_pet_walk_left_returns_to_walk_right_when_running_resumes() {
        let mut state = AppState::new("%0".into());
        let mut pane = test_pane("1");
        pane.status = PaneStatus::Running;
        state.repo_groups = vec![RepoGroup {
            name: "repo".into(),
            has_focus: false,
            panes: vec![(pane, PaneGitInfo::default())],
        }];
        state.pet_state = crate::ui::pet::PetState::WalkLeft;
        state.pet_x = 20;

        state.tick_pet(60);

        assert!(matches!(
            state.pet_state,
            crate::ui::pet::PetState::WalkRight
        ));
        assert_eq!(state.pet_frame, 0);
        assert_eq!(state.pet_x, 20);
    }

    #[test]
    fn tick_pet_walk_left_to_idle_at_home() {
        let mut state = AppState::new("%0".into());
        state.pet_state = crate::ui::pet::PetState::WalkLeft;
        state.pet_x = crate::ui::pet::PET_HOME_X + 1;
        state.tick_pet(60);
        assert_eq!(state.pet_x, crate::ui::pet::PET_HOME_X);
        state.tick_pet(60);
        assert!(matches!(state.pet_state, crate::ui::pet::PetState::Idle));
    }

    #[test]
    fn tick_pet_idle_bob() {
        let mut state = AppState::new("%0".into());
        for _ in 0..crate::ui::pet::BOB_INTERVAL {
            state.tick_pet(60);
        }
        assert_eq!(state.pet_bob_timer, 0);
    }
}
