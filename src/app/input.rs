use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::state::{AppState, BottomTab, Focus};
use crate::worktree::RemoveMode;

/// Dispatch a single crossterm [`Event`] into the [`AppState`], returning
/// `true` when a redraw should be scheduled.
///
/// The terminal handle is only borrowed to query its size for mouse
/// coordinate conversion; it is never written to from here.
pub(super) fn handle_event(
    ev: Event,
    state: &mut AppState,
    git_tab_active: &AtomicBool,
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
) -> bool {
    match ev {
        Event::Key(key) => handle_key_event(key, state, git_tab_active),
        Event::Mouse(mouse) => {
            if handle_ask_mouse_event(mouse, state) {
                return true;
            }
            let term_height = terminal.size().map(|s| s.height).unwrap_or(0);
            let bottom_h = state.bottom_panel_height;
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let bottom_start = term_height.saturating_sub(bottom_h);
                    if mouse.row < bottom_start {
                        state.handle_mouse_click(mouse.row, mouse.column);
                    } else if mouse.row == bottom_start {
                        state.handle_bottom_tab_click(mouse.column);
                        // Keep the background git poller in sync immediately — the
                        // keyboard `BackTab` path does the same update. Without this,
                        // clicking into Git Status leaves polling disabled until the
                        // next refresh tick and the tab renders stale data.
                        git_tab_active
                            .store(state.bottom_tab == BottomTab::GitStatus, Ordering::Relaxed);
                    }
                }
                MouseEventKind::ScrollDown => {
                    state.handle_mouse_scroll(mouse.row, term_height, bottom_h, 3);
                }
                MouseEventKind::ScrollUp => {
                    state.handle_mouse_scroll(mouse.row, term_height, bottom_h, -3);
                }
                _ => {}
            }
            true
        }
        _ => false,
    }
}

fn handle_ask_mouse_event(mouse: crossterm::event::MouseEvent, state: &mut AppState) -> bool {
    if !state.is_ask_popup_open() {
        return false;
    }
    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        state.handle_mouse_click(mouse.row, mouse.column);
    }
    true
}

/// Dispatch a single [`KeyEvent`]. Split out from [`handle_event`] so that
/// unit tests can drive the keyboard path without constructing a real
/// terminal handle (the [`Terminal`] argument is only needed for mouse
/// coordinate conversion).
pub(super) fn handle_key_event(
    key: KeyEvent,
    state: &mut AppState,
    git_tab_active: &AtomicBool,
) -> bool {
    if state.is_ask_popup_open() {
        match key.code {
            KeyCode::Esc => state.close_ask_popup(),
            KeyCode::Tab => state.ask_scope_next(),
            KeyCode::BackTab => state.ask_scope_prev(),
            KeyCode::Backspace => state.ask_input_pop_char(),
            KeyCode::Char(c) => state.ask_input_push_char(c),
            _ => {}
        }
        return true;
    }
    if state.is_notices_popup_open() {
        if key.code == KeyCode::Esc {
            state.close_notices_popup();
        }
        return true;
    }
    if state.is_spawn_input_open() {
        match key.code {
            KeyCode::Esc => state.close_spawn_input(),
            KeyCode::Enter => state.confirm_spawn_input(),
            KeyCode::Tab | KeyCode::Down => state.spawn_input_next_field(),
            KeyCode::BackTab | KeyCode::Up => state.spawn_input_prev_field(),
            KeyCode::Left => state.spawn_input_cycle(-1),
            KeyCode::Right => state.spawn_input_cycle(1),
            KeyCode::Backspace => state.spawn_input_pop_char(),
            KeyCode::Char(c) => state.spawn_input_push_char(c),
            _ => {}
        }
        return true;
    }
    if state.is_remove_confirm_open() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') => state.close_remove_confirm(),
            KeyCode::Char('c') => state.confirm_remove(RemoveMode::WindowOnly),
            KeyCode::Enter | KeyCode::Char('y') => {
                state.confirm_remove(RemoveMode::WindowAndWorktree)
            }
            _ => {}
        }
        return true;
    }
    if state.is_repo_popup_open() {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => state.close_repo_popup(),
            KeyCode::Char('j') | KeyCode::Down => repo_popup_nav_down(state),
            KeyCode::Char('n') if ctrl => repo_popup_nav_down(state),
            KeyCode::Char('k') | KeyCode::Up => repo_popup_nav_up(state),
            KeyCode::Char('p') if ctrl => repo_popup_nav_up(state),
            KeyCode::Enter => state.confirm_repo_popup(),
            _ => {}
        }
        return true;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => {
            if state.focus_state.focus == Focus::ActivityLog
                || state.focus_state.focus == Focus::Filter
            {
                state.focus_state.focus = Focus::Panes;
            }
        }
        KeyCode::Char('j') | KeyCode::Down => pane_nav_down(state),
        KeyCode::Char('n') if ctrl => pane_nav_down(state),
        KeyCode::Char('k') | KeyCode::Up => pane_nav_up(state),
        KeyCode::Char('p') if ctrl => pane_nav_up(state),
        KeyCode::Char('h') | KeyCode::Left => {
            if state.focus_state.focus == Focus::Filter {
                state.global.status_filter = state.global.status_filter.prev();
                state.global.save_filter();
                state.rebuild_row_targets();
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if state.focus_state.focus == Focus::Filter {
                state.global.status_filter = state.global.status_filter.next();
                state.global.save_filter();
                state.rebuild_row_targets();
            }
        }
        KeyCode::Char('r') => {
            if state.focus_state.focus == Focus::Filter {
                state.toggle_repo_popup();
            }
        }
        KeyCode::Char('n') => {
            if state.focus_state.focus == Focus::Panes {
                state.open_spawn_input_from_selection();
            }
        }
        KeyCode::Char('x') => {
            if state.focus_state.focus == Focus::Panes {
                state.open_remove_confirm();
            }
        }
        KeyCode::Char('a') => {
            if state.focus_state.focus == Focus::Panes {
                state.open_ask_popup();
            }
        }
        KeyCode::Enter => {
            if state.focus_state.focus == Focus::Panes {
                state.activate_selected_pane();
            }
        }
        KeyCode::Tab => {
            state.global.status_filter = state.global.status_filter.next();
            state.global.save_filter();
            state.rebuild_row_targets();
        }
        KeyCode::BackTab => {
            state.next_bottom_tab();
            git_tab_active.store(state.bottom_tab == BottomTab::GitStatus, Ordering::Relaxed);
        }
        _ => {}
    }
    true
}

fn pane_nav_down(state: &mut AppState) {
    match state.focus_state.focus {
        Focus::Filter => {
            state.focus_state.focus = Focus::Panes;
        }
        Focus::Panes => {
            if state.move_pane_selection(1) {
                state.global.queue_cursor_save();
            } else {
                state.focus_state.focus = Focus::ActivityLog;
            }
        }
        Focus::ActivityLog => state.scroll_bottom(1),
    }
}

fn pane_nav_up(state: &mut AppState) {
    match state.focus_state.focus {
        Focus::Filter => {}
        Focus::Panes => {
            if state.move_pane_selection(-1) {
                state.global.queue_cursor_save();
            } else {
                state.focus_state.focus = Focus::Filter;
            }
        }
        Focus::ActivityLog => {
            let at_top = match state.bottom_tab {
                BottomTab::Activity => state.activity.scroll.offset == 0,
                BottomTab::GitStatus => state.scrolls.git.offset == 0,
            };
            if at_top {
                state.focus_state.focus = Focus::Panes;
            } else {
                state.scroll_bottom(-1);
            }
        }
    }
}

fn repo_popup_nav_down(state: &mut AppState) {
    let count = state.repo_names().len();
    let current = state.repo_popup_selected();
    if current + 1 < count {
        state.set_repo_popup_selected(current + 1);
    }
}

fn repo_popup_nav_up(state: &mut AppState) {
    let current = state.repo_popup_selected();
    if current > 0 {
        state.set_repo_popup_selected(current - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::RepoGroup;
    use crate::state::RowTarget;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// Build an AppState with three navigable pane rows and Panes focus,
    /// which is the precondition the navigation arms operate against.
    fn state_with_three_panes() -> AppState {
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
        state.focus_state.focus = Focus::Panes;
        state
    }

    fn state_with_repo_popup_open() -> AppState {
        let mut state = AppState::new("%99".into());
        // toggle_repo_popup uses repo_names(), which always includes the
        // "All" sentinel — pad with two named groups so the selection has
        // somewhere to move.
        state.repo_groups = vec![
            RepoGroup {
                name: "repo-a".into(),
                has_focus: false,
                panes: vec![],
            },
            RepoGroup {
                name: "repo-b".into(),
                has_focus: false,
                panes: vec![],
            },
        ];
        state.toggle_repo_popup();
        state.set_repo_popup_selected(0);
        state
    }

    #[test]
    fn ctrl_n_moves_pane_selection_down() {
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);
        handle_key_event(ctrl_key('n'), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 1);
        handle_key_event(ctrl_key('n'), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 2);
    }

    #[test]
    fn ctrl_p_moves_pane_selection_up() {
        let mut state = state_with_three_panes();
        state.global.selected_pane_row = 2;
        let flag = AtomicBool::new(false);
        handle_key_event(ctrl_key('p'), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 1);
        handle_key_event(ctrl_key('p'), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 0);
    }

    #[test]
    fn bare_j_and_k_still_navigate_panes() {
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);
        handle_key_event(key(KeyCode::Char('j')), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 1);
        handle_key_event(key(KeyCode::Char('k')), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 0);
    }

    #[test]
    fn bare_n_does_not_move_selection() {
        // The bare `n` arm is wired to the spawn input flow, not navigation.
        // We don't assert the popup opens (that requires repo_groups +
        // git metadata, exercised elsewhere) — only that it does NOT
        // shadow the Ctrl-N navigation arm.
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);
        handle_key_event(key(KeyCode::Char('n')), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 0);
    }

    #[test]
    fn bare_p_is_unbound_in_panes_focus() {
        let mut state = state_with_three_panes();
        state.global.selected_pane_row = 1;
        let flag = AtomicBool::new(false);
        handle_key_event(key(KeyCode::Char('p')), &mut state, &flag);
        assert_eq!(state.global.selected_pane_row, 1);
    }

    #[test]
    fn bare_a_opens_ask_popup_from_panes() {
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);

        handle_key_event(key(KeyCode::Char('a')), &mut state, &flag);

        assert!(state.is_ask_popup_open());
    }

    #[test]
    fn ask_popup_handles_editing_before_pane_navigation() {
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);
        state.open_ask_popup();

        handle_key_event(key(KeyCode::Char('j')), &mut state, &flag);
        handle_key_event(key(KeyCode::Backspace), &mut state, &flag);

        assert_eq!(state.global.selected_pane_row, 0);
        let crate::state::PopupState::Ask(ask) = &state.popup else {
            panic!("ask popup should remain open");
        };
        assert_eq!(ask.question, crate::state::DEFAULT_ASK_PROMPT);
    }

    #[test]
    fn escape_closes_ask_popup_without_changing_selection() {
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);
        state.global.selected_pane_row = 1;
        state.open_ask_popup();

        handle_key_event(key(KeyCode::Esc), &mut state, &flag);

        assert!(!state.is_ask_popup_open());
        assert_eq!(state.global.selected_pane_row, 1);
    }

    #[test]
    fn tab_cycles_ask_scope_without_editing_question() {
        let mut state = state_with_three_panes();
        let flag = AtomicBool::new(false);
        state.open_ask_popup();

        handle_key_event(key(KeyCode::Tab), &mut state, &flag);
        let crate::state::PopupState::Ask(ask) = &state.popup else {
            panic!("ask popup should remain open");
        };
        assert_eq!(ask.scope, crate::state::AskScope::Repo);
        assert_eq!(ask.question, crate::state::DEFAULT_ASK_PROMPT);

        handle_key_event(key(KeyCode::BackTab), &mut state, &flag);
        let crate::state::PopupState::Ask(ask) = &state.popup else {
            panic!("ask popup should remain open");
        };
        assert_eq!(ask.scope, crate::state::AskScope::Selected);
    }

    #[test]
    fn ask_popup_consumes_bottom_panel_mouse_events_before_tab_routing() {
        let mut state = state_with_three_panes();
        state.open_ask_popup();
        state
            .popup
            .set_ask_area(Some(ratatui::layout::Rect::new(5, 5, 20, 8)));
        let mouse = crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };

        assert!(handle_ask_mouse_event(mouse, &mut state));
        assert!(!state.is_ask_popup_open());
        assert_eq!(state.bottom_tab, BottomTab::Activity);
    }

    #[test]
    fn ctrl_n_navigates_repo_popup_down() {
        let mut state = state_with_repo_popup_open();
        let flag = AtomicBool::new(false);
        handle_key_event(ctrl_key('n'), &mut state, &flag);
        assert_eq!(state.repo_popup_selected(), 1);
        handle_key_event(ctrl_key('n'), &mut state, &flag);
        assert_eq!(state.repo_popup_selected(), 2);
        // Past the last entry the popup nav helper is a no-op.
        handle_key_event(ctrl_key('n'), &mut state, &flag);
        assert_eq!(state.repo_popup_selected(), 2);
    }

    #[test]
    fn ctrl_p_navigates_repo_popup_up() {
        let mut state = state_with_repo_popup_open();
        state.set_repo_popup_selected(2);
        let flag = AtomicBool::new(false);
        handle_key_event(ctrl_key('p'), &mut state, &flag);
        assert_eq!(state.repo_popup_selected(), 1);
        handle_key_event(ctrl_key('p'), &mut state, &flag);
        assert_eq!(state.repo_popup_selected(), 0);
        // Below 0 the popup nav helper is a no-op.
        handle_key_event(ctrl_key('p'), &mut state, &flag);
        assert_eq!(state.repo_popup_selected(), 0);
    }
}
