use std::process::Command;

/// A file entry with its status indicator, name, and per-file diff stats.
#[derive(Debug, Clone, PartialEq)]
pub struct GitFileEntry {
    pub status: char,
    pub name: String,
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
}

/// All git information gathered in a single background pass
#[derive(Debug, Clone, Default)]
pub struct GitData {
    pub diff_stat: Option<(usize, usize)>,
    pub branch: String,
    pub ahead_behind: Option<(usize, usize)>,
    pub staged_files: Vec<GitFileEntry>,
    pub unstaged_files: Vec<GitFileEntry>,
    pub untracked_files: Vec<String>,
    pub remote_url: String,
    pub pr_number: Option<String>,
}

impl GitData {
    pub fn changed_file_count(&self) -> usize {
        self.staged_files.len() + self.unstaged_files.len() + self.untracked_files.len()
    }
}

/// Fetch all git data for a given path. Runs blocking subprocess calls.
/// Designed to be called from a background thread.
pub fn fetch_git_data(path: &str) -> GitData {
    let mut data = GitData::default();

    // Parse git status --short to classify files into staged/unstaged/untracked
    if let Some(text) = run_git(path, &["status", "--short"]) {
        parse_status_short(&text, &mut data);
    }

    if let Some(text) = run_git(path, &["diff", "--shortstat"]) {
        data.diff_stat = parse_diff_stat(&text);
    }

    if let Some(text) = run_git(path, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        data.branch = text;
    }

    if let Some(text) = run_git(
        path,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    ) {
        let parts: Vec<&str> = text.split('\t').collect();
        if parts.len() == 2 {
            let ahead = parts[0].parse().unwrap_or(0);
            let behind = parts[1].parse().unwrap_or(0);
            data.ahead_behind = Some((ahead, behind));
        }
    }

    apply_numstat(
        path,
        &["diff", "--cached", "--numstat"],
        &mut data.staged_files,
    );
    apply_numstat(path, &["diff", "--numstat"], &mut data.unstaged_files);

    if let Some(text) = run_git(path, &["remote", "get-url", "origin"]) {
        data.remote_url = normalize_git_url(&text);
    }

    // Spawn `gh` with a timeout to avoid blocking the git thread indefinitely
    // (e.g. network issues, auth prompts).
    if let Ok(mut child) = Command::new("gh")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["pr", "view", "--json", "number", "-q", ".number"])
        .current_dir(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success()
                        && let Some(stdout) = child.stdout.take()
                    {
                        use std::io::Read;
                        let mut buf = String::new();
                        let mut reader = stdout;
                        let _ = reader.read_to_string(&mut buf);
                        let num = buf.trim().to_string();
                        if !num.is_empty() {
                            data.pr_number = Some(num);
                        }
                    }
                    break;
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => break,
            }
        }
    }

    data
}

/// Parse `git status --short` output into staged/unstaged/untracked categories.
///
/// Each line has the format `XY filename` where:
/// - X = index (staged) status
/// - Y = worktree (unstaged) status
/// - `??` = untracked
pub(crate) fn parse_status_short(text: &str, data: &mut GitData) {
    for line in text.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        // Handle renames: "R  old -> new" format
        let raw_name = &line[3..];
        let full_path = normalize_git_path(raw_name);
        let is_dir = full_path.ends_with('/');
        let name_trimmed = full_path.trim_end_matches('/');
        let mut basename = name_trimmed
            .rsplit('/')
            .next()
            .unwrap_or(name_trimmed)
            .to_string();
        if is_dir {
            basename.push('/');
        }

        if x == '?' && y == '?' {
            data.untracked_files.push(basename);
            continue;
        }

        // Staged: X is M, A, D, R, or C
        if matches!(x, 'M' | 'A' | 'D' | 'R' | 'C') {
            let status = if x == 'R' || x == 'C' { 'M' } else { x };
            data.staged_files.push(GitFileEntry {
                status,
                name: basename.clone(),
                path: full_path.clone(),
                additions: 0,
                deletions: 0,
            });
        }

        // Unstaged: Y is M or D
        if matches!(y, 'M' | 'D') {
            data.unstaged_files.push(GitFileEntry {
                status: y,
                name: basename,
                path: full_path,
                additions: 0,
                deletions: 0,
            });
        }
    }
}

/// Apply numstat diff data to a list of file entries.
fn apply_numstat(path: &str, args: &[&str], entries: &mut [GitFileEntry]) {
    if let Some(text) = run_git(path, args) {
        let numstat = parse_numstat(&text);
        for entry in entries {
            if let Some((add, del)) = numstat
                .get(entry.path.as_str())
                .or_else(|| numstat.get(entry.name.as_str()))
            {
                entry.additions = *add;
                entry.deletions = *del;
            }
        }
    }
}

/// Parse `git diff --numstat` output into a map of filename -> (additions, deletions).
fn parse_numstat(text: &str) -> std::collections::HashMap<String, (usize, usize)> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let add: usize = parts[0].parse().unwrap_or(0);
            let del: usize = parts[1].parse().unwrap_or(0);
            let path = normalize_git_path(parts[2]);
            let basename = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
            map.insert(path.clone(), (add, del));
            if basename != path {
                map.insert(basename, (add, del));
            }
        }
    }
    map
}

fn normalize_git_path(path: &str) -> String {
    let path = path.trim();
    if let Some((_, new_path)) = path.rsplit_once(" -> ") {
        new_path.trim().to_string()
    } else if let Some((_, new_path)) = path.rsplit_once(" => ") {
        new_path.trim().to_string()
    } else {
        path.to_string()
    }
}

pub(crate) fn run_git(path: &str, args: &[&str]) -> Option<String> {
    let mut cmd_args = vec!["-C", path];
    cmd_args.extend_from_slice(args);
    let output = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(&cmd_args)
        .output()
        .ok()?;
    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    }
}

/// Run a git command in `path` and return stderr on non-zero exit. Used by the
/// worktree spawn/remove flow so the UI can show an actionable error message.
pub fn run_git_capture(path: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd_args = vec!["-C", path];
    cmd_args.extend_from_slice(args);
    let output = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(&cmd_args)
        .output()
        .map_err(|e| format!("failed to spawn git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("git exited with status {}", output.status)
        } else {
            stderr
        })
    }
}

/// Resolve the top-level directory of the git repository containing `path`.
pub fn repo_root(path: &str) -> Option<String> {
    run_git(path, &["rev-parse", "--show-toplevel"])
}

/// `true` when `<repo>/refs/heads/<branch>` exists, i.e. the branch name is
/// already taken.
pub fn branch_exists(repo: &str, branch: &str) -> bool {
    run_git_capture(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

/// `git worktree add <worktree_path> -b <branch>` from inside `repo`. Errors
/// bubble up with stderr.
pub fn worktree_add(repo: &str, worktree_path: &str, branch: &str) -> Result<(), String> {
    run_git_capture(repo, &["worktree", "add", worktree_path, "-b", branch]).map(|_| ())
}

/// `git worktree remove --force <worktree_path>`. `--force` is used
/// because the sidebar's remove flow only runs when the user explicitly
/// picks "close window + remove worktree" — agent sessions routinely
/// leave untracked state behind and git would otherwise strand the
/// worktree. Users who want to keep the checkout have `w` (window only).
pub fn worktree_remove(repo: &str, worktree_path: &str) -> Result<(), String> {
    run_git_capture(repo, &["worktree", "remove", "--force", worktree_path]).map(|_| ())
}

/// `git branch -D <branch>`. Used by the spawn rollback path to drop
/// the branch ref that `git worktree add -b` just created — removing
/// only the worktree leaves the branch behind, which later spawns
/// would then collide with via `branch_exists`.
pub fn branch_delete(repo: &str, branch: &str) -> Result<(), String> {
    run_git_capture(repo, &["branch", "-D", branch]).map(|_| ())
}

pub(crate) fn parse_diff_stat(text: &str) -> Option<(usize, usize)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mut insertions = 0usize;
    let mut deletions = 0usize;
    for part in text.split(',') {
        let part = part.trim();
        if part.contains("insertion") {
            insertions = part
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        } else if part.contains("deletion") {
            deletions = part
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }
    Some((insertions, deletions))
}

pub(crate) fn normalize_git_url(url: &str) -> String {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("git@") {
        let converted = rest.replace(':', "/");
        let cleaned = converted.strip_suffix(".git").unwrap_or(&converted);
        format!("https://{cleaned}")
    } else if url.starts_with("https://") || url.starts_with("http://") {
        url.strip_suffix(".git").unwrap_or(url).to_string()
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diff_stat_both() {
        let result = parse_diff_stat("2 files changed, 4 insertions(+), 2 deletions(-)");
        assert_eq!(result, Some((4, 2)));
    }

    #[test]
    fn parse_diff_stat_insertions_only() {
        let result = parse_diff_stat("1 file changed, 5 insertions(+)");
        assert_eq!(result, Some((5, 0)));
    }

    #[test]
    fn parse_diff_stat_deletions_only() {
        let result = parse_diff_stat("1 file changed, 3 deletions(-)");
        assert_eq!(result, Some((0, 3)));
    }

    #[test]
    fn parse_diff_stat_empty() {
        assert_eq!(parse_diff_stat(""), None);
    }

    #[test]
    fn parse_diff_stat_whitespace() {
        assert_eq!(parse_diff_stat("   "), None);
    }

    #[test]
    fn normalize_git_url_ssh() {
        assert_eq!(
            normalize_git_url("git@github.com:user/repo.git"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn normalize_git_url_https_with_git() {
        assert_eq!(
            normalize_git_url("https://github.com/user/repo.git"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn normalize_git_url_https_clean() {
        assert_eq!(
            normalize_git_url("https://github.com/user/repo"),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn normalize_git_url_unknown_format() {
        assert_eq!(normalize_git_url("/local/path/repo"), "/local/path/repo");
    }

    // ─── parse_status_short tests ────────────────────────────────

    #[test]
    fn parse_status_short_staged_modified() {
        let mut data = GitData::default();
        parse_status_short("M  src/app.rs", &mut data);
        assert_eq!(data.staged_files.len(), 1);
        assert_eq!(data.staged_files[0].status, 'M');
        assert_eq!(data.staged_files[0].name, "app.rs");
        assert_eq!(data.staged_files[0].path, "src/app.rs");
        assert!(data.unstaged_files.is_empty());
        assert!(data.untracked_files.is_empty());
    }

    #[test]
    fn parse_status_short_staged_added() {
        let mut data = GitData::default();
        parse_status_short("A  new.rs", &mut data);
        assert_eq!(data.staged_files.len(), 1);
        assert_eq!(data.staged_files[0].status, 'A');
        assert_eq!(data.staged_files[0].name, "new.rs");
    }

    #[test]
    fn parse_status_short_unstaged_modified() {
        let mut data = GitData::default();
        parse_status_short(" M config.toml", &mut data);
        assert!(data.staged_files.is_empty());
        assert_eq!(data.unstaged_files.len(), 1);
        assert_eq!(data.unstaged_files[0].status, 'M');
        assert_eq!(data.unstaged_files[0].name, "config.toml");
    }

    #[test]
    fn parse_status_short_both_staged_and_unstaged() {
        let mut data = GitData::default();
        parse_status_short("MM src/lib.rs", &mut data);
        assert_eq!(data.staged_files.len(), 1);
        assert_eq!(data.staged_files[0].status, 'M');
        assert_eq!(data.unstaged_files.len(), 1);
        assert_eq!(data.unstaged_files[0].status, 'M');
    }

    #[test]
    fn parse_status_short_untracked() {
        let mut data = GitData::default();
        parse_status_short("?? tmp/debug.log", &mut data);
        assert!(data.staged_files.is_empty());
        assert!(data.unstaged_files.is_empty());
        assert_eq!(data.untracked_files, vec!["debug.log"]);
    }

    #[test]
    fn parse_status_short_untracked_directory() {
        let mut data = GitData::default();
        parse_status_short("?? docs/superpowers/specs/", &mut data);
        assert_eq!(data.untracked_files, vec!["specs/"]);
    }

    #[test]
    fn parse_status_short_untracked_top_level_directory() {
        let mut data = GitData::default();
        parse_status_short("?? mydir/", &mut data);
        assert_eq!(data.untracked_files, vec!["mydir/"]);
    }

    #[test]
    fn parse_status_short_deleted() {
        let mut data = GitData::default();
        parse_status_short("D  old.rs", &mut data);
        assert_eq!(data.staged_files.len(), 1);
        assert_eq!(data.staged_files[0].status, 'D');
    }

    #[test]
    fn parse_status_short_unstaged_deleted() {
        let mut data = GitData::default();
        parse_status_short(" D removed.rs", &mut data);
        assert!(data.staged_files.is_empty());
        assert_eq!(data.unstaged_files.len(), 1);
        assert_eq!(data.unstaged_files[0].status, 'D');
    }

    #[test]
    fn parse_status_short_rename() {
        let mut data = GitData::default();
        parse_status_short("R  old.rs -> new.rs", &mut data);
        assert_eq!(data.staged_files.len(), 1);
        assert_eq!(data.staged_files[0].status, 'M'); // renames shown as M
        assert_eq!(data.staged_files[0].name, "new.rs");
        assert_eq!(data.staged_files[0].path, "new.rs");
    }

    #[test]
    fn parse_status_short_same_basename_keeps_distinct_paths() {
        let mut data = GitData::default();
        parse_status_short("M  src/app.rs\nM  tests/app.rs", &mut data);
        assert_eq!(data.staged_files.len(), 2);
        assert_eq!(data.staged_files[0].name, "app.rs");
        assert_eq!(data.staged_files[0].path, "src/app.rs");
        assert_eq!(data.staged_files[1].name, "app.rs");
        assert_eq!(data.staged_files[1].path, "tests/app.rs");
    }

    #[test]
    fn parse_numstat_keys_full_paths_before_basename_fallback() {
        let map = parse_numstat("1\t0\tsrc/app.rs\n2\t1\ttests/app.rs");
        assert_eq!(map.get("src/app.rs"), Some(&(1, 0)));
        assert_eq!(map.get("tests/app.rs"), Some(&(2, 1)));
    }

    #[test]
    fn parse_status_short_multiple_lines() {
        let mut data = GitData::default();
        parse_status_short(
            "M  src/app.rs\nA  src/new.rs\n M config.toml\n?? tmp/log",
            &mut data,
        );
        assert_eq!(data.staged_files.len(), 2); // M staged + A staged
        assert_eq!(data.unstaged_files.len(), 1); // M unstaged
        assert_eq!(data.untracked_files.len(), 1); // ?? untracked
    }

    #[test]
    fn parse_status_short_empty() {
        let mut data = GitData::default();
        parse_status_short("", &mut data);
        assert!(data.staged_files.is_empty());
        assert!(data.unstaged_files.is_empty());
        assert!(data.untracked_files.is_empty());
    }
}
