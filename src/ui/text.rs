use unicode_width::UnicodeWidthStr;

/// Display width of a string (CJK = 2 columns, ASCII = 1)
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Pad string with spaces to fill `target_width` display columns
pub fn pad_to(current_display_width: usize, target_width: usize) -> String {
    let pad = target_width.saturating_sub(current_display_width);
    " ".repeat(pad)
}

pub fn elapsed_label(started_at: Option<u64>, now: u64) -> String {
    let ts = match started_at {
        Some(t) if t > 0 => t,
        _ => return String::new(),
    };
    if now < ts {
        return String::new();
    }
    let elapsed = now - ts;
    let secs = elapsed % 60;
    let mins = (elapsed / 60) % 60;
    let hours = elapsed / 3600;
    if hours > 0 {
        format!("{}h{}m{}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m{}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

pub fn wait_reason_label(reason: &str) -> String {
    match reason {
        "permission_prompt" => "permission required".into(),
        "idle_prompt" => "waiting for input".into(),
        "auth_success" => "auth success".into(),
        "elicitation_dialog" => "waiting for selection".into(),
        "rate_limit" => "rate limit".into(),
        "permission_denied" => "permission denied".into(),
        _ => {
            if reason.is_empty() {
                String::new()
            } else {
                reason.to_string()
            }
        }
    }
}

pub fn branch_label(git_info: &crate::group::PaneGitInfo) -> String {
    match &git_info.branch {
        Some(branch) => {
            if git_info.is_worktree {
                match &git_info.worktree_name {
                    Some(name) if name != branch => format!("+ {}: {}", name, branch),
                    _ => format!("+ {}", branch),
                }
            } else {
                branch.clone()
            }
        }
        None => String::new(),
    }
}

/// Truncate string to fit within max display width, adding … if needed
pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let dw = display_width(text);
    if dw <= max_width {
        return text.to_string();
    }
    let mut result = String::new();
    let mut w = 0;
    for ch in text.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw + 1 > max_width {
            break;
        }
        result.push(ch);
        w += cw;
    }
    result.push('\u{2026}');
    result
}

/// Wrap text by display width (not byte count)
pub fn wrap_text_char(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    wrap_text_inner(text, max_width, max_lines, false)
}

pub fn wrap_text(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    wrap_text_inner(text, max_width, max_lines, true)
}

fn wrap_text_inner(text: &str, max_width: usize, max_lines: usize, word_wrap: bool) -> Vec<String> {
    if max_width == 0 || max_lines == 0 {
        return vec![];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut result: Vec<String> = Vec::new();
    let mut pos = 0;

    while pos < chars.len() && result.len() < max_lines {
        // Collect chars that fit within max_width display columns
        let mut chunk = String::new();
        let mut chunk_width = 0;
        let mut end = pos;

        while end < chars.len() {
            let ch_w = unicode_width::UnicodeWidthChar::width(chars[end]).unwrap_or(0);
            if chunk_width + ch_w > max_width {
                break;
            }
            chunk.push(chars[end]);
            chunk_width += ch_w;
            end += 1;
        }

        if end >= chars.len() {
            // All remaining text fits
            result.push(chunk);
            break;
        }

        // Last allowed line -- truncate with ellipsis
        if result.len() + 1 == max_lines {
            // Re-collect leaving room for ...
            let mut trunc = String::new();
            let mut tw = 0;
            let ellipsis_w = 1; // ... is 1 column wide in most terminals
            for &ch in &chars[pos..] {
                let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                if tw + ch_w + ellipsis_w > max_width {
                    break;
                }
                trunc.push(ch);
                tw += ch_w;
            }
            trunc.push('\u{2026}');
            result.push(trunc);
            break;
        }

        // Try to find word boundary (space) for nicer wrapping
        if word_wrap
            && let Some(space_pos) = chunk.rfind(' ')
            && space_pos > 0
        {
            let nice_chunk = chunk[..space_pos].to_string();
            let char_count = nice_chunk.chars().count();
            result.push(nice_chunk);
            pos += char_count;
            while pos < chars.len() && chars[pos] == ' ' {
                pos += 1;
            }
            continue;
        }

        result.push(chunk);
        pos = end;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    // ─── display_width ─────────────────────────────────────────────

    #[test]
    fn display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
    }

    #[test]
    fn display_width_cjk() {
        assert_eq!(display_width("日本語"), 6); // 3 chars × 2 columns
    }

    #[test]
    fn display_width_mixed() {
        assert_eq!(display_width("hi日本"), 6); // 2 + 2×2
    }

    #[test]
    fn display_width_empty() {
        assert_eq!(display_width(""), 0);
    }

    // ─── pad_to ────────────────────────────────────────────────────

    #[test]
    fn pad_to_normal() {
        let p = pad_to(3, 10);
        assert_eq!(p.len(), 7);
        assert_eq!(p, "       ");
    }

    #[test]
    fn pad_to_overflow() {
        let p = pad_to(10, 5);
        assert_eq!(p, ""); // saturating_sub returns 0
    }

    #[test]
    fn pad_to_exact() {
        let p = pad_to(5, 5);
        assert_eq!(p, "");
    }

    // ─── truncate_to_width ─────────────────────────────────────────

    #[test]
    fn truncate_to_width_zero_width() {
        assert_eq!(truncate_to_width("hello", 0), "");
    }

    #[test]
    fn truncate_to_width_exact() {
        assert_eq!(truncate_to_width("hello", 5), "hello");
    }

    // ─── wrap_text ─────────────────────────────────────────────────

    #[test]
    fn wrap_text_short() {
        let lines = wrap_text("hi", 10, 3);
        assert_eq!(lines, vec!["hi"]);
    }

    #[test]
    fn wrap_text_exact_width() {
        let lines = wrap_text("abcde", 5, 3);
        assert_eq!(lines, vec!["abcde"]);
    }

    #[test]
    fn wrap_text_word_wrap() {
        let lines = wrap_text("hello world foo", 10, 3);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "hello");
        assert_eq!(lines[1], "world foo");
    }

    #[test]
    fn wrap_text_truncation_with_ellipsis() {
        // Force truncation on last line
        let lines = wrap_text("aaa bbb ccc ddd eee fff", 10, 2);
        assert_eq!(lines.len(), 2);
        assert!(
            lines[1].contains('\u{2026}'),
            "last line should have ellipsis"
        );
    }

    #[test]
    fn wrap_text_cjk() {
        // Each CJK char is 2 columns wide; width=6 fits 3 CJK chars
        let lines = wrap_text("あいうえお", 6, 3);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "あいう");
        assert_eq!(lines[1], "えお");
    }

    #[test]
    fn wrap_text_zero_width() {
        let lines = wrap_text("hello", 0, 3);
        assert!(lines.is_empty());
    }

    #[test]
    fn wrap_text_zero_lines() {
        let lines = wrap_text("hello", 10, 0);
        assert!(lines.is_empty());
    }

    // ─── wrap_text_char ───────────────────────────────────────────

    #[test]
    fn wrap_text_char_no_word_boundary() {
        // word-wrap would break at "world", char-wrap fills to max_width
        let lines = wrap_text_char("hello world foobar", 10, 3);
        assert_eq!(lines[0], "hello worl");
        assert_eq!(lines[1], "d foobar");
    }

    #[test]
    fn wrap_text_char_short() {
        let lines = wrap_text_char("hi", 10, 3);
        assert_eq!(lines, vec!["hi"]);
    }

    // ─── elapsed_label ─────────────────────────────────────────────

    #[test]
    fn elapsed_label_none() {
        assert_eq!(elapsed_label(None, 0), "");
    }

    #[test]
    fn elapsed_label_zero() {
        assert_eq!(elapsed_label(Some(0), 0), "");
    }

    #[test]
    fn elapsed_label_future() {
        const TEST_NOW: u64 = 1_000_000;
        assert_eq!(elapsed_label(Some(TEST_NOW + 9999), TEST_NOW), "");
    }

    #[test]
    fn elapsed_label_seconds() {
        const TEST_NOW: u64 = 1_000_000;
        let label = elapsed_label(Some(TEST_NOW - 5), TEST_NOW);
        assert!(label.ends_with('s'));
        assert!(!label.contains('m'));
    }

    #[test]
    fn elapsed_label_minutes() {
        const TEST_NOW: u64 = 1_000_000;
        let label = elapsed_label(Some(TEST_NOW - 125), TEST_NOW);
        assert!(label.contains('m'));
        assert!(label.contains('s'));
        assert!(!label.contains('h'));
    }

    #[test]
    fn elapsed_label_hours() {
        const TEST_NOW: u64 = 1_000_000;
        let label = elapsed_label(Some(TEST_NOW - 3661), TEST_NOW);
        assert!(label.contains('h'));
        assert!(label.contains('m'));
    }

    // ─── wait_reason_label ─────────────────────────────────────────

    #[test]
    fn wait_reason_known() {
        assert_eq!(
            wait_reason_label("permission_prompt"),
            "permission required"
        );
        assert_eq!(wait_reason_label("idle_prompt"), "waiting for input");
        assert_eq!(wait_reason_label("auth_success"), "auth success");
        assert_eq!(
            wait_reason_label("elicitation_dialog"),
            "waiting for selection"
        );
        assert_eq!(wait_reason_label("rate_limit"), "rate limit");
    }

    #[test]
    fn wait_reason_unknown() {
        assert_eq!(wait_reason_label("something_else"), "something_else");
    }

    #[test]
    fn wait_reason_empty() {
        assert_eq!(wait_reason_label(""), "");
    }

    // ─── wait_reason_label: permission_denied ──────────────────────

    #[test]
    fn wait_reason_permission_denied() {
        assert_eq!(wait_reason_label("permission_denied"), "permission denied");
    }

    // ─── branch_label ──────────────────────────────────────────────

    #[test]
    fn branch_label_with_branch() {
        use crate::group::PaneGitInfo;
        let info = PaneGitInfo {
            repo_root: Some("/repo".into()),
            branch: Some("main".into()),
            is_worktree: false,
            worktree_name: None,
        };
        assert_eq!(branch_label(&info), "main");
    }

    #[test]
    fn branch_label_worktree() {
        use crate::group::PaneGitInfo;
        let info = PaneGitInfo {
            repo_root: Some("/repo".into()),
            branch: Some("fix/typo".into()),
            is_worktree: true,
            worktree_name: None,
        };
        assert_eq!(branch_label(&info), "+ fix/typo");
    }

    #[test]
    fn branch_label_no_git() {
        use crate::group::PaneGitInfo;
        let info = PaneGitInfo::default();
        assert_eq!(branch_label(&info), "");
    }

    #[test]
    fn branch_label_worktree_with_name() {
        let info = crate::group::PaneGitInfo {
            branch: Some("feat/auth".into()),
            is_worktree: true,
            worktree_name: Some("auth-wt".into()),
            ..Default::default()
        };
        assert_eq!(branch_label(&info), "+ auth-wt: feat/auth");
    }

    #[test]
    fn branch_label_worktree_name_same_as_branch() {
        let info = crate::group::PaneGitInfo {
            branch: Some("feat/auth".into()),
            is_worktree: true,
            worktree_name: Some("feat/auth".into()),
            ..Default::default()
        };
        assert_eq!(branch_label(&info), "+ feat/auth");
    }

    #[test]
    fn branch_label_worktree_no_name() {
        let info = crate::group::PaneGitInfo {
            branch: Some("main".into()),
            is_worktree: true,
            worktree_name: None,
            ..Default::default()
        };
        assert_eq!(branch_label(&info), "+ main");
    }
}
