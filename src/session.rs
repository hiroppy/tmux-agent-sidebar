use std::collections::HashMap;
use std::path::PathBuf;

/// Metadata scanned from a single Claude Code session file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionMeta {
    /// `/rename`-assigned or derived session name (e.g. `dev-8d`).
    pub name: String,
    /// Session start time in epoch **milliseconds** (JSON `startedAt`). Unlike
    /// the per-pane `@pane_started_at` — which is reset to "now" on every LLM
    /// run — this marks when the session itself began and never resets, so it
    /// is the stable key for ordering sessions by age. `None` if the file
    /// omits `startedAt`.
    pub started_at_ms: Option<u64>,
}

/// Return the path to Claude Code's sessions directory.
fn sessions_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".claude").join("sessions");
    if dir.is_dir() { Some(dir) } else { None }
}

/// Scan `~/.claude/sessions/*.json`, mapping each `sessionId` to its name and
/// start time.
pub fn scan_sessions() -> HashMap<String, SessionMeta> {
    let mut map = HashMap::new();
    let Some(dir) = sessions_dir() else {
        return map;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return map;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Some((session_id, meta)) = parse_session_file(&path) {
            map.insert(session_id, meta);
        }
    }
    map
}

/// Parse a single session JSON file, returning `(sessionId, meta)` when the
/// file has both a `sessionId` and a non-empty `name`.
fn parse_session_file(path: &std::path::Path) -> Option<(String, SessionMeta)> {
    let content = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let session_id = val.get("sessionId")?.as_str()?.trim();
    let name = val.get("name")?.as_str()?.trim();
    if session_id.is_empty() || name.is_empty() {
        return None;
    }
    let started_at_ms = val.get("startedAt").and_then(|v| v.as_u64());
    Some((
        session_id.to_string(),
        SessionMeta {
            name: name.to_string(),
            started_at_ms,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_session_file_with_name() {
        let dir = std::env::temp_dir().join("session_test_with_name");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("12345.json");
        fs::write(
            &path,
            r#"{"pid":12345,"sessionId":"abc-def","name":"my-session","cwd":"/tmp"}"#,
        )
        .unwrap();

        let result = parse_session_file(&path);
        assert_eq!(
            result,
            Some((
                "abc-def".into(),
                SessionMeta {
                    name: "my-session".into(),
                    started_at_ms: None,
                }
            ))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_file_captures_started_at() {
        let dir = std::env::temp_dir().join("session_test_started_at");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("12345.json");
        fs::write(
            &path,
            r#"{"pid":12345,"sessionId":"abc-def","name":"my-session","startedAt":1782914396099,"cwd":"/tmp"}"#,
        )
        .unwrap();

        let result = parse_session_file(&path);
        assert_eq!(
            result,
            Some((
                "abc-def".into(),
                SessionMeta {
                    name: "my-session".into(),
                    started_at_ms: Some(1782914396099),
                }
            ))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_file_without_name() {
        let dir = std::env::temp_dir().join("session_test_no_name");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("12345.json");
        fs::write(&path, r#"{"pid":12345,"sessionId":"abc-def","cwd":"/tmp"}"#).unwrap();

        assert!(parse_session_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_file_empty_name() {
        let dir = std::env::temp_dir().join("session_test_empty_name");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("12345.json");
        fs::write(
            &path,
            r#"{"pid":12345,"sessionId":"abc-def","name":"","cwd":"/tmp"}"#,
        )
        .unwrap();

        assert!(parse_session_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_file_whitespace_only_name() {
        let dir = std::env::temp_dir().join("session_test_whitespace_name");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("12345.json");
        fs::write(
            &path,
            r#"{"pid":12345,"sessionId":"abc-def","name":"   ","cwd":"/tmp"}"#,
        )
        .unwrap();

        assert!(parse_session_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_file_malformed_json() {
        let dir = std::env::temp_dir().join("session_test_malformed");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("12345.json");
        fs::write(&path, "not json at all").unwrap();

        assert!(parse_session_file(&path).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_session_file_nonexistent() {
        let path = std::env::temp_dir().join("session_test_nonexistent/99999.json");
        assert!(parse_session_file(&path).is_none());
    }
}
