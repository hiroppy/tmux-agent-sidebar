//! `spawn` subcommand — create a new worktree + tmux window running an agent.

use std::path::PathBuf;

use crate::git;
use crate::tmux;
use crate::worktree::{self, SpawnRequest};

pub fn cmd_spawn(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("usage: tmux-agent-sidebar spawn <name>");
        return 2;
    }
    let pane = std::env::var("TMUX_PANE").unwrap_or_default();
    if pane.is_empty() {
        eprintln!("error: TMUX_PANE is not set; spawn must be run from inside tmux");
        return 1;
    }
    let Some(cwd) = tmux::get_pane_path(&pane).filter(|s| !s.is_empty()) else {
        eprintln!("error: could not resolve current pane path");
        return 1;
    };
    let Some(repo_root) = git::repo_root(&cwd) else {
        eprintln!("error: {cwd} is not inside a git repository");
        return 1;
    };
    let Some(session) = tmux::pane_session_name(&pane) else {
        eprintln!("error: could not resolve current tmux session");
        return 1;
    };

    let opts = tmux::get_all_global_options();
    let agent = opts
        .get(worktree::AGENT_OPTION)
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| worktree::DEFAULT_AGENT.into());

    let req = SpawnRequest {
        repo_root: PathBuf::from(repo_root),
        task_name: args.join(" "),
        session,
        agent,
        mode: worktree::DEFAULT_MODE.into(),
    };
    match worktree::spawn(&req) {
        Ok(branch) => {
            println!("spawned {branch}");
            0
        }
        Err(e) => {
            eprintln!("spawn failed: {e}");
            1
        }
    }
}
