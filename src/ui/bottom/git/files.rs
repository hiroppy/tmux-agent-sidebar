use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::ui::colors::ColorTheme;
use crate::ui::text::{display_width, pad_to, truncate_to_width};

pub(super) const MAX_CHANGED_FILES: usize = 10;

/// Rendered file rows plus their per-row open targets.
pub(super) struct RenderedFileSection {
    /// Lines to draw in the Git panel.
    pub lines: Vec<Line<'static>>,
    /// Open targets aligned to visible file-name spans.
    pub targets: Vec<RenderedFileTarget>,
}

/// File metadata needed to register a clickable target after layout.
pub(super) struct RenderedFileTarget {
    /// Row index within the rendered file section.
    pub line_index: usize,
    /// Repository-relative path to open.
    pub file_path: String,
    /// Display width of the rendered file name.
    pub name_width: usize,
}

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
) -> RenderedFileSection {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut targets: Vec<RenderedFileTarget> = Vec::new();

    if files.is_empty() {
        return RenderedFileSection { lines, targets };
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
        let (diff_spans, diff_w) = if show_diff && (entry.additions > 0 || entry.deletions > 0) {
            super::diff_stat_spans(entry.additions, entry.deletions, theme)
        } else {
            (Vec::new(), 0)
        };

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
            Style::default()
                .fg(theme.pr_link)
                .add_modifier(Modifier::UNDERLINED),
        ));
        targets.push(RenderedFileTarget {
            line_index: lines.len(),
            file_path: if entry.path.is_empty() {
                entry.name.clone()
            } else {
                entry.path.clone()
            },
            name_width: name_w,
        });

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

    RenderedFileSection { lines, targets }
}

/// Render untracked files section.
pub(super) fn render_untracked_section(
    files: &[String],
    paths: &[String],
    inner_w: usize,
    theme: &ColorTheme,
) -> RenderedFileSection {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut targets: Vec<RenderedFileTarget> = Vec::new();

    if files.is_empty() {
        return RenderedFileSection { lines, targets };
    }

    lines.push(Line::from(Span::styled(
        format!("Untracked ({})", files.len()),
        Style::default().fg(theme.section_title),
    )));

    for name in files.iter().take(MAX_CHANGED_FILES) {
        let max_name_w = inner_w.saturating_sub(2); // "? " prefix
        let truncated_name = truncate_to_width(name, max_name_w);
        let name_w = display_width(&truncated_name);
        targets.push(RenderedFileTarget {
            line_index: lines.len(),
            file_path: paths
                .get(targets.len())
                .cloned()
                .unwrap_or_else(|| name.clone()),
            name_width: name_w,
        });
        lines.push(Line::from(vec![
            Span::styled("?", Style::default().fg(theme.text_muted)),
            Span::raw(" "),
            Span::styled(
                truncated_name,
                Style::default()
                    .fg(theme.pr_link)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]));
    }

    if files.len() > MAX_CHANGED_FILES {
        lines.push(render_more_indicator(
            files.len() - MAX_CHANGED_FILES,
            inner_w,
            theme,
        ));
    }

    RenderedFileSection { lines, targets }
}
