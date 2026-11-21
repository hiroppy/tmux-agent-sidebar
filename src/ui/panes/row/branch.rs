use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::ctx::RowCtx;
use crate::ui::text::{display_width, truncate_to_width};

/// Left indent before the branch label inside [`branch_ports_row`].
const BRANCH_ROW_LEFT_PREFIX: &str = "  ";

/// The branch label for rendering: empty when hidden via
/// `@sidebar_show_branch off` (carried in `AppState::show_branch` and passed
/// down explicitly so rendering never reads tmux state itself).
fn effective_branch_label(git_info: &crate::group::PaneGitInfo, show_branch: bool) -> String {
    if show_branch {
        crate::ui::text::branch_label(git_info)
    } else {
        String::new()
    }
}

/// Port-info prefix placed between the branch text and the port list
/// when both are shown on the same row.
const BRANCH_ROW_PORT_PREFIX: &str = "  ";

/// Build the port text for the right side of the branch row, if any.
fn port_display_text(ports: Option<&[u16]>) -> Option<String> {
    ports.and_then(|ports| {
        if ports.is_empty() {
            return None;
        }
        let joined = ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(":{}", joined))
    })
}

/// Whether the trailing `×` remove marker should even be considered
/// for this pane. Gated on sidebar-spawn + a visible worktree `+`
/// prefix so plain branches never get a spurious action affordance.
fn should_emit_remove_marker(
    git_info: &crate::group::PaneGitInfo,
    sidebar_spawned: bool,
    show_branch: bool,
) -> bool {
    sidebar_spawned && effective_branch_label(git_info, show_branch).starts_with("+ ")
}

/// Compute the column offset (within the full pane row) where the
/// trailing remove-`×` marker lands for a sidebar-spawned worktree.
/// The marker is pinned to the rightmost column of the row so it
/// mirrors the repo header's right-aligned `+` spawn button —
/// "action buttons always live at the right edge". Returns `None`
/// when the pane is not eligible.
pub fn sidebar_remove_marker_col(
    git_info: &crate::group::PaneGitInfo,
    _ports: Option<&[u16]>,
    sidebar_spawned: bool,
    inner_width: usize,
    show_branch: bool,
) -> Option<u16> {
    if !should_emit_remove_marker(git_info, sidebar_spawned, show_branch) {
        return None;
    }
    // Row total width = marker(1) + space(1) + inner_width, so the
    // last column (0-indexed) is `inner_width + 1`.
    Some((inner_width + 1) as u16)
}

pub(super) fn branch_ports_row(
    git_info: &crate::group::PaneGitInfo,
    ports: Option<&[u16]>,
    sidebar_spawned: bool,
    show_branch: bool,
    ctx: &RowCtx,
) -> Option<Line<'static>> {
    let branch = effective_branch_label(git_info, show_branch);
    let port_text = port_display_text(ports);

    if branch.is_empty() && port_text.is_none() {
        return None;
    }

    let theme = ctx.theme;
    let left_prefix = BRANCH_ROW_LEFT_PREFIX;
    let right_prefix = BRANCH_ROW_PORT_PREFIX;

    // The sidebar-spawned remove affordance is pinned to the right
    // edge, mirroring the repo header's right-aligned `+` spawn
    // button. When ports are also present they stack to the left of
    // the `×`, separated by a single space.
    let emit_remove_marker = sidebar_spawned && branch.starts_with("+ ");

    let mut right_spans: Vec<Span<'static>> = Vec::new();
    let mut right_width: usize = 0;
    if let Some(text) = port_text.as_ref() {
        let display = format!("{}{}", right_prefix, text);
        let width = display_width(&display);
        right_spans.push(Span::styled(
            display,
            ctx.apply_bg(Style::default().fg(theme.port)),
        ));
        right_width += width;
    }
    if emit_remove_marker {
        if right_width > 0 {
            right_spans.push(Span::styled(
                " ".to_string(),
                ctx.apply_bg(Style::default()),
            ));
            right_width += 1;
        }
        right_spans.push(Span::styled(
            "×".to_string(),
            ctx.apply_bg(Style::default().fg(theme.status_error)),
        ));
        right_width += 1;
    }

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

#[cfg(test)]
mod show_branch_tests {
    use crate::ui::show_branch_from_options;
    use std::collections::HashMap;

    fn opts(value: Option<&str>) -> HashMap<String, String> {
        let mut m = HashMap::new();
        if let Some(v) = value {
            m.insert(crate::tmux::SIDEBAR_SHOW_BRANCH.to_string(), v.to_string());
        }
        m
    }

    #[test]
    fn unset_means_on() {
        assert!(show_branch_from_options(&opts(None)));
    }

    #[test]
    fn off_values_hide() {
        for v in ["off", "false", "0", "no", " OFF ", "No"] {
            assert!(!show_branch_from_options(&opts(Some(v))), "{v} should hide");
        }
    }

    #[test]
    fn other_values_show() {
        for v in ["on", "true", "1", "yes", "anything"] {
            assert!(show_branch_from_options(&opts(Some(v))), "{v} should show");
        }
    }

    #[test]
    fn hidden_branch_suppresses_label_but_keeps_ports_row_logic() {
        let git = crate::group::PaneGitInfo {
            repo_root: Some("/tmp/repo".into()),
            branch: Some("main".into()),
            is_worktree: false,
            worktree_name: None,
        };
        assert_eq!(super::effective_branch_label(&git, true), "main");
        assert_eq!(super::effective_branch_label(&git, false), "");
    }
}
