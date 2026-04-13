use ratatui::style::Color;

use crate::tmux::{self, AgentType, PaneStatus};

/// Runtime color theme, loaded from tmux @sidebar_color_* variables on startup.
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
            status_error: Color::Indexed(203),
            status_unknown: Color::Indexed(244),
            filter_inactive: Color::Indexed(245),
            agent_claude: Color::Indexed(174),
            agent_codex: Color::Indexed(141),
            text_active: Color::Indexed(255),
            text_muted: Color::Indexed(252),
            text_inactive: Color::Indexed(244),
            session_header: Color::Indexed(39),
            port: Color::Indexed(246),
            wait_reason: Color::Indexed(221),
            selection_bg: Color::Indexed(237),
            branch: Color::Indexed(109),
            badge_danger: Color::Indexed(203),
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
            response_arrow: Color::Indexed(74),
        }
    }
}

impl ColorTheme {
    /// Load colors from tmux @sidebar_color_* variables, falling back to defaults.
    /// Fetches all global options in a single tmux call to avoid N subprocess forks.
    pub fn from_tmux() -> Self {
        let mut theme = Self::default();

        let all_opts = tmux::get_all_global_options();

        let read = |var: &str, fallback: Color| -> Color {
            all_opts
                .get(var)
                .and_then(|s| s.parse::<u8>().ok())
                .map(Color::Indexed)
                .unwrap_or(fallback)
        };

        theme.accent = read("@sidebar_color_accent", theme.accent);
        theme.border_inactive = read("@sidebar_color_border", theme.border_inactive);
        theme.status_all = read("@sidebar_color_all", theme.status_all);
        theme.status_running = read("@sidebar_color_running", theme.status_running);
        theme.status_waiting = read("@sidebar_color_waiting", theme.status_waiting);
        theme.status_idle = read("@sidebar_color_idle", theme.status_idle);
        theme.status_error = read("@sidebar_color_error", theme.status_error);
        theme.filter_inactive = read("@sidebar_color_filter_inactive", theme.filter_inactive);
        theme.agent_claude = read("@sidebar_color_agent_claude", theme.agent_claude);
        theme.agent_codex = read("@sidebar_color_agent_codex", theme.agent_codex);
        theme.text_active = read("@sidebar_color_text_active", theme.text_active);
        theme.text_muted = read("@sidebar_color_text_muted", theme.text_muted);
        theme.text_inactive = read("@sidebar_color_text_inactive", theme.text_inactive);
        theme.session_header = read("@sidebar_color_session", theme.session_header);
        theme.port = read("@sidebar_color_port", theme.port);
        theme.wait_reason = read("@sidebar_color_wait_reason", theme.wait_reason);
        theme.selection_bg = read("@sidebar_color_selection", theme.selection_bg);
        theme.branch = read("@sidebar_color_branch", theme.branch);
        theme.task_progress = read("@sidebar_color_task_progress", theme.task_progress);
        theme.subagent = read("@sidebar_color_subagent", theme.subagent);
        theme.commit_hash = read("@sidebar_color_commit_hash", theme.commit_hash);
        theme.diff_added = read("@sidebar_color_diff_added", theme.diff_added);
        theme.diff_deleted = read("@sidebar_color_diff_deleted", theme.diff_deleted);
        theme.file_change = read("@sidebar_color_file_change", theme.file_change);
        theme.pr_link = read("@sidebar_color_pr_link", theme.pr_link);
        theme.section_title = read("@sidebar_color_section_title", theme.section_title);
        theme.activity_timestamp = read(
            "@sidebar_color_activity_timestamp",
            theme.activity_timestamp,
        );
        theme.response_arrow = read("@sidebar_color_response_arrow", theme.response_arrow);

        theme
    }

    pub fn status_color(&self, status: &PaneStatus, attention: bool) -> Color {
        if attention {
            return self.status_waiting;
        }
        match status {
            PaneStatus::Running => self.status_running,
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
            AgentType::Unknown => self.status_unknown,
        }
    }
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
            Color::Indexed(203)
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
        assert_eq!(theme.agent_color(&AgentType::Unknown), theme.status_unknown);
    }
}
