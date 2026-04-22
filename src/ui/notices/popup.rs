use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::button::missing_hooks_has_copy_button;
use crate::state::{AppState, ClaudePluginNotice, NoticesCopyTarget, debug_forced_display};
use crate::tmux::CLAUDE_AGENT;
use crate::ui::text::{display_width, truncate_to_width};

const COPY_LABEL: &str = "[copy]";
const COPIED_LABEL: &str = "[copied]";
const PROMPT_LABEL: &str = "[prompt]";

/// How long the `[copied]` label remains after a successful copy before
/// reverting to `[copy]`.
const COPIED_FEEDBACK_DURATION: std::time::Duration = std::time::Duration::from_secs(1);

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

/// Description of how the `Plugin / claude` sub-item should render.
/// `body` is the text that appears after `- ` on the sub-item line, and
/// `show_prompt_button` toggles the right-aligned `[prompt]` clickable
/// label (set for both `InstallRecommended` and `DuplicateHooks`, since
/// both states are resolved by the same migration recipe).
struct PluginSubItem {
    body: String,
    show_prompt_button: bool,
}

fn notices_popup_plugin_subitem(notice: Option<&ClaudePluginNotice>) -> Option<PluginSubItem> {
    match (debug_forced_display(), notice) {
        (_, Some(ClaudePluginNotice::InstallRecommended)) => Some(PluginSubItem {
            body: "migrate".to_string(),
            show_prompt_button: true,
        }),
        (_, Some(ClaudePluginNotice::DuplicateHooks)) => Some(PluginSubItem {
            body: "cleanup".to_string(),
            show_prompt_button: true,
        }),
        (_, Some(ClaudePluginNotice::Stale)) => Some(PluginSubItem {
            body: "run /plugin update".to_string(),
            show_prompt_button: false,
        }),
        // Debug forced-display fallback when no real notice is set: show
        // the Stale hint so layout is exercised.
        (true, None) => Some(PluginSubItem {
            body: "run /plugin update".to_string(),
            show_prompt_button: false,
        }),
        (false, None) => None,
    }
}

/// Maximum display width of any clickable label
/// (`[copy]` / `[copied]` / `[prompt]`). Used to reserve constant space
/// so the popup layout does not shift when switching between states.
const LABEL_MAX_WIDTH: usize = {
    let a = COPY_LABEL.len();
    let b = COPIED_LABEL.len();
    let c = PROMPT_LABEL.len();
    let ab = if a > b { a } else { b };
    if ab > c { ab } else { c }
};

pub(in crate::ui) fn render_notices_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let groups = state.notices.missing_hook_groups.clone();
    let version_text = notices_popup_version_text(state.version_notice.as_ref());
    let show_version = debug_forced_display() || version_text.is_some();
    let show_hooks = debug_forced_display() || !groups.is_empty();
    let plugin_subitem = notices_popup_plugin_subitem(state.notices.claude_plugin_notice.as_ref());
    let copied_agent: Option<String> = state
        .notices
        .copied_at
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
    if let Some(ref sub) = plugin_subitem {
        widest_line = widest_line.max(display_width(&format!("{}Plugin", SECTION_INDENT)));
        widest_line = widest_line.max(display_width(&format!("{}claude", ITEM_INDENT)));
        // Plugin sub-items drop the `- ` bullet prefix that Missing
        // hooks uses, but still indent two extra columns under
        // `claude` so the hierarchy reads at a glance.
        let head = format!("{}  {}", ITEM_INDENT, sub.body);
        let sub_width = if sub.show_prompt_button {
            display_width(&head) + 2 + LABEL_MAX_WIDTH
        } else {
            display_width(&head)
        };
        widest_line = widest_line.max(sub_width);
        // section header + agent line + body sub-item
        lines_len += 3;
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
            let group_width = if missing_hooks_has_copy_button(&group.agent) {
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
    // Width: padding(1 left + 1 right) + widest rendered line + borders(2).
    // Apply the 12-wide minimum BEFORE clamping to `area` so it cannot
    // push the popup past the available sidebar width on a very narrow
    // split — an oversize popup would otherwise leave `state.popup`
    // with an invalid hit-test region whose click rects extend past
    // the terminal buffer.
    let popup_width = (widest_line + 4).max(12).min(area.width as usize) as u16;
    // Left-aligned, below the 2-row header. Clamp the height to the space
    // *below* the header so the rect never extends past the widget bottom
    // on short sidebars (capping against `area.height` would overflow).
    let popup_x = area.x;
    let popup_y = area.y + 2;
    let height_budget = area.height.saturating_sub(2);
    // `lines_len` counts the inner text rows; add 2 for the border frame.
    // Same ordering as `popup_width`: enforce the 3-row floor first so
    // the final `.min(height_budget)` still caps the rect to a short
    // sidebar instead of overflowing it.
    let popup_height = ((lines_len as u16) + 2).max(3).min(height_budget);

    // On a sidebar that genuinely has no room (the header alone already
    // fills it, or the terminal is only a couple of columns wide) there
    // is nothing the popup can paint. Dropping the hit-test region here
    // is safer than rendering a degenerate one that swallows clicks
    // outside the visible popup.
    if popup_height < 3 || popup_width < 2 {
        state.popup.set_notices_area(None);
        state.notices.copy_targets.clear();
        return;
    }

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
    state.popup.set_notices_area(Some(popup_rect));

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

    if let Some(sub) = plugin_subitem {
        push_padded(
            &mut lines,
            format!("{}Plugin", SECTION_INDENT),
            Style::default().fg(theme.accent),
        );
        push_padded(
            &mut lines,
            format!("{}claude", ITEM_INDENT),
            Style::default()
                .fg(theme.accent)
                .add_modifier(ratatui::style::Modifier::BOLD),
        );
        let head = format!("{}  {}", ITEM_INDENT, sub.body);
        if sub.show_prompt_button && inner_width >= display_width(&head) + 2 + LABEL_MAX_WIDTH {
            // Space-between layout: body on the left, `[prompt]` slot
            // pinned to the right edge of the popup so the click hit
            // region stays in a constant column even if the body width
            // changes (e.g. between `[prompt]` and `[copied]`).
            let is_copied = copied_agent.as_deref() == Some(CLAUDE_AGENT);
            let (label_text, label_color) = if is_copied {
                (COPIED_LABEL, theme.status_running)
            } else {
                (PROMPT_LABEL, theme.status_waiting)
            };
            let head_width = display_width(&head);
            let label_width = display_width(label_text);
            let label_slot_start = inner_width - LABEL_MAX_WIDTH;
            let gap_before_label = inner_width - head_width - label_width;
            let line_index = lines.len();
            // Only register the click hit region for rows that the
            // paragraph will actually render. When the popup is
            // height-clipped, a label belonging to a hidden row
            // would otherwise intercept clicks that land OUTSIDE
            // the visible popup.
            if line_index < inner.height as usize {
                copy_targets.push(NoticesCopyTarget {
                    area: Rect::new(
                        inner.x + label_slot_start as u16,
                        inner.y + line_index as u16,
                        LABEL_MAX_WIDTH as u16,
                        1,
                    ),
                    agent: CLAUDE_AGENT.to_string(),
                });
            }
            lines.push(Line::from(vec![
                Span::styled(head, Style::default().fg(theme.status_waiting)),
                Span::raw(" ".repeat(gap_before_label)),
                Span::styled(label_text.to_string(), Style::default().fg(label_color)),
            ]));
        } else {
            push_padded(&mut lines, head, Style::default().fg(theme.status_waiting));
        }
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
            let has_prompt = missing_hooks_has_copy_button(&group.agent);

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

                // Same clipping guard as the plugin subitem path:
                // skip the click target when this row falls outside
                // the paragraph's visible area so hidden labels do
                // not intercept clicks below the rendered widget.
                if line_index < inner.height as usize {
                    copy_targets.push(NoticesCopyTarget {
                        area: Rect::new(
                            inner.x + label_slot_start as u16,
                            inner.y + line_index as u16,
                            LABEL_MAX_WIDTH as u16,
                            1,
                        ),
                        agent: group.agent.clone(),
                    });
                }

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

    state.notices.copy_targets = copy_targets;
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::NoticesMissingHookGroup;
    use ratatui::{Terminal, backend::TestBackend};

    fn state_with(version: Option<(&str, &str)>, groups: Vec<(&str, Vec<&str>)>) -> AppState {
        let mut state = AppState::new(String::new());
        state.version_notice = version.map(|(local, latest)| crate::version::UpdateNotice {
            local_version: local.into(),
            latest_version: latest.into(),
        });
        state.notices.missing_hook_groups = groups
            .into_iter()
            .map(|(agent, hooks)| NoticesMissingHookGroup {
                agent: agent.into(),
                hooks: hooks.into_iter().map(String::from).collect(),
            })
            .collect();
        state
    }

    fn state_with_plugin_stale() -> AppState {
        let mut state = AppState::new(String::new());
        state.notices.claude_plugin_notice = Some(ClaudePluginNotice::Stale);
        state
    }

    fn state_with_plugin_install_recommended() -> AppState {
        let mut state = AppState::new(String::new());
        state.notices.claude_plugin_notice = Some(ClaudePluginNotice::InstallRecommended);
        state
    }

    fn state_with_plugin_duplicate_hooks() -> AppState {
        let mut state = AppState::new(String::new());
        state.notices.claude_plugin_notice = Some(ClaudePluginNotice::DuplicateHooks);
        state
    }

    fn render_notices_popup_text(state: &mut AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
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
        let mut state = AppState::new(String::new());
        state.version_notice = Some(crate::version::UpdateNotice {
            local_version: "0.2.6".into(),
            latest_version: "0.2.7".into(),
        });
        state.notices.missing_hook_groups = vec![
            NoticesMissingHookGroup {
                agent: "claude".into(),
                hooks: vec!["SessionStart".into(), "Stop".into()],
            },
            NoticesMissingHookGroup {
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
        │     claude            │
        │     - SessionStart    │
        │     - Stop            │
        │     codex       [copy]│
        │     - Stop            │
        └───────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_plugin_duplicate_hooks() {
        // Plugin is installed AND the user still has legacy
        // settings.json hook entries → render the cleanup nudge with
        // the [prompt] button. The migration prompt handles cleanup.
        let mut state = state_with_plugin_duplicate_hooks();
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @r"
        ┌──────────────────────────┐
        │ Notices                  │
        │   Plugin                 │
        │     claude               │
        │       cleanup    [prompt]│
        └──────────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_plugin_install_recommended_only() {
        // No plugin install recorded → show migration prompt with the
        // [prompt] click button right-aligned on the sub-item row.
        let mut state = state_with_plugin_install_recommended();
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @r"
        ┌──────────────────────────┐
        │ Notices                  │
        │   Plugin                 │
        │     claude               │
        │       migrate    [prompt]│
        └──────────────────────────┘
        ");
    }

    #[test]
    fn rendering_populates_copy_target_for_plugin_install_recommended() {
        let mut state = state_with_plugin_install_recommended();
        let _ = render_notices_popup_text(&mut state, 40, 10);
        // The Plugin section's [prompt] button must register a click
        // target so `notices_copy_target_at` can route the click into
        // `prompt_for_agent("claude")`.
        assert_eq!(state.notices.copy_targets.len(), 1);
        assert_eq!(state.notices.copy_targets[0].agent, "claude");
        assert_eq!(
            state.notices.copy_targets[0].area.width,
            LABEL_MAX_WIDTH as u16
        );
    }

    #[test]
    fn rendering_skips_copy_target_for_plugin_stale() {
        // Stale sub-item is informational only — no [prompt]
        // button, so no copy target should be registered.
        let mut state = state_with_plugin_stale();
        let _ = render_notices_popup_text(&mut state, 40, 10);
        assert!(state.notices.copy_targets.is_empty());
    }

    #[test]
    fn snapshot_notices_popup_plugin_stale_only() {
        let mut state = state_with_plugin_stale();
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @r"
        ┌───────────────────────────┐
        │ Notices                   │
        │   Plugin                  │
        │     claude                │
        │       run /plugin update  │
        └───────────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_plugin_stale_with_codex_missing_hooks() {
        // Plugin install path: the Claude row is suppressed (the plugin
        // owns it) and only Codex shows up in the missing-hooks section.
        let mut state = state_with_plugin_stale();
        state.notices.missing_hook_groups = vec![NoticesMissingHookGroup {
            agent: "codex".into(),
            hooks: vec!["Stop".into()],
        }];
        let text = render_notices_popup_text(&mut state, 40, 14);
        insta::assert_snapshot!(text, @r"
        ┌───────────────────────────┐
        │ Notices                   │
        │   Plugin                  │
        │     claude                │
        │       run /plugin update  │
        │   Missing hooks           │
        │     codex           [copy]│
        │     - Stop                │
        └───────────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_version_only() {
        let mut state = AppState::new(String::new());
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
        let mut state = AppState::new(String::new());
        state.notices.missing_hook_groups = vec![
            NoticesMissingHookGroup {
                agent: "claude".into(),
                hooks: vec!["SessionStart".into(), "Stop".into()],
            },
            NoticesMissingHookGroup {
                agent: "codex".into(),
                hooks: vec!["Stop".into()],
            },
        ];
        let text = render_notices_popup_text(&mut state, 40, 14);
        insta::assert_snapshot!(text, @"
        ┌──────────────────────┐
        │ Notices              │
        │   Missing hooks      │
        │     claude           │
        │     - SessionStart   │
        │     - Stop           │
        │     codex      [copy]│
        │     - Stop           │
        └──────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_single_agent_single_hook() {
        let mut state = state_with(None, vec![("claude", vec!["Stop"])]);
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌──────────────────┐
        │ Notices          │
        │   Missing hooks  │
        │     claude       │
        │     - Stop       │
        └──────────────────┘
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
        │     claude              │
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
        │     claude            │
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
        state.notices.copied_at = Some(("claude".into(), std::time::Instant::now()));
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌──────────────────┐
        │ Notices          │
        │   Missing hooks  │
        │     claude       │
        │     - Stop       │
        └──────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_copied_label_stays_per_agent() {
        // Only codex was copied recently — claude must still show `[copy]`.
        let mut state = state_with(
            None,
            vec![("claude", vec!["Stop"]), ("codex", vec!["Stop"])],
        );
        state.notices.copied_at = Some(("codex".into(), std::time::Instant::now()));
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @"
        ┌──────────────────────┐
        │ Notices              │
        │   Missing hooks      │
        │     claude           │
        │     - Stop           │
        │     codex    [copied]│
        │     - Stop           │
        └──────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_copied_label_expires_after_feedback_window() {
        let mut state = state_with(None, vec![("claude", vec!["Stop"])]);
        // Past the feedback window → should render `[copy]` again.
        state.notices.copied_at = Some((
            "claude".into(),
            std::time::Instant::now()
                - COPIED_FEEDBACK_DURATION
                - std::time::Duration::from_millis(100),
        ));
        let text = render_notices_popup_text(&mut state, 40, 8);
        insta::assert_snapshot!(text, @"
        ┌──────────────────┐
        │ Notices          │
        │   Missing hooks  │
        │     claude       │
        │     - Stop       │
        └──────────────────┘
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
    fn rendering_populates_copy_target_only_for_codex_in_missing_hooks() {
        // Claude must NOT register a copy target in the missing-hooks
        // section — its [prompt] button lives in the Plugin section
        // and the two would race on the shared `[copied]` feedback.
        let mut state = state_with(
            None,
            vec![("claude", vec!["Stop"]), ("codex", vec!["Stop"])],
        );
        let _ = render_notices_popup_text(&mut state, 40, 10);
        assert_eq!(state.notices.copy_targets.len(), 1);
        assert_eq!(state.notices.copy_targets[0].agent, "codex");
        assert_eq!(
            state.notices.copy_targets[0].area.width,
            LABEL_MAX_WIDTH as u16
        );
        assert_eq!(state.notices.copy_targets[0].area.height, 1);
    }

    #[test]
    fn rendering_skips_copy_targets_for_unknown_agents() {
        let mut state = state_with(None, vec![("gemini", vec!["Stop"])]);
        let _ = render_notices_popup_text(&mut state, 40, 8);
        assert!(state.notices.copy_targets.is_empty());
    }

    #[test]
    fn rendering_skips_copy_targets_when_popup_too_narrow() {
        let mut state = state_with(None, vec![("codex", vec!["ThisIsAnExtremelyLongHookName"])]);
        let _ = render_notices_popup_text(&mut state, 20, 8);
        assert!(state.notices.copy_targets.is_empty());
    }

    #[test]
    fn rendering_drops_popup_when_area_is_too_small() {
        // Regression: the `.max(12)` / `.max(3)` floors used to run
        // AFTER `.min(area)` so the popup_rect could exceed the
        // terminal buffer on a tiny sidebar. Guard now drops the
        // hit-test region entirely when there is no room.
        let mut state = state_with(None, vec![("codex", vec!["Stop"])]);
        // 1 column × 2 rows leaves no space for a bordered popup.
        let _ = render_notices_popup_text(&mut state, 1, 2);
        assert!(
            state.notices.copy_targets.is_empty(),
            "tiny area must not register click targets"
        );
    }

    #[test]
    fn rendering_height_clipped_rows_do_not_register_click_targets() {
        // Regression: a height-clipped popup used to still push a
        // click target for the clipped `[copy]` row, so clicks
        // below the visible popup would land on a hidden hit
        // region. The paragraph scrolls nothing, so rows beyond
        // `inner.height` should be treated as invisible.
        let mut state = state_with(None, vec![("codex", vec!["Stop"])]);
        // Force the popup taller than the clip window — height 5
        // means `popup_height` gets clamped and the "[copy]" row
        // for codex (row index 3 within inner) should fall outside
        // the visible 1-row inner area after border subtraction.
        let _ = render_notices_popup_text(&mut state, 40, 5);
        for target in &state.notices.copy_targets {
            let inner_y_max = target.area.y;
            // Every surviving target must land within the rendered
            // popup rect. The popup starts at y=2 (below the 2-row
            // header) and its inner region height is
            // `popup_height - 2`. If our guard worked, no target
            // should be at or below `popup_y + popup_height`.
            // With height_budget=3 the popup is 3 rows total, inner
            // is 1 row, so any target past y=3 means a clipped row
            // slipped through.
            assert!(inner_y_max < 5, "clipped click target at y={inner_y_max}");
        }
    }

    #[test]
    fn rendering_copy_target_reserves_label_slot_flush_right() {
        let mut state = state_with(None, vec![("codex", vec!["Stop"])]);
        let _ = render_notices_popup_text(&mut state, 40, 8);
        let target = &state.notices.copy_targets[0];
        // The popup is left-aligned at x=0 with a single-column border.
        // Space-between rendering pins the label's `LABEL_MAX_WIDTH`-wide
        // slot to the right edge of the inner area. The label always
        // ends at `border + inner_width`, regardless of inner width.
        assert_eq!(target.area.x + target.area.width, 1 + 22);
        assert_eq!(target.area.width, LABEL_MAX_WIDTH as u16);
        assert_eq!(target.area.y, 2 + 1 + 2); // popup_y + border + title + "Missing hooks"
    }
}
