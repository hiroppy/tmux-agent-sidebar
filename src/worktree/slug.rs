use std::path::{Path, PathBuf};

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

/// `<parent>/<repo_name>-worktrees/<slug>` — sibling to the repo.
pub fn worktree_path_for(repo_root: &Path, slug: &str) -> Option<PathBuf> {
    let parent = repo_root.parent()?;
    let name = repo_root.file_name()?.to_str()?;
    Some(parent.join(format!("{name}-worktrees")).join(slug))
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
    fn worktree_path_for_returns_none_without_parent() {
        assert!(worktree_path_for(Path::new("/"), "foo").is_none());
    }
}
