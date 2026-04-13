use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::state::{AppState, NoticesCopyTarget, debug_forced_display};

use super::text::{display_width, truncate_to_width};

/// Whether `agent` has a runnable setup prompt. Pure check used by the
/// popup renderer to decide if a `[copy]` button should be shown — kept
/// separate from `prompt_for_agent` so layout calculations do not pay the
/// cost of resolving the running binary path on every frame.
pub(crate) fn agent_has_prompt(agent: &str) -> bool {
    matches!(agent, "claude" | "codex")
}

/// Build the ready-to-paste LLM setup prompt for the given agent name,
/// using the *currently running* binary path (`std::env::current_exe`)
/// instead of a hardcoded `~/.tmux/plugins/...` template. This makes the
/// pasted command runnable for non-default install layouts (Cargo target
/// dirs, plugin-managers that put binaries under `bin/`, custom checkouts,
/// etc.). Returns `None` for unknown agents or when the running exe path
/// cannot be resolved.
pub(crate) fn prompt_for_agent(agent: &str) -> Option<String> {
    if !agent_has_prompt(agent) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let exe_path = exe.to_string_lossy();
    let (config_path, subcommand) = match agent {
        "claude" => ("~/.claude/settings.json", "claude"),
        "codex" => ("~/.codex/hooks.json", "codex"),
        _ => return None,
    };
    Some(format!(
        "Run {exe_path} setup {subcommand}. Add these hooks to {config_path}. \
         If hooks already exist, merge them without making destructive changes."
    ))
}

/// Width (in columns) reserved for the notices indicator button in the
/// secondary header: the glyph plus a trailing space.
pub(super) const BUTTON_WIDTH: usize = 2;

const COPY_LABEL: &str = "[copy]";
const COPIED_LABEL: &str = "[copied]";

/// How long the `[copied]` label remains after a successful copy before
/// reverting to `[copy]`.
const COPIED_FEEDBACK_DURATION: std::time::Duration = std::time::Duration::from_secs(1);

/// Whether the secondary header should show the notices indicator.
pub(super) fn has_info(state: &AppState) -> bool {
    debug_forced_display()
        || state.version_notice.is_some()
        || !state.notices_missing_hook_groups.is_empty()
}

/// Span for the notices indicator glyph. Always rendered in the waiting
/// (yellow) color so it reads as an information badge.
pub(super) fn button_span<'a>(state: &AppState) -> Span<'a> {
    Span::styled("ⓘ", Style::default().fg(state.theme.status_waiting))
}

fn notices_popup_version_text(notice: Option<&crate::version::UpdateNotice>) -> Option<String> {
    if debug_forced_display() {
        Some(match notice {
            Some(notice) => format!("v{} -> v{}", notice.local_version, notice.latest_version),
            None => format!("v{} -> v{}", crate::VERSION, crate::VERSION),
        })
    } else {
        notice.map(|notice| format!("v{} -> v{}", notice.local_version, notice.latest_version))
    }
}

/// Maximum display width of any copy-state label (`[copy]` / `[copied]`).
/// Used to reserve constant space so the popup layout does not shift when
/// switching between states.
const LABEL_MAX_WIDTH: usize = {
    let a = COPY_LABEL.len();
    let b = COPIED_LABEL.len();
    if a > b { a } else { b }
};

pub(super) fn render_notices_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let groups = state.notices_missing_hook_groups.clone();
    let version_text = notices_popup_version_text(state.version_notice.as_ref());
    let show_version = debug_forced_display() || version_text.is_some();
    let show_hooks = debug_forced_display() || !groups.is_empty();
    let copied_agent: Option<String> = state
        .notices_copied_at
        .as_ref()
        .filter(|(_, at)| at.elapsed() < COPIED_FEEDBACK_DURATION)
        .map(|(agent, _)| agent.clone());
    const SECTION_INDENT: &str = "   ";
    const ITEM_INDENT: &str = "     ";
    const SUBITEM_INDENT: &str = "     ";

    let title = "Notices";
    let mut widest_line = display_width(title) + 1;
    let mut lines_len = 1usize;
    if let Some(ref text) = version_text {
        widest_line = widest_line.max(display_width(&format!("{}New Version", SECTION_INDENT)));
        widest_line = widest_line.max(display_width(&format!("{}{}", ITEM_INDENT, text)));
        lines_len += 1;
    }
    if show_version {
        lines_len += 1;
    }
    if show_hooks {
        widest_line = widest_line.max(display_width(&format!("{}Missing hooks", SECTION_INDENT)));
        lines_len += 1;
        if groups.is_empty() {
            widest_line =
                widest_line.max(display_width(&format!("{}No missing hooks", ITEM_INDENT)));
            lines_len += 1;
        }
        for group in groups.iter() {
            let group_width = if agent_has_prompt(&group.agent) {
                display_width(ITEM_INDENT) + display_width(&group.agent) + 2 + LABEL_MAX_WIDTH
            } else {
                display_width(ITEM_INDENT) + display_width(&group.agent)
            };
            widest_line = widest_line.max(group_width);
            lines_len += 1;
            for hook in &group.hooks {
                widest_line =
                    widest_line.max(display_width(&format!("{}- {}", SUBITEM_INDENT, hook)));
                lines_len += 1;
            }
        }
    }
    // Width: padding(1 left + 1 right) + widest rendered line + borders(2)
    let popup_width = (widest_line + 4).min(area.width as usize).max(12) as u16;
    // Left-aligned, below the 2-row header. Clamp the height to the space
    // *below* the header so the rect never extends past the widget bottom
    // on short sidebars (capping against `area.height` would overflow).
    let popup_x = area.x;
    let popup_y = area.y + 2;
    let height_budget = area.height.saturating_sub(2);
    // `lines_len` counts the inner text rows; add 2 for the border frame.
    let popup_height = ((lines_len as u16) + 2).min(height_budget).max(3);

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
    state.notices_popup_area = Some(popup_rect);

    frame.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(popup_rect);
    frame.render_widget(block, popup_rect);

    let inner_width = inner.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut copy_targets: Vec<NoticesCopyTarget> = Vec::new();

    let push_padded = |lines: &mut Vec<Line<'static>>, text: String, style: Style| {
        let padding = " ".repeat(inner_width.saturating_sub(display_width(&text)));
        lines.push(Line::from(Span::styled(
            format!("{}{}", text, padding),
            style,
        )));
    };

    let title_text = truncate_to_width(title, inner_width.saturating_sub(1));
    push_padded(
        &mut lines,
        format!(" {}", title_text),
        Style::default().fg(theme.text_active),
    );

    if let Some(text) = version_text {
        push_padded(
            &mut lines,
            format!("{}New Version", SECTION_INDENT),
            Style::default().fg(theme.accent),
        );
        push_padded(
            &mut lines,
            format!("{}{}", ITEM_INDENT, text),
            Style::default().fg(theme.status_waiting),
        );
    }

    if show_hooks {
        push_padded(
            &mut lines,
            format!("{}Missing hooks", SECTION_INDENT),
            Style::default().fg(theme.accent),
        );

        for group in groups.iter() {
            let agent_text = truncate_to_width(&group.agent, inner_width.saturating_sub(1));
            let agent_width = display_width(&agent_text);
            let line_index = lines.len();
            let has_prompt = agent_has_prompt(&group.agent);

            if has_prompt
                && inner_width >= display_width(ITEM_INDENT) + agent_width + 2 + LABEL_MAX_WIDTH
            {
                let is_copied = copied_agent.as_deref() == Some(group.agent.as_str());
                let (label_text, label_color) = if is_copied {
                    (COPIED_LABEL, theme.status_running)
                } else {
                    (COPY_LABEL, theme.status_waiting)
                };

                // Space-between layout: agent name stays on the left,
                // the label is pushed to the right edge of the popup.
                // The click target always covers the full `LABEL_MAX_WIDTH`
                // slot so the hit region does not shift when the label
                // flips between `[copy]` (6) and `[copied]` (8).
                let head = format!("{}{}", ITEM_INDENT, agent_text);
                let head_width = display_width(&head);
                let label_width = display_width(label_text);
                let label_slot_start = inner_width - LABEL_MAX_WIDTH;
                let gap_before_label = inner_width - head_width - label_width;
                let leading_slot_pad = LABEL_MAX_WIDTH - label_width;

                copy_targets.push(NoticesCopyTarget {
                    area: Rect::new(
                        inner.x + label_slot_start as u16,
                        inner.y + line_index as u16,
                        LABEL_MAX_WIDTH as u16,
                        1,
                    ),
                    agent: group.agent.clone(),
                });

                // `gap_before_label` stretches to keep the label flush-right;
                // `leading_slot_pad` right-aligns the (shorter) `[copy]`
                // label within the reserved `[copied]`-sized slot so the
                // glyph always ends at `inner_width`.
                let _ = leading_slot_pad;
                lines.push(Line::from(vec![
                    Span::styled(
                        head,
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::raw(" ".repeat(gap_before_label)),
                    Span::styled(label_text.to_string(), Style::default().fg(label_color)),
                ]));
            } else {
                push_padded(
                    &mut lines,
                    format!("{}{}", ITEM_INDENT, agent_text),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );
            }

            for hook in &group.hooks {
                let truncated = truncate_to_width(hook, inner_width.saturating_sub(3));
                push_padded(
                    &mut lines,
                    format!("{}- {}", SUBITEM_INDENT, truncated),
                    Style::default().fg(theme.text_muted),
                );
            }
        }
        if groups.is_empty() {
            let empty = truncate_to_width("No missing hooks", inner_width.saturating_sub(1));
            push_padded(
                &mut lines,
                format!("{}{}", ITEM_INDENT, empty),
                Style::default().fg(theme.text_muted),
            );
        }
    }

    state.notices_copy_targets = copy_targets;
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn state_with(version: Option<(&str, &str)>, groups: Vec<(&str, Vec<&str>)>) -> AppState {
        let mut state = crate::state::AppState::new(String::new());
        state.version_notice = version.map(|(local, latest)| crate::version::UpdateNotice {
            local_version: local.into(),
            latest_version: latest.into(),
        });
        state.notices_missing_hook_groups = groups
            .into_iter()
            .map(|(agent, hooks)| crate::state::NoticesMissingHookGroup {
                agent: agent.into(),
                hooks: hooks.into_iter().map(String::from).collect(),
            })
            .collect();
        state
    }

    // ─── prompt_for_agent ────────────────────────────────────────────

    #[test]
    fn prompt_for_agent_uses_running_executable_path() {
        // The prompt must reference the *real* current executable, not
        // a hardcoded ~/.tmux/plugins/... template — otherwise paste
        // instructions are unrunnable on custom install layouts.
        let exe = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let claude = prompt_for_agent("claude").unwrap();
        assert!(
            claude.contains(&exe),
            "claude prompt missing current_exe path: {claude}"
        );
        assert!(claude.contains("setup claude"));
        assert!(claude.contains("~/.claude/settings.json"));

        let codex = prompt_for_agent("codex").unwrap();
        assert!(
            codex.contains(&exe),
            "codex prompt missing current_exe path: {codex}"
        );
        assert!(codex.contains("setup codex"));
        assert!(codex.contains("~/.codex/hooks.json"));
    }

    #[test]
    fn prompt_for_agent_none_for_unknown_agent() {
        assert_eq!(prompt_for_agent("gemini"), None);
        assert_eq!(prompt_for_agent(""), None);
    }

    #[test]
    fn agent_has_prompt_only_known_agents() {
        assert!(agent_has_prompt("claude"));
        assert!(agent_has_prompt("codex"));
        assert!(!agent_has_prompt("gemini"));
        assert!(!agent_has_prompt(""));
    }

    // ─── has_info branches ───────────────────────────────────────────

    #[test]
    fn has_info_false_when_no_version_and_no_hooks() {
        let state = state_with(None, vec![]);
        assert!(!has_info(&state));
    }

    #[test]
    fn has_info_true_when_only_version_notice() {
        let state = state_with(Some(("0.2.6", "0.2.7")), vec![]);
        assert!(has_info(&state));
    }

    #[test]
    fn has_info_true_when_only_missing_hooks() {
        let state = state_with(None, vec![("claude", vec!["Stop"])]);
        assert!(has_info(&state));
    }

    #[test]
    fn has_info_true_when_both_version_and_hooks() {
        let state = state_with(Some(("0.2.6", "0.2.7")), vec![("claude", vec!["Stop"])]);
        assert!(has_info(&state));
    }

    // ─── button_span style ───────────────────────────────────────────

    #[test]
    fn button_span_uses_waiting_color_and_info_glyph() {
        let state = crate::state::AppState::new(String::new());
        let span = button_span(&state);
        assert_eq!(span.content.as_ref(), "ⓘ");
        assert_eq!(span.style.fg, Some(state.theme.status_waiting));
    }

    // ─── popup rendering ─────────────────────────────────────────────

    fn render_notices_popup_text(state: &mut AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.size();
                render_notices_popup(frame, state, area);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let area = buf.area;
        let mut lines = Vec::new();
        for y in area.y..area.y + area.height {
            let mut line = String::new();
            let mut has_content = false;
            let mut has_border_cap = false;

            for x in area.x..area.x + area.width {
                let cell = &buf[(x, y)];
                let symbol = cell.symbol();
                if symbol != " " {
                    if !matches!(symbol, "│" | "╭" | "╮" | "╰" | "╯") {
                        has_content = true;
                    }
                    if matches!(symbol, "╭" | "╮" | "╰" | "╯") {
                        has_border_cap = true;
                    }
                }
                line.push_str(symbol);
            }

            if has_content || has_border_cap {
                lines.push(line.trim_end().to_string());
            }
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    #[test]
    fn snapshot_notices_popup_layout() {
        let mut state = crate::state::AppState::new(String::new());
        state.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });
        state.notices_missing_hook_groups = vec![
            crate::state::NoticesMissingHookGroup {
                agent: "claude".into(),
                hooks: vec!["SessionStart".into(), "Stop".into()],
            },
            crate::state::NoticesMissingHookGroup {
                agent: "codex".into(),
                hooks: vec!["Stop".into()],
            },
        ];

        let text = render_notices_popup_text(&mut state, 40, 16);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   New Version         │
        │     v0.2.6 -> v0.2.7  │
        │   Missing hooks       │
        │     claude      [copy]│
        │     - SessionStart    │
        │     - Stop            │
        │     codex       [copy]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_version_only() {
        let mut state = crate::state::AppState::new(String::new());
        state.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   New Version         │
        │     v0.2.6 -> v0.2.7  │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_hooks_only() {
        let mut state = crate::state::AppState::new(String::new());
        state.notices_missing_hook_groups = vec![
            crate::state::NoticesMissingHookGroup {
                agent: "claude".into(),
                hooks: vec!["SessionStart".into(), "Stop".into()],
            },
            crate::state::NoticesMissingHookGroup {
                agent: "codex".into(),
                hooks: vec!["Stop".into()],
            },
        ];
        let text = render_notices_popup_text(&mut state, 40, 14);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   Missing hooks       │
        │     claude      [copy]│
        │     - SessionStart    │
        │     - Stop            │
        │     codex       [copy]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_single_agent_single_hook() {
        let mut state = state_with(None, vec![("claude", vec!["Stop"])]);
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   Missing hooks       │
        │     claude      [copy]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_single_agent_many_hooks() {
        let mut state = state_with(
            None,
            vec![(
                "claude",
                vec![
                    "SessionStart",
                    "SessionEnd",
                    "Stop",
                    "UserPromptSubmit",
                    "Notification",
                ],
            )],
        );
        let text = render_notices_popup_text(&mut state, 40, 12);
        insta::assert_snapshot!(text, @"
        ┌─────────────────────────┐
        │ Notices                 │
        │   Missing hooks         │
        │     claude        [copy]│
        │     - SessionStart      │
        │     - SessionEnd        │
        │     - Stop              │
        │     - UserPromptSubmit  │
        │     - Notification      │
        └─────────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_version_and_single_hook() {
        let mut state = state_with(Some(("0.2.6", "0.2.7")), vec![("claude", vec!["Stop"])]);
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   New Version         │
        │     v0.2.6 -> v0.2.7  │
        │   Missing hooks       │
        │     claude      [copy]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_long_hook_name_truncated_to_narrow_width() {
        let mut state = state_with(
            None,
            vec![(
                "claude",
                vec!["ThisIsAnExtremelyLongHookNameThatWillDefinitelyOverflow"],
            )],
        );
        // Deliberately narrow terminal to force truncation.
        let text = render_notices_popup_text(&mut state, 20, 8);
        insta::assert_snapshot!(text, @"
        ┌──────────────────┐
        │ Notices          │
        │   Missing hooks  │
        │     claude       │
        │     - ThisIsAnExt│
        └──────────────────┘
        ");
    }

    // ─── `[copied]` feedback label ───────────────────────────────────

    #[test]
    fn snapshot_notices_popup_shows_copied_label_for_recently_copied_agent() {
        let mut state = state_with(None, vec![("claude", vec!["Stop"])]);
        state.notices_copied_at = Some(("claude".into(), std::time::Instant::now()));
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   Missing hooks       │
        │     claude    [copied]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_copied_label_stays_per_agent() {
        // Only codex was copied recently — claude must still show `[copy]`.
        let mut state = state_with(
            None,
            vec![("claude", vec!["Stop"]), ("codex", vec!["Stop"])],
        );
        state.notices_copied_at = Some(("codex".into(), std::time::Instant::now()));
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   Missing hooks       │
        │     claude      [copy]│
        │     - Stop            │
        │     codex     [copied]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_copied_label_expires_after_feedback_window() {
        let mut state = state_with(None, vec![("claude", vec!["Stop"])]);
        // Past the feedback window → should render `[copy]` again.
        state.notices_copied_at = Some((
            "claude".into(),
            std::time::Instant::now()
                - COPIED_FEEDBACK_DURATION
                - std::time::Duration::from_millis(100),
        ));
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌───────────────────────┐
        │ Notices               │
        │   Missing hooks       │
        │     claude      [copy]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_unknown_agent_has_no_copy_label() {
        let mut state = state_with(None, vec![("gemini", vec!["Stop"])]);
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌──────────────────┐
        │ Notices          │
        │   Missing hooks  │
        │     gemini       │
        │     - Stop       │
        └──────────────────┘
        ");
    }

    // ─── click target tracking ──────────────────────────────────────

    #[test]
    fn rendering_populates_copy_targets_for_each_supported_agent() {
        let mut state = state_with(
            None,
            vec![("claude", vec!["Stop"]), ("codex", vec!["Stop"])],
        );
        let _ = render_notices_popup_text(&mut state, 40, 10);
        assert_eq!(state.notices_copy_targets.len(), 2);
        assert_eq!(state.notices_copy_targets[0].agent, "claude");
        assert_eq!(state.notices_copy_targets[1].agent, "codex");
        for target in &state.notices_copy_targets {
            assert_eq!(target.area.width, LABEL_MAX_WIDTH as u16);
            assert_eq!(target.area.height, 1);
        }
    }

    #[test]
    fn rendering_skips_copy_targets_for_unknown_agents() {
        let mut state = state_with(None, vec![("gemini", vec!["Stop"])]);
        let _ = render_notices_popup_text(&mut state, 40, 8);
        assert!(state.notices_copy_targets.is_empty());
    }

    #[test]
    fn rendering_skips_copy_targets_when_popup_too_narrow() {
        let mut state = state_with(
            None,
            vec![("claude", vec!["ThisIsAnExtremelyLongHookName"])],
        );
        let _ = render_notices_popup_text(&mut state, 20, 8);
        assert!(state.notices_copy_targets.is_empty());
    }

    #[test]
    fn rendering_copy_target_reserves_label_slot_flush_right() {
        let mut state = state_with(None, vec![("claude", vec!["Stop"])]);
        let _ = render_notices_popup_text(&mut state, 40, 8);
        let target = &state.notices_copy_targets[0];
        // The popup is left-aligned at x=0 with a single-column border.
        // Space-between rendering pins the label's `LABEL_MAX_WIDTH`-wide
        // slot to the right edge of the inner area. With inner_width 23
        // (popup width 25), the slot spans columns [15, 23) which maps
        // to screen columns [16, 24).
        assert_eq!(target.area.x + target.area.width, 1 + 23);
        assert_eq!(target.area.width, LABEL_MAX_WIDTH as u16);
        assert_eq!(target.area.y, 2 + 1 + 2); // popup_y + border + title + "Missing hooks"
    }
}
