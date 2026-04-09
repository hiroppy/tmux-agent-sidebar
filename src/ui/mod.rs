pub mod panes;
pub mod bottom;
pub mod cat;
pub mod colors;
pub mod icons;
pub mod text;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::state::AppState;

pub const BOTTOM_PANEL_HEIGHT: u16 = 20;

// ── public entry point ──────────────────────────────────────────────

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    state.hyperlink_overlays.clear();
    let area = frame.area();

    let bot_h = BOTTOM_PANEL_HEIGHT;

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
