use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
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
    let mut needs_redraw = false;
    match ev {
        Event::Key(key) if state.is_notices_popup_open() => {
            needs_redraw = true;
            if key.code == KeyCode::Esc {
                state.close_notices_popup();
            }
        }
        Event::Key(key) if state.is_spawn_input_open() => {
            needs_redraw = true;
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
        }
        Event::Key(key) if state.is_remove_confirm_open() => {
            needs_redraw = true;
            match key.code {
                KeyCode::Esc | KeyCode::Char('n') => state.close_remove_confirm(),
                KeyCode::Char('c') => state.confirm_remove(RemoveMode::WindowOnly),
                KeyCode::Enter | KeyCode::Char('y') => {
                    state.confirm_remove(RemoveMode::WindowAndWorktree)
                }
                _ => {}
            }
        }
        Event::Key(key) if state.is_repo_popup_open() => {
            needs_redraw = true;
            match key.code {
                KeyCode::Esc => state.close_repo_popup(),
                KeyCode::Char('j') | KeyCode::Down => {
                    let count = state.repo_names().len();
                    let current = state.repo_popup_selected();
                    if current + 1 < count {
                        state.set_repo_popup_selected(current + 1);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let current = state.repo_popup_selected();
                    if current > 0 {
                        state.set_repo_popup_selected(current - 1);
                    }
                }
                KeyCode::Enter => state.confirm_repo_popup(),
                _ => {}
            }
        }
        Event::Key(key) => {
            needs_redraw = true;
            match key.code {
                KeyCode::Esc => {
                    if state.focus_state.focus == Focus::ActivityLog
                        || state.focus_state.focus == Focus::Filter
                    {
                        state.focus_state.focus = Focus::Panes;
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => match state.focus_state.focus {
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
                },
                KeyCode::Char('k') | KeyCode::Up => match state.focus_state.focus {
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
                },
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
                    git_tab_active
                        .store(state.bottom_tab == BottomTab::GitStatus, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        Event::Mouse(mouse) => {
            needs_redraw = true;
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
        }
        _ => {}
    }
    needs_redraw
}
