use ratatui::Frame;
use ratatui::layout::Rect;

use crate::state::AppState;

pub(super) fn render_if_open(frame: &mut Frame, state: &mut AppState, area: Rect) {
    if state.is_notices_popup_open() {
        crate::ui::notices::render_notices_popup(frame, state, area);
    } else if state.is_repo_popup_open() {
        super::render_repo_popup(frame, state, area);
    } else if state.is_spawn_input_open() {
        super::render_spawn_input_popup(frame, state, area);
    } else if state.is_remove_confirm_open() {
        super::render_remove_confirm_popup(frame, state, area);
    }
}
