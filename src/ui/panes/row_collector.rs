use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::SPAWN_BUTTON;
use super::row;
use crate::state::{AppState, Focus};
use crate::ui::text::display_width;

#[derive(Debug, Default)]
pub(super) struct CollectedRows {
    pub lines: Vec<Line<'static>>,
    pub line_to_row: Vec<Option<usize>>,
    pub pending_spawn: Vec<(usize, String, String)>,
    pub pending_remove: Vec<(usize, u16, String)>,
}

pub(super) fn collect(state: &AppState, width: u16) -> CollectedRows {
    let width = width as usize;
    let theme = &state.theme;

    let mut collected = CollectedRows::default();
    let filter = state.global.status_filter;
    let mut first_group = true;
    let mut row_index: usize = 0;

    for group in &state.repo_groups {
        if !state.global.repo_filter.matches_group(&group.name) {
            continue;
        }
        let filtered_panes: Vec<_> = group
            .panes
            .iter()
            .filter(|(pane, _)| filter.matches(&pane.status))
            .collect();
        if filtered_panes.is_empty() {
            continue;
        }

        if !first_group {
            // Separate repo groups, but do not add a leading blank before
            // the first repo so the list starts immediately below the header.
            collected.lines.push(Line::from(""));
            collected.line_to_row.push(None);
        }
        first_group = false;

        let group_has_focused_pane = state
            .focus_state
            .focused_pane_id
            .as_ref()
            .is_some_and(|fid| group.panes.iter().any(|(p, _)| p.pane_id == *fid));

        // Plain repo header at column 0, with a `[+]` spawn button
        // right-aligned on the same row. Only rendered when the group
        // has a resolved repo_root — panes outside a git repo get a
        // plain title.
        let title = &group.name;
        let title_color = if group_has_focused_pane {
            theme.accent
        } else {
            theme.text_active
        };
        let repo_root = group
            .panes
            .iter()
            .find_map(|(_, git)| git.repo_root.clone());
        let spans: Vec<Span<'static>> = if let Some(ref root) = repo_root {
            let title_w = display_width(title);
            let pad_width = width
                .saturating_sub(title_w)
                .saturating_sub(SPAWN_BUTTON.len());
            collected
                .pending_spawn
                .push((collected.lines.len(), group.name.clone(), root.clone()));
            let button_color = if group_has_focused_pane {
                theme.accent
            } else {
                theme.text_active
            };
            vec![
                Span::styled(title.clone(), Style::default().fg(title_color)),
                Span::raw(" ".repeat(pad_width)),
                Span::styled(SPAWN_BUTTON, Style::default().fg(button_color)),
            ]
        } else {
            vec![Span::styled(
                title.clone(),
                Style::default().fg(title_color),
            )]
        };
        collected.lines.push(Line::from(spans));
        collected.line_to_row.push(None);

        for (pane, git_info) in filtered_panes.iter() {
            let is_selected = state.focus_state.sidebar_focused
                && state.focus_state.focus == Focus::Panes
                && row_index == state.global.selected_pane_row;

            let is_active = state.focus_state.focused_pane_id.as_ref() == Some(&pane.pane_id);

            let pane_state = state.pane_state(&pane.pane_id);
            let ports = pane_state.map(|s| s.ports.as_slice());
            let task_progress = pane_state.and_then(|s| s.task_progress.as_ref());
            let status_line_idx = collected.lines.len();
            let pane_lines = row::render_pane_lines_with_ports(
                pane,
                git_info,
                ports,
                task_progress,
                is_selected,
                is_active,
                width,
                &state.icons,
                theme,
                state.spinner_frame,
                state.now,
            );
            let pane_line_count = pane_lines.len();
            collected.lines.extend(pane_lines);
            for _ in 0..pane_line_count {
                collected.line_to_row.push(Some(row_index));
            }

            // The branch row is always `status_line_idx + 1` when
            // `branch_ports_row` emits a line (which requires a
            // non-empty branch). Look up the exact column of the
            // trailing `×` from the row helper so the click target
            // lines up with the rendered glyph even when the branch
            // name truncates.
            if pane.sidebar_spawned
                && git_info.is_worktree
                && pane_line_count >= 2
                && let Some(x) =
                    row::sidebar_remove_marker_col(git_info, ports, true, width.saturating_sub(2))
            {
                collected
                    .pending_remove
                    .push((status_line_idx + 1, x, pane.pane_id.clone()));
            }

            row_index += 1;
        }
    }

    collected
}
