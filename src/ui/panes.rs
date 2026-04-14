mod row;

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::state::{AppState, Focus, RepoFilter, StatusFilter};
use crate::tmux::PaneStatus;

use super::text::{display_width, truncate_to_width};

/// Render the status filter bar.
fn render_filter_bar<'a>(state: &AppState, bar_width: u16) -> Line<'a> {
    let theme = &state.theme;
    let icons = &state.icons;
    let (all, running, waiting, idle, error) = state.status_counts();

    let items: Vec<(StatusFilter, (&str, ratatui::style::Color), usize)> = vec![
        (StatusFilter::All, (icons.all_icon(), theme.status_all), all),
        (
            StatusFilter::Running,
            (
                icons.status_icon(&PaneStatus::Running),
                theme.status_running,
            ),
            running,
        ),
        (
            StatusFilter::Waiting,
            (
                icons.status_icon(&PaneStatus::Waiting),
                theme.status_waiting,
            ),
            waiting,
        ),
        (
            StatusFilter::Idle,
            (icons.status_icon(&PaneStatus::Idle), theme.status_idle),
            idle,
        ),
        (
            StatusFilter::Error,
            (icons.status_icon(&PaneStatus::Error), theme.status_error),
            error,
        ),
    ];

    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw(" "));

    for (i, (filter, (icon, icon_color), count)) in items.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }

        let is_selected = state.global.status_filter == filter;
        let icon_style = if is_selected {
            Style::default().fg(icon_color)
        } else {
            Style::default().fg(theme.filter_inactive)
        };
        spans.push(Span::styled(icon.to_string(), icon_style));

        let count_str = format!("{count}");
        let count_style = if count == 0 {
            Style::default().fg(theme.filter_inactive)
        } else {
            Style::default().fg(theme.text_active)
        };
        spans.push(Span::styled(count_str, count_style));
    }

    let _ = bar_width;

    Line::from(spans)
}

fn render_secondary_header<'a>(
    state: &AppState,
    width: u16,
) -> (Line<'a>, Option<u16>, Option<u16>) {
    let theme = &state.theme;
    let repo_icon = "▾";

    let repo_has_filter = !matches!(state.global.repo_filter, RepoFilter::All);
    let repo_style = if state.is_repo_popup_open() || repo_has_filter {
        Style::default().fg(theme.text_active)
    } else {
        Style::default().fg(theme.text_muted)
    };

    let has_notices_info = super::notices::has_info(state);
    let notices_button_col = has_notices_info.then_some(0);
    let notices_width = super::notices::BUTTON_WIDTH;
    let max_repo_label_width = width.saturating_sub((notices_width + 3) as u16) as usize;
    let repo_label = match &state.global.repo_filter {
        RepoFilter::All => "—".to_string(),
        RepoFilter::Repo(name) => truncate_to_width(name, max_repo_label_width),
    };
    let repo_btn_width = display_width(&repo_label) + 2; // label + space + arrow

    let gap = (width as usize).saturating_sub(repo_btn_width + notices_width);
    let repo_button_col = Some((notices_width + gap) as u16);

    let mut spans: Vec<Span<'a>> = Vec::new();
    if has_notices_info {
        spans.push(super::notices::button_span(state));
        spans.push(Span::raw(" "));
    } else {
        spans.push(Span::raw("  "));
    }
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(Span::styled(repo_label, repo_style));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(repo_icon, repo_style));

    (Line::from(spans), notices_button_col, repo_button_col)
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

pub fn draw_agents(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let width = area.width as usize;

    // Fixed filter bar (1 row)
    let filter_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1.min(area.height),
    };
    let filter_line = render_filter_bar(state, area.width);
    frame.render_widget(Paragraph::new(vec![filter_line]), filter_area);

    let secondary_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: 1.min(area.height.saturating_sub(1)),
    };
    let (secondary_line, notices_btn_col, repo_btn_col) =
        render_secondary_header(state, area.width);
    state.notices.button_col = notices_btn_col;
    state.repo_button_col = repo_btn_col;
    frame.render_widget(Paragraph::new(vec![secondary_line]), secondary_area);

    // Scrollable agent list below
    let list_area = Rect {
        x: area.x,
        y: area.y + 2,
        width: area.width,
        height: area.height.saturating_sub(2),
    };

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut line_to_row: Vec<Option<usize>> = Vec::new();
    let mut row_index: usize = 0;

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
            .focused_pane_id
            .as_ref()
            .is_some_and(|fid| group.panes.iter().any(|(p, _)| p.pane_id == *fid));

        // Plain repo header at column 0 (no frame).
        let title = &group.name;
        let title_color = if group_has_focused_pane {
            theme.accent
        } else {
            theme.text_active
        };
        lines.push(Line::from(Span::styled(
            title.clone(),
            Style::default().fg(title_color),
        )));
        line_to_row.push(None);

        for (pane, git_info) in filtered_panes.iter() {
            let is_selected = state.sidebar_focused
                && state.focus == Focus::Panes
                && row_index == state.global.selected_pane_row;

            let is_active = state.focused_pane_id.as_ref() == Some(&pane.pane_id);

            let pane_state = state.pane_state(&pane.pane_id);
            let ports = pane_state.map(|s| s.ports.as_slice());
            let task_progress = pane_state.and_then(|s| s.task_progress.as_ref());
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

            row_index += 1;
        }
    }

    state.line_to_row = line_to_row;
    state.panes_scroll.total_lines = lines.len();
    state.panes_scroll.visible_height = list_area.height as usize;

    // Auto-scroll to keep selected agent visible
    if state.sidebar_focused && state.focus == Focus::Panes {
        let mut first_line: Option<usize> = None;
        let mut last_line: Option<usize> = None;
        for (i, mapping) in state.line_to_row.iter().enumerate() {
            if *mapping == Some(state.global.selected_pane_row) {
                if first_line.is_none() {
                    first_line = Some(i);
                }
                last_line = Some(i);
            }
        }
        if let (Some(first), Some(last)) = (first_line, last_line) {
            let visible_h = list_area.height as usize;
            let offset = state.panes_scroll.offset;
            if first < offset {
                state.panes_scroll.offset = first.saturating_sub(1);
            } else if last >= offset + visible_h {
                state.panes_scroll.offset = (last + 1).saturating_sub(visible_h);
            }
        }
    }

    let paragraph = Paragraph::new(lines).scroll((state.panes_scroll.offset as u16, 0));
    frame.render_widget(paragraph, list_area);

    // Render popup overlay on top if open
    if state.is_notices_popup_open() {
        super::notices::render_notices_popup(frame, state, area);
    } else if state.is_repo_popup_open() {
        render_repo_popup(frame, state, area);
    }
}

#[cfg(test)]
use crate::group::PaneGitInfo;

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn snapshot_secondary_header_omits_version_banner_when_notice_present() {
        // Version notices light up the `ⓘ` indicator in the header but
        // must not leak the "new release vX.Y.Z" banner into the row —
        // the banner lives in the popup, not the header. A snapshot here
        // catches any regression that would put banner text back on the
        // row, including subtle width or spacing drift.
        let mut state = crate::state::AppState::new(String::new());
        state.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });

        let text = line_text(&render_secondary_header(&state, 30).0);
        insta::assert_snapshot!(text, @"ⓘ                          — ▾");
    }

    #[test]
    fn render_secondary_header_keeps_repo_position_with_or_without_notices_info() {
        let mut with_info = AppState::new(String::new());
        with_info.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });
        with_info.notices.missing_hook_groups = vec![crate::state::NoticesMissingHookGroup {
            agent: "claude".into(),
            hooks: vec!["SessionStart".into()],
        }];

        let without_info = AppState::new(String::new());

        let (_, _, with_repo_col) = render_secondary_header(&with_info, 30);
        let (_, _, without_repo_col) = render_secondary_header(&without_info, 30);

        assert_eq!(with_repo_col, without_repo_col);
        assert_eq!(with_repo_col, Some(27));
    }

    #[test]
    fn snapshot_secondary_header_without_notices_info() {
        let state = AppState::new(String::new());
        let text = line_text(&render_secondary_header(&state, 30).0);
        insta::assert_snapshot!(text, @"                           — ▾");
    }

    #[test]
    fn snapshot_secondary_header_with_version_only() {
        let mut state = AppState::new(String::new());
        state.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });
        let text = line_text(&render_secondary_header(&state, 30).0);
        insta::assert_snapshot!(text, @"ⓘ                          — ▾");
    }

    #[test]
    fn snapshot_secondary_header_with_hooks_only() {
        let mut state = AppState::new(String::new());
        state.notices.missing_hook_groups = vec![crate::state::NoticesMissingHookGroup {
            agent: "claude".into(),
            hooks: vec!["SessionStart".into()],
        }];
        let text = line_text(&render_secondary_header(&state, 30).0);
        insta::assert_snapshot!(text, @"ⓘ                          — ▾");
    }

    #[test]
    fn snapshot_secondary_header_with_version_and_hooks() {
        let mut state = AppState::new(String::new());
        state.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });
        state.notices.missing_hook_groups = vec![crate::state::NoticesMissingHookGroup {
            agent: "claude".into(),
            hooks: vec!["SessionStart".into()],
        }];
        let text = line_text(&render_secondary_header(&state, 30).0);
        insta::assert_snapshot!(text, @"ⓘ                          — ▾");
    }

    // ─── render_filter_bar tests ──────────────────────────────

    fn make_state_with_groups(groups: Vec<crate::group::RepoGroup>) -> AppState {
        let mut state = AppState::new("%99".into());
        state.repo_groups = groups;
        state.rebuild_row_targets();
        state
    }

    fn filter_bar_text(state: &AppState, width: u16) -> String {
        let line = render_filter_bar(state, width);
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn render_filter_bar_is_status_only() {
        let state = make_state_with_groups(vec![]);
        let text = filter_bar_text(&state, 28);
        assert!(
            !text.contains("▾"),
            "status filter bar should not contain repo button"
        );
    }

    #[test]
    fn render_filter_bar_uses_selected_and_inactive_icon_colors() {
        let pane1 = crate::tmux::PaneInfo {
            pane_id: "%2".into(),
            pane_active: true,
            status: PaneStatus::Running,
            attention: false,
            agent: crate::tmux::AgentType::Claude,
            path: String::new(),
            current_command: String::new(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: crate::tmux::PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree_name: String::new(),
            worktree_branch: String::new(),
            session_id: None,
            session_name: String::new(),
        };
        let pane2 = crate::tmux::PaneInfo {
            pane_id: "%3".into(),
            pane_active: false,
            status: PaneStatus::Idle,
            attention: false,
            agent: crate::tmux::AgentType::Codex,
            path: String::new(),
            current_command: String::new(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: crate::tmux::PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree_name: String::new(),
            worktree_branch: String::new(),
            session_id: None,
            session_name: String::new(),
        };
        let mut state = make_state_with_groups(vec![crate::group::RepoGroup {
            name: "project".into(),
            has_focus: true,
            panes: vec![
                (pane1, PaneGitInfo::default()),
                (pane2, PaneGitInfo::default()),
            ],
        }]);
        state.global.status_filter = StatusFilter::Running;
        let theme = &state.theme;

        let line = render_filter_bar(&state, 30);
        let cells: Vec<_> = line
            .spans
            .iter()
            .filter(|span| !span.content.as_ref().trim().is_empty())
            .collect();

        assert_eq!(cells.len(), 10);

        assert_eq!(cells[0].content.as_ref(), "≡");
        assert_eq!(cells[0].style.fg, Some(theme.filter_inactive));
        assert!(!cells[0].style.add_modifier.contains(Modifier::UNDERLINED));

        assert_eq!(cells[1].content.as_ref(), "2");
        assert_eq!(cells[1].style.fg, Some(theme.text_active));

        assert_eq!(cells[2].content.as_ref(), "●");
        assert_eq!(cells[2].style.fg, Some(theme.status_running));
        assert!(!cells[2].style.add_modifier.contains(Modifier::UNDERLINED));

        assert_eq!(cells[3].content.as_ref(), "1");
        assert_eq!(cells[3].style.fg, Some(theme.text_active));

        assert_eq!(cells[4].content.as_ref(), "◐");
        assert_eq!(cells[4].style.fg, Some(theme.filter_inactive));

        assert_eq!(cells[5].content.as_ref(), "0");
        assert_eq!(cells[5].style.fg, Some(theme.filter_inactive));

        assert_eq!(cells[6].content.as_ref(), "○");
        assert_eq!(cells[6].style.fg, Some(theme.filter_inactive));

        assert_eq!(cells[7].content.as_ref(), "1");
        assert_eq!(cells[7].style.fg, Some(theme.text_active));

        assert_eq!(cells[8].content.as_ref(), "✕");
        assert_eq!(cells[8].style.fg, Some(theme.filter_inactive));

        assert_eq!(cells[9].content.as_ref(), "0");
        assert_eq!(cells[9].style.fg, Some(theme.filter_inactive));
    }

    #[test]
    fn render_secondary_header_repo_button_col_returned() {
        let state = make_state_with_groups(vec![]);
        let (_, _, col) = render_secondary_header(&state, 28);
        assert_eq!(col, Some(25), "repo button should be right-aligned");
    }

    #[test]
    fn snapshot_secondary_header_shows_notices_indicator_when_missing_hooks_exist() {
        // Visual regression check: the indicator MUST sit at column 0
        // and the repo filter MUST stay pinned to the right edge when
        // missing hooks are present. A snapshot catches any drift in
        // spacing, glyph, or column alignment that a `starts_with` /
        // `contains` probe would silently miss.
        let mut state = make_state_with_groups(vec![]);
        state.notices.missing_hook_groups = vec![crate::state::NoticesMissingHookGroup {
            agent: "claude".into(),
            hooks: vec!["SessionStart".into(), "Stop".into()],
        }];

        let (line, notices_col, repo_col) = render_secondary_header(&state, 28);
        let text = line_text(&line);
        insta::assert_snapshot!(text, @"ⓘ                        — ▾");
        // Click-target columns are layout state, not visible characters,
        // so they stay as direct equality checks alongside the snapshot.
        assert_eq!(notices_col, Some(0));
        assert_eq!(repo_col, Some(25));
    }

    #[test]
    fn render_secondary_header_shows_repo_name_when_filtered() {
        let mut state = make_state_with_groups(vec![crate::group::RepoGroup {
            name: "my-app".into(),
            has_focus: true,
            panes: vec![],
        }]);
        state.global.repo_filter = RepoFilter::Repo("my-app".into());
        let text = line_text(&render_secondary_header(&state, 40).0);
        assert!(
            text.contains("my-app"),
            "secondary header should show filtered repo name, got: {text}"
        );
        assert!(
            text.find("my-app").unwrap() < text.find("▾").unwrap(),
            "repo name should come before the arrow"
        );
        let (line, _, _) = render_secondary_header(&state, 40);
        let repo_span = line
            .spans
            .iter()
            .find(|span| span.content.contains("my-app"))
            .unwrap();
        assert!(
            !repo_span.style.add_modifier.contains(Modifier::BOLD),
            "filtered repo label should not be bold"
        );
    }

    #[test]
    fn render_secondary_header_truncates_long_repo_name() {
        let mut state = make_state_with_groups(vec![crate::group::RepoGroup {
            name: "very-long-repository-name-that-exceeds-width".into(),
            has_focus: true,
            panes: vec![],
        }]);
        state.global.repo_filter =
            RepoFilter::Repo("very-long-repository-name-that-exceeds-width".into());
        let text = line_text(&render_secondary_header(&state, 28).0);
        assert!(
            text.contains('…'),
            "repo name should be truncated with an ellipsis"
        );
        assert!(text.contains("▾"));
        assert!(
            !text.contains("very-long-repository-name-that-exceeds-width"),
            "repo name should not fit in full at this width"
        );
        assert!(
            text.find('…').unwrap() < text.find("▾").unwrap(),
            "repo name should come before the arrow"
        );
    }

    #[test]
    fn render_secondary_header_popup_open_styling() {
        let mut state = make_state_with_groups(vec![]);
        state.popup = crate::state::PopupState::Repo {
            selected: 0,
            area: None,
        };
        let (line, _, _) = render_secondary_header(&state, 28);
        let last_span = line.spans.last().unwrap();
        assert!(
            !last_span.style.add_modifier.contains(Modifier::UNDERLINED),
            "repo button should not be underlined when popup is open"
        );
        assert!(
            !last_span.style.add_modifier.contains(Modifier::BOLD),
            "repo button should not be bold when popup is open"
        );
    }
}
