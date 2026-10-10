use std::collections::HashMap;
use std::path::PathBuf;

/// Return the path to Claude Code's sessions directory.
fn sessions_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".claude").join("sessions");
    if dir.is_dir() { Some(dir) } else { None }
}

/// A parsed session file, before duplicates are resolved.
#[derive(Debug, PartialEq)]
struct SessionEntry {
    session_id: String,
    name: String,
    pid: Option<i32>,
    updated_at: u64,
}

/// Scan `~/.claude/sessions/*.json` for session names.
///
/// Claude Code leaves a file behind when a process dies, and `--resume`
/// starts a new process (and file) with the same `sessionId`. Several files
/// can therefore claim one session id with different names, so prefer the
/// one whose process is still running, then the most recently updated.
pub fn scan_session_names() -> HashMap<String, String> {
    let Some(dir) = sessions_dir() else {
        return HashMap::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return HashMap::new();
    };
    let sessions = entries.flatten().filter_map(|entry| {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            return None;
        }
        parse_session_file(&path)
    });
    resolve_session_names(sessions, pid_alive)
}

/// Collapse entries to one name per session id, preferring live processes
/// and then the newest `updatedAt`.
fn resolve_session_names(
    sessions: impl IntoIterator<Item = SessionEntry>,
    is_alive: impl Fn(i32) -> bool,
) -> HashMap<String, String> {
    let mut best: HashMap<String, (bool, u64, String)> = HashMap::new();
    for entry in sessions {
        let rank = (entry.pid.is_some_and(&is_alive), entry.updated_at);
        let replace = best
            .get(&entry.session_id)
            .is_none_or(|(alive, updated, _)| rank > (*alive, *updated));
        if replace {
            best.insert(entry.session_id, (rank.0, rank.1, entry.name));
        }
    }
    best.into_iter()
        .map(|(id, (_, _, name))| (id, name))
        .collect()
}

fn pid_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // Signal 0 only checks existence. EPERM means the process exists but
    // belongs to another user.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Parse a single session JSON file, returning its entry if it has both a
/// session id and a name.
fn parse_session_file(path: &std::path::Path) -> Option<SessionEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    let session_id = val.get("sessionId")?.as_str()?.trim();
    let name = val.get("name")?.as_str()?.trim();
    if session_id.is_empty() || name.is_empty() {
        return None;
    }
    Some(SessionEntry {
        session_id: session_id.to_string(),
        name: name.to_string(),
        pid: val
            .get("pid")
            .and_then(|v| v.as_i64())
            .and_then(|p| i32::try_from(p).ok()),
        updated_at: val.get("updatedAt").and_then(|v| v.as_u64()).unwrap_or(0),
    })
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

        let result = parse_session_file(&path).unwrap();
        assert_eq!(result.session_id, "abc-def");
        assert_eq!(result.name, "my-session");
        assert_eq!(result.pid, Some(12345));
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

    fn entry(id: &str, name: &str, pid: i32, updated_at: u64) -> SessionEntry {
        SessionEntry {
            session_id: id.into(),
            name: name.into(),
            pid: Some(pid),
            updated_at,
        }
    }

    #[test]
    fn resolve_prefers_live_process_over_newer_dead_one() {
        // A resumed session leaves the dead process's file behind with the
        // same session id. The live process must win even if the stale file
        // was updated more recently.
        let names = resolve_session_names(
            [entry("s1", "stale", 1, 200), entry("s1", "live", 2, 100)],
            |pid| pid == 2,
        );
        assert_eq!(names.get("s1").map(String::as_str), Some("live"));
    }

    #[test]
    fn resolve_is_independent_of_read_order() {
        let a = entry("s1", "live", 2, 100);
        let b = entry("s1", "stale", 1, 200);
        let forward = resolve_session_names([a, b], |pid| pid == 2);
        let reverse = resolve_session_names(
            [entry("s1", "stale", 1, 200), entry("s1", "live", 2, 100)],
            |pid| pid == 2,
        );
        assert_eq!(forward, reverse);
    }

    #[test]
    fn resolve_falls_back_to_newest_when_none_alive() {
        let names = resolve_session_names(
            [entry("s1", "old", 1, 100), entry("s1", "new", 2, 200)],
            |_| false,
        );
        assert_eq!(names.get("s1").map(String::as_str), Some("new"));
    }

    #[test]
    fn resolve_keeps_distinct_sessions() {
        let names =
            resolve_session_names([entry("s1", "one", 1, 0), entry("s2", "two", 2, 0)], |_| {
                true
            });
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn pid_alive_detects_current_process() {
        assert!(pid_alive(std::process::id() as i32));
        assert!(!pid_alive(0));
        assert!(!pid_alive(-1));
    }
}
