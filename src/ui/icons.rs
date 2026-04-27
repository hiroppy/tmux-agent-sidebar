use std::collections::HashMap;

use crate::tmux::{self, PaneStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusIcons {
    /// Icon for the "All" filter in the top filter bar.
    all: String,
    running: String,
    background: String,
    waiting: String,
    idle: String,
    error: String,
    unknown: String,
}

impl Default for StatusIcons {
    fn default() -> Self {
        Self {
            all: "≡".into(),
            running: "●".into(),
            background: "◎".into(),
            waiting: "◐".into(),
            idle: "○".into(),
            error: "✕".into(),
            unknown: "·".into(),
        }
    }
}

impl StatusIcons {
    /// Load status icons from tmux @sidebar_icon_* variables, falling back to defaults.
    pub fn from_tmux() -> Self {
        let all_opts = tmux::get_all_global_options();
        Self::from_options(&all_opts)
    }

    pub fn from_options(all_opts: &HashMap<String, String>) -> Self {
        let mut icons = Self::default();

        let read = |var: &str, fallback: &str| -> String {
            all_opts
                .get(var)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| fallback.to_string())
        };

        icons.all = read(tmux::SIDEBAR_ICON_ALL, &icons.all);
        icons.running = read(tmux::SIDEBAR_ICON_RUNNING, &icons.running);
        icons.background = read(tmux::SIDEBAR_ICON_BACKGROUND, &icons.background);
        icons.waiting = read(tmux::SIDEBAR_ICON_WAITING, &icons.waiting);
        icons.idle = read(tmux::SIDEBAR_ICON_IDLE, &icons.idle);
        icons.error = read(tmux::SIDEBAR_ICON_ERROR, &icons.error);
        icons.unknown = read(tmux::SIDEBAR_ICON_UNKNOWN, &icons.unknown);
        icons
    }

    /// Icon used for the "All" filter (not tied to any PaneStatus).
    pub fn all_icon(&self) -> &str {
        self.all.as_str()
    }

    pub fn status_icon(&self, status: &PaneStatus) -> &str {
        match status {
            PaneStatus::Running => self.running.as_str(),
            PaneStatus::Background => self.background.as_str(),
            PaneStatus::Waiting => self.waiting.as_str(),
            PaneStatus::Idle => self.idle.as_str(),
            PaneStatus::Error => self.error.as_str(),
            PaneStatus::Unknown => self.unknown.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_icons_match_current_glyphs() {
        let icons = StatusIcons::default();
        assert_eq!(icons.all_icon(), "≡");
        assert_eq!(icons.status_icon(&PaneStatus::Running), "●");
        assert_eq!(icons.status_icon(&PaneStatus::Background), "◎");
        assert_eq!(icons.status_icon(&PaneStatus::Waiting), "◐");
        assert_eq!(icons.status_icon(&PaneStatus::Idle), "○");
        assert_eq!(icons.status_icon(&PaneStatus::Error), "✕");
        assert_eq!(icons.status_icon(&PaneStatus::Unknown), "·");
    }

    #[test]
    fn tmux_options_override_defaults() {
        let mut opts = HashMap::new();
        opts.insert(tmux::SIDEBAR_ICON_ALL.into(), "∀".into());
        opts.insert(tmux::SIDEBAR_ICON_RUNNING.into(), "◉".into());
        opts.insert(tmux::SIDEBAR_ICON_BACKGROUND.into(), "⊙".into());
        opts.insert(tmux::SIDEBAR_ICON_UNKNOWN.into(), "∎".into());

        let icons = StatusIcons::from_options(&opts);
        assert_eq!(icons.all_icon(), "∀");
        assert_eq!(icons.status_icon(&PaneStatus::Running), "◉");
        assert_eq!(icons.status_icon(&PaneStatus::Background), "⊙");
        assert_eq!(icons.status_icon(&PaneStatus::Unknown), "∎");
        assert_eq!(icons.status_icon(&PaneStatus::Waiting), "◐");
    }
}
