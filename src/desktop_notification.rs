use std::collections::HashMap;
use std::process::Command;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tmux;

pub(crate) const DESKTOP_NOTIFICATION_COOLDOWN_SECS: u64 = 120;
pub(crate) const WAIT_TOO_LONG_THRESHOLD_SECS: u64 = 300;

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
}

impl Default for DesktopNotificationSettings {
    fn default() -> Self {
        Self { enabled: true }
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
    let normalized_fingerprint = normalize_fingerprint(fingerprint);
    let now = now_epoch_secs();
    let current = tmux::get_pane_option_value(pane_id, key);
    if let Some(stamp) = parse_stamp(&current)
        && stamp.fingerprint == normalized_fingerprint
        && now.saturating_sub(stamp.timestamp) < DESKTOP_NOTIFICATION_COOLDOWN_SECS
    {
        return false;
    }

    match send_desktop_notification(title, body) {
        Ok(()) => {
            tmux::set_pane_option(pane_id, key, &encode_stamp(now, &normalized_fingerprint));
            true
        }
        Err(err) => {
            eprintln!("desktop notification failed: {err}");
            false
        }
    }
}

fn read_bool(opts: &HashMap<String, String>, key: &str) -> Option<bool> {
    let raw = opts.get(key)?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "1" | "true" | "on" | "yes" | "y" => Some(true),
        "0" | "false" | "off" | "no" | "n" => Some(false),
        _ => None,
    }
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
    format!("{}|{}", timestamp, fingerprint)
}

fn parse_stamp(raw: &str) -> Option<NotificationStamp> {
    let (ts, fingerprint) = raw.split_once('|')?;
    Some(NotificationStamp {
        timestamp: ts.parse().ok()?,
        fingerprint: fingerprint.to_string(),
    })
}

fn normalize_fingerprint(value: &str) -> String {
    value.replace(['|', '\n', '\r'], " ")
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn send_desktop_notification(title: &str, body: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escape_applescript(body),
            escape_applescript(title)
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|err| format!("failed to spawn osascript: {err}"))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("osascript exited with status {}", output.status)
        } else {
            stderr
        });
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("notify-send")
            .args([
                "--app-name=tmux-agent-sidebar",
                "--urgency=normal",
                title,
                body,
            ])
            .output()
            .map_err(|err| format!("failed to spawn notify-send: {err}"))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("notify-send exited with status {}", output.status)
        } else {
            stderr
        });
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (title, body);
        return Err("desktop notifications are not supported on Windows yet".into());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (title, body);
        Err("desktop notifications are not supported on this platform".into())
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

        let settings = DesktopNotificationSettings::from_tmux_options_with_backend(&opts, true);
        assert!(settings.enabled);
    }

    #[test]
    fn settings_default_when_invalid() {
        let mut opts = HashMap::new();
        opts.insert("@sidebar_os_notifications".into(), "maybe".into());

        let settings = DesktopNotificationSettings::from_tmux_options_with_backend(&opts, true);
        assert!(settings.enabled);
    }

    #[test]
    fn settings_disable_when_backend_missing() {
        let opts = HashMap::new();
        let settings = DesktopNotificationSettings::from_tmux_options_with_backend(&opts, false);
        assert!(!settings.enabled);
    }

    #[test]
    fn stamp_round_trip() {
        let stamp = encode_stamp(123, "foo bar");
        let parsed = parse_stamp(&stamp).unwrap();
        assert_eq!(parsed.timestamp, 123);
        assert_eq!(parsed.fingerprint, "foo bar");
    }

    #[test]
    fn fingerprint_is_normalized() {
        assert_eq!(
            normalize_fingerprint("foo|bar\nbaz\rqux"),
            "foo bar baz qux"
        );
    }
}
