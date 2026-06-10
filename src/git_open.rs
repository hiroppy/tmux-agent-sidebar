use std::path::Path;

use crate::tmux;

const DEFAULT_OPEN_COMMAND: &str = "${EDITOR:-vim} {file}";
const DEFAULT_POPUP_WIDTH: &str = "95%";
const DEFAULT_POPUP_HEIGHT: &str = "95%";

/// Opens a Git file row from the sidebar in a tmux popup.
pub fn open_git_file(sidebar_pane: &str, repo_root: &str, file_path: &str) -> Result<(), String> {
    let mut ops = RealTmuxOps;
    open_git_file_with_ops(sidebar_pane, repo_root, file_path, &mut ops)
}

/// Tmux operations needed by Git file opening, abstracted for tests.
pub(crate) trait GitOpenTmuxOps {
    /// Runs a tmux command and returns captured stdout.
    fn run_tmux_capture(&mut self, args: &[&str]) -> Result<String, String>;
}

struct RealTmuxOps;

impl GitOpenTmuxOps for RealTmuxOps {
    fn run_tmux_capture(&mut self, args: &[&str]) -> Result<String, String> {
        tmux::run_tmux_capture(args)
    }
}

/// Opens a Git file using injected tmux operations.
pub(crate) fn open_git_file_with_ops<T: GitOpenTmuxOps>(
    _sidebar_pane: &str,
    repo_root: &str,
    file_path: &str,
    ops: &mut T,
) -> Result<(), String> {
    if repo_root.is_empty() {
        return Err("missing repository root".into());
    }
    if file_path.is_empty() {
        return Err("missing file path".into());
    }

    let command = build_open_command(DEFAULT_OPEN_COMMAND, repo_root, file_path);
    run_tmux_args(
        ops,
        vec![
            "display-popup".into(),
            "-E".into(),
            "-w".into(),
            DEFAULT_POPUP_WIDTH.into(),
            "-h".into(),
            DEFAULT_POPUP_HEIGHT.into(),
            "-d".into(),
            repo_root.into(),
            command,
        ],
    )
    .map(|_| ())
}

fn run_tmux_args<T: GitOpenTmuxOps>(ops: &mut T, args: Vec<String>) -> Result<String, String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    ops.run_tmux_capture(&refs)
}

/// Expands the configured command template with shell-escaped file paths.
pub(crate) fn build_open_command(template: &str, repo_root: &str, file_path: &str) -> String {
    let abs_file = Path::new(repo_root).join(file_path);
    let quoted_abs_file = shell_quote(&abs_file.to_string_lossy());
    let quoted_file = shell_quote(file_path);
    let mut command = String::new();
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        command.push_str(&rest[..start]);
        let after_open = &rest[start..];
        if let Some(next) = after_open.strip_prefix("{abs_file}") {
            command.push_str(&quoted_abs_file);
            rest = next;
        } else if let Some(next) = after_open.strip_prefix("{file}") {
            command.push_str(&quoted_file);
            rest = next;
        } else {
            command.push('{');
            rest = &after_open[1..];
        }
    }

    command.push_str(rest);
    command
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".into();
    }

    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_open_command_shell_quotes_file_placeholders() {
        let command = build_open_command(
            "nvim {file} -- {abs_file}",
            "/repo root",
            "src/weird file's.rs",
        );

        assert_eq!(
            command,
            "nvim 'src/weird file'\\''s.rs' -- '/repo root/src/weird file'\\''s.rs'"
        );
    }

    #[test]
    fn build_open_command_does_not_expand_placeholder_text_inside_paths() {
        let command = build_open_command("nvim {abs_file} -- {file}", "/repo", "src/{file}.rs");

        assert_eq!(command, "nvim '/repo/src/{file}.rs' -- 'src/{file}.rs'");
    }

    #[test]
    fn popup_target_uses_default_editor_fallback_command() {
        let mut ops = FakeTmuxOps::default();

        open_git_file_with_ops("%sidebar", "/repo", "src/main.rs", &mut ops).unwrap();

        assert_eq!(
            ops.commands,
            vec![vec![
                "display-popup".to_string(),
                "-E".to_string(),
                "-w".to_string(),
                "95%".to_string(),
                "-h".to_string(),
                "95%".to_string(),
                "-d".to_string(),
                "/repo".to_string(),
                "${EDITOR:-vim} 'src/main.rs'".to_string(),
            ]]
        );
    }

    #[derive(Default)]
    struct FakeTmuxOps {
        commands: Vec<Vec<String>>,
    }

    impl GitOpenTmuxOps for FakeTmuxOps {
        fn run_tmux_capture(&mut self, args: &[&str]) -> Result<String, String> {
            self.commands
                .push(args.iter().map(|arg| arg.to_string()).collect());
            Ok(String::new())
        }
    }
}
