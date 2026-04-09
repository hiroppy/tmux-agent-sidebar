use std::collections::HashMap;

use crate::tmux::PaneStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusIcons {
    running: String,
    waiting: String,
    idle: String,
    error: String,
    unknown: String,
}

impl Default for StatusIcons {
    fn default() -> Self {
        Self {
            running: "●".into(),
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
        let all_opts = crate::tmux::get_all_global_options();
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

        icons.running = read("@sidebar_icon_running", &icons.running);
        icons.waiting = read("@sidebar_icon_waiting", &icons.waiting);
        icons.idle = read("@sidebar_icon_idle", &icons.idle);
        icons.error = read("@sidebar_icon_error", &icons.error);
        icons.unknown = read("@sidebar_icon_unknown", &icons.unknown);
        icons
    }

    pub fn status_icon(&self, status: &PaneStatus) -> &str {
        match status {
            PaneStatus::Running => self.running.as_str(),
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
        assert_eq!(icons.status_icon(&PaneStatus::Running), "●");
        assert_eq!(icons.status_icon(&PaneStatus::Waiting), "◐");
        assert_eq!(icons.status_icon(&PaneStatus::Idle), "○");
        assert_eq!(icons.status_icon(&PaneStatus::Error), "✕");
        assert_eq!(icons.status_icon(&PaneStatus::Unknown), "·");
    }

    #[test]
    fn tmux_options_override_defaults() {
        let mut opts = HashMap::new();
        opts.insert("@sidebar_icon_running".into(), "◉".into());
        opts.insert("@sidebar_icon_unknown".into(), "∎".into());

        let icons = StatusIcons::from_options(&opts);
        assert_eq!(icons.status_icon(&PaneStatus::Running), "◉");
        assert_eq!(icons.status_icon(&PaneStatus::Unknown), "∎");
        assert_eq!(icons.status_icon(&PaneStatus::Waiting), "◐");
    }
}
