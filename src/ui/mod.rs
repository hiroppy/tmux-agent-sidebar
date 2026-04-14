pub mod bottom;
pub mod colors;
pub mod icons;
pub mod notices;
pub mod panes;
pub mod text;

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::state::AppState;

pub const BOTTOM_PANEL_HEIGHT: u16 = 20;

/// Read `@sidebar_bottom_height` from tmux global options, falling back to the default.
/// A value of 0 hides the bottom panel entirely.
pub fn bottom_panel_height_from_options(opts: &HashMap<String, String>) -> u16 {
    opts.get("@sidebar_bottom_height")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(BOTTOM_PANEL_HEIGHT)
}

pub fn bottom_panel_height_from_tmux() -> u16 {
    let opts = crate::tmux::get_all_global_options();
    bottom_panel_height_from_options(&opts)
}

// ── public entry point ──────────────────────────────────────────────

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    state.layout.hyperlink_overlays.clear();
    let area = frame.area();

    let bot_h = state.bottom_panel_height;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if bot_h > 0 {
            vec![
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(bot_h),
            ]
        } else {
            vec![Constraint::Min(1)]
        })
        .split(area);

    panes::draw_agents(frame, state, chunks[0]);

    if bot_h > 0 && chunks.len() > 2 {
        bottom::draw_bottom(frame, state, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_with(key: &str, value: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(key.into(), value.into());
        m
    }

    #[test]
    fn bottom_height_defaults_when_option_missing() {
        let opts = HashMap::new();
        assert_eq!(bottom_panel_height_from_options(&opts), BOTTOM_PANEL_HEIGHT);
    }

    #[test]
    fn bottom_height_parses_valid_value() {
        let opts = opts_with("@sidebar_bottom_height", "12");
        assert_eq!(bottom_panel_height_from_options(&opts), 12);
    }

    #[test]
    fn bottom_height_trims_whitespace() {
        let opts = opts_with("@sidebar_bottom_height", "  8  ");
        assert_eq!(bottom_panel_height_from_options(&opts), 8);
    }

    #[test]
    fn bottom_height_zero_hides_panel() {
        let opts = opts_with("@sidebar_bottom_height", "0");
        assert_eq!(bottom_panel_height_from_options(&opts), 0);
    }

    #[test]
    fn bottom_height_falls_back_on_invalid_value() {
        let opts = opts_with("@sidebar_bottom_height", "abc");
        assert_eq!(bottom_panel_height_from_options(&opts), BOTTOM_PANEL_HEIGHT);
    }

    #[test]
    fn bottom_height_falls_back_on_empty_value() {
        let opts = opts_with("@sidebar_bottom_height", "");
        assert_eq!(bottom_panel_height_from_options(&opts), BOTTOM_PANEL_HEIGHT);
    }
}
