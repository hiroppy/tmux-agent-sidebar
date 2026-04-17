use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::state::AppState;
use crate::ui::text::{display_width, pad_to, truncate_to_width};

const MAX_CHANGED_FILES: usize = 10;

fn render_more_indicator(
    remaining: usize,
    inner_w: usize,
    theme: &crate::ui::colors::ColorTheme,
) -> Line<'static> {
    let more_text = format!("+{} more", remaining);
    let more_w = display_width(&more_text);
    let gap = pad_to(more_w, inner_w);
    Line::from(vec![
        Span::raw(gap),
        Span::styled(more_text, Style::default().fg(theme.text_muted)),
    ])
}

/// Info about a PR link position within the header (relative to header origin).
struct PrLinkInfo {
    /// X offset from the left edge of the header area.
    x_offset: u16,
    /// Display text (e.g. "#123").
    text: String,
    /// Full URL to open.
    url: String,
}

/// Render the fixed header: branch+PR line, diff summary line, separator.
/// Returns the lines and optional PR link position info.
fn render_git_header(state: &AppState, inner_w: usize) -> (Vec<Line<'static>>, Option<PrLinkInfo>) {
    let theme = &state.theme;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut pr_link_info: Option<PrLinkInfo> = None;

    // Leave one blank row at the top of the Git panel header.
    lines.push(Line::from(""));

    // Line 1 is blank.
    // Line 2: branch (left) + ahead/behind + PR number (right)
    if !state.git.branch.is_empty() {
        let mut left_spans: Vec<Span> = Vec::new();

        // Build branch text
        let branch_text = state.git.branch.clone();
        let mut movement_spans: Vec<Span> = Vec::new();
        if let Some((ahead, behind)) = state.git.ahead_behind {
            if ahead > 0 {
                movement_spans.push(Span::raw(" "));
                movement_spans.push(Span::styled("↑", Style::default().fg(theme.diff_added)));
                movement_spans.push(Span::styled(
                    ahead.to_string(),
                    Style::default().fg(theme.text_active),
                ));
            }
            if behind > 0 {
                movement_spans.push(Span::styled("↓", Style::default().fg(theme.diff_deleted)));
                movement_spans.push(Span::styled(
                    behind.to_string(),
                    Style::default().fg(theme.text_active),
                ));
            }
        }

        // Build PR text (no trailing space — underline should not extend)
        let pr_text = state.git.pr_number.as_ref().map(|n| format!("#{n}"));

        // Reserve space for the PR text itself.
        let pr_w = pr_text.as_ref().map_or(0, |t| display_width(t));
        let movement_w = movement_spans
            .iter()
            .map(|span| display_width(span.content.as_ref()))
            .sum::<usize>();
        let separator_w = if movement_w > 0 && pr_w > 0 { 1 } else { 0 };
        let right_w = movement_w + separator_w + pr_w;

        // Truncate branch if it collides with PR number
        let max_branch_w = inner_w.saturating_sub(right_w + if right_w > 0 { 1 } else { 0 });
        let truncated_branch = truncate_to_width(&branch_text, max_branch_w);
        let branch_w = display_width(&truncated_branch);

        left_spans.push(Span::styled(
            truncated_branch,
            Style::default().fg(theme.text_active),
        ));

        if let Some(ref pr) = pr_text {
            let gap = pad_to(branch_w + right_w, inner_w);
            let pr_x_offset = (inner_w - pr_w) as u16;
            left_spans.push(Span::raw(gap));
            left_spans.extend(movement_spans);
            if movement_w > 0 {
                left_spans.push(Span::raw(" "));
            }
            left_spans.push(Span::styled(
                pr.clone(),
                Style::default()
                    .fg(theme.pr_link)
                    .add_modifier(Modifier::UNDERLINED),
            ));
            // Build PR URL from remote_url
            if !state.git.remote_url.is_empty()
                && let Some(num) = &state.git.pr_number
            {
                pr_link_info = Some(PrLinkInfo {
                    x_offset: pr_x_offset,
                    text: pr.clone(),
                    url: format!("{}/pull/{num}", state.git.remote_url),
                });
            }
        } else if !movement_spans.is_empty() {
            let gap = pad_to(branch_w + right_w, inner_w);
            left_spans.push(Span::raw(gap));
            left_spans.extend(movement_spans);
        } else {
            let gap = pad_to(branch_w + right_w, inner_w);
            left_spans.push(Span::raw(gap));
        }

        lines.push(Line::from(left_spans));
    }

    let has_changes = state.git.diff_stat.is_some() || state.git.changed_file_count() > 0;

    // Line 3: diff summary (+ins -del   N files)
    if has_changes {
        let mut left_spans: Vec<Span> = Vec::new();
        let mut diff_w = 0;

        if let Some((ins, del)) = state.git.diff_stat {
            let s_ins = format!("+{ins}");
            diff_w += display_width(&s_ins);
            left_spans.push(Span::styled(s_ins, Style::default().fg(theme.diff_added)));

            left_spans.push(Span::styled("/", Style::default().fg(theme.text_muted)));
            diff_w += 1;

            let s_del = format!("-{del}");
            diff_w += display_width(&s_del);
            left_spans.push(Span::styled(s_del, Style::default().fg(theme.diff_deleted)));
        }

        let files_text = format!("{} files", state.git.changed_file_count());
        let files_w = display_width(&files_text);
        let gap = pad_to(diff_w + files_w, inner_w);
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
        Style::default().fg(theme.border_inactive),
    )));

    (lines, pr_link_info)
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

pub(super) fn draw_git_content(frame: &mut Frame, state: &mut AppState, inner: Rect) {
    let theme = &state.theme;
    let inner_w = inner.width as usize;

    // No git data loaded yet
    if state.git.branch.is_empty()
        && state.git.staged_files.is_empty()
        && state.git.unstaged_files.is_empty()
        && state.git.untracked_files.is_empty()
        && state.git.diff_stat.is_none()
    {
        super::render_centered(frame, inner, "Working tree clean", theme.text_muted);
        return;
    }

    // Render fixed header
    let (header_lines, pr_link) = render_git_header(state, inner_w);
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

    // Store PR hyperlink overlay for OSC 8 post-render
    if let Some(info) = pr_link {
        state
            .layout
            .hyperlink_overlays
            .push(crate::state::HyperlinkOverlay {
                x: inner.x + info.x_offset,
                y: inner.y + 1,
                text: info.text,
                url: info.url,
            });
    }

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
        super::render_centered(frame, content_area, "Working tree clean", theme.text_muted);
        return;
    }

    state.scrolls.git.total_lines = lines.len();
    state.scrolls.git.visible_height = content_height as usize;
    // Clamp `offset` to the new viewport. Without this, shrinking
    // content (e.g. the diff list drops entries between frames) can
    // leave the paragraph scrolled past its last line.
    state.scrolls.git.scroll(0);

    let scroll_offset = state.scrolls.git.offset as u16;
    let paragraph = Paragraph::new(lines).scroll((scroll_offset, 0));
    frame.render_widget(paragraph, content_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::GitFileEntry;
    use crate::state::AppState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    /// Render the git panel at the given size and return the resulting
    /// `AppState` plus the buffer-backed terminal. Used by every
    /// visual-assertion test in this file.
    fn draw(state: &mut AppState, width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 0, width, height);
                draw_git_content(frame, state, area);
            })
            .unwrap();
        terminal
    }

    /// Snapshot the git panel as plain text. Trailing whitespace on each row
    /// and trailing empty rows are trimmed so inline snapshots stay readable.
    fn render(state: &mut AppState, width: u16, height: u16) -> String {
        let terminal = draw(state, width, height);
        let buf = terminal.backend().buffer().clone();
        let mut rows: Vec<String> = Vec::new();
        for y in 0..buf.area.height {
            let mut line = String::new();
            for x in 0..buf.area.width {
                line.push_str(buf[(x, y)].symbol());
            }
            rows.push(line.trim_end().to_string());
        }
        while rows.last().is_some_and(|l| l.is_empty()) {
            rows.pop();
        }
        rows.join("\n")
    }

    /// Snapshot the git panel with foreground color and text modifier
    /// annotations per cell. Used when the assertion is about color or
    /// underline rather than plain characters.
    fn render_styled(state: &mut AppState, width: u16, height: u16) -> String {
        let terminal = draw(state, width, height);
        let buf = terminal.backend().buffer().clone();
        let mut rows: Vec<String> = Vec::new();
        for y in 0..buf.area.height {
            let mut line = String::new();
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                line.push_str(cell.symbol());
                let mut attrs: Vec<String> = Vec::new();
                if let Color::Indexed(n) = cell.fg {
                    attrs.push(format!("fg:{n}"));
                }
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    attrs.push("underline".into());
                }
                if cell.modifier.contains(Modifier::BOLD) {
                    attrs.push("bold".into());
                }
                if !attrs.is_empty() {
                    line.push_str(&format!("[{}]", attrs.join(",")));
                }
            }
            rows.push(line.trim_end().to_string());
        }
        while rows.last().is_some_and(|l| l.is_empty()) {
            rows.pop();
        }
        rows.join("\n")
    }

    fn file_entry(status: char, name: &str, additions: usize, deletions: usize) -> GitFileEntry {
        GitFileEntry {
            status,
            name: name.into(),
            additions,
            deletions,
            path: String::new(),
        }
    }

    // ─── PR hyperlink overlay state (non-visual) ────────────────────

    #[test]
    fn pr_link_overlay_has_correct_url() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.pr_number = Some("42".into());
        state.git.remote_url = "https://github.com/user/repo".into();
        draw(&mut state, 30, 4);
        let overlay = state
            .layout
            .hyperlink_overlays
            .first()
            .expect("PR overlay should be registered");
        assert_eq!(overlay.url, "https://github.com/user/repo/pull/42");
        assert_eq!(overlay.text, "#42");
    }

    #[test]
    fn pr_link_overlay_absent_without_remote_url() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.pr_number = Some("10".into());
        draw(&mut state, 30, 4);
        assert!(state.layout.hyperlink_overlays.is_empty());
    }

    #[test]
    fn pr_link_overlay_absent_without_pr_number() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.remote_url = "https://github.com/user/repo".into();
        draw(&mut state, 30, 4);
        assert!(state.layout.hyperlink_overlays.is_empty());
    }

    #[test]
    fn pr_link_overlay_right_aligned_on_second_row() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.pr_number = Some("7".into());
        state.git.remote_url = "https://github.com/user/repo".into();
        let width: u16 = 30;
        draw(&mut state, width, 4);
        let overlay = state.layout.hyperlink_overlays.first().unwrap();
        assert_eq!(
            overlay.x as usize,
            width as usize - display_width(&overlay.text),
        );
        assert_eq!(overlay.y, 1);
    }

    // ─── Branch / PR header rendering ────────────────────────────────

    #[test]
    fn header_renders_branch_with_ahead_behind_and_pr() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.ahead_behind = Some((2, 1));
        state.git.pr_number = Some("7".into());
        insta::assert_snapshot!(render(&mut state, 40, 4), @"

        main                             ↑2↓1 #7
        ────────────────────────────────────────
                   Working tree clean
        ");
    }

    #[test]
    fn header_renders_branch_with_ahead_behind_without_pr() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.ahead_behind = Some((2, 1));
        insta::assert_snapshot!(render(&mut state, 40, 4), @"

        main                                ↑2↓1
        ────────────────────────────────────────
                   Working tree clean
        ");
    }

    #[test]
    fn header_truncates_long_branch_to_fit_width() {
        let mut state = AppState::new(String::new());
        state.git.branch = "feature/sidebar/really-long-branch-name-that-should-truncate".into();
        state.git.ahead_behind = Some((2, 1));
        state.git.pr_number = Some("7".into());
        insta::assert_snapshot!(render(&mut state, 32, 4), @"

        feature/sidebar/really…  ↑2↓1 #7
        ────────────────────────────────
               Working tree clean
        ");
    }

    #[test]
    fn header_pr_number_is_underlined_and_colored() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.pr_number = Some("5".into());
        insta::assert_snapshot!(render_styled(&mut state, 30, 4), @"

        m[fg:255]a[fg:255]i[fg:255]n[fg:255]                        #[fg:117,underline]5[fg:117,underline]
        ─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]
              W[fg:252]o[fg:252]r[fg:252]k[fg:252]i[fg:252]n[fg:252]g[fg:252] [fg:252]t[fg:252]r[fg:252]e[fg:252]e[fg:252] [fg:252]c[fg:252]l[fg:252]e[fg:252]a[fg:252]n[fg:252]
        ");
    }

    // ─── Header structure (diff summary row) ─────────────────────────

    #[test]
    fn header_includes_blank_row_branch_and_diff_summary() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.diff_stat = Some((1, 0));
        insta::assert_snapshot!(render(&mut state, 40, 6), @"

        main
        +1/-0                            0 files
        ────────────────────────────────────────
                   Working tree clean
        ");
    }

    #[test]
    fn header_has_no_diff_row_when_no_changes() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        insta::assert_snapshot!(render(&mut state, 40, 5), @"

        main
        ────────────────────────────────────────
                   Working tree clean
        ");
    }

    #[test]
    fn header_diff_summary_right_aligns_file_count_with_stats() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.diff_stat = Some((10, 3));
        insta::assert_snapshot!(render(&mut state, 40, 4), @"

        main
        +10/-3                           0 files
        ────────────────────────────────────────
        ");
    }

    #[test]
    fn header_diff_summary_right_aligns_file_count_without_stats() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.staged_files = vec![file_entry('A', "new.rs", 1, 0)];
        insta::assert_snapshot!(render(&mut state, 40, 6), @"

        main
                                         1 files
        ────────────────────────────────────────
        Staged (1)
        A new.rs                           +1/-0
        ");
    }

    // ─── Section title color ─────────────────────────────────────────

    #[test]
    fn staged_section_title_uses_section_title_color() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.staged_files = vec![file_entry('M', "a.rs", 1, 0)];
        insta::assert_snapshot!(render_styled(&mut state, 40, 6), @"

        m[fg:255]a[fg:255]i[fg:255]n[fg:255]
                                         1[fg:252] [fg:252]f[fg:252]i[fg:252]l[fg:252]e[fg:252]s[fg:252]
        ─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]
        S[fg:109]t[fg:109]a[fg:109]g[fg:109]e[fg:109]d[fg:109] [fg:109]([fg:109]1[fg:109])[fg:109]
        M[fg:221] a[fg:252].[fg:252]r[fg:252]s[fg:252]                             +[fg:114]1[fg:114]/[fg:252]-[fg:174]0[fg:174]
        ");
    }

    #[test]
    fn untracked_section_title_uses_section_title_color() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.untracked_files = vec!["tmp.log".into()];
        insta::assert_snapshot!(render_styled(&mut state, 40, 6), @"

        m[fg:255]a[fg:255]i[fg:255]n[fg:255]
                                         1[fg:252] [fg:252]f[fg:252]i[fg:252]l[fg:252]e[fg:252]s[fg:252]
        ─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]─[fg:240]
        U[fg:109]n[fg:109]t[fg:109]r[fg:109]a[fg:109]c[fg:109]k[fg:109]e[fg:109]d[fg:109] [fg:109]([fg:109]1[fg:109])[fg:109]
        ?[fg:252] t[fg:252]m[fg:252]p[fg:252].[fg:252]l[fg:252]o[fg:252]g[fg:252]
        ");
    }

    // ─── "+N more" indicator right-alignment (untracked) ─────────────

    #[test]
    fn untracked_more_indicator_right_aligned_single_overflow() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.untracked_files = (0..11).map(|i| format!("file{i}.tmp")).collect();
        insta::assert_snapshot!(render(&mut state, 30, 20), @"

        main
                              11 files
        ──────────────────────────────
        Untracked (11)
        ? file0.tmp
        ? file1.tmp
        ? file2.tmp
        ? file3.tmp
        ? file4.tmp
        ? file5.tmp
        ? file6.tmp
        ? file7.tmp
        ? file8.tmp
        ? file9.tmp
                               +1 more
        ");
    }

    #[test]
    fn untracked_more_indicator_right_aligned_two_overflow() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.untracked_files = (0..12).map(|i| format!("file{i}.tmp")).collect();
        insta::assert_snapshot!(render(&mut state, 30, 20), @"

        main
                              12 files
        ──────────────────────────────
        Untracked (12)
        ? file0.tmp
        ? file1.tmp
        ? file2.tmp
        ? file3.tmp
        ? file4.tmp
        ? file5.tmp
        ? file6.tmp
        ? file7.tmp
        ? file8.tmp
        ? file9.tmp
                               +2 more
        ");
    }

    // ─── Edge case: truncation & narrow widths ───────────────────────

    #[test]
    fn staged_file_without_diff_uses_full_width() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.staged_files = vec![file_entry('M', "medium-length-name.rs", 0, 0)];
        insta::assert_snapshot!(render(&mut state, 40, 6), @"

        main
                                         1 files
        ────────────────────────────────────────
        Staged (1)
        M medium-length-name.rs
        ");
    }

    #[test]
    fn untracked_filename_is_truncated_with_ellipsis_at_narrow_width() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.untracked_files =
            vec!["a-very-long-untracked-filename-that-exceeds-width.tmp".into()];
        insta::assert_snapshot!(render(&mut state, 25, 6), @"

        main
                          1 files
        ─────────────────────────
        Untracked (1)
        ? a-very-long-untracked-…
        ");
    }

    #[test]
    fn staged_file_at_narrow_width_fits_diff_and_name() {
        let mut state = AppState::new(String::new());
        state.git.branch = "main".into();
        state.git.staged_files = vec![file_entry('A', "index.tsx", 100, 50)];
        insta::assert_snapshot!(render(&mut state, 20, 6), @"

        main
                     1 files
        ────────────────────
        Staged (1)
        A index.tsx +100/-50
        ");
    }

    #[test]
    fn header_fits_narrow_width_with_long_diff_stats() {
        let mut state = AppState::new(String::new());
        state.git.branch = "feature/branch".into();
        state.git.pr_number = Some("1".into());
        state.git.diff_stat = Some((999, 888));
        insta::assert_snapshot!(render(&mut state, 20, 5), @"

        feature/branch    #1
        +999/-888    0 files
        ────────────────────
         Working tree clean
        ");
    }
}
