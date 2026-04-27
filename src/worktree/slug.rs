use std::path::{Component, Path, PathBuf};

use super::config::DEFAULT_WORKTREE_DIR;

pub(super) const MAX_COLLISION_ATTEMPTS: usize = 99;

pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_hyphen = true;
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_hyphen = false;
        } else if !last_hyphen {
            out.push('-');
            last_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Try `slug`, then `slug-2`, `slug-3`, … until `is_free` returns true.
pub fn pick_unique_slug(slug: &str, is_free: impl Fn(&str) -> bool) -> Option<String> {
    if is_free(slug) {
        return Some(slug.to_string());
    }
    (2..=MAX_COLLISION_ATTEMPTS + 1)
        .map(|n| format!("{slug}-{n}"))
        .find(|candidate| is_free(candidate))
}

/// `<repo>/<worktree_dir>/<slug>` — repo-local worktree directory.
pub fn worktree_path_for(
    repo_root: &Path,
    slug: &str,
    worktree_dir: Option<&str>,
) -> Option<PathBuf> {
    let dir = worktree_dir
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_WORKTREE_DIR);
    let dir = Path::new(dir);
    if dir.is_absolute() || dir.components().any(|c| c == Component::ParentDir) {
        return None;
    }
    Some(repo_root.join(dir).join(slug))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_control_and_zero_width_chars() {
        // Non-ASCII alphanumerics collapse to separators; control chars do too.
        assert_eq!(slugify("hello\tworld\n"), "hello-world");
        assert_eq!(slugify("tab\ttab"), "tab-tab");
    }

    #[test]
    fn pick_unique_slug_stops_at_limit() {
        // Artificial "nothing is free": must exhaust up to MAX_COLLISION_ATTEMPTS+1 and give up.
        let result = pick_unique_slug("slug", |_| false);
        assert!(result.is_none());
    }

    #[test]
    fn pick_unique_slug_prefers_first_free_candidate() {
        let result = pick_unique_slug("slug", |c| c == "slug-3");
        assert_eq!(result.as_deref(), Some("slug-3"));
    }

    #[test]
    fn worktree_path_for_rejects_absolute_worktree_dir() {
        assert!(worktree_path_for(Path::new("/repo"), "foo", Some("/tmp/wt")).is_none());
    }

    #[test]
    fn worktree_path_for_rejects_parent_relative_worktree_dir() {
        assert!(worktree_path_for(Path::new("/repo"), "foo", Some("../wt")).is_none());
    }
}
