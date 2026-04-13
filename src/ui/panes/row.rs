use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::tmux::PaneStatus;
use crate::ui::colors::ColorTheme;
use crate::ui::icons::StatusIcons;
use crate::ui::text::{
    display_width, elapsed_label, pad_to, truncate_to_width, wait_reason_label, wrap_text,
    wrap_text_char,
};

/// Left-edge marker character used for the currently selected pane.
const SELECTION_MARKER: &str = "┃";

struct RowCtx<'a> {
    /// 1-column left marker: `┃` when the pane is selected, otherwise a space.
    marker_char: &'static str,
    /// Style for the left marker (fg + optional bg already applied).
    marker_style: Style,
    /// Usable inner width for content after the marker and its trailing space.
    inner_width: usize,
    theme: &'a ColorTheme,
    bg: Option<Color>,
    active: bool,
}

impl RowCtx<'_> {
    #[inline]
    fn apply_bg(&self, style: Style) -> Style {
        match self.bg {
            Some(c) => style.bg(c),
            None => style,
        }
    }

    fn row_line(&self, content_spans: Vec<Span<'static>>, content_width: usize) -> Line<'static> {
        let padding = pad_to(content_width, self.inner_width);
        let bg_default = self.apply_bg(Style::default());
        let mut spans = Vec::with_capacity(content_spans.len() + 3);
        spans.push(Span::styled(self.marker_char, self.marker_style));
        spans.push(Span::styled(" ", bg_default));
        spans.extend(content_spans);
        spans.push(Span::styled(padding, bg_default));
        Line::from(spans)
    }

    fn row_line_split(
        &self,
        left_spans: Vec<Span<'static>>,
        left_width: usize,
        right_spans: Vec<Span<'static>>,
        right_width: usize,
    ) -> Line<'static> {
        let padding = self.inner_width.saturating_sub(left_width + right_width);
        let bg_default = self.apply_bg(Style::default());
        let mut spans = Vec::with_capacity(left_spans.len() + right_spans.len() + 3);
        spans.push(Span::styled(self.marker_char, self.marker_style));
        spans.push(Span::styled(" ", bg_default));
        spans.extend(left_spans);
        spans.push(Span::styled(" ".repeat(padding), bg_default));
        spans.extend(right_spans);
        Line::from(spans)
    }
}

fn status_row(
    pane: &crate::tmux::PaneInfo,
    ctx: &RowCtx,
    icons: &StatusIcons,
    spinner_frame: usize,
    now: u64,
) -> Line<'static> {
    use crate::tmux::PermissionMode;
    let theme = ctx.theme;

    let (icon, pulse_color) = running_icon_for(&pane.status, spinner_frame, icons);
    let icon_color =
        pulse_color.unwrap_or_else(|| theme.status_color(&pane.status, pane.attention));
    let title = pane.agent.label();
    let badge = pane.permission_mode.badge();
    let elapsed = elapsed_label(pane.started_at, now);

    let title_fg = theme.agent_color(&pane.agent);
    let is_active_status = matches!(pane.status, PaneStatus::Running | PaneStatus::Waiting);
    let elapsed_fg = if is_active_status {
        theme.text_active
    } else {
        theme.text_muted
    };

    let badge_extra = if badge.is_empty() { 0 } else { 1 };
    let left_width =
        display_width(icon) + 1 + display_width(title) + badge_extra + display_width(badge);
    let available_for_elapsed = ctx.inner_width.saturating_sub(left_width);
    let elapsed = truncate_to_width(&elapsed, available_for_elapsed);
    let elapsed_width = display_width(&elapsed);

    let mut left_spans: Vec<Span<'static>> = Vec::with_capacity(3);
    left_spans.push(Span::styled(
        icon.to_string(),
        ctx.apply_bg(Style::default().fg(icon_color)),
    ));
    left_spans.push(Span::styled(
        format!(" {}", title),
        ctx.apply_bg(Style::default().fg(title_fg)),
    ));
    if !badge.is_empty() {
        let badge_color = match pane.permission_mode {
            PermissionMode::BypassPermissions => theme.badge_danger,
            PermissionMode::Auto => theme.badge_auto,
            PermissionMode::DontAsk => theme.badge_auto,
            PermissionMode::Plan => theme.badge_plan,
            PermissionMode::AcceptEdits => theme.badge_auto,
            PermissionMode::Default => theme.text_muted,
        };
        left_spans.push(Span::styled(
            format!(" {}", badge),
            ctx.apply_bg(Style::default().fg(badge_color)),
        ));
    }

    let right_spans = vec![Span::styled(
        elapsed,
        ctx.apply_bg(Style::default().fg(elapsed_fg)),
    )];

    ctx.row_line_split(left_spans, left_width, right_spans, elapsed_width)
}

fn branch_ports_row(
    git_info: &crate::group::PaneGitInfo,
    ports: Option<&[u16]>,
    ctx: &RowCtx,
) -> Option<Line<'static>> {
    let branch = crate::ui::text::branch_label(git_info);
    let port_text = ports.and_then(|ports| {
        if ports.is_empty() {
            return None;
        }
        let joined = ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(":{}", joined))
    });

    if branch.is_empty() && port_text.is_none() {
        return None;
    }

    let theme = ctx.theme;
    let left_prefix = "  ";
    let right_prefix = "  ";

    let (right_spans, right_width) = match &port_text {
        Some(text) => {
            let display = format!("{}{}", right_prefix, text);
            let width = display_width(&display);
            (
                vec![Span::styled(
                    display,
                    ctx.apply_bg(Style::default().fg(theme.port)),
                )],
                width,
            )
        }
        None => (vec![], 0),
    };

    let (left_spans, left_width) = if branch.is_empty() {
        (vec![], 0)
    } else {
        let left_room = ctx.inner_width.saturating_sub(right_width);
        let max_branch_width = left_room.saturating_sub(display_width(left_prefix));
        let truncated = truncate_to_width(&branch, max_branch_width);
        let text = format!("{}{}", left_prefix, truncated);
        let width = display_width(&text);
        (
            vec![Span::styled(
                text,
                ctx.apply_bg(Style::default().fg(theme.branch)),
            )],
            width,
        )
    };

    Some(ctx.row_line_split(left_spans, left_width, right_spans, right_width))
}

fn task_progress_row(
    task_progress: Option<&crate::activity::TaskProgress>,
    ctx: &RowCtx,
) -> Option<Line<'static>> {
    use crate::activity::TaskStatus;
    let progress = task_progress?;
    if progress.is_empty() {
        return None;
    }

    let mut icons = String::with_capacity(progress.tasks.len() * 3);
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
    let task_color = ctx.theme.task_progress;
    Some(ctx.row_line(
        vec![Span::styled(
            summary,
            ctx.apply_bg(Style::default().fg(task_color)),
        )],
        summary_dw,
    ))
}

fn subagent_rows(subagents: &[String], ctx: &RowCtx) -> Vec<Line<'static>> {
    if subagents.is_empty() {
        return Vec::new();
    }
    let theme = ctx.theme;
    let subagent_color = theme.subagent;
    let tree_color = theme.text_muted;
    let last_idx = subagents.len() - 1;
    let mut out = Vec::with_capacity(subagents.len());
    for (i, sa) in subagents.iter().enumerate() {
        let connector = if i == last_idx { "└ " } else { "├ " };
        let numbered = if sa.contains('#') {
            sa.clone()
        } else {
            format!("{} #{}", sa, i + 1)
        };
        let prefix = format!("  {}", connector);
        let prefix_dw = display_width(&prefix);
        let max_sa_w = ctx.inner_width.saturating_sub(prefix_dw);
        let truncated_sa = truncate_to_width(&numbered, max_sa_w);
        let text_dw = prefix_dw + display_width(&truncated_sa);
        out.push(ctx.row_line(
            vec![
                Span::styled(prefix, ctx.apply_bg(Style::default().fg(tree_color))),
                Span::styled(
                    truncated_sa,
                    ctx.apply_bg(Style::default().fg(subagent_color)),
                ),
            ],
            text_dw,
        ));
    }
    out
}

fn wait_reason_row(wait_reason: &str, status: &PaneStatus, ctx: &RowCtx) -> Option<Line<'static>> {
    if wait_reason.is_empty() {
        return None;
    }
    let reason = wait_reason_label(wait_reason);
    let text = format!("  {}", reason);
    let text_dw = display_width(&text);
    let reason_color = if matches!(status, PaneStatus::Error) {
        ctx.theme.status_error
    } else {
        ctx.theme.wait_reason
    };
    Some(ctx.row_line(
        vec![Span::styled(
            text,
            ctx.apply_bg(Style::default().fg(reason_color)),
        )],
        text_dw,
    ))
}

fn prompt_rows(pane: &crate::tmux::PaneInfo, ctx: &RowCtx) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let is_response = pane.prompt_is_response;
    let prompt_color = if ctx.active {
        theme.text_active
    } else {
        theme.text_inactive
    };
    let wrap_width = ctx.inner_width.saturating_sub(2);
    let wrapped = if is_response {
        wrap_text_char(&pane.prompt, wrap_width, 3)
    } else {
        wrap_text(&pane.prompt, wrap_width, 3)
    };

    let mut out = Vec::with_capacity(wrapped.len());
    for (li, wl) in wrapped.iter().enumerate() {
        if is_response && li == 0 {
            let arrow_color = theme.response_arrow;
            let text_dw = 2 + display_width(wl); // "▶ " width
            out.push(ctx.row_line(
                vec![
                    Span::styled(
                        "▶ ",
                        ctx.apply_bg(
                            Style::default()
                                .fg(arrow_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ),
                    Span::styled(wl.clone(), ctx.apply_bg(Style::default().fg(prompt_color))),
                ],
                text_dw,
            ));
        } else {
            let indent = "  ";
            let text = format!("{}{}", indent, wl);
            let text_dw = display_width(&text);
            out.push(ctx.row_line(
                vec![Span::styled(
                    text,
                    ctx.apply_bg(Style::default().fg(prompt_color)),
                )],
                text_dw,
            ));
        }
    }
    out
}

fn idle_hint_row(ctx: &RowCtx) -> Line<'static> {
    let text = "  Waiting for prompt…";
    let text_dw = display_width(text);
    let idle_color = if ctx.active {
        ctx.theme.text_active
    } else {
        ctx.theme.text_inactive
    };
    ctx.row_line(
        vec![Span::styled(
            text.to_string(),
            ctx.apply_bg(Style::default().fg(idle_color)),
        )],
        text_dw,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_pane_lines_with_ports(
    pane: &crate::tmux::PaneInfo,
    git_info: &crate::group::PaneGitInfo,
    ports: Option<&[u16]>,
    task_progress: Option<&crate::activity::TaskProgress>,
    selected: bool,
    active: bool,
    width: usize,
    icons: &StatusIcons,
    theme: &ColorTheme,
    spinner_frame: usize,
    now: u64,
) -> Vec<Line<'static>> {
    let bg = if selected {
        Some(theme.selection_bg)
    } else {
        None
    };
    let apply_bg = |style: Style| match bg {
        Some(c) => style.bg(c),
        None => style,
    };
    // The left marker `┃` highlights the pane that is currently focused in
    // tmux (`active`). To keep the active accent compact, it only appears on
    // the status row and the branch/ports row (when present) — never on
    // deeper details like task progress or prompt wrapping. The sidebar
    // cursor position (`selected`) still paints the full pane with the
    // selection background.
    let marker_ctx = RowCtx {
        marker_char: if active { SELECTION_MARKER } else { " " },
        marker_style: if active {
            apply_bg(Style::default().fg(theme.accent))
        } else {
            apply_bg(Style::default())
        },
        inner_width: width.saturating_sub(2),
        theme,
        bg,
        active,
    };
    let plain_ctx = RowCtx {
        marker_char: " ",
        marker_style: Style::default(),
        inner_width: width.saturating_sub(2),
        theme,
        bg: None,
        active,
    };

    let mut out: Vec<Line<'static>> = Vec::with_capacity(8);
    out.push(status_row(pane, &marker_ctx, icons, spinner_frame, now));
    if let Some(line) = branch_ports_row(git_info, ports, &marker_ctx) {
        out.push(line);
    }
    let ctx = &plain_ctx;
    if let Some(line) = task_progress_row(task_progress, ctx) {
        out.push(line);
    }
    out.extend(subagent_rows(&pane.subagents, ctx));
    if let Some(line) = wait_reason_row(&pane.wait_reason, &pane.status, ctx) {
        out.push(line);
    }
    if !pane.prompt.is_empty() {
        out.extend(prompt_rows(pane, ctx));
    } else if matches!(pane.status, PaneStatus::Idle) {
        out.push(idle_hint_row(&ctx));
    }
    out
}

fn running_icon_for<'a>(
    status: &PaneStatus,
    spinner_frame: usize,
    icons: &'a StatusIcons,
) -> (&'a str, Option<Color>) {
    use crate::SPINNER_PULSE;

    match status {
        PaneStatus::Running => {
            let color_idx = SPINNER_PULSE[spinner_frame % SPINNER_PULSE.len()];
            (icons.status_icon(status), Some(Color::Indexed(color_idx)))
        }
        _ => (icons.status_icon(status), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::PaneGitInfo;
    use crate::tmux::{AgentType, PaneInfo, PermissionMode};
    use crate::ui::icons::StatusIcons;

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
            current_command: String::new(),
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

    fn test_ctx<'a>(theme: &'a ColorTheme, inner_width: usize, active: bool) -> RowCtx<'a> {
        RowCtx {
            marker_char: " ",
            marker_style: Style::default(),
            inner_width,
            theme,
            bg: None,
            active,
        }
    }

    #[test]
    fn render_pane_lines_shows_permission_badge() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Auto, PaneStatus::Running, "");
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        let status = line_text(&lines[0]);
        assert!(status.contains(" codex auto"));
    }

    #[test]
    fn render_pane_lines_shows_branch_and_ports_on_same_row() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Default, PaneStatus::Running, "");
        let ports = vec![3000, 5173];
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo {
                repo_root: Some("/tmp/project".into()),
                branch: Some("feature/sidebar".into()),
                is_worktree: false,
                worktree_name: None,
            },
            Some(&ports),
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        let branch_port_line = line_text(&lines[1]);
        assert!(branch_port_line.contains("feature/sidebar"));
        assert!(branch_port_line.contains(":3000, 5173"));
        assert!(branch_port_line.find("feature/sidebar") < branch_port_line.find(":3000, 5173"));
    }

    #[test]
    fn render_pane_lines_truncates_long_branch_when_ports_present() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Default, PaneStatus::Running, "");
        let ports = vec![3000];
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo {
                repo_root: Some("/tmp/project".into()),
                branch: Some("feature/sidebar/really-long-branch-name-that-should-truncate".into()),
                is_worktree: false,
                worktree_name: None,
            },
            Some(&ports),
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        let branch_port_line = line_text(&lines[1]);
        assert!(
            branch_port_line.contains('…'),
            "long branch should be truncated"
        );
        assert!(branch_port_line.contains(":3000"));
        assert!(
            branch_port_line.find('…') < branch_port_line.find(":3000"),
            "branch truncation should remain left of the port text"
        );
    }

    #[test]
    fn render_pane_lines_uses_injected_now_for_elapsed() {
        let theme = ColorTheme::default();
        let mut pane = pane(PermissionMode::Default, PaneStatus::Running, "");
        pane.started_at = Some(1_000_000 - 125);
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            1_000_000,
        );

        let status = line_text(&lines[0]);
        assert!(status.contains("2m5s"));
    }

    #[test]
    fn running_icon_for_all_statuses() {
        let icons = StatusIcons::default();
        assert_eq!(running_icon_for(&PaneStatus::Idle, 0, &icons), ("○", None));
        assert_eq!(
            running_icon_for(&PaneStatus::Waiting, 0, &icons),
            ("◐", None)
        );
        assert_eq!(running_icon_for(&PaneStatus::Error, 0, &icons), ("✕", None));
        assert_eq!(
            running_icon_for(&PaneStatus::Unknown, 0, &icons),
            ("·", None)
        );

        let (icon, color) = running_icon_for(&PaneStatus::Running, 0, &icons);
        assert_eq!(icon, "●");
        assert_eq!(color, Some(Color::Indexed(82)));
    }

    #[test]
    fn render_pane_lines_shows_idle_prompt_hint() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Default, PaneStatus::Idle, "");
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
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
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            18,
            &StatusIcons::default(),
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
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

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
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

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
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

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
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
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
        let p = pane_with_response(
            PermissionMode::Default,
            PaneStatus::Idle,
            "abcdef ghijk lmnop qrstu vwxyz",
            true,
        );
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            20,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        assert!(lines.len() >= 2);
        let first = line_text(&lines[1]);
        assert!(first.contains("▶"));
        // char-wrap must not trim inter-word spaces like word-wrap does
        let second = line_text(&lines[2]);
        assert!(!second.starts_with("│  ghijk"));
    }

    #[test]
    fn render_pane_lines_normal_prompt_not_detected_as_response() {
        let theme = ColorTheme::default();
        let p = pane(PermissionMode::Default, PaneStatus::Running, "fix the bug");
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            false,
            40,
            &StatusIcons::default(),
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
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            Some(&progress),
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

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
        let lines = render_pane_lines_with_ports(
            &p,
            &PaneGitInfo::default(),
            None,
            Some(&progress),
            false,
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        assert_eq!(lines.len(), 2);
        let hint = line_text(&lines[1]);
        assert!(hint.contains("Waiting for prompt"));
    }

    #[test]
    fn branch_ports_row_renders_port_only_without_branch() {
        let theme = ColorTheme::default();
        let ctx = test_ctx(&theme, 40, false);
        let ports = vec![3000];
        let line = branch_ports_row(&PaneGitInfo::default(), Some(&ports), &ctx)
            .expect("should render port line");
        assert!(line_text(&line).contains(":3000"));
    }

    #[test]
    fn wait_reason_row_uses_error_color_when_status_is_error() {
        let theme = ColorTheme::default();
        let ctx = test_ctx(&theme, 40, false);
        let line = wait_reason_row("permission_prompt", &PaneStatus::Error, &ctx)
            .expect("should render reason line");
        let text_span = line
            .spans
            .iter()
            .find(|s| s.content.contains("permission"))
            .expect("reason text should be present");
        assert_eq!(text_span.style.fg, Some(theme.status_error));
    }

    #[test]
    fn render_pane_lines_selected_applies_background_to_spans() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Auto, PaneStatus::Running, "do work");
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            true, // selected
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        // Every inner (non-marker) span on the status line must carry the selection bg.
        // The left marker uses marker_style only.
        let status = &lines[0];
        let has_bg = status
            .spans
            .iter()
            .any(|s| s.style.bg == Some(theme.selection_bg));
        assert!(
            has_bg,
            "selected row should apply selection_bg to inner spans"
        );
    }

    #[test]
    fn render_pane_lines_selected_leaves_content_rows_unhighlighted() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Auto, PaneStatus::Running, "do work");
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            true, // selected
            false,
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        assert!(
            lines
                .iter()
                .skip(1)
                .flat_map(|line| &line.spans)
                .all(|span| span.style.bg != Some(theme.selection_bg)),
            "content rows should not carry the selection background"
        );
    }

    #[test]
    fn render_pane_lines_active_shows_left_marker_on_status_row() {
        let theme = ColorTheme::default();
        let pane = pane(PermissionMode::Default, PaneStatus::Running, "");
        let lines = render_pane_lines_with_ports(
            &pane,
            &PaneGitInfo::default(),
            None,
            None,
            false,
            true, // active
            40,
            &StatusIcons::default(),
            &theme,
            0,
            0,
        );

        // The status row (line 0) must start with the SELECTION_MARKER in the
        // accent fg; no BOLD is applied to the title span.
        let marker_span = &lines[0].spans[0];
        assert_eq!(marker_span.content, SELECTION_MARKER);
        assert_eq!(marker_span.style.fg, Some(theme.accent));

        let title_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("codex"))
            .expect("title span should be present");
        assert!(
            !title_span.style.add_modifier.contains(Modifier::BOLD),
            "active pane title should not be BOLD"
        );
    }

    #[test]
    fn status_row_default_permission_mode_omits_badge() {
        let theme = ColorTheme::default();
        let ctx = test_ctx(&theme, 40, false);
        let pane = pane(PermissionMode::Default, PaneStatus::Running, "");
        let line = status_row(&pane, &ctx, &StatusIcons::default(), 0, 0);
        let text = line_text(&line);
        // Default mode has an empty badge string — no extra badge token should appear.
        assert!(
            !text.contains(" auto") && !text.contains(" plan") && !text.contains(" !"),
            "default permission mode should not render a badge, got: {text}"
        );
    }

    #[test]
    fn prompt_rows_indents_continuation_lines() {
        let theme = ColorTheme::default();
        let ctx = test_ctx(&theme, 20, false);
        let mut p = pane(
            PermissionMode::Default,
            PaneStatus::Running,
            "aaaa bbbb cccc dddd eeee",
        );
        p.prompt_is_response = false;
        let lines = prompt_rows(&p, &ctx);
        assert!(
            lines.len() >= 2,
            "expected prompt to wrap across multiple lines"
        );
        for line in &lines {
            let text = line_text(line);
            // Each line starts with marker(1) + space(1) + indent(2) = "    " for non-selected.
            assert!(
                text.starts_with("    "),
                "each wrapped line should carry the left padding, got: {text}"
            );
        }
    }
}
