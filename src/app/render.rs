use std::io::{self, Write as _};

use crossterm::{cursor::MoveTo, execute};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::clipboard;
use crate::git::{self, GitData};
use crate::state::{AppState, HyperlinkOverlay};
use crate::tmux;
use crate::ui;

pub(super) fn render_frame(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> io::Result<()> {
    terminal.draw(|frame| ui::draw(frame, state))?;

    // Write OSC 8 hyperlink overlays after frame render.
    write_hyperlink_overlays(terminal.backend_mut(), &state.layout.hyperlink_overlays)?;

    // Flush any pending OSC 52 clipboard payload (set by notices copy).
    // On I/O failure, restore the payload and propagate the error so the
    // user's copy request survives a transient backend hiccup instead of
    // silently disappearing.
    if let Some(payload) = state.pending_osc52_copy.take() {
        let seq = clipboard::osc52_sequence(&payload);
        let write_result = {
            let backend = terminal.backend_mut();
            backend
                .write_all(seq.as_bytes())
                .and_then(|_| backend.flush())
        };
        if let Err(err) = write_result {
            state.pending_osc52_copy = Some(payload);
            return Err(err);
        }
    }

    Ok(())
}

pub(super) fn refresh_git_for_focused_pane(state: &mut AppState) {
    refresh_git_for_focused_pane_with(
        state.focus_state.focused_pane_id.clone(),
        tmux::get_pane_path,
        git::fetch_git_data,
        |data| state.apply_git_data(data),
    );
}

pub(super) fn refresh_git_for_focused_pane_with<FGetPath, FFetchGit, FApply>(
    focused_pane_id: Option<String>,
    get_pane_path: FGetPath,
    mut fetch_git_data: FFetchGit,
    mut apply_git_data: FApply,
) where
    FGetPath: Fn(&str) -> Option<String>,
    FFetchGit: FnMut(&str) -> GitData,
    FApply: FnMut(GitData),
{
    if let Some(pane_id) = focused_pane_id
        && let Some(path) = get_pane_path(&pane_id)
    {
        apply_git_data(fetch_git_data(&path));
    }
}

/// Write OSC 8 hyperlink escape sequences over already-rendered PR text.
pub(super) fn write_hyperlink_overlays(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_git_for_focused_pane_with_fetches_and_applies_git_data() {
        let mut fetched_path = None;
        let mut applied = None;

        refresh_git_for_focused_pane_with(
            Some("%1".into()),
            |pane_id| {
                assert_eq!(pane_id, "%1");
                Some("/tmp/project".into())
            },
            |path| {
                fetched_path = Some(path.to_string());
                GitData {
                    branch: "main".into(),
                    ..GitData::default()
                }
            },
            |data| applied = Some(data),
        );

        assert_eq!(fetched_path.as_deref(), Some("/tmp/project"));
        assert_eq!(applied.map(|data| data.branch), Some("main".into()));
    }

    #[test]
    fn test_refresh_git_for_focused_pane_with_skips_when_no_path() {
        let mut fetch_called = false;
        let mut applied = false;

        refresh_git_for_focused_pane_with(
            Some("%1".into()),
            |_pane_id| None,
            |_path| {
                fetch_called = true;
                GitData::default()
            },
            |_data| applied = true,
        );

        assert!(!fetch_called);
        assert!(!applied);
    }
}
