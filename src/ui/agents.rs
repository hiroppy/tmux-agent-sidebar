use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::state::{AgentFilter, AppState, Focus, RepoFilter};
use crate::tmux::PaneStatus;
use crate::ui::colors::ColorTheme;

use super::text::{
    display_width, elapsed_label, pad_to, truncate_to_width, wait_reason_label, wrap_text,
    wrap_text_char,
};

/// Render the filter bar. Returns (Line, repo_button_col).
fn render_filter_bar<'a>(state: &AppState, bar_width: u16) -> (Line<'a>, u16) {
    let theme = &state.theme;
    let (all, running, waiting, idle, error) = state.status_counts();

    let items: Vec<(AgentFilter, Option<(&str, ratatui::style::Color)>, usize)> = vec![
        (AgentFilter::All, None, all),
        (
            AgentFilter::Running,
            Some((PaneStatus::Running.icon(), theme.status_running)),
            running,
        ),
        (
            AgentFilter::Waiting,
            Some((PaneStatus::Waiting.icon(), theme.status_waiting)),
            waiting,
        ),
        (
            AgentFilter::Idle,
            Some((PaneStatus::Idle.icon(), theme.status_idle)),
            idle,
        ),
        (
            AgentFilter::Error,
            Some((PaneStatus::Error.icon(), theme.status_error)),
            error,
        ),
    ];

    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw(" "));
    let mut current_width: usize = 1;

    for (i, (filter, icon_info, count)) in items.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
            current_width += 2;
        }

        let is_selected = state.global.agent_filter == filter;

        if let Some((icon, icon_color)) = icon_info {
            let mut icon_style = if is_selected || count > 0 {
                Style::default().fg(icon_color)
            } else {
                Style::default().fg(theme.border_inactive)
            };
            if is_selected {
                icon_style = icon_style.add_modifier(Modifier::UNDERLINED);
            }
            spans.push(Span::styled(icon.to_string(), icon_style));
            current_width += display_width(icon);

            let count_str = format!("{count}");
            let count_style = if is_selected {
                Style::default()
                    .fg(theme.text_active)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if count == 0 {
                Style::default().fg(theme.border_inactive)
            } else {
                Style::default().fg(theme.text_muted)
            };
            current_width += count_str.len();
            spans.push(Span::styled(count_str, count_style));
        } else {
            let style = if is_selected {
                Style::default()
                    .fg(theme.text_active)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(theme.text_muted)
            };
            spans.push(Span::styled("All", style));
            current_width += 3;
        }
    }

    // Repo filter button — right-aligned
    let repo_icon = "▼";
    let repo_label = match &state.global.repo_filter {
        RepoFilter::All => repo_icon.to_string(),
        RepoFilter::Repo(name) => {
            let max_w = 8;
            let truncated = truncate_to_width(name, max_w);
            format!("{} {}", repo_icon, truncated)
        }
    };
    let repo_btn_width = display_width(&repo_label) + 1; // 1 for leading space
    let gap = (bar_width as usize).saturating_sub(current_width + repo_btn_width);
    let repo_button_col = (current_width + gap) as u16;

    spans.push(Span::raw(" ".repeat(gap)));

    let repo_has_filter = !matches!(state.global.repo_filter, RepoFilter::All);
    let repo_style = if state.repo_popup_open {
        Style::default()
            .fg(theme.text_active)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else if repo_has_filter {
        Style::default()
            .fg(theme.text_active)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_muted)
    };
    spans.push(Span::styled(format!(" {}", repo_label), repo_style));

    (Line::from(spans), repo_button_col)
}

fn render_repo_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let repos = state.repo_names();
    if repos.is_empty() {
        return;
    }

    let max_name_len = repos.iter().map(|r| display_width(r)).max().unwrap_or(3);
    // Width: marker(2) + name + padding(1) + borders(2)
    let popup_width = (max_name_len + 5).min(area.width as usize).max(10) as u16;
    let popup_height = (repos.len() as u16 + 2).min(area.height.saturating_sub(1)); // +2 for borders

    // Right-aligned, below filter bar
    let popup_x = area.x + area.width.saturating_sub(popup_width);
    let popup_y = area.y + 1;

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
    state.repo_popup_area = Some(popup_rect);

    frame.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active));
    let inner = block.inner(popup_rect);
    frame.render_widget(block, popup_rect);

    let inner_width = inner.width as usize;
    for (i, name) in repos.iter().enumerate() {
        if i >= inner.height as usize {
            break;
        }

        let is_highlighted = i == state.repo_popup_selected;
        let is_current = match &state.global.repo_filter {
            RepoFilter::All => i == 0,
            RepoFilter::Repo(n) => *n == *name,
        };

        let marker = if is_current { "● " } else { "  " };
        let truncated = truncate_to_width(name, inner_width.saturating_sub(2));
        let text = format!("{}{}", marker, truncated);
        let text_dw = display_width(&text);
        let padding = " ".repeat(inner_width.saturating_sub(text_dw));

        let style = if is_highlighted {
            Style::default()
                .fg(theme.text_active)
                .bg(theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .fg(theme.text_active)
                .add_modifier(Modifier::BOLD)
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
    let (filter_line, repo_btn_col) = render_filter_bar(state, area.width);
    state.repo_button_col = repo_btn_col;
    frame.render_widget(Paragraph::new(vec![filter_line]), filter_area);

    // Scrollable agent list below
    let list_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut line_to_row: Vec<Option<usize>> = Vec::new();
    let mut row_index: usize = 0;

    let filter = state.global.agent_filter;

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

        let group_has_focused_pane = state.focused_pane_id.as_ref().map_or(false, |fid| {
            group.panes.iter().any(|(p, _)| p.pane_id == *fid)
        });

        let border_color = if group_has_focused_pane {
            theme.border_active
        } else {
            theme.border_inactive
        };
        let title = &group.name;

        let title_dw = display_width(title);
        let fill_len = width.saturating_sub(3 + title_dw + 1);
        let title_color = if group_has_focused_pane {
            theme.border_active
        } else {
            theme.text_muted
        };
        lines.push(Line::from(vec![
            Span::styled("╭ ", Style::default().fg(border_color)),
            Span::styled(title.clone(), Style::default().fg(title_color)),
            Span::styled(
                format!(" {}╮", "─".repeat(fill_len)),
                Style::default().fg(border_color),
            ),
        ]));
        line_to_row.push(None);

        for (pi, (pane, git_info)) in filtered_panes.iter().enumerate() {
            if pi > 0 {
                let gray = Style::default().fg(theme.border_inactive);
                let dashes = "─".repeat(width.saturating_sub(4));
                lines.push(Line::from(vec![
                    Span::styled("│", Style::default().fg(border_color)),
                    Span::styled(format!(" {} ", dashes), gray),
                    Span::styled("│", Style::default().fg(border_color)),
                ]));
                line_to_row.push(None);
            }

            let is_selected = state.sidebar_focused
                && state.focus == Focus::Agents
                && row_index == state.global.selected_agent_row;

            let is_active = state
                .focused_pane_id
                .as_ref()
                .map_or(false, |id| id == &pane.pane_id);

            let task_progress = state.pane_task_progress.get(&pane.pane_id);
            let pane_lines = render_pane_lines(
                pane,
                git_info,
                task_progress,
                is_selected,
                is_active,
                border_color,
                width,
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

        let bottom_line = format!("╰{}╯", "─".repeat(width.saturating_sub(2)));
        lines.push(Line::from(Span::styled(
            bottom_line,
            Style::default().fg(border_color),
        )));
        line_to_row.push(None);
    }

    state.line_to_row = line_to_row;
    state.agents_scroll.total_lines = lines.len();
    state.agents_scroll.visible_height = list_area.height as usize;

    // Auto-scroll to keep selected agent visible
    if state.sidebar_focused && state.focus == Focus::Agents {
        let mut first_line: Option<usize> = None;
        let mut last_line: Option<usize> = None;
        for (i, mapping) in state.line_to_row.iter().enumerate() {
            if *mapping == Some(state.global.selected_agent_row) {
                if first_line.is_none() {
                    first_line = Some(i);
                }
                last_line = Some(i);
            }
        }
        if let (Some(first), Some(last)) = (first_line, last_line) {
            let mut effective_last = last;
            for i in (last + 1)..state.line_to_row.len() {
                if state.line_to_row[i].is_none() {
                    effective_last = i;
                } else {
                    break;
                }
            }
            let visible_h = list_area.height as usize;
            let offset = state.agents_scroll.offset;
            if first < offset {
                state.agents_scroll.offset = first.saturating_sub(1);
            } else if effective_last >= offset + visible_h {
                state.agents_scroll.offset = (effective_last + 1).saturating_sub(visible_h);
            }
        }
    }

    let paragraph = Paragraph::new(lines).scroll((state.agents_scroll.offset as u16, 0));
    frame.render_widget(paragraph, list_area);

    // Render popup overlay on top if open
    if state.repo_popup_open {
        render_repo_popup(frame, state, area);
    }
}

fn bordered_line<'a>(
    border_style: Style,
    apply_bg: &dyn Fn(Style) -> Style,
    inner_width: usize,
    content_spans: Vec<Span<'a>>,
    content_width: usize,
) -> Line<'a> {
    let padding = pad_to(content_width, inner_width);
    let mut spans = vec![
        Span::styled("│", border_style),
        Span::styled(" ", apply_bg(Style::default())),
    ];
    spans.extend(content_spans);
    spans.push(Span::styled(padding, apply_bg(Style::default())));
    spans.push(Span::styled("│", border_style));
    Line::from(spans)
}

fn render_pane_lines<'a>(
    pane: &crate::tmux::PaneInfo,
    git_info: &crate::group::PaneGitInfo,
    task_progress: Option<&crate::activity::TaskProgress>,
    selected: bool,
    active: bool,
    border_color: ratatui::style::Color,
    width: usize,
    theme: &ColorTheme,
    spinner_frame: usize,
    now: u64,
) -> Vec<Line<'a>> {
    let mut out: Vec<Line<'a>> = Vec::new();

    let border_style = Style::default().fg(border_color);
    let inner_width = width.saturating_sub(3);

    let (icon, pulse_color) = running_icon_for(&pane.status, spinner_frame);
    let icon_color =
        pulse_color.unwrap_or_else(|| theme.status_color(&pane.status, pane.attention));
    use crate::tmux::PermissionMode;
    let label = pane.agent.label();
    let badge = pane.permission_mode.badge();
    let elapsed = elapsed_label(pane.started_at, now);

    let agent_fg = theme.agent_color(&pane.agent);
    let is_active_status = matches!(pane.status, PaneStatus::Running | PaneStatus::Waiting);
    let elapsed_fg = if is_active_status {
        theme.text_active
    } else {
        theme.text_muted
    };
    let active_mod = if active {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };
    let bg = if selected {
        Some(theme.selection_bg)
    } else {
        None
    };

    let apply_bg = |s: Style| match bg {
        Some(c) => s.bg(c),
        None => s,
    };

    let badge_extra = if badge.is_empty() { 0 } else { 1 };
    let left_dw =
        display_width(icon) + 1 + display_width(label) + badge_extra + display_width(badge);
    let available_for_elapsed = inner_width.saturating_sub(left_dw);
    let elapsed = truncate_to_width(&elapsed, available_for_elapsed);
    let elapsed_dw = display_width(&elapsed);
    let padding = pad_to(left_dw + elapsed_dw, inner_width);

    let mut status_spans = vec![
        Span::styled("│", border_style),
        Span::styled(" ", apply_bg(Style::default())),
        Span::styled(icon.to_string(), apply_bg(Style::default().fg(icon_color))),
        Span::styled(
            format!(" {}", label),
            apply_bg(Style::default().fg(agent_fg).add_modifier(active_mod)),
        ),
    ];
    if !badge.is_empty() {
        let badge_color = match pane.permission_mode {
            PermissionMode::BypassPermissions => theme.badge_danger,
            PermissionMode::Auto => theme.badge_auto,
            PermissionMode::Plan => theme.badge_plan,
            PermissionMode::AcceptEdits => theme.badge_auto,
            PermissionMode::Default => theme.text_muted,
        };
        status_spans.push(Span::styled(
            format!(" {}", badge),
            apply_bg(Style::default().fg(badge_color)),
        ));
    }
    status_spans.push(Span::styled(padding, apply_bg(Style::default())));
    status_spans.push(Span::styled(
        elapsed,
        apply_bg(Style::default().fg(elapsed_fg)),
    ));
    status_spans.push(Span::styled("│", border_style));
    out.push(Line::from(status_spans));

    // Branch info line
    let branch = super::text::branch_label(git_info);
    if !branch.is_empty() {
        let branch_color = theme.branch;
        let prefix = "  ";
        let max_branch_width = inner_width.saturating_sub(display_width(prefix));
        let truncated = truncate_to_width(&branch, max_branch_width);
        let text = format!("{}{}", prefix, truncated);
        let text_dw = display_width(&text);
        out.push(bordered_line(
            border_style,
            &apply_bg,
            inner_width,
            vec![Span::styled(
                text,
                apply_bg(Style::default().fg(branch_color)),
            )],
            text_dw,
        ));
    }

    // Task progress line
    if let Some(progress) = task_progress {
        if !progress.is_empty() {
            use crate::activity::TaskStatus;
            let mut icons = String::new();
            for (_, status) in &progress.tasks {
                let ch = match status {
                    TaskStatus::Completed => "✔",
                    TaskStatus::InProgress => "◼",
                    TaskStatus::Pending => "◻",
                };
                icons.push_str(ch);
            }
            let summary = format!(
                "  {} {}/{}",
                icons,
                progress.completed_count(),
                progress.total()
            );
            let summary_dw = display_width(&summary);
            let task_color = theme.task_progress;
            out.push(bordered_line(
                border_style,
                &apply_bg,
                inner_width,
                vec![Span::styled(
                    summary,
                    apply_bg(Style::default().fg(task_color)),
                )],
                summary_dw,
            ));
        }
    }

    if !pane.subagents.is_empty() {
        let subagent_color = theme.subagent;
        let tree_color = theme.text_muted;
        let last_idx = pane.subagents.len() - 1;
        for (i, sa) in pane.subagents.iter().enumerate() {
            let connector = if i == last_idx { "└ " } else { "├ " };
            let numbered = if sa.contains('#') {
                sa.clone()
            } else {
                format!("{} #{}", sa, i + 1)
            };
            let prefix = format!("  {}", connector);
            let prefix_dw = display_width(&prefix);
            let max_sa_w = inner_width.saturating_sub(prefix_dw);
            let truncated_sa = truncate_to_width(&numbered, max_sa_w);
            let text_dw = prefix_dw + display_width(&truncated_sa);
            out.push(bordered_line(
                border_style,
                &apply_bg,
                inner_width,
                vec![
                    Span::styled(prefix, apply_bg(Style::default().fg(tree_color))),
                    Span::styled(truncated_sa, apply_bg(Style::default().fg(subagent_color))),
                ],
                text_dw,
            ));
        }
    }

    if !pane.wait_reason.is_empty() {
        let reason = wait_reason_label(&pane.wait_reason);
        let text = format!("  {}", reason);
        let text_dw = display_width(&text);
        let reason_color = if matches!(pane.status, PaneStatus::Error) {
            theme.status_error
        } else {
            theme.wait_reason
        };
        out.push(bordered_line(
            border_style,
            &apply_bg,
            inner_width,
            vec![Span::styled(
                text,
                apply_bg(Style::default().fg(reason_color)),
            )],
            text_dw,
        ));
    }

    if !pane.prompt.is_empty() {
        let is_response = pane.prompt_is_response;
        let prompt_color = if active {
            theme.text_active
        } else {
            theme.text_muted
        };
        let display_prompt = pane.prompt.clone();
        let wrap_width = inner_width.saturating_sub(if is_response { 4 } else { 2 });
        let wrapped = if is_response {
            wrap_text_char(&display_prompt, wrap_width, 3)
        } else {
            wrap_text(&display_prompt, wrap_width, 3)
        };
        for (li, wl) in wrapped.iter().enumerate() {
            if is_response && li == 0 {
                let arrow_color = theme.diff_added;
                let text_dw = 4 + display_width(wl); // "  ▶ " + text
                out.push(bordered_line(
                    border_style,
                    &apply_bg,
                    inner_width,
                    vec![
                        Span::styled(
                            "  ▶ ",
                            apply_bg(
                                Style::default()
                                    .fg(arrow_color)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ),
                        Span::styled(wl.clone(), apply_bg(Style::default().fg(prompt_color))),
                    ],
                    text_dw,
                ));
            } else {
                let indent = if is_response { "    " } else { "  " };
                let text = format!("{}{}", indent, wl);
                let text_dw = display_width(&text);
                out.push(bordered_line(
                    border_style,
                    &apply_bg,
                    inner_width,
                    vec![Span::styled(
                        text,
                        apply_bg(Style::default().fg(prompt_color)),
                    )],
                    text_dw,
                ));
            }
        }
    } else if matches!(pane.status, PaneStatus::Idle) {
        let text = "  Waiting for prompt…";
        let text_dw = display_width(text);
        let idle_color = if active {
            theme.text_active
        } else {
            theme.text_muted
        };
        out.push(bordered_line(
            border_style,
            &apply_bg,
            inner_width,
            vec![Span::styled(
                text.to_string(),
                apply_bg(Style::default().fg(idle_color)),
            )],
            text_dw,
        ));
    }

    out
}

pub(crate) fn running_icon_for(
    status: &PaneStatus,
    spinner_frame: usize,
) -> (&'static str, Option<ratatui::style::Color>) {
    use crate::{SPINNER_ICON, SPINNER_PULSE};

    match status {
        PaneStatus::Running => {
            let color_idx = SPINNER_PULSE[spinner_frame % SPINNER_PULSE.len()];
            (
                SPINNER_ICON,
                Some(ratatui::style::Color::Indexed(color_idx)),
            )
        }
        _ => (status.icon(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::PaneGitInfo;
    use crate::tmux::{AgentType, PaneInfo, PermissionMode};

    fn pane(permission_mode: PermissionMode, status: PaneStatus, prompt: &str) -> PaneInfo {
        pane_with_response(permission_mode, status, prompt, false)
    }

    fn pane_with_response(
        permission_mode: PermissionMode,
        status: PaneStatus,
        prompt: &str,
        is_response: bool,
    ) -> PaneInfo {
        PaneInfo {
            pane_id: "%1".into(),
            pane_active: false,
            status,
            attention: false,
            agent: AgentType::Codex,
            path: "/tmp/project".into(),
            prompt: prompt.into(),
            prompt_is_response: is_response,
            started_at: None,
            wait_reason: String::new(),
            permission_mode,
            subagents: vec![],
            pane_pid: None,
            worktree_name: String::new(),
            worktree_branch: String::new(),
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn render_pane_lines_shows_permission_badge() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Auto, PaneStatus::Running, "");
        let lines = render_pane_lines(
            &pane,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        let status = line_text(&lines[0]);
        assert!(status.contains(" codex auto"));
    }

    #[test]
    fn render_pane_lines_uses_injected_now_for_elapsed() {
        let theme = ColorTheme::default();
        let mut pane = pane(PermissionMode::Default, PaneStatus::Running, "");
        pane.started_at = Some(1_000_000 - 125);
        let lines = render_pane_lines(
            &pane,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            1_000_000,
        );

        let status = line_text(&lines[0]);
        assert!(status.contains("2m5s"));
    }

    #[test]
    fn running_icon_for_all_statuses() {
        assert_eq!(running_icon_for(&PaneStatus::Idle, 0), ("○", None));
        assert_eq!(running_icon_for(&PaneStatus::Waiting, 0), ("◐", None));
        assert_eq!(running_icon_for(&PaneStatus::Error, 0), ("✕", None));
        assert_eq!(running_icon_for(&PaneStatus::Unknown, 0), ("·", None));

        let (icon, color) = running_icon_for(&PaneStatus::Running, 0);
        assert_eq!(icon, "●");
        assert_eq!(color, Some(ratatui::style::Color::Indexed(82)));
    }

    #[test]
    fn render_pane_lines_shows_idle_prompt_hint() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Default, PaneStatus::Idle, "");
        let lines = render_pane_lines(
            &pane,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        assert_eq!(lines.len(), 2);
        let hint = line_text(&lines[1]);
        assert!(hint.contains("Waiting for prompt"));
    }

    #[test]
    fn render_pane_lines_wraps_prompt_when_present() {
        let theme = ColorTheme::default();
        let pane = pane(
            PermissionMode::BypassPermissions,
            PaneStatus::Idle,
            "hello world from codex",
        );
        let lines = render_pane_lines(
            &pane,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            18,
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        let status = line_text(&lines[0]);
        assert!(status.contains(" codex !"));
        assert!(!line_text(&lines[1]).contains("Waiting for prompt"));
    }

    #[test]
    fn render_pane_lines_shows_single_subagent() {
        let theme = ColorTheme::default();
        let mut p = pane(PermissionMode::Default, PaneStatus::Running, "test");
        p.subagents = vec!["Explore".into()];
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        // status + subagent + prompt = 3 lines minimum
        assert!(lines.len() >= 3);
        let sub_line = line_text(&lines[1]);
        assert!(sub_line.contains("└ "));
        assert!(sub_line.contains("Explore #1"));
    }

    #[test]
    fn render_pane_lines_shows_multiple_subagents_tree() {
        let theme = ColorTheme::default();
        let mut p = pane(PermissionMode::Default, PaneStatus::Running, "test");
        p.subagents = vec!["Explore #1".into(), "Plan".into(), "Explore #2".into()];
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        // status + 3 subagents + prompt = 5 lines minimum
        assert!(lines.len() >= 5);
        assert!(line_text(&lines[1]).contains("├ "));
        assert!(line_text(&lines[1]).contains("Explore #1"));
        assert!(line_text(&lines[2]).contains("├ "));
        assert!(line_text(&lines[2]).contains("Plan #2"));
        assert!(line_text(&lines[3]).contains("└ "));
        assert!(line_text(&lines[3]).contains("Explore #2"));
    }

    #[test]
    fn render_pane_lines_subagents_before_wait_reason() {
        let theme = ColorTheme::default();
        let mut p = pane(PermissionMode::Default, PaneStatus::Waiting, "");
        p.subagents = vec!["Explore".into()];
        p.wait_reason = "permission_prompt".into();
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        // status + subagent + wait_reason + idle hint = 4
        assert!(lines.len() >= 3);
        let sub_line = line_text(&lines[1]);
        assert!(sub_line.contains("Explore #1"));
        let reason_line = line_text(&lines[2]);
        assert!(reason_line.contains("permission required"));
    }

    #[test]
    fn render_pane_lines_response_shows_arrow() {
        let theme = ColorTheme::default();
        let p = pane_with_response(
            PermissionMode::Default,
            PaneStatus::Idle,
            "Task completed successfully",
            true,
        );
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        let response_line = line_text(&lines[1]);
        assert!(response_line.contains("▶"));
        assert!(response_line.contains("Task completed successfully"));
    }

    #[test]
    fn render_pane_lines_response_uses_char_wrap() {
        let theme = ColorTheme::default();
        // Long response that would word-wrap at spaces but should char-wrap instead
        let p = pane_with_response(
            PermissionMode::Default,
            PaneStatus::Idle,
            "abcdef ghijk lmnop qrstu vwxyz",
            true,
        );
        // Width 20: inner_width=17, prefix=4, so wrap at 13 chars
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            20,
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        // First line has ▶ + start of text
        let first = line_text(&lines[1]);
        assert!(first.contains("▶"));
        // Second line should NOT have trimmed spaces (char-wrap, not word-wrap)
        // With word-wrap "abcdef ghijk " would break at "ghijk", char-wrap fills fully
        let second = line_text(&lines[2]);
        assert!(!second.starts_with("│  ghijk"));
    }

    #[test]
    fn render_pane_lines_normal_prompt_not_detected_as_response() {
        let theme = ColorTheme::default();
        let p = pane(PermissionMode::Default, PaneStatus::Running, "fix the bug");
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            None,
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        let prompt_line = line_text(&lines[1]);
        assert!(!prompt_line.contains("▶"));
        assert!(prompt_line.contains("fix the bug"));
    }

    #[test]
    fn render_pane_lines_shows_task_progress() {
        use crate::activity::{TaskProgress, TaskStatus};
        let theme = ColorTheme::default();
        let p = pane(PermissionMode::Default, PaneStatus::Running, "");
        let progress = TaskProgress {
            tasks: vec![
                ("Task A".into(), TaskStatus::Completed),
                ("Task B".into(), TaskStatus::InProgress),
                ("Task C".into(), TaskStatus::Pending),
            ],
        };
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            Some(&progress),
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        // status + task progress + idle hint = 3 lines
        assert!(lines.len() >= 2);
        let task_line = line_text(&lines[1]);
        assert!(task_line.contains("✔◼◻"));
        assert!(task_line.contains("1/3"));
    }

    #[test]
    fn render_pane_lines_no_task_line_when_empty() {
        use crate::activity::TaskProgress;
        let theme = ColorTheme::default();
        let p = pane(PermissionMode::Default, PaneStatus::Idle, "");
        let progress = TaskProgress { tasks: vec![] };
        let lines = render_pane_lines(
            &p,
            &PaneGitInfo::default(),
            Some(&progress),
            false,
            false,
            theme.border_active,
            40,
            &theme,
            0,
            0,
        );

        // Should not have task line, just status + idle hint
        assert_eq!(lines.len(), 2);
        let hint = line_text(&lines[1]);
        assert!(hint.contains("Waiting for prompt"));
    }

    // ─── render_filter_bar tests ──────────────────────────────

    fn make_state_with_groups(groups: Vec<crate::group::RepoGroup>) -> AppState {
        let mut state = AppState::new("%99".into());
        state.repo_groups = groups;
        state.rebuild_row_targets();
        state
    }

    fn filter_bar_text(state: &AppState, width: u16) -> String {
        let (line, _) = render_filter_bar(state, width);
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn render_filter_bar_includes_repo_button() {
        let state = make_state_with_groups(vec![]);
        let text = filter_bar_text(&state, 28);
        assert!(
            text.contains("▼"),
            "filter bar should contain repo button ▼"
        );
    }

    #[test]
    fn render_filter_bar_repo_button_col_returned() {
        let state = make_state_with_groups(vec![]);
        let (_, col) = render_filter_bar(&state, 28);
        // repo button should be near the right edge
        assert!(
            col > 15,
            "repo button col should be right-aligned, got {col}"
        );
        assert!(
            col < 28,
            "repo button col should be within width, got {col}"
        );
    }

    #[test]
    fn render_filter_bar_shows_repo_name_when_filtered() {
        let mut state = make_state_with_groups(vec![crate::group::RepoGroup {
            name: "my-app".into(),
            has_focus: true,
            panes: vec![],
        }]);
        state.global.repo_filter = RepoFilter::Repo("my-app".into());
        let text = filter_bar_text(&state, 40);
        assert!(
            text.contains("my-app"),
            "filter bar should show filtered repo name, got: {text}"
        );
    }

    #[test]
    fn render_filter_bar_truncates_long_repo_name() {
        let mut state = make_state_with_groups(vec![crate::group::RepoGroup {
            name: "very-long-repository-name".into(),
            has_focus: true,
            panes: vec![],
        }]);
        state.global.repo_filter = RepoFilter::Repo("very-long-repository-name".into());
        let text = filter_bar_text(&state, 28);
        // Should be truncated, not the full name
        assert!(
            !text.contains("very-long-repository-name"),
            "long repo name should be truncated, got: {text}"
        );
        assert!(text.contains("▼"));
    }

    #[test]
    fn render_filter_bar_popup_open_styling() {
        let mut state = make_state_with_groups(vec![]);
        state.repo_popup_open = true;
        let (line, _) = render_filter_bar(&state, 28);
        // Find the repo button span and check it has UNDERLINED modifier
        let last_span = line.spans.last().unwrap();
        assert!(
            last_span.style.add_modifier.contains(Modifier::UNDERLINED),
            "repo button should be underlined when popup is open"
        );
    }
}
