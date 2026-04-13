use std::collections::HashMap;
use std::path::PathBuf;

/// Return the path to Claude Code's sessions directory.
fn sessions_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".claude").join("sessions");
    if dir.is_dir() { Some(dir) } else { None }
}

/// Scan `~/.claude/sessions/*.json` for session names.
pub fn scan_session_names() -> HashMap<String, String> {
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
        if let Some((session_id, name)) = parse_session_file(&path) {
            map.insert(session_id, name);
        }
    }
    map
}

/// Parse a single session JSON file, returning `(sessionId, name)` if both exist.
fn parse_session_file(path: &std::path::Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let session_id = val.get("sessionId")?.as_str()?.trim();
    let name = val.get("name")?.as_str()?.trim();
    if session_id.is_empty() || name.is_empty() {
        return None;
    }
    Some((session_id.to_string(), name.to_string()))
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
        assert_eq!(result, Some(("abc-def".into(), "my-session".into())));
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
