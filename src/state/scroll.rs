#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub total_lines: usize,
    pub visible_height: usize,
}

impl ScrollState {
    pub fn scroll(&mut self, delta: isize) {
        let max = self.total_lines.saturating_sub(self.visible_height);
        let next = self.offset as isize + delta;
        self.offset = next.max(0).min(max as isize) as usize;
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScrollStates {
    pub panes: ScrollState,
    pub git: ScrollState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_state_default_is_all_zero() {
        let state = ScrollState::default();
        assert_eq!(state.offset, 0);
        assert_eq!(state.total_lines, 0);
        assert_eq!(state.visible_height, 0);
    }

    #[test]
    fn scroll_forward_advances_within_bounds() {
        let mut state = ScrollState {
            offset: 0,
            total_lines: 20,
            visible_height: 5,
        };
        state.scroll(3);
        assert_eq!(state.offset, 3);
    }

    #[test]
    fn scroll_clamps_to_max_offset() {
        // max = total_lines - visible_height = 15
        let mut state = ScrollState {
            offset: 10,
            total_lines: 20,
            visible_height: 5,
        };
        state.scroll(100);
        assert_eq!(
            state.offset, 15,
            "offset should clamp to total_lines - visible_height"
        );
    }

    #[test]
    fn scroll_clamps_to_zero_on_negative_overshoot() {
        let mut state = ScrollState {
            offset: 2,
            total_lines: 20,
            visible_height: 5,
        };
        state.scroll(-50);
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn scroll_with_visible_height_exceeding_total_clamps_to_zero() {
        // total_lines.saturating_sub(visible_height) = 0, so any forward
        // scroll collapses back to 0 rather than underflowing.
        let mut state = ScrollState {
            offset: 0,
            total_lines: 3,
            visible_height: 10,
        };
        state.scroll(5);
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn scroll_states_default_both_at_default() {
        let states = ScrollStates::default();
        assert_eq!(states.panes.offset, 0);
        assert_eq!(states.panes.total_lines, 0);
        assert_eq!(states.panes.visible_height, 0);
        assert_eq!(states.git.offset, 0);
        assert_eq!(states.git.total_lines, 0);
        assert_eq!(states.git.visible_height, 0);
    }
}
