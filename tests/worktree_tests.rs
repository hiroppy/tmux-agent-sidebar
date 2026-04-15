use std::path::PathBuf;

use tmux_agent_sidebar::worktree::{
    AGENTS, CLAUDE_MODES, CODEX_MODES, agent_command, modes_for, pick_unique_slug, slugify,
    worktree_path_for,
};

#[test]
fn slugify_lowercases_and_hyphenates_spaces() {
    assert_eq!(slugify("Add login form"), "add-login-form");
}

#[test]
fn slugify_collapses_runs_of_separators() {
    assert_eq!(slugify("Fix --  the   bug!!"), "fix-the-bug");
}

#[test]
fn slugify_strips_leading_and_trailing_punctuation() {
    assert_eq!(slugify("--hello--"), "hello");
    assert_eq!(slugify("   .. world .. "), "world");
}

#[test]
fn slugify_keeps_digits() {
    assert_eq!(slugify("issue 123 fix"), "issue-123-fix");
}

#[test]
fn slugify_returns_empty_for_pure_punctuation() {
    assert_eq!(slugify("!!!"), "");
    assert_eq!(slugify("   "), "");
    assert_eq!(slugify(""), "");
}

#[test]
fn slugify_drops_unicode_and_symbols() {
    // Non-ASCII characters are dropped; surrounding ASCII is still joined.
    assert_eq!(slugify("日本語 task"), "task");
}

#[test]
fn pick_unique_slug_returns_input_when_free() {
    let picked = pick_unique_slug("foo", |_| true).unwrap();
    assert_eq!(picked, "foo");
}

#[test]
fn pick_unique_slug_appends_suffix_on_collision() {
    let taken = ["foo".to_string()];
    let picked = pick_unique_slug("foo", |s| !taken.contains(&s.to_string())).unwrap();
    assert_eq!(picked, "foo-2");
}

#[test]
fn pick_unique_slug_skips_multiple_taken_suffixes() {
    let taken: Vec<String> = ["foo", "foo-2", "foo-3", "foo-4"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let picked = pick_unique_slug("foo", |s| !taken.contains(&s.to_string())).unwrap();
    assert_eq!(picked, "foo-5");
}

#[test]
fn pick_unique_slug_returns_none_when_exhausted() {
    let picked = pick_unique_slug("foo", |_| false);
    assert!(picked.is_none());
}

#[test]
fn worktree_path_uses_sibling_directory() {
    let repo = PathBuf::from("/home/jess/code/myproj");
    let path = worktree_path_for(&repo, "feature").unwrap();
    assert_eq!(
        path,
        PathBuf::from("/home/jess/code/myproj-worktrees/feature")
    );
}

#[test]
fn worktree_path_handles_trailing_components() {
    let repo = PathBuf::from("/tmp/repo");
    let path = worktree_path_for(&repo, "task-2").unwrap();
    assert_eq!(path, PathBuf::from("/tmp/repo-worktrees/task-2"));
}

#[test]
fn worktree_path_returns_none_for_root_path() {
    // `/` has no parent we can build a sibling directory under.
    let repo = PathBuf::from("/");
    assert!(worktree_path_for(&repo, "anything").is_none());
}

// ─── agent_command / modes_for ───────────────────────────────────────────

#[test]
fn agent_command_claude_default_has_no_flag() {
    assert_eq!(agent_command("claude", "default"), "claude");
    assert_eq!(agent_command("claude", ""), "claude");
}

#[test]
fn agent_command_claude_nondefault_uses_permission_mode_flag() {
    assert_eq!(
        agent_command("claude", "plan"),
        "claude --permission-mode plan"
    );
    assert_eq!(
        agent_command("claude", "acceptEdits"),
        "claude --permission-mode acceptEdits"
    );
    assert_eq!(
        agent_command("claude", "dontAsk"),
        "claude --permission-mode dontAsk"
    );
    assert_eq!(
        agent_command("claude", "bypassPermissions"),
        "claude --permission-mode bypassPermissions"
    );
}

#[test]
fn agent_command_codex_maps_to_known_flags() {
    assert_eq!(agent_command("codex", "default"), "codex");
    assert_eq!(agent_command("codex", "auto"), "codex --full-auto");
    assert_eq!(
        agent_command("codex", "bypassPermissions"),
        "codex --dangerously-bypass-approvals-and-sandbox"
    );
}

#[test]
fn agent_command_codex_unknown_mode_falls_back_to_bare_codex() {
    assert_eq!(agent_command("codex", "plan"), "codex");
    assert_eq!(agent_command("codex", ""), "codex");
}

#[test]
fn agent_command_unknown_agent_is_echoed_raw() {
    assert_eq!(agent_command("opencode", "default"), "opencode");
}

#[test]
fn modes_for_claude_returns_claude_modes() {
    assert_eq!(modes_for("claude"), CLAUDE_MODES);
}

#[test]
fn modes_for_codex_returns_codex_modes() {
    assert_eq!(modes_for("codex"), CODEX_MODES);
}

#[test]
fn modes_for_unknown_agent_defaults_to_claude_list() {
    assert_eq!(modes_for("gemini"), CLAUDE_MODES);
    assert_eq!(modes_for(""), CLAUDE_MODES);
}

#[test]
fn agents_list_is_non_empty_and_unique() {
    assert!(!AGENTS.is_empty());
    let mut seen = std::collections::HashSet::new();
    for a in AGENTS {
        assert!(seen.insert(*a), "duplicate agent {a:?}");
    }
}

#[test]
fn mode_lists_start_with_default() {
    assert_eq!(CLAUDE_MODES.first().copied(), Some("default"));
    assert_eq!(CODEX_MODES.first().copied(), Some("default"));
}
