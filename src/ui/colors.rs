use ratatui::style::Color;

use crate::tmux::{self, AgentType, PaneStatus};

/// Runtime color theme, loaded from tmux @sidebar_color_* variables on startup.
/// Overrides may be xterm-256 indexes or six-digit RGB hex values.
/// Falls back to defaults if tmux variables are not set.
#[derive(Debug, Clone)]
pub struct ColorTheme {
    /// Accent color shared by every "active / focused" affordance:
    /// the `┃` marker on the active pane, the focused repo header, the
    /// bottom panel border when Activity/Git is focused, and the repo
    /// popup border.
    pub accent: Color,
    pub border_inactive: Color,
    pub status_all: Color,
    pub status_running: Color,
    pub status_waiting: Color,
    pub status_idle: Color,
    pub status_error: Color,
    pub status_unknown: Color,
    pub filter_inactive: Color,
    pub agent_claude: Color,
    pub agent_codex: Color,
    pub agent_opencode: Color,
    pub pet_body: Color,
    pub pet_eye: Color,
    pub text_active: Color,
    pub text_muted: Color,
    pub text_inactive: Color,
    pub session_header: Color,
    pub port: Color,
    pub wait_reason: Color,
    pub selection_bg: Color,
    pub branch: Color,
    pub badge_danger: Color,
    pub badge_auto: Color,
    pub badge_plan: Color,
    pub task_progress: Color,
    pub subagent: Color,
    pub commit_hash: Color,
    pub diff_added: Color,
    pub diff_deleted: Color,
    pub file_change: Color,
    pub pr_link: Color,
    pub section_title: Color,
    pub activity_timestamp: Color,
    pub response_arrow: Color,
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self {
            accent: Color::Indexed(153),
            border_inactive: Color::Indexed(240),
            status_all: Color::Indexed(111),
            status_running: Color::Indexed(114),
            status_waiting: Color::Indexed(221),
            status_idle: Color::Indexed(110),
            status_error: Color::Indexed(167),
            status_unknown: Color::Indexed(244),
            filter_inactive: Color::Indexed(245),
            agent_claude: Color::Indexed(174),
            agent_codex: Color::Indexed(141),
            agent_opencode: Color::Indexed(117),
            pet_body: Color::Indexed(208),
            pet_eye: Color::Indexed(114),
            text_active: Color::Indexed(255),
            text_muted: Color::Indexed(252),
            text_inactive: Color::Indexed(244),
            session_header: Color::Indexed(39),
            port: Color::Indexed(246),
            wait_reason: Color::Indexed(221),
            selection_bg: Color::Indexed(239),
            branch: Color::Indexed(109),
            badge_danger: Color::Indexed(167),
            badge_auto: Color::Indexed(221),
            badge_plan: Color::Indexed(117),
            task_progress: Color::Indexed(223),
            subagent: Color::Indexed(73),
            commit_hash: Color::Indexed(221),
            diff_added: Color::Indexed(114),
            diff_deleted: Color::Indexed(174),
            file_change: Color::Indexed(221),
            pr_link: Color::Indexed(117),
            section_title: Color::Indexed(109),
            activity_timestamp: Color::Indexed(109),
            response_arrow: Color::Indexed(81),
        }
    }
}

impl ColorTheme {
    /// Load colors from tmux @sidebar_color_* variables, falling back to defaults.
    /// Fetches all global options in a single tmux call to avoid N subprocess forks.
    pub fn from_tmux() -> Self {
        let all_opts = tmux::get_all_global_options();
        Self::from_options(&all_opts)
    }

    fn from_options(all_opts: &std::collections::HashMap<String, String>) -> Self {
        let mut theme = Self::default();

        let read = |var: &str, fallback: Color| -> Color {
            all_opts
                .get(var)
                .and_then(|s| parse_tmux_color(s))
                .unwrap_or(fallback)
        };

        theme.accent = read(tmux::SIDEBAR_COLOR_ACCENT, theme.accent);
        theme.border_inactive = read(tmux::SIDEBAR_COLOR_BORDER, theme.border_inactive);
        theme.status_all = read(tmux::SIDEBAR_COLOR_ALL, theme.status_all);
        theme.status_running = read(tmux::SIDEBAR_COLOR_RUNNING, theme.status_running);
        theme.status_waiting = read(tmux::SIDEBAR_COLOR_WAITING, theme.status_waiting);
        theme.status_idle = read(tmux::SIDEBAR_COLOR_IDLE, theme.status_idle);
        theme.status_error = read(tmux::SIDEBAR_COLOR_ERROR, theme.status_error);
        theme.filter_inactive = read(tmux::SIDEBAR_COLOR_FILTER_INACTIVE, theme.filter_inactive);
        theme.agent_claude = read(tmux::SIDEBAR_COLOR_AGENT_CLAUDE, theme.agent_claude);
        theme.agent_codex = read(tmux::SIDEBAR_COLOR_AGENT_CODEX, theme.agent_codex);
        theme.agent_opencode = read(tmux::SIDEBAR_COLOR_AGENT_OPENCODE, theme.agent_opencode);
        theme.pet_body = read(tmux::SIDEBAR_COLOR_PET_BODY, theme.pet_body);
        theme.pet_eye = read(tmux::SIDEBAR_COLOR_PET_EYE, theme.pet_eye);
        theme.text_active = read(tmux::SIDEBAR_COLOR_TEXT_ACTIVE, theme.text_active);
        theme.text_muted = read(tmux::SIDEBAR_COLOR_TEXT_MUTED, theme.text_muted);
        theme.text_inactive = read(tmux::SIDEBAR_COLOR_TEXT_INACTIVE, theme.text_inactive);
        theme.session_header = read(tmux::SIDEBAR_COLOR_SESSION, theme.session_header);
        theme.port = read(tmux::SIDEBAR_COLOR_PORT, theme.port);
        theme.wait_reason = read(tmux::SIDEBAR_COLOR_WAIT_REASON, theme.wait_reason);
        theme.selection_bg = read(tmux::SIDEBAR_COLOR_SELECTION, theme.selection_bg);
        theme.branch = read(tmux::SIDEBAR_COLOR_BRANCH, theme.branch);
        theme.task_progress = read(tmux::SIDEBAR_COLOR_TASK_PROGRESS, theme.task_progress);
        theme.subagent = read(tmux::SIDEBAR_COLOR_SUBAGENT, theme.subagent);
        theme.commit_hash = read(tmux::SIDEBAR_COLOR_COMMIT_HASH, theme.commit_hash);
        theme.diff_added = read(tmux::SIDEBAR_COLOR_DIFF_ADDED, theme.diff_added);
        theme.diff_deleted = read(tmux::SIDEBAR_COLOR_DIFF_DELETED, theme.diff_deleted);
        theme.file_change = read(tmux::SIDEBAR_COLOR_FILE_CHANGE, theme.file_change);
        theme.pr_link = read(tmux::SIDEBAR_COLOR_PR_LINK, theme.pr_link);
        theme.section_title = read(tmux::SIDEBAR_COLOR_SECTION_TITLE, theme.section_title);
        theme.activity_timestamp = read(
            tmux::SIDEBAR_COLOR_ACTIVITY_TIMESTAMP,
            theme.activity_timestamp,
        );
        theme.response_arrow = read(tmux::SIDEBAR_COLOR_RESPONSE_ARROW, theme.response_arrow);

        theme
    }

    pub fn status_color(&self, status: &PaneStatus, attention: bool) -> Color {
        if attention {
            return self.status_waiting;
        }
        match status {
            PaneStatus::Running => self.status_running,
            PaneStatus::Background => self.status_running,
            PaneStatus::Waiting => self.status_waiting,
            PaneStatus::Idle => self.status_idle,
            PaneStatus::Error => self.status_error,
            PaneStatus::Unknown => self.status_unknown,
        }
    }

    pub fn agent_color(&self, agent: &AgentType) -> Color {
        match agent {
            AgentType::Claude => self.agent_claude,
            AgentType::Codex => self.agent_codex,
            AgentType::OpenCode => self.agent_opencode,
            AgentType::Unknown => self.status_unknown,
        }
    }
}

fn parse_tmux_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if let Ok(index) = value.parse::<u8>() {
        return Some(Color::Indexed(index));
    }

    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::Rgb(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn status_color_attention_overrides() {
        let theme = ColorTheme::default();
        // attention=true should always return status_waiting regardless of status
        assert_eq!(
            theme.status_color(&PaneStatus::Idle, true),
            theme.status_waiting
        );
        assert_eq!(
            theme.status_color(&PaneStatus::Running, true),
            theme.status_waiting
        );
        assert_eq!(
            theme.status_color(&PaneStatus::Error, true),
            theme.status_waiting
        );
    }

    #[test]
    fn status_color_normal() {
        let theme = ColorTheme::default();
        assert_eq!(
            theme.status_color(&PaneStatus::Running, false),
            Color::Indexed(114)
        );
        assert_eq!(
            theme.status_color(&PaneStatus::Waiting, false),
            Color::Indexed(221)
        );
        assert_eq!(
            theme.status_color(&PaneStatus::Idle, false),
            Color::Indexed(110)
        );
        assert_eq!(
            theme.status_color(&PaneStatus::Error, false),
            Color::Indexed(167)
        );
        assert_eq!(
            theme.status_color(&PaneStatus::Unknown, false),
            Color::Indexed(244)
        );
    }

    #[test]
    fn agent_color_all() {
        let theme = ColorTheme::default();
        assert_eq!(theme.agent_color(&AgentType::Claude), Color::Indexed(174));
        assert_eq!(theme.agent_color(&AgentType::Codex), Color::Indexed(141));
        assert_eq!(theme.agent_color(&AgentType::OpenCode), Color::Indexed(117));
        assert_eq!(theme.agent_color(&AgentType::Unknown), theme.status_unknown);
    }

    #[test]
    fn pet_color_defaults_match_current_palette() {
        let theme = ColorTheme::default();
        assert_eq!(theme.pet_body, Color::Indexed(208));
        assert_eq!(theme.pet_eye, Color::Indexed(114));
    }

    #[test]
    fn from_options_accepts_hex_and_indexed_colors() {
        let mut options = std::collections::HashMap::new();
        options.insert(
            tmux::SIDEBAR_COLOR_ACCENT.to_string(),
            "#1a2b3c".to_string(),
        );
        options.insert(
            tmux::SIDEBAR_COLOR_AGENT_CODEX.to_string(),
            "d0e7ff".to_string(),
        );
        options.insert(tmux::SIDEBAR_COLOR_BORDER.to_string(), "42".to_string());

        let theme = ColorTheme::from_options(&options);

        assert_eq!(theme.accent, Color::Rgb(0x1a, 0x2b, 0x3c));
        assert_eq!(theme.agent_codex, Color::Rgb(0xd0, 0xe7, 0xff));
        assert_eq!(theme.border_inactive, Color::Indexed(42));
    }

    #[test]
    fn from_options_falls_back_for_invalid_colors() {
        let mut options = std::collections::HashMap::new();
        options.insert(tmux::SIDEBAR_COLOR_ACCENT.to_string(), "#12".to_string());
        options.insert(
            tmux::SIDEBAR_COLOR_AGENT_CLAUDE.to_string(),
            "not-a-color".to_string(),
        );
        options.insert(tmux::SIDEBAR_COLOR_BORDER.to_string(), "256".to_string());

        let theme = ColorTheme::from_options(&options);
        let default_theme = ColorTheme::default();

        assert_eq!(theme.accent, default_theme.accent);
        assert_eq!(theme.agent_claude, default_theme.agent_claude);
        assert_eq!(theme.border_inactive, default_theme.border_inactive);
    }
}
