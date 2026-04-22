use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

use crate::state::AppState;

mod files;
mod header;

use files::{render_file_section, render_untracked_section};
use header::render_git_header;

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
    use crate::ui::text::display_width;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

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
