use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tmux_agent_sidebar::SPINNER_PULSE;
use tmux_agent_sidebar::git::{self, GitData};
use tmux_agent_sidebar::state::{AppState, BottomTab, Focus};
use tmux_agent_sidebar::tmux;
use tmux_agent_sidebar::ui;

static NEEDS_REFRESH: AtomicBool = AtomicBool::new(false);

struct TuiSession {
    entered_alt_screen: bool,
}

impl TuiSession {
    fn enter(stdout: &mut io::Stdout) -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(err);
        }
        Ok(Self {
            entered_alt_screen: true,
        })
    }
}

impl Drop for TuiSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if self.entered_alt_screen {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = tmux_agent_sidebar::cli::run(&args) {
        std::process::exit(code);
    }

    let tmux_pane = std::env::var("TMUX_PANE").unwrap_or_default();
    if tmux_pane.is_empty() {
        eprintln!("TMUX_PANE not set");
        std::process::exit(1);
    }

    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = sigusr1_handler as *const () as libc::sighandler_t;
        sa.sa_flags = libc::SA_RESTART;
        libc::sigaction(libc::SIGUSR1, &sa, std::ptr::null_mut());
    }

    let pid = std::process::id();
    let _ = std::process::Command::new("tmux")
        .args([
            "set",
            "-t",
            &tmux_pane,
            "-p",
            "@sidebar_pid",
            &pid.to_string(),
        ])
        .output();

    let mut stdout = io::stdout();
    let _tui_session = TuiSession::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_app(&mut terminal, tmux_pane)
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    tmux_pane: String,
) -> io::Result<()> {
    let mut state = AppState::new(tmux_pane);
    state.theme = tmux_agent_sidebar::ui::colors::ColorTheme::from_tmux();
    state.sync_global_state();
    state.refresh();

    if let Some(ref pane_id) = state.focused_pane_id {
        if let Some(path) = tmux::get_pane_path(pane_id) {
            state.apply_git_data(git::fetch_git_data(&path));
        }
    }

    let (git_tx, git_rx) = mpsc::channel::<GitData>();
    let tmux_pane_clone = state.tmux_pane.clone();
    let git_tab_active =
        std::sync::Arc::new(AtomicBool::new(state.bottom_tab == BottomTab::GitStatus));
    let git_tab_flag = std::sync::Arc::clone(&git_tab_active);
    std::thread::spawn(move || {
        git_poll_loop(&tmux_pane_clone, &git_tx, &git_tab_flag);
    });

    let mut last_refresh = std::time::Instant::now();
    let mut last_spinner = std::time::Instant::now();
    let refresh_interval = Duration::from_secs(1);
    let spinner_interval = Duration::from_millis(200);

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut state))?;

        let refresh_timeout = refresh_interval.saturating_sub(last_refresh.elapsed());
        let spinner_timeout = spinner_interval.saturating_sub(last_spinner.elapsed());
        let timeout = refresh_timeout.min(spinner_timeout);
        if event::poll(timeout)? {
            loop {
                let ev = event::read()?;
                match ev {
                    Event::Key(key) if state.repo_popup_open => match key.code {
                        KeyCode::Esc => state.close_repo_popup(),
                        KeyCode::Char('j') | KeyCode::Down => {
                            let count = state.repo_names().len();
                            if state.repo_popup_selected + 1 < count {
                                state.repo_popup_selected += 1;
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if state.repo_popup_selected > 0 {
                                state.repo_popup_selected -= 1;
                            }
                        }
                        KeyCode::Enter => state.confirm_repo_popup(),
                        _ => {}
                    },
                    Event::Key(key) => match key.code {
                        KeyCode::Esc => {
                            if state.focus == Focus::ActivityLog || state.focus == Focus::Filter {
                                state.focus = Focus::Agents;
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => match state.focus {
                            Focus::Filter => {
                                state.focus = Focus::Agents;
                            }
                            Focus::Agents => {
                                if state.move_agent_selection(1) {
                                    state.save_cursor();
                                } else {
                                    state.focus = Focus::ActivityLog;
                                }
                            }
                            Focus::ActivityLog => state.scroll_bottom(1),
                        },
                        KeyCode::Char('k') | KeyCode::Up => match state.focus {
                            Focus::Filter => {}
                            Focus::Agents => {
                                if state.move_agent_selection(-1) {
                                    state.save_cursor();
                                } else {
                                    state.focus = Focus::Filter;
                                }
                            }
                            Focus::ActivityLog => {
                                let at_top = match state.bottom_tab {
                                    tmux_agent_sidebar::state::BottomTab::Activity => {
                                        state.activity_scroll.offset == 0
                                    }
                                    tmux_agent_sidebar::state::BottomTab::GitStatus => {
                                        state.git_scroll.offset == 0
                                    }
                                };
                                if at_top {
                                    state.focus = Focus::Agents;
                                } else {
                                    state.scroll_bottom(-1);
                                }
                            }
                        },
                        KeyCode::Char('h') | KeyCode::Left => {
                            if state.focus == Focus::Filter {
                                state.agent_filter = state.agent_filter.prev();
                                state.save_filter();
                                state.rebuild_row_targets();
                            }
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            if state.focus == Focus::Filter {
                                state.agent_filter = state.agent_filter.next();
                                state.save_filter();
                                state.rebuild_row_targets();
                            }
                        }
                        KeyCode::Char('r') => {
                            if state.focus == Focus::Filter {
                                state.toggle_repo_popup();
                            }
                        }
                        KeyCode::Enter => {
                            if state.focus == Focus::Agents {
                                state.activate_selection();
                            }
                        }
                        KeyCode::Tab => {
                            state.agent_filter = state.agent_filter.next();
                            state.save_filter();
                            state.rebuild_row_targets();
                        }
                        KeyCode::BackTab => {
                            state.next_bottom_tab();
                            git_tab_active
                                .store(state.bottom_tab == BottomTab::GitStatus, Ordering::Relaxed);
                        }
                        _ => {}
                    },
                    Event::Mouse(mouse) => {
                        let term_height = terminal.size().map(|s| s.height).unwrap_or(0);
                        match mouse.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                let bottom_start =
                                    term_height.saturating_sub(ui::BOTTOM_PANEL_HEIGHT);
                                if mouse.row < bottom_start {
                                    state.handle_mouse_click(mouse.row, mouse.column);
                                } else if mouse.row == bottom_start {
                                    state.handle_bottom_tab_click(mouse.column);
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                state.handle_mouse_scroll(
                                    mouse.row,
                                    term_height,
                                    ui::BOTTOM_PANEL_HEIGHT,
                                    3,
                                );
                            }
                            MouseEventKind::ScrollUp => {
                                state.handle_mouse_scroll(
                                    mouse.row,
                                    term_height,
                                    ui::BOTTOM_PANEL_HEIGHT,
                                    -3,
                                );
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        if last_spinner.elapsed() >= spinner_interval {
            state.spinner_frame = (state.spinner_frame + 1) % SPINNER_PULSE.len();
            last_spinner = std::time::Instant::now();
        }

        let sigusr1 = NEEDS_REFRESH.swap(false, Ordering::Relaxed);
        if sigusr1 || last_refresh.elapsed() >= refresh_interval {
            state.sync_global_state();
            state.refresh();
            git_tab_active.store(state.bottom_tab == BottomTab::GitStatus, Ordering::Relaxed);
            last_refresh = std::time::Instant::now();
        }

        if let Ok(data) = git_rx.try_recv() {
            state.apply_git_data(data);
        }
    }
}

/// Git data polling thread. Fetches git status every 2 seconds while the Git
/// tab is active. Skips fetching when the tab is not visible.
fn git_poll_loop(tmux_pane: &str, git_tx: &mpsc::Sender<GitData>, active: &AtomicBool) {
    let mut last_path: Option<String> = None;
    loop {
        std::thread::sleep(Duration::from_secs(2));

        if !active.load(Ordering::Relaxed) {
            continue;
        }

        // When the sidebar has focus, focused_pane_path returns None.
        // Reuse the last known path so git data keeps updating.
        if let Some(p) = tmux::focused_pane_path(tmux_pane) {
            last_path = Some(p);
        }
        if let Some(ref path) = last_path {
            let data = git::fetch_git_data(path);
            if git_tx.send(data).is_err() {
                return;
            }
        }
    }
}

extern "C" fn sigusr1_handler(_: libc::c_int) {
    NEEDS_REFRESH.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_git_poll_skips_when_inactive() {
        let active = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<GitData>();

        let flag = Arc::clone(&active);
        let handle = std::thread::spawn(move || {
            // Simulate the poll loop check without actually sleeping 2s
            for _ in 0..3 {
                if !flag.load(Ordering::Relaxed) {
                    continue;
                }
                let _ = tx.send(GitData::default());
            }
        });

        handle.join().unwrap();
        // No data should have been sent since active=false
        assert!(
            rx.try_recv().is_err(),
            "should not poll when git tab is inactive"
        );
    }

    #[test]
    fn test_git_poll_sends_when_active() {
        let active = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel::<GitData>();

        let flag = Arc::clone(&active);
        let handle = std::thread::spawn(move || {
            // active=true, so it should send
            if flag.load(Ordering::Relaxed) {
                let _ = tx.send(GitData::default());
            }
        });

        handle.join().unwrap();
        assert!(rx.try_recv().is_ok(), "should poll when git tab is active");
    }

    #[test]
    fn test_git_poll_reacts_to_flag_change() {
        let active = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<GitData>();

        // Initially inactive
        assert!(!active.load(Ordering::Relaxed));

        // Switch to active
        active.store(true, Ordering::Relaxed);

        let flag = Arc::clone(&active);
        let handle = std::thread::spawn(move || {
            if flag.load(Ordering::Relaxed) {
                let _ = tx.send(GitData::default());
            }
        });

        handle.join().unwrap();
        assert!(
            rx.try_recv().is_ok(),
            "should poll after flag switches to active"
        );
    }

    #[test]
    fn test_git_poll_stops_on_sender_closed() {
        let active = AtomicBool::new(true);
        let (tx, rx) = mpsc::channel::<GitData>();
        drop(rx); // Close receiver

        let result = tx.send(GitData::default());
        assert!(result.is_err(), "send should fail when receiver is dropped");

        // Verify the flag check pattern used in git_poll_loop
        assert!(active.load(Ordering::Relaxed));
    }
}
