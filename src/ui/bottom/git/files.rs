use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::ui::colors::ColorTheme;
use crate::ui::text::{display_width, pad_to, truncate_to_width};

pub(super) const MAX_CHANGED_FILES: usize = 10;

fn render_more_indicator(remaining: usize, inner_w: usize, theme: &ColorTheme) -> Line<'static> {
    let more_text = format!("+{} more", remaining);
    let more_w = display_width(&more_text);
    let gap = pad_to(more_w, inner_w);
    Line::from(vec![
        Span::raw(gap),
        Span::styled(more_text, Style::default().fg(theme.text_muted)),
    ])
}

/// Render a single file section (Staged/Unstaged/Untracked).
pub(super) fn render_file_section(
    title: &str,
    files: &[crate::git::GitFileEntry],
    inner_w: usize,
    theme: &ColorTheme,
    show_diff: bool,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if files.is_empty() {
        return lines;
    }

    // Section header
    lines.push(Line::from(Span::styled(
        format!("{title} ({})", files.len()),
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
        let status_text = entry.status.to_string();
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
        }

        // Filename (truncated to fit, with a single gap before change stats)
        let max_name_w = if diff_w > 0 {
            inner_w.saturating_sub(status_w + diff_w + 2)
        } else {
            inner_w.saturating_sub(status_w + 1)
        };
        let truncated_name = truncate_to_width(&entry.name, max_name_w);
        let name_w = display_width(&truncated_name);

        spans.push(Span::raw(" "));

        spans.push(Span::styled(
            truncated_name,
            Style::default().fg(theme.text_muted),
        ));

        if !diff_spans.is_empty() {
            spans.push(Span::raw(" "));
            let gap = pad_to(status_w + 1 + name_w + 1 + diff_w, inner_w);
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
pub(super) fn render_untracked_section(
    files: &[String],
    inner_w: usize,
    theme: &ColorTheme,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    if files.is_empty() {
        return lines;
    }

    lines.push(Line::from(Span::styled(
        format!("Untracked ({})", files.len()),
        Style::default().fg(theme.section_title),
    )));

    for name in files.iter().take(MAX_CHANGED_FILES) {
        let max_name_w = inner_w.saturating_sub(2); // "? " prefix
        let truncated_name = truncate_to_width(name, max_name_w);
        lines.push(Line::from(vec![
            Span::styled("?", Style::default().fg(theme.text_muted)),
            Span::raw(" "),
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
