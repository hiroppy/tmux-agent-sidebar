mod filter_bar;
mod row;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::state::{
    AppState, Focus, PopupState, RepoFilter, RepoSpawnTarget, SpawnField, SpawnRemoveTarget,
};

const SPAWN_BUTTON: &str = "+";

/// Width of the clickable region around the `×` marker. One column of
/// slack on either side makes it comfortable to hit without stealing
/// clicks from adjacent branch text.
const REMOVE_MARKER_HIT_WIDTH: u16 = 3;

use super::text::{display_width, truncate_to_width};

/// Compute a popup Rect centered inside `area`, clamped so it never
/// exceeds the parent (a narrow sidebar can't end up with a popup wider
/// than its own pane, which used to crash ratatui).
fn center_popup(area: Rect, desired_width: u16, desired_height: u16) -> Rect {
    let width = desired_width.min(area.width);
    let height = desired_height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

/// Place a popup directly below screen row `anchor_y`, left-aligned to
/// `area`, and shift upward when it would overflow the bottom edge.
fn anchor_below(area: Rect, anchor_y: u16, desired_width: u16, desired_height: u16) -> Rect {
    let width = desired_width.min(area.width);
    let height = desired_height.min(area.height);
    let below = anchor_y.saturating_add(1);
    let bottom = area.y.saturating_add(area.height);
    let y = if below + height <= bottom {
        below
    } else {
        bottom.saturating_sub(height).max(area.y)
    };
    Rect::new(area.x, y, width, height)
}

struct PaneLayout {
    filter_area: Rect,
    secondary_area: Rect,
    list_area: Rect,
}

impl PaneLayout {
    fn compute(area: Rect) -> Self {
        let filter_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1.min(area.height),
        };
        let secondary_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: 1.min(area.height.saturating_sub(1)),
        };
        let list_area = Rect {
            x: area.x,
            y: area.y + 2,
            width: area.width,
            height: area.height.saturating_sub(2),
        };
        Self {
            filter_area,
            secondary_area,
            list_area,
        }
    }
}

/// Minimum agents-panel height the expanded Vercel-style spawn modal
/// needs. Below this the popup falls back to a compact label-less
/// layout to avoid clipping rows (the default 20-row bottom panel can
/// leave only ~10 rows for the agents panel on short terminals).
const SPAWN_MODAL_EXPANDED_MIN_HEIGHT: u16 = 12;

/// Border rows contributed to the total popup height (top + bottom).
const POPUP_BORDER_ROWS: u16 = 2;

// Row offsets inside the inner area of the compact popup.
const COMPACT_TASK_Y: u16 = 0;
const COMPACT_AGENT_Y: u16 = 1;
const COMPACT_MODE_Y: u16 = 2;
const COMPACT_ERROR_Y: u16 = 3;

// Row offsets inside the inner area of the expanded Vercel popup.
// Each section is label → value with a blank spacer between them.
const EXP_TASK_LABEL_Y: u16 = 1;
const EXP_TASK_VALUE_Y: u16 = 2;
const EXP_AGENT_LABEL_Y: u16 = 4;
const EXP_AGENT_VALUE_Y: u16 = 5;
const EXP_MODE_LABEL_Y: u16 = 7;
const EXP_MODE_VALUE_Y: u16 = 8;
const EXP_ERROR_Y: u16 = 10;

fn render_spawn_input_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let PopupState::SpawnInput {
        input,
        agent_idx,
        mode_idx,
        field,
        anchor_y,
        error,
        ..
    } = &state.popup
    else {
        return;
    };
    let input = input.clone();
    let field = *field;
    let anchor_y = *anchor_y;
    let error = error.clone();
    let agent = crate::worktree::AGENTS
        .get(*agent_idx)
        .copied()
        .unwrap_or("");
    let mode = crate::worktree::modes_for(agent)
        .get(*mode_idx)
        .copied()
        .unwrap_or("");
    let theme = &state.theme;

    let popup_width = area.width.min(32).max(area.width.min(14));
    let compact = area.height < SPAWN_MODAL_EXPANDED_MIN_HEIGHT;
    let content_rows: u16 = if compact { 4 } else { 10 };
    let error_rows: u16 = if error.is_some() { 1 } else { 0 };
    let popup_height = content_rows + error_rows + POPUP_BORDER_ROWS;
    let popup_rect = match anchor_y {
        Some(y) => anchor_below(area, y, popup_width, popup_height),
        None => center_popup(area, popup_width, popup_height),
    };
    state.popup.set_spawn_input_area(Some(popup_rect));

    frame.render_widget(Clear, popup_rect);
    let title_trunc = truncate_to_width(
        " Spawn worktree ",
        popup_rect.width.saturating_sub(2) as usize,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            title_trunc,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup_rect);
    frame.render_widget(block, popup_rect);

    // Row 0 is left blank as a top gutter in expanded mode. Content
    // rows get one column of left padding so they don't hug the border.
    let render_at = |frame: &mut Frame, y_offset: u16, spans: Vec<Span<'_>>| {
        if y_offset < inner.height {
            let row = Rect::new(
                inner.x + 1,
                inner.y + y_offset,
                inner.width.saturating_sub(2),
                1,
            );
            frame.render_widget(Paragraph::new(Line::from(spans)), row);
        }
    };

    let label_style = |target: SpawnField| {
        let base = Style::default().add_modifier(Modifier::BOLD);
        if field == target {
            base.fg(theme.accent)
        } else {
            base.fg(theme.text_muted)
        }
    };
    let value_style = |target: SpawnField| {
        if field == target {
            Style::default().fg(theme.text_active)
        } else {
            Style::default().fg(theme.text_muted)
        }
    };

    let content_width = inner.width.saturating_sub(2) as usize;
    let visible_input = tail_fit(&input, content_width.saturating_sub(1));
    let mut task_spans: Vec<Span<'_>> =
        vec![Span::styled(visible_input, value_style(SpawnField::Task))];
    if field == SpawnField::Task {
        task_spans.push(Span::styled("█", Style::default().fg(theme.accent)));
    }
    let agent_value = truncate_to_width(agent, content_width);
    let mode_value = truncate_to_width(mode, content_width);
    let error_spans = error.as_ref().map(|err| {
        vec![Span::styled(
            truncate_to_width(err, content_width),
            Style::default().fg(theme.status_error),
        )]
    });

    if compact {
        render_at(frame, COMPACT_TASK_Y, task_spans);
        render_at(
            frame,
            COMPACT_AGENT_Y,
            vec![Span::styled(agent_value, value_style(SpawnField::Agent))],
        );
        render_at(
            frame,
            COMPACT_MODE_Y,
            vec![Span::styled(mode_value, value_style(SpawnField::Mode))],
        );
        if let Some(err) = error_spans {
            render_at(frame, COMPACT_ERROR_Y, err);
        }
    } else {
        render_at(
            frame,
            EXP_TASK_LABEL_Y,
            vec![Span::styled("NAME", label_style(SpawnField::Task))],
        );
        render_at(frame, EXP_TASK_VALUE_Y, task_spans);
        render_at(
            frame,
            EXP_AGENT_LABEL_Y,
            vec![Span::styled("AGENT", label_style(SpawnField::Agent))],
        );
        render_at(
            frame,
            EXP_AGENT_VALUE_Y,
            vec![Span::styled(agent_value, value_style(SpawnField::Agent))],
        );
        render_at(
            frame,
            EXP_MODE_LABEL_Y,
            vec![Span::styled("MODE", label_style(SpawnField::Mode))],
        );
        render_at(
            frame,
            EXP_MODE_VALUE_Y,
            vec![Span::styled(mode_value, value_style(SpawnField::Mode))],
        );
        if let Some(err) = error_spans {
            render_at(frame, EXP_ERROR_Y, err);
        }
    }
}

/// Keep only the trailing `max_width` display cells of `text` so the
/// cursor at the end stays visible in a narrow input box. Prepends `…`
/// when truncation is applied.
fn tail_fit(text: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    if max_width == 0 {
        return String::new();
    }
    if display_width(text) <= max_width {
        return text.to_string();
    }
    let budget = max_width.saturating_sub(1);
    let mut taken = 0usize;
    let mut byte_start = text.len();
    for (i, ch) in text.char_indices().rev() {
        let w = ch.width().unwrap_or(0);
        if taken + w > budget {
            break;
        }
        taken += w;
        byte_start = i;
    }
    let mut out = String::with_capacity(3 + (text.len() - byte_start));
    out.push('…');
    out.push_str(&text[byte_start..]);
    out
}

fn render_remove_confirm_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let (branch, error) = match &state.popup {
        PopupState::RemoveConfirm { branch, error, .. } => (branch.clone(), error.clone()),
        _ => return,
    };
    let theme = &state.theme;

    // Narrow-friendly: put the branch in the title, keep option rows
    // short enough to fit in ~16 columns. Reserve an extra row when
    // an inline error is present.
    let popup_height: u16 = if error.is_some() { 7 } else { 6 };
    let popup_rect = center_popup(area, area.width.min(28), popup_height);
    state.popup.set_remove_confirm_area(Some(popup_rect));

    frame.render_widget(Clear, popup_rect);
    let title_text = format!(" {branch} ");
    let title = truncate_to_width(&title_text, popup_rect.width.saturating_sub(2) as usize);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.status_error))
        .title(Span::styled(title, Style::default().fg(theme.status_error)));
    let inner = block.inner(popup_rect);
    frame.render_widget(block, popup_rect);

    let render_row = |frame: &mut Frame, y_offset: u16, text: &str, style: Style| {
        if y_offset < inner.height {
            let row = Rect::new(inner.x, inner.y + y_offset, inner.width, 1);
            let truncated = truncate_to_width(text, row.width as usize);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(truncated, style))),
                row,
            );
        }
    };

    render_row(
        frame,
        0,
        "[y] remove worktree",
        Style::default().fg(theme.status_error),
    );
    render_row(
        frame,
        1,
        "[c] close window only",
        Style::default().fg(theme.text_active),
    );
    render_row(
        frame,
        2,
        "[n] cancel",
        Style::default().fg(theme.text_muted),
    );
    if let Some(err) = error {
        render_row(frame, 4, &err, Style::default().fg(theme.status_error));
    }
}

fn render_repo_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let repos = state.repo_names();
    if repos.is_empty() {
        return;
    }

    let max_name_len = repos.iter().map(|r| display_width(r)).max().unwrap_or(3);
    // Width: padding(1 left + 1 right) + name + borders(2)
    let popup_width = (max_name_len + 4).min(area.width as usize).max(10) as u16;
    let popup_height = (repos.len() as u16 + 2).min(area.height.saturating_sub(2)); // +2 for borders

    // Right-aligned, below the 2-row header
    let popup_x = area.x + area.width.saturating_sub(popup_width);
    let popup_y = area.y + 2;

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
    state.popup.set_repo_area(Some(popup_rect));

    frame.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup_rect);
    frame.render_widget(block, popup_rect);

    let inner_width = inner.width as usize;
    for (i, name) in repos.iter().enumerate() {
        if i >= inner.height as usize {
            break;
        }

        let is_highlighted = i == state.repo_popup_selected();
        let is_current = match &state.global.repo_filter {
            RepoFilter::All => i == 0,
            RepoFilter::Repo(n) => *n == *name,
        };

        let truncated = truncate_to_width(name, inner_width.saturating_sub(1));
        let text = format!(" {}", truncated);
        let text_dw = display_width(&text);
        let padding = " ".repeat(inner_width.saturating_sub(text_dw));

        let style = if is_highlighted {
            Style::default()
                .fg(theme.text_active)
                .bg(theme.selection_bg)
        } else if is_current {
            Style::default().fg(theme.text_active)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let line_rect = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("{}{}", text, padding),
                style,
            ))),
            line_rect,
        );
    }
}

fn render_filter_bar_into(frame: &mut Frame, state: &AppState, area: Rect) {
    let line = filter_bar::render_filter_bar(state);
    frame.render_widget(Paragraph::new(vec![line]), area);
}

fn render_secondary_header_into(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let (line, notices_btn_col, repo_btn_col) =
        filter_bar::render_secondary_header(state, area.width);
    state.notices.button_col = notices_btn_col;
    state.layout.repo_button_col = repo_btn_col;
    frame.render_widget(Paragraph::new(vec![line]), area);
}

pub fn draw_agents(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let width = area.width as usize;

    let layout = PaneLayout::compute(area);
    render_filter_bar_into(frame, state, layout.filter_area);
    render_secondary_header_into(frame, state, layout.secondary_area);

    let theme = &state.theme;
    let list_area = layout.list_area;

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut line_to_row: Vec<Option<usize>> = Vec::new();
    let mut row_index: usize = 0;
    // Collected during line construction, converted to absolute screen
    // rects once the scroll offset is known.
    let mut pending_spawn_targets: Vec<(usize, String, String)> = Vec::new();
    // Each entry carries the branch-row line index, the precomputed
    // column where the `×` glyph lands, and the pane id to route
    // clicks to. The column is captured here (rather than during
    // click-target materialization) because it depends on the same
    // truncation math that `branch_ports_row` runs at draw time.
    let mut pending_remove_targets: Vec<(usize, u16, String)> = Vec::new();

    let filter = state.global.status_filter;

    let mut first_group = true;
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
            lines.push(Line::from(""));
            line_to_row.push(None);
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
        let spans: Vec<Span<'_>> = if let Some(ref root) = repo_root {
            let title_w = display_width(title);
            let pad_width = width
                .saturating_sub(title_w)
                .saturating_sub(SPAWN_BUTTON.len());
            pending_spawn_targets.push((lines.len(), group.name.clone(), root.clone()));
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
        lines.push(Line::from(spans));
        line_to_row.push(None);

        for (pane, git_info) in filtered_panes.iter() {
            let is_selected = state.focus_state.sidebar_focused
                && state.focus_state.focus == Focus::Panes
                && row_index == state.global.selected_pane_row;

            let is_active = state.focus_state.focused_pane_id.as_ref() == Some(&pane.pane_id);

            let pane_state = state.pane_state(&pane.pane_id);
            let ports = pane_state.map(|s| s.ports.as_slice());
            let task_progress = pane_state.and_then(|s| s.task_progress.as_ref());
            let status_line_idx = lines.len();
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
            lines.extend(pane_lines);
            for _ in 0..pane_line_count {
                line_to_row.push(Some(row_index));
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
                pending_remove_targets.push((status_line_idx + 1, x, pane.pane_id.clone()));
            }

            row_index += 1;
        }
    }

    state.layout.line_to_row = line_to_row;
    state.scrolls.panes.total_lines = lines.len();
    state.scrolls.panes.visible_height = list_area.height as usize;

    // Auto-scroll to keep selected agent visible
    if state.focus_state.sidebar_focused && state.focus_state.focus == Focus::Panes {
        let mut first_line: Option<usize> = None;
        let mut last_line: Option<usize> = None;
        for (i, mapping) in state.layout.line_to_row.iter().enumerate() {
            if *mapping == Some(state.global.selected_pane_row) {
                if first_line.is_none() {
                    first_line = Some(i);
                }
                last_line = Some(i);
            }
        }
        if let (Some(first), Some(last)) = (first_line, last_line) {
            let visible_h = list_area.height as usize;
            let offset = state.scrolls.panes.offset;
            if first < offset {
                state.scrolls.panes.offset = first.saturating_sub(1);
            } else if last >= offset + visible_h {
                state.scrolls.panes.offset = (last + 1).saturating_sub(visible_h);
            }
        }
    }

    let scroll_offset = state.scrolls.panes.offset;
    let btn_width = SPAWN_BUTTON.len() as u16;
    state.layout.repo_spawn_targets = pending_spawn_targets
        .into_iter()
        .filter_map(|(line_idx, repo_name, repo_root)| {
            if line_idx < scroll_offset {
                return None;
            }
            let screen_row = (line_idx - scroll_offset) as u16;
            if screen_row >= list_area.height {
                return None;
            }
            let btn_x = list_area.x + list_area.width.saturating_sub(btn_width);
            let btn_y = list_area.y + screen_row;
            Some(RepoSpawnTarget {
                rect: Rect::new(btn_x, btn_y, btn_width, 1),
                repo_name,
                repo_root,
            })
        })
        .collect();

    state.layout.spawn_remove_targets = pending_remove_targets
        .into_iter()
        .filter_map(|(line_idx, marker_col, pane_id)| {
            if line_idx < scroll_offset {
                return None;
            }
            let screen_row = (line_idx - scroll_offset) as u16;
            if screen_row >= list_area.height {
                return None;
            }
            // The `×` sits at the rightmost row column, so the hit
            // region can only extend leftward. Extending by
            // `REMOVE_MARKER_HIT_WIDTH - 1` keeps the glyph at the
            // right edge of the click rect with two columns of slack
            // to its left (which normally covers the space or port
            // digits just left of the marker).
            let btn_x = list_area
                .x
                .saturating_add(marker_col.saturating_sub(REMOVE_MARKER_HIT_WIDTH - 1));
            let btn_y = list_area.y + screen_row;
            Some(SpawnRemoveTarget {
                rect: Rect::new(btn_x, btn_y, REMOVE_MARKER_HIT_WIDTH, 1),
                pane_id,
            })
        })
        .collect();

    let paragraph = Paragraph::new(lines).scroll((state.scrolls.panes.offset as u16, 0));
    frame.render_widget(paragraph, list_area);

    // Render flash banner (spawn / remove feedback) before popups so
    // popups stay on top.
    if let Some(text) = state.take_flash() {
        let flash_y = area.y + area.height.saturating_sub(1);
        let flash_rect = Rect::new(area.x, flash_y, area.width, 1);
        frame.render_widget(Clear, flash_rect);
        let theme = &state.theme;
        let color = if text.contains("failed") {
            theme.status_error
        } else {
            theme.accent
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(text, Style::default().fg(color)))),
            flash_rect,
        );
    }

    // Render popup overlay on top if open
    if state.is_notices_popup_open() {
        super::notices::render_notices_popup(frame, state, area);
    } else if state.is_repo_popup_open() {
        render_repo_popup(frame, state, area);
    } else if state.is_spawn_input_open() {
        render_spawn_input_popup(frame, state, area);
    } else if state.is_remove_confirm_open() {
        render_remove_confirm_popup(frame, state, area);
    }
}
