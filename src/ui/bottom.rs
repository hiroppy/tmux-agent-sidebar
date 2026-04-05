use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::state::{AppState, BottomTab};

use super::text::{display_width, pad_to, truncate_to_width, wrap_text_char};

const MAX_CHANGED_FILES: usize = 5;

fn render_more_indicator(
    remaining: usize,
    inner_w: usize,
    theme: &crate::ui::colors::ColorTheme,
) -> Line<'static> {
    let more_text = format!("+{} more ", remaining);
    let more_w = display_width(&more_text);
    let gap = pad_to(more_w, inner_w);
    Line::from(vec![
        Span::raw(gap),
        Span::styled(more_text, Style::default().fg(theme.text_muted)),
    ])
}

fn render_centered(frame: &mut Frame, area: Rect, text: &str, color: Color) {
    // Vertically center: pad with empty lines above
    let top_pad = area.height.saturating_sub(1) / 2;
    let mut lines: Vec<Line<'_>> = Vec::new();
    for _ in 0..top_pad {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(text, Style::default().fg(color))));
    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

pub fn draw_bottom(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let border_color = theme.border_active;

    let tab_title = build_tab_title(state);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(tab_title)
        .style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match state.bottom_tab {
        BottomTab::Activity => draw_activity_content(frame, state, inner),
        BottomTab::GitStatus => draw_git_content(frame, state, inner),
    }
}

fn build_tab_title(state: &AppState) -> Line<'static> {
    let theme = &state.theme;

    let activity_style = if state.bottom_tab == BottomTab::Activity {
        Style::default()
            .fg(theme.text_active)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };

    let git_style = if state.bottom_tab == BottomTab::GitStatus {
        Style::default()
            .fg(theme.text_active)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };

    let sep_style = Style::default().fg(theme.border_inactive);

    Line::from(vec![
        Span::styled(" Activity ", activity_style),
        Span::styled("\u{2502}", sep_style),
        Span::styled(" Git ", git_style),
    ])
}

fn draw_activity_content(frame: &mut Frame, state: &mut AppState, inner: Rect) {
    let theme = &state.theme;

    if state.activity_entries.is_empty() {
        render_centered(frame, inner, "No activity yet", theme.text_muted);
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::new();
    let inner_w = inner.width as usize;

    for entry in &state.activity_entries {
        let tool_color = Color::Indexed(entry.tool_color_index());

        let ts_dw = display_width(&entry.timestamp);
        let tool_dw = display_width(&entry.tool);
        let gap = pad_to(ts_dw + tool_dw, inner_w);
        let line1 = Line::from(vec![
            Span::styled(
                entry.timestamp.clone(),
                Style::default().fg(theme.activity_timestamp),
            ),
            Span::raw(gap),
            Span::styled(entry.tool.clone(), Style::default().fg(tool_color)),
        ]);
        lines.push(line1);

        if !entry.label.is_empty() {
            let label_max_w = inner_w.saturating_sub(2);
            let wrapped = wrap_text_char(&entry.label, label_max_w, 3);
            for wl in wrapped {
                lines.push(Line::from(Span::styled(
                    format!("  {wl}"),
                    Style::default().fg(theme.text_muted),
                )));
            }
        }
    }

    state.activity_scroll.total_lines = lines.len();
    state.activity_scroll.visible_height = inner.height as usize;

    let scroll_offset = state.activity_scroll.offset as u16;
    let paragraph = Paragraph::new(lines).scroll((scroll_offset, 0));
    frame.render_widget(paragraph, inner);
}

/// Render the fixed header: branch+PR line, diff summary line, separator.
/// Returns the lines and the number of rows consumed.
fn render_git_header(state: &AppState, inner_w: usize) -> Vec<Line<'static>> {
    let theme = &state.theme;
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Line 1: branch (left) + PR number (right)
    if !state.git.branch.is_empty() {
        let mut left_spans: Vec<Span> = Vec::new();

        // Build branch text with ahead/behind
        let mut branch_text = format!(" {}", state.git.branch);
        if let Some((ahead, behind)) = state.git.ahead_behind {
            if ahead > 0 {
                branch_text.push_str(&format!(" ↑{ahead}"));
            }
            if behind > 0 {
                branch_text.push_str(&format!(" ↓{behind}"));
            }
        }

        // Build PR text (no trailing space — underline should not extend)
        let pr_text = state.git.pr_number.as_ref().map(|n| format!("#{n}"));

        // Reserve space: PR text + 1 trailing space for right margin
        let pr_w = pr_text.as_ref().map_or(0, |t| display_width(t) + 1);

        // Truncate branch if it collides with PR number
        let max_branch_w = inner_w.saturating_sub(pr_w + if pr_w > 0 { 1 } else { 0 });
        let truncated_branch = truncate_to_width(&branch_text, max_branch_w);
        let branch_w = display_width(&truncated_branch);

        left_spans.push(Span::styled(
            truncated_branch,
            Style::default().fg(theme.text_active),
        ));

        if let Some(ref pr) = pr_text {
            let gap = pad_to(branch_w + pr_w, inner_w);
            left_spans.push(Span::raw(gap));
            left_spans.push(Span::styled(
                pr.clone(),
                Style::default()
                    .fg(theme.pr_link)
                    .add_modifier(Modifier::UNDERLINED),
            ));
            left_spans.push(Span::raw(" "));
        }

        lines.push(Line::from(left_spans));
    }

    // Blank line between branch and diff summary
    let has_changes = state.git.diff_stat.is_some() || state.git.changed_file_count() > 0;
    if !state.git.branch.is_empty() && has_changes {
        lines.push(Line::from(""));
    }

    // Line 2: diff summary (+ins -del   N files)
    if has_changes {
        let mut left_spans: Vec<Span> = Vec::new();
        let mut left_w = 1; // leading space

        left_spans.push(Span::raw(" "));

        if let Some((ins, del)) = state.git.diff_stat {
            let s_ins = format!("+{ins}");
            left_w += display_width(&s_ins);
            left_spans.push(Span::styled(s_ins, Style::default().fg(theme.diff_added)));

            left_spans.push(Span::styled("/", Style::default().fg(theme.text_muted)));
            left_w += 1;

            let s_del = format!("-{del}");
            left_w += display_width(&s_del);
            left_spans.push(Span::styled(s_del, Style::default().fg(theme.diff_deleted)));
        }

        let files_text = format!("{} files ", state.git.changed_file_count());
        let files_w = display_width(&files_text);
        let gap = pad_to(left_w + files_w, inner_w);
        left_spans.push(Span::raw(gap));
        left_spans.push(Span::styled(
            files_text,
            Style::default().fg(theme.text_muted),
        ));

        lines.push(Line::from(left_spans));
    }

    let sep = "─".repeat(inner_w);
    lines.push(Line::from(Span::styled(
        sep,
        Style::default().fg(theme.text_muted),
    )));

    lines
}

/// Render a single file section (Staged/Unstaged/Untracked).
fn render_file_section(
    title: &str,
    files: &[crate::git::GitFileEntry],
    inner_w: usize,
    theme: &crate::ui::colors::ColorTheme,
    show_diff: bool,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if files.is_empty() {
        return lines;
    }

    // Section header
    lines.push(Line::from(Span::styled(
        format!(" {title} ({})", files.len()),
        Style::default().fg(theme.section_title),
    )));

    for entry in files.iter().take(MAX_CHANGED_FILES) {
        let status_color = match entry.status {
            'M' => theme.badge_auto,
            'A' => theme.status_running,
            'D' => theme.badge_danger,
            _ => theme.text_muted,
        };

        let mut spans: Vec<Span> = Vec::new();

        // Status indicator — aligned with section title (1 space indent)
        let status_text = format!(" {} ", entry.status);
        spans.push(Span::styled(
            status_text.clone(),
            Style::default().fg(status_color),
        ));
        let status_w = display_width(&status_text);

        // Build diff stat text for right side
        let mut diff_spans: Vec<Span> = Vec::new();
        let mut diff_w = 0;

        if show_diff && (entry.additions > 0 || entry.deletions > 0) {
            let s_ins = format!("+{}", entry.additions);
            diff_w += display_width(&s_ins);
            diff_spans.push(Span::styled(s_ins, Style::default().fg(theme.diff_added)));

            diff_spans.push(Span::styled("/", Style::default().fg(theme.text_muted)));
            diff_w += 1;

            let s_del = format!("-{}", entry.deletions);
            diff_w += display_width(&s_del);
            diff_spans.push(Span::styled(s_del, Style::default().fg(theme.diff_deleted)));

            diff_w += 1; // trailing space
            diff_spans.push(Span::raw(" "));
        }

        // Filename (truncated to fit, with margin before change stats)
        let margin = if diff_w > 0 { 2 } else { 0 }; // gap between name and stats
        let max_name_w = inner_w.saturating_sub(status_w + diff_w + margin);
        let truncated_name = truncate_to_width(&entry.name, max_name_w);
        let name_w = display_width(&truncated_name);

        spans.push(Span::styled(
            truncated_name,
            Style::default().fg(theme.text_muted),
        ));

        if !diff_spans.is_empty() {
            let gap = pad_to(status_w + name_w + diff_w, inner_w);
            spans.push(Span::raw(gap));
            spans.extend(diff_spans);
        }

        lines.push(Line::from(spans));
    }

    if files.len() > MAX_CHANGED_FILES {
        lines.push(render_more_indicator(
            files.len() - MAX_CHANGED_FILES,
            inner_w,
            theme,
        ));
    }

    lines
}

/// Render untracked files section.
fn render_untracked_section(
    files: &[String],
    inner_w: usize,
    theme: &crate::ui::colors::ColorTheme,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if files.is_empty() {
        return lines;
    }

    lines.push(Line::from(Span::styled(
        format!(" Untracked ({})", files.len()),
        Style::default().fg(theme.section_title),
    )));

    for name in files.iter().take(MAX_CHANGED_FILES) {
        let max_name_w = inner_w.saturating_sub(3); // " U " prefix
        let truncated_name = truncate_to_width(name, max_name_w);
        lines.push(Line::from(vec![
            Span::styled(" ? ", Style::default().fg(theme.text_muted)),
            Span::styled(truncated_name, Style::default().fg(theme.text_muted)),
        ]));
    }

    if files.len() > MAX_CHANGED_FILES {
        lines.push(render_more_indicator(
            files.len() - MAX_CHANGED_FILES,
            inner_w,
            theme,
        ));
    }

    lines
}

fn draw_git_content(frame: &mut Frame, state: &mut AppState, inner: Rect) {
    let theme = &state.theme;
    let inner_w = inner.width as usize;

    // No git data loaded yet
    if state.git.branch.is_empty()
        && state.git.staged_files.is_empty()
        && state.git.unstaged_files.is_empty()
        && state.git.untracked_files.is_empty()
        && state.git.diff_stat.is_none()
    {
        render_centered(frame, inner, "Working tree clean", theme.text_muted);
        return;
    }

    // Render fixed header
    let header_lines = render_git_header(state, inner_w);
    let header_height = header_lines.len() as u16;

    // Render header in a fixed area at the top
    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: header_height.min(inner.height),
    };
    let header_paragraph = Paragraph::new(header_lines);
    frame.render_widget(header_paragraph, header_area);

    // Remaining area for scrollable file list
    let content_y = inner.y + header_height;
    let content_height = inner.height.saturating_sub(header_height);
    if content_height == 0 {
        return;
    }
    let content_area = Rect {
        x: inner.x,
        y: content_y,
        width: inner.width,
        height: content_height,
    };

    // Build scrollable content
    let mut lines: Vec<Line<'_>> = Vec::new();

    let staged = render_file_section("Staged", &state.git.staged_files, inner_w, theme, true);
    let unstaged = render_file_section("Unstaged", &state.git.unstaged_files, inner_w, theme, true);
    let untracked = render_untracked_section(&state.git.untracked_files, inner_w, theme);

    if !staged.is_empty() {
        lines.extend(staged);
    }
    if !unstaged.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.extend(unstaged);
    }
    if !untracked.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.extend(untracked);
    }

    // Working tree clean
    if lines.is_empty() {
        render_centered(frame, content_area, "Working tree clean", theme.text_muted);
        return;
    }

    state.git_scroll.total_lines = lines.len();
    state.git_scroll.visible_height = content_height as usize;

    let scroll_offset = state.git_scroll.offset as u16;
    let paragraph = Paragraph::new(lines).scroll((scroll_offset, 0));
    frame.render_widget(paragraph, content_area);
}

#[cfg(test)]
fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_to_width_short() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn truncate_to_width_exact() {
        assert_eq!(truncate_to_width("hello", 5), "hello");
    }

    #[test]
    fn truncate_to_width_truncated() {
        let result = truncate_to_width("hello world", 8);
        assert!(result.ends_with('…'));
        assert!(result.len() <= 10); // 7 chars + ellipsis in bytes
    }

    // ─── PR underline tests ─────────────────────────────────────

    #[test]
    fn pr_number_no_trailing_underline() {
        let mut state = crate::state::AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.pr_number = Some("5".into());
        let lines = render_git_header(&state, 30);
        let spans = &lines[0].spans;
        let pr_span = spans.iter().find(|s| s.content.contains('#')).unwrap();
        assert_eq!(pr_span.content.as_ref(), "#5");
        assert!(pr_span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    // ─── Section title color tests ───────────────────────────────

    #[test]
    fn section_title_uses_section_title_color() {
        let theme = crate::ui::colors::ColorTheme::default();
        let files = vec![crate::git::GitFileEntry {
            status: 'M',
            name: "a.rs".into(),
            additions: 1,
            deletions: 0,
        }];
        let lines = render_file_section("Staged", &files, 40, &theme, true);
        let header_span = &lines[0].spans[0];
        assert_eq!(header_span.style.fg, Some(theme.section_title));
    }

    #[test]
    fn untracked_title_uses_section_title_color() {
        let theme = crate::ui::colors::ColorTheme::default();
        let files = vec!["tmp.log".to_string()];
        let lines = render_untracked_section(&files, 40, &theme);
        let header_span = &lines[0].spans[0];
        assert_eq!(header_span.style.fg, Some(theme.section_title));
    }

    // ─── More indicator right-alignment (untracked) ──────────────

    #[test]
    fn more_indicator_right_aligned_untracked() {
        let theme = crate::ui::colors::ColorTheme::default();
        let files: Vec<String> = (0..7).map(|i| format!("file{i}.tmp")).collect();
        let lines = render_untracked_section(&files, 30, &theme);
        let more_line = lines.last().unwrap();
        let text = line_text(more_line);
        assert!(text.contains("+2 more"));
        assert_eq!(display_width(&text), 30);
    }

    // ─── Header structure tests ──────────────────────────────────

    #[test]
    fn header_blank_line_between_branch_and_diff() {
        let mut state = crate::state::AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.diff_stat = Some((1, 0));
        let lines = render_git_header(&state, 40);
        assert_eq!(lines.len(), 4);
        assert!(line_text(&lines[1]).is_empty());
    }

    #[test]
    fn header_no_blank_line_without_changes() {
        let mut state = crate::state::AppState::new(String::new());
        state.git.branch = "main".into();
        let lines = render_git_header(&state, 40);
        assert_eq!(lines.len(), 2);
    }

    // ─── Edge case: truncation & narrow width ────────────────────

    #[test]
    fn long_filename_no_diff_uses_full_width() {
        let theme = crate::ui::colors::ColorTheme::default();
        let files = vec![crate::git::GitFileEntry {
            status: 'M',
            name: "medium-length-name.rs".into(),
            additions: 0,
            deletions: 0,
        }];
        let lines = render_file_section("Staged", &files, 40, &theme, true);
        let file_text = line_text(&lines[1]);
        assert!(file_text.contains("medium-length-name.rs"));
        assert!(!file_text.contains('…'));
    }

    #[test]
    fn long_untracked_filename_truncated() {
        let theme = crate::ui::colors::ColorTheme::default();
        let files = vec!["a-very-long-untracked-filename-that-exceeds-width.tmp".to_string()];
        let lines = render_untracked_section(&files, 25, &theme);
        let file_text = line_text(&lines[1]);
        assert!(display_width(&file_text) <= 25);
        assert!(file_text.contains('…'));
    }

    #[test]
    fn narrow_width_file_section_fits() {
        let theme = crate::ui::colors::ColorTheme::default();
        let files = vec![crate::git::GitFileEntry {
            status: 'A',
            name: "index.tsx".into(),
            additions: 100,
            deletions: 50,
        }];
        let lines = render_file_section("Staged", &files, 20, &theme, true);
        let file_text = line_text(&lines[1]);
        assert!(display_width(&file_text) <= 20);
        assert!(file_text.contains("+100/-50"));
    }

    #[test]
    fn narrow_width_header_fits() {
        let mut state = crate::state::AppState::new(String::new());
        state.git.branch = "feature/branch".into();
        state.git.pr_number = Some("1".into());
        state.git.diff_stat = Some((999, 888));
        let lines = render_git_header(&state, 20);
        for line in &lines {
            let text = line_text(line);
            assert!(
                display_width(&text) <= 20,
                "line exceeds width: '{text}' ({})",
                display_width(&text)
            );
        }
    }
}
