use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::state::AppState;
use crate::ui::text::{display_width, pad_to, wrap_text_char};

pub(super) fn draw_activity_content(frame: &mut Frame, state: &mut AppState, inner: Rect) {
    let theme = &state.theme;

    if state.activity.entries.is_empty() {
        super::render_centered(frame, inner, "No activity yet", theme.text_muted);
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::new();
    let inner_w = inner.width as usize;

    // Leave one blank row above the first activity entry for breathing room.
    lines.push(Line::from(""));

    for entry in &state.activity.entries {
        let tool_color = Color::Indexed(entry.tool_color_index());

        let ts_dw = display_width(&entry.timestamp);
        let tool_dw = display_width(&entry.tool);
        let gap = pad_to(ts_dw + tool_dw, inner_w);
        let line1 = Line::from(vec![
            Span::styled(
                entry.timestamp.clone(),
                Style::default().fg(theme.activity_timestamp),
            ),
            Span::raw(gap),
            Span::styled(entry.tool.clone(), Style::default().fg(tool_color)),
        ]);
        lines.push(line1);

        if !entry.label.is_empty() {
            let label_max_w = inner_w.saturating_sub(2);
            let wrapped = wrap_text_char(&entry.label, label_max_w, 3);
            for wl in wrapped {
                lines.push(Line::from(Span::styled(
                    format!("  {wl}"),
                    Style::default().fg(theme.text_muted),
                )));
            }
        }
    }

    state.activity.scroll.total_lines = lines.len();
    state.activity.scroll.visible_height = inner.height as usize;
    // Clamp `offset` to the new total/visible. When entries shrink
    // (focus change, log trim) the stale offset would otherwise produce
    // an empty or over-scrolled paragraph.
    state.activity.scroll.scroll(0);

    let scroll_offset = state.activity.scroll.offset as u16;
    let paragraph = Paragraph::new(lines).scroll((scroll_offset, 0));
    frame.render_widget(paragraph, inner);
}
