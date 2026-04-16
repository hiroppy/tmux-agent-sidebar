#[allow(unused_imports)]
use ratatui::style::{Color, Modifier};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use tmux_agent_sidebar::state::AppState;
use tmux_agent_sidebar::tmux::{AgentType, PaneInfo, PaneStatus, SessionInfo};
use tmux_agent_sidebar::ui;

pub const FIXED_NOW: u64 = 1_700_000_000;

/// Convert buffer to a human-readable UI snapshot WITHOUT style annotations.
/// This keeps the visible UI structure but drops empty spacer rows so snapshots
/// read more like a compact screenshot than a full terminal-sized cell dump.
pub fn buffer_to_string(buf: &Buffer) -> String {
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

/// Convert buffer to string WITH style annotations for color/modifier verification
/// Format: each cell's symbol is followed by style info if non-default
/// e.g., "○[fg:250]" or " [rev]" or "c[fg:174,bold]"
pub fn buffer_to_styled_string(buf: &Buffer) -> String {
    let area = buf.area;
    let mut lines = Vec::new();
    for y in area.y..area.y + area.height {
        let mut line = String::new();
        for x in area.x..area.x + area.width {
            let cell = &buf[(x, y)];
            line.push_str(cell.symbol());

            let mut attrs = Vec::new();
            if let Color::Indexed(n) = cell.fg {
                attrs.push(format!("fg:{n}"));
            }
            if let Color::Indexed(n) = cell.bg {
                attrs.push(format!("bg:{n}"));
            }
            if let Color::Indexed(n) = cell.underline_color {
                attrs.push(format!("ul:{n}"));
            }
            if cell.modifier.contains(Modifier::BOLD) {
                attrs.push("bold".into());
            }
            if cell.modifier.contains(Modifier::REVERSED) {
                attrs.push("rev".into());
            }
            if cell.modifier.contains(Modifier::UNDERLINED) {
                attrs.push("underline".into());
            }
            if !attrs.is_empty() {
                line.push_str(&format!("[{}]", attrs.join(",")));
            }
        }
        lines.push(line.trim_end().to_string());
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

pub fn render_to_string(state: &mut AppState, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, state)).unwrap();
    let buf = terminal.backend().buffer().clone();
    buffer_to_string(&buf)
}

pub fn render_to_styled_string(state: &mut AppState, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, state)).unwrap();
    let buf = terminal.backend().buffer().clone();
    buffer_to_styled_string(&buf)
}

pub fn make_pane(agent: AgentType, status: PaneStatus) -> PaneInfo {
    PaneInfo {
        pane_id: "%1".into(),
        pane_active: true,
        status,
        attention: false,
        agent,
        path: "/home/user/project".into(),
        current_command: String::new(),
        prompt: String::new(),
        prompt_is_response: false,
        started_at: None,
        wait_reason: String::new(),
        permission_mode: tmux_agent_sidebar::tmux::PermissionMode::Default,
        subagents: vec![],
        pane_pid: None,
        worktree_name: String::new(),
        worktree_branch: String::new(),
        session_id: None,
        session_name: String::new(),
        sidebar_spawned: false,
    }
}

pub fn make_repo_group(name: &str, panes: Vec<PaneInfo>) -> tmux_agent_sidebar::group::RepoGroup {
    tmux_agent_sidebar::group::RepoGroup {
        name: name.into(),
        has_focus: true,
        panes: panes
            .into_iter()
            .map(|p| (p, tmux_agent_sidebar::group::PaneGitInfo::default()))
            .collect(),
    }
}

/// Build a baseline `AppState` for UI/render tests.
///
/// The `_sessions` argument is kept so the call sites can document the
/// intended pane topology, but the field itself was removed from
/// `AppState`. Callers that need a populated tree should set
/// `state.repo_groups` directly (which they already do).
pub fn make_state(_sessions: Vec<SessionInfo>) -> AppState {
    let mut state = AppState::new("%99".into());
    state.now = FIXED_NOW;
    state.sidebar_focused = true;
    state.focused_pane_id = Some("%1".into());
    state.notices.missing_hook_groups = vec![tmux_agent_sidebar::state::NoticesMissingHookGroup {
        agent: "claude".into(),
        hooks: vec!["Stop".into()],
    }];
    state
}

pub fn assert_right_border_intact(output: &str) {
    for (i, line) in output.lines().enumerate() {
        if line.starts_with('│') || line.starts_with('╭') || line.starts_with('╰') {
            let trimmed = line.trim_end();
            assert!(
                trimmed.ends_with('│') || trimmed.ends_with('╮') || trimmed.ends_with('╯'),
                "Line {} missing right border: {:?}",
                i + 1,
                trimmed,
            );
        }
    }
}
