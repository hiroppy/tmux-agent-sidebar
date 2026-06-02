use std::collections::HashMap;
use std::path::Path;

use crate::tmux;

const DEFAULT_OPEN_COMMAND: &str = "${EDITOR:-nvim} {file}";
const TARGET_RIGHT_PANE: &str = "right-pane";

pub fn open_git_file(sidebar_pane: &str, repo_root: &str, file_path: &str) -> Result<(), String> {
    let opts = tmux::get_all_global_options();
    let mut ops = RealTmuxOps;
    open_git_file_with_ops(sidebar_pane, repo_root, file_path, &opts, &mut ops)
}

pub(crate) trait GitOpenTmuxOps {
    fn run_tmux_capture(&mut self, args: &[&str]) -> Result<String, String>;
    fn display_message(&mut self, target: &str, format: &str) -> String;
    fn get_pane_option_value(&mut self, pane: &str, key: &str) -> String;
    fn set_pane_option(&mut self, pane: &str, key: &str, value: &str);
}

struct RealTmuxOps;

impl GitOpenTmuxOps for RealTmuxOps {
    fn run_tmux_capture(&mut self, args: &[&str]) -> Result<String, String> {
        tmux::run_tmux_capture(args)
    }

    fn display_message(&mut self, target: &str, format: &str) -> String {
        tmux::display_message(target, format)
    }

    fn get_pane_option_value(&mut self, pane: &str, key: &str) -> String {
        tmux::get_pane_option_value(pane, key)
    }

    fn set_pane_option(&mut self, pane: &str, key: &str, value: &str) {
        tmux::set_pane_option(pane, key, value);
    }
}

pub(crate) fn open_git_file_with_ops<T: GitOpenTmuxOps>(
    sidebar_pane: &str,
    repo_root: &str,
    file_path: &str,
    opts: &HashMap<String, String>,
    ops: &mut T,
) -> Result<(), String> {
    if repo_root.is_empty() {
        return Err("missing repository root".into());
    }
    if file_path.is_empty() {
        return Err("missing file path".into());
    }

    let template = opts
        .get(tmux::SIDEBAR_GIT_OPEN_COMMAND)
        .filter(|s| !s.trim().is_empty())
        .map(String::as_str)
        .unwrap_or(DEFAULT_OPEN_COMMAND);
    let command = build_open_command(template, repo_root, file_path);
    let target = opts
        .get(tmux::SIDEBAR_GIT_OPEN_TARGET)
        .map(String::as_str)
        .unwrap_or("popup");

    if target == TARGET_RIGHT_PANE {
        open_in_right_pane(sidebar_pane, repo_root, &command, ops)
    } else {
        run_tmux_args(
            ops,
            vec![
                "display-popup".into(),
                "-E".into(),
                "-d".into(),
                repo_root.into(),
                command,
            ],
        )
        .map(|_| ())
    }
}

fn open_in_right_pane<T: GitOpenTmuxOps>(
    sidebar_pane: &str,
    repo_root: &str,
    command: &str,
    ops: &mut T,
) -> Result<(), String> {
    let existing = ops.get_pane_option_value(sidebar_pane, tmux::SIDEBAR_GIT_OPEN_PANE);
    if !existing.is_empty() && ops.display_message(&existing, "#{pane_id}") == existing {
        return run_tmux_args(
            ops,
            vec![
                "respawn-pane".into(),
                "-k".into(),
                "-t".into(),
                existing,
                "-c".into(),
                repo_root.into(),
                command.into(),
            ],
        )
        .map(|_| ());
    }

    let pane_id = run_tmux_args(
        ops,
        vec![
            "split-window".into(),
            "-h".into(),
            "-t".into(),
            sidebar_pane.into(),
            "-c".into(),
            repo_root.into(),
            "-P".into(),
            "-F".into(),
            "#{pane_id}".into(),
            command.into(),
        ],
    )?;
    if !pane_id.is_empty() {
        ops.set_pane_option(sidebar_pane, tmux::SIDEBAR_GIT_OPEN_PANE, &pane_id);
    }
    Ok(())
}

fn run_tmux_args<T: GitOpenTmuxOps>(ops: &mut T, args: Vec<String>) -> Result<String, String> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    ops.run_tmux_capture(&refs)
}

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
    use std::collections::HashMap;

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
        let opts = HashMap::new();

        open_git_file_with_ops("%sidebar", "/repo", "src/main.rs", &opts, &mut ops).unwrap();

        assert_eq!(
            ops.commands,
            vec![vec![
                "display-popup".to_string(),
                "-E".to_string(),
                "-d".to_string(),
                "/repo".to_string(),
                "${EDITOR:-nvim} 'src/main.rs'".to_string(),
            ]]
        );
    }

    #[test]
    fn right_pane_target_reuses_existing_sidebar_pane() {
        let mut ops = FakeTmuxOps {
            stored_pane: "%git".into(),
            pane_exists: true,
            ..FakeTmuxOps::default()
        };
        let opts = HashMap::from([(
            crate::tmux::SIDEBAR_GIT_OPEN_TARGET.to_string(),
            "right-pane".to_string(),
        )]);

        open_git_file_with_ops("%sidebar", "/repo", "src/main.rs", &opts, &mut ops).unwrap();

        assert_eq!(
            ops.commands,
            vec![vec![
                "respawn-pane".to_string(),
                "-k".to_string(),
                "-t".to_string(),
                "%git".to_string(),
                "-c".to_string(),
                "/repo".to_string(),
                "${EDITOR:-nvim} 'src/main.rs'".to_string(),
            ]]
        );
    }

    #[test]
    fn right_pane_target_creates_and_remembers_missing_sidebar_pane() {
        let mut ops = FakeTmuxOps {
            split_output: "%new".into(),
            ..FakeTmuxOps::default()
        };
        let opts = HashMap::from([(
            crate::tmux::SIDEBAR_GIT_OPEN_TARGET.to_string(),
            "right-pane".to_string(),
        )]);

        open_git_file_with_ops("%sidebar", "/repo", "src/main.rs", &opts, &mut ops).unwrap();

        assert_eq!(
            ops.commands,
            vec![vec![
                "split-window".to_string(),
                "-h".to_string(),
                "-t".to_string(),
                "%sidebar".to_string(),
                "-c".to_string(),
                "/repo".to_string(),
                "-P".to_string(),
                "-F".to_string(),
                "#{pane_id}".to_string(),
                "${EDITOR:-nvim} 'src/main.rs'".to_string(),
            ]]
        );
        assert_eq!(ops.set_options, vec![("%sidebar".into(), "%new".into())]);
    }

    #[derive(Default)]
    struct FakeTmuxOps {
        commands: Vec<Vec<String>>,
        set_options: Vec<(String, String)>,
        stored_pane: String,
        split_output: String,
        pane_exists: bool,
    }

    impl GitOpenTmuxOps for FakeTmuxOps {
        fn run_tmux_capture(&mut self, args: &[&str]) -> Result<String, String> {
            self.commands
                .push(args.iter().map(|arg| arg.to_string()).collect());
            if args.first() == Some(&"split-window") {
                Ok(self.split_output.clone())
            } else {
                Ok(String::new())
            }
        }

        fn display_message(&mut self, target: &str, _: &str) -> String {
            if self.pane_exists {
                target.to_string()
            } else {
                String::new()
            }
        }

        fn get_pane_option_value(&mut self, _: &str, _: &str) -> String {
            self.stored_pane.clone()
        }

        fn set_pane_option(&mut self, pane: &str, _: &str, value: &str) {
            self.set_options.push((pane.to_string(), value.to_string()));
        }
    }
}
