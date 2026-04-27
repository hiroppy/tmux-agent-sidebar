pub mod bottom;
pub mod colors;
pub mod icons;
pub mod mascot;
pub mod notices;
pub mod panes;
pub mod text;

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::{state::AppState, tmux};

pub const BOTTOM_PANEL_HEIGHT: u16 = 20;

/// Rows reserved between the pane list and the bottom panel when the mascot is
/// enabled. The mascot and its desk/chair all render inside this band so they
/// never overdraw the pane list above or the bottom panel's border below.
pub const MASCOT_SCENE_HEIGHT: u16 = 5;

/// Read `@sidebar_bottom_height` from tmux global options, falling back to the default.
/// A value of 0 hides the bottom panel entirely.
pub fn bottom_panel_height_from_options(opts: &HashMap<String, String>) -> u16 {
    opts.get(tmux::SIDEBAR_BOTTOM_HEIGHT)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(BOTTOM_PANEL_HEIGHT)
}

pub fn bottom_panel_height_from_tmux() -> u16 {
    let opts = tmux::get_all_global_options();
    bottom_panel_height_from_options(&opts)
}

/// Read `@sidebar_mascot` from tmux global options, defaulting to `false` (off).
/// Accepts `on`/`off`, `true`/`false`, `1`/`0` (case-insensitive).
pub fn mascot_enabled_from_options(opts: &HashMap<String, String>) -> bool {
    opts.get("@sidebar_mascot")
        .map(|s| s.trim().to_ascii_lowercase())
        .map(|s| matches!(s.as_str(), "on" | "true" | "1" | "yes"))
        .unwrap_or(false)
}

pub fn mascot_enabled_from_tmux() -> bool {
    let opts = crate::tmux::get_all_global_options();
    mascot_enabled_from_options(&opts)
}

// ── public entry point ──────────────────────────────────────────────

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    state.layout.hyperlink_overlays.clear();
    let area = frame.area();

    let bot_h = state.bottom_panel_height;
    let divider_h = if bot_h > 0 && state.mascot_enabled {
        MASCOT_SCENE_HEIGHT
    } else {
        1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if bot_h > 0 {
            vec![
                Constraint::Min(1),
                Constraint::Length(divider_h),
                Constraint::Length(bot_h),
            ]
        } else {
            vec![Constraint::Min(1)]
        })
        .split(area);

    panes::draw_agents(frame, state, chunks[0]);

    if bot_h > 0 && chunks.len() > 2 {
        bottom::draw_bottom(frame, state, chunks[2]);
        if state.mascot_enabled {
            let running_count = state.running_count();
            mascot::draw_mascot(frame, state, chunks[1], running_count);
        }
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
        let opts = opts_with(tmux::SIDEBAR_BOTTOM_HEIGHT, "12");
        assert_eq!(bottom_panel_height_from_options(&opts), 12);
    }

    #[test]
    fn bottom_height_trims_whitespace() {
        let opts = opts_with(tmux::SIDEBAR_BOTTOM_HEIGHT, "  8  ");
        assert_eq!(bottom_panel_height_from_options(&opts), 8);
    }

    #[test]
    fn bottom_height_zero_hides_panel() {
        let opts = opts_with(tmux::SIDEBAR_BOTTOM_HEIGHT, "0");
        assert_eq!(bottom_panel_height_from_options(&opts), 0);
    }

    #[test]
    fn bottom_height_falls_back_on_invalid_value() {
        let opts = opts_with(tmux::SIDEBAR_BOTTOM_HEIGHT, "abc");
        assert_eq!(bottom_panel_height_from_options(&opts), BOTTOM_PANEL_HEIGHT);
    }

    #[test]
    fn bottom_height_falls_back_on_empty_value() {
        let opts = opts_with(tmux::SIDEBAR_BOTTOM_HEIGHT, "");
        assert_eq!(bottom_panel_height_from_options(&opts), BOTTOM_PANEL_HEIGHT);
    }

    #[test]
    fn mascot_defaults_off_when_option_missing() {
        let opts = HashMap::new();
        assert!(!mascot_enabled_from_options(&opts));
    }

    #[test]
    fn mascot_enabled_when_on() {
        for value in ["on", "ON", "true", "1", "yes"] {
            let opts = opts_with("@sidebar_mascot", value);
            assert!(
                mascot_enabled_from_options(&opts),
                "expected {value} to enable"
            );
        }
    }

    #[test]
    fn mascot_disabled_when_off() {
        for value in ["off", "false", "0", "no", ""] {
            let opts = opts_with("@sidebar_mascot", value);
            assert!(
                !mascot_enabled_from_options(&opts),
                "expected {value} to disable"
            );
        }
    }
}
