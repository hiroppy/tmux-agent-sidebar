pub mod bottom;
pub mod colors;
pub mod icons;
pub mod notices;
pub mod panes;
pub mod pet;
pub mod text;

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    state::{AppState, Focus},
    tmux,
};

pub const BOTTOM_PANEL_HEIGHT: u16 = 20;

/// Rows reserved between the pane list and the bottom panel when the pet is
/// enabled. The pet and its desk/chair all render inside this band so they
/// never overdraw the pane list above or the bottom panel's border below.
pub const PET_SCENE_HEIGHT: u16 = 5;

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

/// Read `@sidebar_pet` from tmux global options, defaulting to `false` (off).
/// Accepts `on`/`off`, `true`/`false`, `1`/`0` (case-insensitive).
pub fn pet_enabled_from_options(opts: &HashMap<String, String>) -> bool {
    opts.get(tmux::SIDEBAR_PET)
        .map(|s| s.trim().to_ascii_lowercase())
        .map(|s| matches!(s.as_str(), "on" | "true" | "1" | "yes"))
        .unwrap_or(false)
}

pub fn pet_enabled_from_tmux() -> bool {
    let opts = crate::tmux::get_all_global_options();
    pet_enabled_from_options(&opts)
}

// ── public entry point ──────────────────────────────────────────────

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    state.layout.hyperlink_overlays.clear();
    state.layout.bottom_toggle_target = None;
    let area = frame.area();
    // Cache the viewport height so keyboard handlers (which lack a terminal
    // handle) can resolve bottom-panel visibility for toggles and focus moves.
    state.last_viewport_height = area.height;

    // Resolve the compact/tall regime, then let a threshold crossing win over
    // any stale manual toggle before reading the final visibility.
    let (compact, _) = state.bottom_visibility(area.height);
    state.sync_bottom_regime(compact);
    let (compact, shown) = state.bottom_visibility(area.height);

    // Keep focus consistent with what is actually on screen. When the panel
    // is hidden, focus must not linger in the invisible Activity log; in the
    // compact accordion the list is hidden, so focus lives in the panel.
    if !shown {
        if state.focus_state.focus == Focus::ActivityLog {
            state.focus_state.focus = Focus::Panes;
        }
    } else if compact {
        state.focus_state.focus = Focus::ActivityLog;
    }

    // Compact window: the list and a fixed panel can't share the height, so
    // reserve the last row for a drawer handle and give the rest to either the
    // revealed accordion or the list. The handle is the discoverable, clickable
    // affordance for the otherwise keyboard-only `b` toggle.
    if compact {
        let handle_h = 1u16.min(area.height);
        let body_area = Rect {
            height: area.height.saturating_sub(handle_h),
            ..area
        };
        let handle_area = Rect {
            y: area.y + area.height.saturating_sub(handle_h),
            height: handle_h,
            ..area
        };
        if shown {
            bottom::draw_bottom(frame, state, body_area);
        } else {
            panes::draw_agents(frame, state, body_area);
        }
        draw_panel_handle(frame, state, handle_area, shown);
        state.layout.bottom_toggle_target = Some(handle_area);
        // Keep an open popup on top. In the hidden state `draw_agents` already
        // rendered it into `body_area` (above the handle row, so the handle
        // can't cover it); the accordion draws the bottom panel instead, so
        // render the overlay here — last, over `body_area` — to match.
        if shown {
            panes::render_popups(frame, state, body_area);
        }
        return;
    }

    // Panel disabled (`@sidebar_bottom_height` = 0): the list gets the full
    // height and there is no handle. (A tall window with the panel enabled is
    // always `shown`, so this only fires when the panel is turned off.)
    if !shown {
        panes::draw_agents(frame, state, area);
        return;
    }

    // Tall window: list + divider + fixed bottom panel, always shown.
    let bot_h = state.bottom_panel_height;
    let divider_h = if state.pet_enabled {
        PET_SCENE_HEIGHT
    } else {
        1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Min(1),
            Constraint::Length(divider_h),
            Constraint::Length(bot_h),
        ])
        .split(area);

    panes::draw_agents(frame, state, chunks[0]);
    bottom::draw_bottom(frame, state, chunks[2]);
    if state.pet_enabled {
        let running_count = state.running_count();
        pet::draw_pet(frame, state, chunks[1], running_count);
    }
}

/// Draw the one-row drawer handle that toggles the bottom panel on a compact
/// window. It doubles as the click target (the whole row) and the discoverable
/// hint for the `b` shortcut. `expanded` is the panel's current visibility: a
/// revealed accordion shows `▾ Activity / Git` (click to hide), a hidden panel
/// shows `▸ Activity / Git` (click to reveal).
fn draw_panel_handle(frame: &mut Frame, state: &AppState, area: Rect, expanded: bool) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let theme = &state.theme;
    let marker = if expanded { "▾" } else { "▸" };
    let label = "Activity / Git";
    let key_hint = "[b]";

    let left_dw = 1 + text::display_width(marker) + 1 + text::display_width(label);
    let gap = (area.width as usize).saturating_sub(left_dw + text::display_width(key_hint) + 1);

    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(marker.to_string(), Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::styled(label.to_string(), Style::default().fg(theme.text_muted)),
        Span::raw(" ".repeat(gap)),
        Span::styled(
            key_hint.to_string(),
            Style::default().fg(theme.border_inactive),
        ),
        Span::raw(" "),
    ]);
    frame.render_widget(Paragraph::new(line), area);
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
    fn pet_defaults_off_when_option_missing() {
        let opts = HashMap::new();
        assert!(!pet_enabled_from_options(&opts));
    }

    #[test]
    fn pet_enabled_when_on() {
        for value in ["on", "ON", "true", "1", "yes"] {
            let opts = opts_with(tmux::SIDEBAR_PET, value);
            assert!(
                pet_enabled_from_options(&opts),
                "expected {value} to enable"
            );
        }
    }

    #[test]
    fn pet_disabled_when_off() {
        for value in ["off", "false", "0", "no", ""] {
            let opts = opts_with(tmux::SIDEBAR_PET, value);
            assert!(
                !pet_enabled_from_options(&opts),
                "expected {value} to disable"
            );
        }
    }
}
