use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::state::{AppState, ClaudePluginNotice, NoticesCopyTarget, debug_forced_display};
use crate::tmux::{CLAUDE_AGENT, CODEX_AGENT};

use super::text::{display_width, truncate_to_width};

/// Whether the missing-hooks section should render a `[copy]` button
/// next to `agent`. Only Codex qualifies — Claude's setup story is
/// owned by the dedicated `Plugin / claude` section (which has its own
/// `[prompt]` button), so adding a second clickable copy target on the
/// Claude row would race with it and flip the shared `[copied]` feedback
/// state for both buttons at once.
///
/// Kept as a pure check so layout calculations do not pay the cost of
/// resolving the running binary path on every frame.
fn missing_hooks_has_copy_button(agent: &str) -> bool {
    agent == CODEX_AGENT
}

/// Build the ready-to-paste LLM prompt for the given agent name.
///
/// - **Claude**: emits a *migration* prompt that asks the LLM to delete any
///   existing `tmux-agent-sidebar/hook.sh` entries from the user's
///   `~/.claude/settings.json` and then point the user at `/plugin install`.
///   This is the only supported wiring path going forward — bundled
///   hooks via the Claude Code plugin manifest. The prompt embeds the
///   plugin root resolved from the running binary so users with custom
///   install layouts get a working install path. If `current_exe()`
///   fails, the prompt still works because the plugin root falls back
///   to the canonical TPM path.
/// - **Codex**: emits the legacy `setup codex` prompt because Codex CLI
///   has no plugin mechanism upstream. This branch genuinely needs the
///   running executable path, so it returns `None` if `current_exe()`
///   cannot be resolved.
///
/// Returns `None` for unknown agents.
pub(crate) fn prompt_for_agent(agent: &str) -> Option<String> {
    match agent {
        CLAUDE_AGENT => Some(build_claude_migration_prompt(
            plugin_root_from_exe().as_deref(),
        )),
        CODEX_AGENT => {
            let exe_path = std::env::current_exe().ok()?.to_string_lossy().into_owned();
            Some(format!(
                "Run {exe_path} setup codex. Add these hooks to ~/.codex/hooks.json. \
                 If hooks already exist, merge them without making destructive changes."
            ))
        }
        _ => None,
    }
}

/// Walk up from the running binary looking for `.claude-plugin/plugin.json`,
/// matching the install layouts supported elsewhere in the project
/// (`<plugin>/bin/tmux-agent-sidebar` and
/// `<plugin>/target/release/tmux-agent-sidebar`). Shares the upward-walk
/// loop with `cli::setup::resolve_hook_script`.
fn plugin_root_from_exe() -> Option<String> {
    crate::cli::setup::walk_up_from_exe(3, |dir| {
        dir.join(".claude-plugin")
            .join("plugin.json")
            .is_file()
            .then(|| dir.to_string_lossy().into_owned())
    })
}

/// Collapse an absolute path to a `~`-prefixed form when it lives
/// under the user's home directory. Used to make the migration prompt
/// portable across machines: `/Users/hiroppy/.tmux/plugins/...`
/// renders as `~/.tmux/plugins/...` so a screenshot or copy-paste
/// from one user does not bake in another user's literal home path.
fn tildify(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) => tildify_with_home(path, &home),
        Err(_) => path.to_string(),
    }
}

fn tildify_with_home(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_string();
    }
    if path == home {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(home).and_then(|s| s.strip_prefix('/')) {
        return format!("~/{}", rest);
    }
    path.to_string()
}

/// Compose the LLM migration prompt for Claude Code users. `plugin_root`
/// is `Some` when the binary lives next to a `.claude-plugin/plugin.json`
/// (the common case), in which case the prompt names that exact path so
/// the user can paste a runnable `/plugin marketplace add` command. The
/// resolved path is tilde-collapsed when it lives under the user's home
/// so the rendered command stays portable (and fits narrower sidebars).
/// The fallback is the canonical TPM install path documented in the
/// README.
fn build_claude_migration_prompt(plugin_root: Option<&str>) -> String {
    let marketplace_path = plugin_root
        .map(tildify)
        .unwrap_or_else(|| "~/.tmux/plugins/tmux-agent-sidebar".to_string());
    format!(
        "Migrate this user from the manual ~/.claude/settings.json setup to the \
         tmux-agent-sidebar Claude Code plugin:\n\
         \n\
         1. Edit ~/.claude/settings.json and remove every \"command\" entry whose \
         value contains \"tmux-agent-sidebar/hook.sh\" from each \"hooks\" section. \
         Clean up any \"hooks\" arrays that become empty (drop the trigger key) and \
         remove the top-level \"hooks\" object if it becomes empty. If no such \
         entries exist, skip this step silently.\n\
         \n\
         2. Then tell the user verbatim:\n\
         \"Run these two commands in this Claude Code session, then restart \
         Claude Code so the bundled hooks take effect:\n\
         /plugin marketplace add {marketplace_path}\n\
         /plugin install tmux-agent-sidebar@hiroppy\""
    )
}

/// Width (in columns) reserved for the notices indicator button in the
/// secondary header: the glyph plus a trailing space.
pub(super) const BUTTON_WIDTH: usize = 2;

const COPY_LABEL: &str = "[copy]";
const COPIED_LABEL: &str = "[copied]";
const PROMPT_LABEL: &str = "[prompt]";

/// How long the `[copied]` label remains after a successful copy before
/// reverting to `[copy]`.
const COPIED_FEEDBACK_DURATION: std::time::Duration = std::time::Duration::from_secs(1);

/// Whether the secondary header should show the notices indicator.
pub(super) fn has_info(state: &AppState) -> bool {
    debug_forced_display()
        || state.version_notice.is_some()
        || state.claude_plugin_notice.is_some()
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
        (_, Some(ClaudePluginNotice::Stale { installed, current })) => Some(PluginSubItem {
            body: format!("v{} -> v{}", installed, current),
            show_prompt_button: false,
        }),
        // Debug forced-display fallback when no real notice is set: show
        // the dummy "vX -> vX" version line so layout is exercised.
        (true, None) => Some(PluginSubItem {
            body: format!("v{} -> v{}", crate::VERSION, crate::VERSION),
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

pub(super) fn render_notices_popup(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = &state.theme;
    let groups = state.notices_missing_hook_groups.clone();
    let version_text = notices_popup_version_text(state.version_notice.as_ref());
    let show_version = debug_forced_display() || version_text.is_some();
    let show_hooks = debug_forced_display() || !groups.is_empty();
    let plugin_subitem = notices_popup_plugin_subitem(state.claude_plugin_notice.as_ref());
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
            copy_targets.push(NoticesCopyTarget {
                area: Rect::new(
                    inner.x + label_slot_start as u16,
                    inner.y + line_index as u16,
                    LABEL_MAX_WIDTH as u16,
                    1,
                ),
                agent: CLAUDE_AGENT.to_string(),
            });
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

    fn state_with_plugin_stale(installed: &str, current: &str) -> AppState {
        let mut state = crate::state::AppState::new(String::new());
        state.claude_plugin_notice = Some(ClaudePluginNotice::Stale {
            installed: installed.into(),
            current: current.into(),
        });
        state
    }

    fn state_with_plugin_install_recommended() -> AppState {
        let mut state = crate::state::AppState::new(String::new());
        state.claude_plugin_notice = Some(ClaudePluginNotice::InstallRecommended);
        state
    }

    fn state_with_plugin_duplicate_hooks() -> AppState {
        let mut state = crate::state::AppState::new(String::new());
        state.claude_plugin_notice = Some(ClaudePluginNotice::DuplicateHooks);
        state
    }

    // ─── prompt_for_agent ────────────────────────────────────────────

    #[test]
    fn prompt_for_agent_codex_uses_running_executable_path() {
        // Codex stays on the legacy `setup` flow because Codex CLI has
        // no plugin mechanism upstream.
        let exe = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let codex = prompt_for_agent("codex").unwrap();
        assert!(
            codex.contains(&exe),
            "codex prompt missing current_exe path: {codex}"
        );
        assert!(codex.contains("setup codex"));
        assert!(codex.contains("~/.codex/hooks.json"));
    }

    #[test]
    fn prompt_for_agent_claude_is_a_migration_prompt() {
        // The Claude prompt must steer users toward the plugin install
        // and away from the legacy settings.json hook setup. It also
        // needs a concrete removal step so users currently on the manual
        // path get cleaned up before the plugin takes over.
        let claude = prompt_for_agent("claude").unwrap();
        // Claude Code's `/plugin install` does not accept local paths
        // directly; users must register a marketplace first. The prompt
        // therefore needs both `marketplace add` and `install` lines.
        assert!(
            claude.contains("/plugin marketplace add"),
            "claude prompt must surface the marketplace add command: {claude}"
        );
        assert!(
            claude.contains("/plugin install tmux-agent-sidebar@hiroppy"),
            "claude prompt must surface the plugin install command keyed to \
             the bundled marketplace name: {claude}"
        );
        assert!(
            claude.contains("~/.claude/settings.json"),
            "claude prompt must reference settings.json so the LLM knows \
             which file to clean up: {claude}"
        );
        assert!(
            claude.contains("tmux-agent-sidebar/hook.sh"),
            "claude prompt must tell the LLM exactly which existing entries \
             to remove: {claude}"
        );
        assert!(
            claude.contains("restart Claude Code"),
            "claude prompt must remind the user to restart so the plugin's \
             bundled hooks load: {claude}"
        );
        assert!(
            !claude.contains("setup claude"),
            "claude prompt must NOT recommend the legacy `setup claude` \
             flow anymore: {claude}"
        );
    }

    #[test]
    fn build_claude_migration_prompt_uses_resolved_plugin_root_when_available() {
        // Use a path that is guaranteed NOT to live under HOME so the
        // tildify pass cannot rewrite it on either the dev machine or
        // CI (where HOME varies). The tilde-collapse behavior is
        // covered directly by the `tildify_with_home` tests below.
        let prompt = build_claude_migration_prompt(Some("/opt/tmux-agent-sidebar"));
        assert!(prompt.contains("/opt/tmux-agent-sidebar"));
    }

    #[test]
    fn build_claude_migration_prompt_falls_back_to_canonical_path() {
        // No plugin root resolved → fall back to the README-documented
        // TPM install path so the pasted command is still runnable for
        // the typical user.
        let prompt = build_claude_migration_prompt(None);
        assert!(prompt.contains("~/.tmux/plugins/tmux-agent-sidebar"));
    }

    // ─── tildify_with_home ───────────────────────────────────────────

    #[test]
    fn tildify_collapses_paths_under_home_to_tilde() {
        assert_eq!(
            tildify_with_home(
                "/Users/alice/.tmux/plugins/tmux-agent-sidebar",
                "/Users/alice"
            ),
            "~/.tmux/plugins/tmux-agent-sidebar"
        );
    }

    #[test]
    fn tildify_returns_lone_tilde_when_path_is_home() {
        assert_eq!(tildify_with_home("/Users/alice", "/Users/alice"), "~");
    }

    #[test]
    fn tildify_leaves_paths_outside_home_unchanged() {
        assert_eq!(
            tildify_with_home("/opt/tmux-agent-sidebar", "/Users/alice"),
            "/opt/tmux-agent-sidebar"
        );
    }

    #[test]
    fn tildify_does_not_collapse_a_prefix_that_only_shares_a_path_segment() {
        // `/Users/aliceother/x` must not collapse against `/Users/alice`.
        // The strip is gated on the trailing `/` so partial-name matches
        // do not produce nonsense like `~other/x`.
        assert_eq!(
            tildify_with_home("/Users/aliceother/x", "/Users/alice"),
            "/Users/aliceother/x"
        );
    }

    #[test]
    fn tildify_no_op_when_home_is_empty() {
        // Defensive: an empty HOME (set but blank) must not collapse
        // every absolute path to `~/...`.
        assert_eq!(tildify_with_home("/Users/alice/x", ""), "/Users/alice/x");
    }

    #[test]
    fn prompt_for_agent_none_for_unknown_agent() {
        assert_eq!(prompt_for_agent("gemini"), None);
        assert_eq!(prompt_for_agent(""), None);
    }

    #[test]
    fn missing_hooks_has_copy_button_only_for_codex() {
        // Claude is excluded because the Plugin / claude section owns
        // its own [prompt] button — leaving a [copy] on the Claude row
        // would race with it on the shared `[copied]` feedback state.
        assert!(missing_hooks_has_copy_button("codex"));
        assert!(!missing_hooks_has_copy_button("claude"));
        assert!(!missing_hooks_has_copy_button("gemini"));
        assert!(!missing_hooks_has_copy_button(""));
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
        assert_eq!(state.notices_copy_targets.len(), 1);
        assert_eq!(state.notices_copy_targets[0].agent, "claude");
        assert_eq!(
            state.notices_copy_targets[0].area.width,
            LABEL_MAX_WIDTH as u16
        );
    }

    #[test]
    fn rendering_skips_copy_target_for_plugin_stale() {
        // Stale-version sub-item is informational only — no [prompt]
        // button, so no copy target should be registered.
        let mut state = state_with_plugin_stale("0.4.3", "0.5.0");
        let _ = render_notices_popup_text(&mut state, 40, 10);
        assert!(state.notices_copy_targets.is_empty());
    }

    #[test]
    fn snapshot_notices_popup_plugin_stale_only() {
        let mut state = state_with_plugin_stale("0.4.3", "0.5.0");
        let text = render_notices_popup_text(&mut state, 40, 10);
        insta::assert_snapshot!(text, @r"
        ┌─────────────────────────┐
        │ Notices                 │
        │   Plugin                │
        │     claude              │
        │       v0.4.3 -> v0.5.0  │
        └─────────────────────────┘
        ");
    }

    #[test]
    fn snapshot_notices_popup_plugin_stale_with_codex_missing_hooks() {
        // Plugin install path: the Claude row is suppressed (the plugin
        // owns it) and only Codex shows up in the missing-hooks section.
        let mut state = state_with_plugin_stale("0.4.3", "0.5.0");
        state.notices_missing_hook_groups = vec![crate::state::NoticesMissingHookGroup {
            agent: "codex".into(),
            hooks: vec!["Stop".into()],
        }];
        let text = render_notices_popup_text(&mut state, 40, 14);
        insta::assert_snapshot!(text, @r"
        ┌─────────────────────────┐
        │ Notices                 │
        │   Plugin                │
        │     claude              │
        │       v0.4.3 -> v0.5.0  │
        │   Missing hooks         │
        │     codex         [copy]│
        │     - Stop              │
        └─────────────────────────┘
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
        state.notices_copied_at = Some(("claude".into(), std::time::Instant::now()));
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
        state.notices_copied_at = Some(("codex".into(), std::time::Instant::now()));
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
        state.notices_copied_at = Some((
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
        assert_eq!(state.notices_copy_targets.len(), 1);
        assert_eq!(state.notices_copy_targets[0].agent, "codex");
        assert_eq!(
            state.notices_copy_targets[0].area.width,
            LABEL_MAX_WIDTH as u16
        );
        assert_eq!(state.notices_copy_targets[0].area.height, 1);
    }

    #[test]
    fn rendering_skips_copy_targets_for_unknown_agents() {
        let mut state = state_with(None, vec![("gemini", vec!["Stop"])]);
        let _ = render_notices_popup_text(&mut state, 40, 8);
        assert!(state.notices_copy_targets.is_empty());
    }

    #[test]
    fn rendering_skips_copy_targets_when_popup_too_narrow() {
        let mut state = state_with(None, vec![("codex", vec!["ThisIsAnExtremelyLongHookName"])]);
        let _ = render_notices_popup_text(&mut state, 20, 8);
        assert!(state.notices_copy_targets.is_empty());
    }

    #[test]
    fn rendering_copy_target_reserves_label_slot_flush_right() {
        let mut state = state_with(None, vec![("codex", vec!["Stop"])]);
        let _ = render_notices_popup_text(&mut state, 40, 8);
        let target = &state.notices_copy_targets[0];
        // The popup is left-aligned at x=0 with a single-column border.
        // Space-between rendering pins the label's `LABEL_MAX_WIDTH`-wide
        // slot to the right edge of the inner area. The label always
        // ends at `border + inner_width`, regardless of inner width.
        assert_eq!(target.area.x + target.area.width, 1 + 22);
        assert_eq!(target.area.width, LABEL_MAX_WIDTH as u16);
        assert_eq!(target.area.y, 2 + 1 + 2); // popup_y + border + title + "Missing hooks"
    }
}
