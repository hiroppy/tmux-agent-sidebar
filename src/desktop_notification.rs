use std::collections::HashMap;
use std::process::Command;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tmux;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DesktopNotificationKind {
    TaskCompleted,
    TaskFailed,
    PermissionRequired,
    WaitingTooLong,
    PortOpened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopNotificationSettings {
    pub enabled: bool,
    pub cooldown_secs: u64,
    pub wait_threshold_secs: u64,
}

impl Default for DesktopNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_secs: 120,
            wait_threshold_secs: 300,
        }
    }
}

impl DesktopNotificationSettings {
    pub fn from_tmux_options(opts: &HashMap<String, String>) -> Self {
        Self::from_tmux_options_with_backend(opts, notification_backend_available())
    }

    fn from_tmux_options_with_backend(
        opts: &HashMap<String, String>,
        backend_available: bool,
    ) -> Self {
        let mut settings = Self::default();
        settings.enabled = read_bool(opts, "@sidebar_os_notifications").unwrap_or(true);
        settings.cooldown_secs =
            read_u64(opts, "@sidebar_os_notification_cooldown").unwrap_or(settings.cooldown_secs);
        settings.wait_threshold_secs = read_u64(opts, "@sidebar_os_notification_wait_threshold")
            .unwrap_or(settings.wait_threshold_secs);
        if settings.enabled && !backend_available {
            settings.enabled = false;
        }
        settings
    }

    pub fn from_tmux() -> Self {
        Self::from_tmux_options(&tmux::get_all_global_options())
    }
}

pub fn format_title(repo: Option<&str>, agent: &str) -> String {
    match repo.map(str::trim).filter(|s| !s.is_empty()) {
        Some(repo) => format!("{repo} / {agent}"),
        None => agent.to_string(),
    }
}

pub fn notify_if_allowed(
    settings: &DesktopNotificationSettings,
    pane_id: &str,
    kind: DesktopNotificationKind,
    fingerprint: &str,
    title: &str,
    body: &str,
) -> bool {
    if !settings.enabled || pane_id.is_empty() {
        return false;
    }

    let key = stamp_option_key(kind);
    let now = now_epoch_secs();
    let current = tmux::get_pane_option_value(pane_id, key);
    if let Some(stamp) = parse_stamp(&current)
        && stamp.fingerprint == fingerprint
        && now.saturating_sub(stamp.timestamp) < settings.cooldown_secs
    {
        return false;
    }

    tmux::set_pane_option(pane_id, key, &encode_stamp(now, fingerprint));
    send_desktop_notification(title, body)
}

fn read_bool(opts: &HashMap<String, String>, key: &str) -> Option<bool> {
    let raw = opts.get(key)?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "1" | "true" | "on" | "yes" | "y" => Some(true),
        "0" | "false" | "off" | "no" | "n" => Some(false),
        _ => None,
    }
}

fn read_u64(opts: &HashMap<String, String>, key: &str) -> Option<u64> {
    opts.get(key)?.trim().parse::<u64>().ok()
}

struct NotificationStamp {
    timestamp: u64,
    fingerprint: String,
}

fn stamp_option_key(kind: DesktopNotificationKind) -> &'static str {
    match kind {
        DesktopNotificationKind::TaskCompleted => "@pane_os_notify_task_completed",
        DesktopNotificationKind::TaskFailed => "@pane_os_notify_task_failed",
        DesktopNotificationKind::PermissionRequired => "@pane_os_notify_permission_required",
        DesktopNotificationKind::WaitingTooLong => "@pane_os_notify_waiting_too_long",
        DesktopNotificationKind::PortOpened => "@pane_os_notify_port_opened",
    }
}

fn encode_stamp(timestamp: u64, fingerprint: &str) -> String {
    format!("{}|{}", timestamp, sanitize_fingerprint(fingerprint))
}

fn parse_stamp(raw: &str) -> Option<NotificationStamp> {
    let (ts, fingerprint) = raw.split_once('|')?;
    Some(NotificationStamp {
        timestamp: ts.parse().ok()?,
        fingerprint: fingerprint.to_string(),
    })
}

fn sanitize_fingerprint(value: &str) -> String {
    value.replace(['|', '\n', '\r'], " ")
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn send_desktop_notification(title: &str, body: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escape_applescript(body),
            escape_applescript(title)
        );
        return Command::new("osascript")
            .args(["-e", &script])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }

    #[cfg(target_os = "linux")]
    {
        return Command::new("notify-send")
            .args([
                "--app-name=tmux-agent-sidebar",
                "--urgency=normal",
                title,
                body,
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (title, body);
        return false;
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (title, body);
        false
    }
}

fn notification_backend_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        return Command::new("osascript")
            .args(["-e", "return 0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .is_ok();
    }

    #[cfg(target_os = "linux")]
    {
        return Command::new("notify-send")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .is_ok();
    }

    #[cfg(target_os = "windows")]
    {
        false
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn escape_applescript(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_parse_bool_and_numbers() {
        let mut opts = HashMap::new();
        opts.insert("@sidebar_os_notifications".into(), "on".into());
        opts.insert("@sidebar_os_notification_cooldown".into(), "30".into());
        opts.insert(
            "@sidebar_os_notification_wait_threshold".into(),
            "90".into(),
        );

        let settings = DesktopNotificationSettings::from_tmux_options_with_backend(&opts, true);
        assert!(settings.enabled);
        assert_eq!(settings.cooldown_secs, 30);
        assert_eq!(settings.wait_threshold_secs, 90);
    }

    #[test]
    fn settings_default_when_invalid() {
        let mut opts = HashMap::new();
        opts.insert("@sidebar_os_notifications".into(), "maybe".into());
        opts.insert("@sidebar_os_notification_cooldown".into(), "abc".into());

        let settings = DesktopNotificationSettings::from_tmux_options_with_backend(&opts, true);
        assert!(settings.enabled);
        assert_eq!(settings.cooldown_secs, 120);
        assert_eq!(settings.wait_threshold_secs, 300);
    }

    #[test]
    fn settings_disable_when_backend_missing() {
        let opts = HashMap::new();
        let settings = DesktopNotificationSettings::from_tmux_options_with_backend(&opts, false);
        assert!(!settings.enabled);
    }

    #[test]
    fn stamp_round_trip() {
        let stamp = encode_stamp(123, "foo|bar");
        let parsed = parse_stamp(&stamp).unwrap();
        assert_eq!(parsed.timestamp, 123);
        assert_eq!(parsed.fingerprint, "foo bar");
    }
}
