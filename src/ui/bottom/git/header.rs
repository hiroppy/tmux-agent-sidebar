use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::state::AppState;
use crate::ui::text::{display_width, pad_to, truncate_to_width};

/// Info about a PR link position within the header (relative to header origin).
pub(super) struct PrLinkInfo {
    /// X offset from the left edge of the header area.
    pub(super) x_offset: u16,
    /// Display text (e.g. "#123").
    pub(super) text: String,
    /// Full URL to open.
    pub(super) url: String,
}

/// Render the fixed header: branch+PR line, diff summary line, separator.
/// Returns the lines and optional PR link position info.
pub(super) fn render_git_header(
    state: &AppState,
    inner_w: usize,
) -> (Vec<Line<'static>>, Option<PrLinkInfo>) {
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
            // `saturating_sub` so a very narrow tmux split (pane
            // width smaller than the PR label) can't panic in debug
            // builds or underflow-wrap in release builds.
            let pr_x_offset = inner_w.saturating_sub(pr_w) as u16;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_git_header_narrow_pane_does_not_panic() {
        // Regression: `inner_w - pr_w` used to underflow on debug
        // builds when the pane was narrower than the PR label
        // (`#1234`). The sidebar can genuinely get this narrow on a
        // two-column split; header rendering must stay panic-free.
        let mut state = AppState::new("%99".into());
        state.git.branch = "main".into();
        state.git.pr_number = Some("12345".into());
        state.git.remote_url = "https://github.com/o/r".into();

        // `inner_w=3` is narrower than the "#12345" PR label (6).
        let (lines, link) = render_git_header(&state, 3);
        assert!(!lines.is_empty(), "header must render something");
        // The overlay x_offset is clamped to 0 instead of underflowing.
        assert_eq!(link.map(|l| l.x_offset), Some(0));
    }
}
