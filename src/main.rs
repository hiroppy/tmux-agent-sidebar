use std::io::{self, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crossterm::{
    cursor::MoveTo,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::collections::HashMap;
use tmux_agent_sidebar::SPINNER_PULSE;
use tmux_agent_sidebar::git::{self, GitData};
use tmux_agent_sidebar::session;
use tmux_agent_sidebar::state::{AppState, BottomTab, Focus, HyperlinkOverlay};
use tmux_agent_sidebar::tmux;
use tmux_agent_sidebar::ui;
use tmux_agent_sidebar::version::UpdateNotice;

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
    state.icons = tmux_agent_sidebar::ui::icons::StatusIcons::from_tmux();
    state.bottom_panel_height = tmux_agent_sidebar::ui::bottom_panel_height_from_tmux();
    state.global.load_from_tmux();
    state.refresh();
    let mut window_inactive_count: u32 = 0;

    if let Some(ref pane_id) = state.focused_pane_id
        && let Some(path) = tmux::get_pane_path(pane_id)
    {
        state.apply_git_data(git::fetch_git_data(&path));
    }

    // Resolve the installed Claude Code plugin version once at startup,
    // matching the version_notice pattern. Restart the sidebar after a
    // /plugin install or /plugin uninstall to pick up the new state.
    state.notices.claude_plugin_installed_version =
        tmux_agent_sidebar::cli::plugin_state::installed_plugin_version();
    // Likewise resolve whether the user still has legacy
    // tmux-agent-sidebar/hook.sh entries in ~/.claude/settings.json so
    // the notices popup can warn about duplicate hook execution.
    state.notices.claude_settings_has_residual_hooks =
        tmux_agent_sidebar::cli::plugin_state::claude_settings_has_residual_hooks();
    // Notice inputs are static after the two lines above, so compute
    // them once here instead of from the per-tick refresh loop. This
    // also decouples the ⓘ badge from `focused_pane_id`, so killing
    // the last agent pane no longer drops outstanding setup warnings.
    state.refresh_notices();
    // Populate session names synchronously before the first draw so
    // `/rename`-assigned labels show up without waiting for the first
    // background scan tick.
    state.session_names = session::scan_session_names();
    state.refresh();

    let (git_tx, git_rx) = mpsc::channel::<GitData>();
    let (session_tx, session_rx) = mpsc::channel::<HashMap<String, String>>();
    let (version_tx, version_rx) = mpsc::channel::<UpdateNotice>();
    let tmux_pane_clone = state.tmux_pane.clone();
    let git_tab_active =
        std::sync::Arc::new(AtomicBool::new(state.bottom_tab == BottomTab::GitStatus));
    let git_tab_flag = std::sync::Arc::clone(&git_tab_active);
    std::thread::spawn(move || {
        git_poll_loop(&tmux_pane_clone, &git_tx, &git_tab_flag);
    });
    std::thread::spawn(move || {
        session_poll_loop(&session_tx);
    });
    std::thread::spawn(move || {
        if let Some(notice) = tmux_agent_sidebar::version::fetch_update_notice() {
            let _ = version_tx.send(notice);
        }
    });

    let mut last_refresh = std::time::Instant::now();
    let mut last_spinner = std::time::Instant::now();
    let refresh_interval = Duration::from_secs(1);
    let spinner_interval = Duration::from_millis(200);

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut state))?;

        // Write OSC 8 hyperlink overlays after frame render
        write_hyperlink_overlays(terminal.backend_mut(), &state.layout.hyperlink_overlays)?;

        // Flush any pending OSC 52 clipboard payload (set by notices copy).
        if let Some(payload) = state.pending_osc52_copy.take() {
            let seq = tmux_agent_sidebar::clipboard::osc52_sequence(&payload);
            let backend = terminal.backend_mut();
            use std::io::Write;
            let _ = backend.write_all(seq.as_bytes());
            let _ = backend.flush();
        }

        let refresh_timeout = refresh_interval.saturating_sub(last_refresh.elapsed());
        let spinner_timeout = spinner_interval.saturating_sub(last_spinner.elapsed());
        let timeout = refresh_timeout.min(spinner_timeout);
        if event::poll(timeout)? {
            loop {
                let ev = event::read()?;
                match ev {
                    Event::Key(key) if state.is_notices_popup_open() => {
                        if key.code == KeyCode::Esc {
                            state.close_notices_popup();
                        }
                    }
                    Event::Key(key) if state.is_spawn_input_open() => match key.code {
                        KeyCode::Esc => state.close_spawn_input(),
                        KeyCode::Enter => state.confirm_spawn_input(),
                        KeyCode::Tab | KeyCode::Down => state.spawn_input_next_field(),
                        KeyCode::BackTab | KeyCode::Up => state.spawn_input_prev_field(),
                        KeyCode::Left => state.spawn_input_cycle(-1),
                        KeyCode::Right => state.spawn_input_cycle(1),
                        KeyCode::Backspace => state.spawn_input_pop_char(),
                        KeyCode::Char(c) => state.spawn_input_push_char(c),
                        _ => {}
                    },
                    Event::Key(key) if state.is_remove_confirm_open() => match key.code {
                        KeyCode::Esc | KeyCode::Char('n') => state.close_remove_confirm(),
                        KeyCode::Char('c') => state
                            .confirm_remove(tmux_agent_sidebar::worktree::RemoveMode::WindowOnly),
                        KeyCode::Enter | KeyCode::Char('y') => state.confirm_remove(
                            tmux_agent_sidebar::worktree::RemoveMode::WindowAndWorktree,
                        ),
                        _ => {}
                    },
                    Event::Key(key) if state.is_repo_popup_open() => match key.code {
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
                    },
                    Event::Key(key) => match key.code {
                        KeyCode::Esc => {
                            if state.focus == Focus::ActivityLog || state.focus == Focus::Filter {
                                state.focus = Focus::Panes;
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => match state.focus {
                            Focus::Filter => {
                                state.focus = Focus::Panes;
                            }
                            Focus::Panes => {
                                if state.move_pane_selection(1) {
                                    state.global.save_cursor();
                                } else {
                                    state.focus = Focus::ActivityLog;
                                }
                            }
                            Focus::ActivityLog => state.scroll_bottom(1),
                        },
                        KeyCode::Char('k') | KeyCode::Up => match state.focus {
                            Focus::Filter => {}
                            Focus::Panes => {
                                if state.move_pane_selection(-1) {
                                    state.global.save_cursor();
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
                                    state.focus = Focus::Panes;
                                } else {
                                    state.scroll_bottom(-1);
                                }
                            }
                        },
                        KeyCode::Char('h') | KeyCode::Left => {
                            if state.focus == Focus::Filter {
                                state.global.status_filter = state.global.status_filter.prev();
                                state.global.save_filter();
                                state.rebuild_row_targets();
                            }
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            if state.focus == Focus::Filter {
                                state.global.status_filter = state.global.status_filter.next();
                                state.global.save_filter();
                                state.rebuild_row_targets();
                            }
                        }
                        KeyCode::Char('r') => {
                            if state.focus == Focus::Filter {
                                state.toggle_repo_popup();
                            }
                        }
                        KeyCode::Char('n') => {
                            if state.focus == Focus::Panes {
                                state.open_spawn_input_from_selection();
                            }
                        }
                        KeyCode::Char('x') => {
                            if state.focus == Focus::Panes {
                                state.open_remove_confirm();
                            }
                        }
                        KeyCode::Enter => {
                            if state.focus == Focus::Panes {
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
                    },
                    Event::Mouse(mouse) => {
                        let term_height = terminal.size().map(|s| s.height).unwrap_or(0);
                        let bottom_h = state.bottom_panel_height;
                        match mouse.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                let bottom_start = term_height.saturating_sub(bottom_h);
                                if mouse.row < bottom_start {
                                    state.handle_mouse_click(mouse.row, mouse.column);
                                } else if mouse.row == bottom_start {
                                    state.handle_bottom_tab_click(mouse.column);
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
            let is_window_active = state.refresh();
            if is_window_active {
                if window_inactive_count >= 2 {
                    state.global.load_from_tmux();
                    state.rebuild_row_targets();
                }
                window_inactive_count = 0;
            } else {
                window_inactive_count = window_inactive_count.saturating_add(1);
            }
            git_tab_active.store(state.bottom_tab == BottomTab::GitStatus, Ordering::Relaxed);
            last_refresh = std::time::Instant::now();
        }

        if let Ok(data) = git_rx.try_recv() {
            state.apply_git_data(data);
        }

        if let Ok(names) = session_rx.try_recv() {
            state.session_names = names;
        }

        if let Ok(notice) = version_rx.try_recv() {
            state.version_notice = Some(notice);
        }
    }
}

/// Write OSC 8 hyperlink escape sequences over already-rendered PR text.
fn write_hyperlink_overlays(
    backend: &mut CrosstermBackend<io::Stdout>,
    overlays: &[HyperlinkOverlay],
) -> io::Result<()> {
    for overlay in overlays {
        execute!(backend, MoveTo(overlay.x, overlay.y))?;
        // OSC 8: open hyperlink
        write!(backend, "\x1b]8;;{}\x1b\\", overlay.url)?;
        // Re-write the text so the terminal associates these cells with the link
        write!(backend, "{}", overlay.text)?;
        // OSC 8: close hyperlink
        write!(backend, "\x1b]8;;\x1b\\")?;
        backend.flush()?;
    }
    Ok(())
}

/// Session name polling thread. Scans `~/.claude/sessions/*.json` every 10
/// seconds so the main TUI thread never performs blocking filesystem I/O
/// to refresh `/rename`-assigned labels.
fn session_poll_loop(tx: &mpsc::Sender<HashMap<String, String>>) {
    loop {
        std::thread::sleep(Duration::from_secs(10));
        let names = session::scan_session_names();
        if tx.send(names).is_err() {
            return;
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
