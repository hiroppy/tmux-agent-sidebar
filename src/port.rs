use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::process::Command;

use crate::tmux::{AgentType, SessionInfo};

#[derive(Debug, Default, Clone)]
pub struct PaneProcessSnapshot {
    pub ports_by_pane: HashMap<String, Vec<u16>>,
    pub command_by_pane: HashMap<String, String>,
    pub live_agent_panes: HashSet<String>,
}

fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

fn parse_pane_pids(sessions: &[SessionInfo]) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for session in sessions {
        for window in &session.windows {
            for pane in &window.panes {
                if let Some(pid) = pane.pane_pid {
                    out.insert(pane.pane_id.clone(), pid);
                }
            }
        }
    }
    out
}

fn parse_ps_processes(ps_output: &str) -> (HashMap<u32, Vec<u32>>, HashMap<u32, String>) {
    let mut children_of: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut args_by_pid: HashMap<u32, String> = HashMap::new();
    for line in ps_output.lines() {
        let mut parts = line.split_whitespace();
        let Some(pid_str) = parts.next() else {
            continue;
        };
        let Some(ppid_str) = parts.next() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let Ok(ppid) = ppid_str.parse::<u32>() else {
            continue;
        };
        children_of.entry(ppid).or_default().push(pid);
        args_by_pid.insert(pid, parts.collect::<Vec<_>>().join(" "));
    }
    (children_of, args_by_pid)
}

fn descendant_pids(seed_pids: &[u32], children_of: &HashMap<u32, Vec<u32>>) -> HashSet<u32> {
    let mut seen = HashSet::new();
    let mut queue: VecDeque<u32> = seed_pids.iter().copied().collect();

    while let Some(pid) = queue.pop_front() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some(children) = children_of.get(&pid) {
            for &child in children {
                if !seen.contains(&child) {
                    queue.push_back(child);
                }
            }
        }
    }

    seen
}

fn process_tree_has_agent(
    seed_pids: &[u32],
    children_of: &HashMap<u32, Vec<u32>>,
    args_by_pid: &HashMap<u32, String>,
    agent: &AgentType,
) -> bool {
    let agent_name = agent.label();
    let descendants = descendant_pids(seed_pids, children_of);
    descendants.into_iter().any(|pid| {
        args_by_pid
            .get(&pid)
            .map(|args| process_matches_agent(args, agent_name))
            .unwrap_or(false)
    })
}

fn process_matches_agent(args: &str, agent_name: &str) -> bool {
    let Some(command) = args.split_whitespace().next() else {
        return false;
    };
    let command = command.trim_matches('"');
    let basename = command.rsplit('/').next().unwrap_or(command);
    basename == agent_name
}

fn process_basename(args: &str) -> Option<&str> {
    let command = args.split_whitespace().next()?;
    let command = command.trim_matches('"');
    Some(command.rsplit('/').next().unwrap_or(command))
}

fn is_shell_command(basename: &str) -> bool {
    matches!(
        basename,
        "bash" | "sh" | "zsh" | "fish" | "tmux" | "login" | "sudo"
    )
}

fn best_command_for_pane(
    pane_pid: u32,
    children_of: &HashMap<u32, Vec<u32>>,
    args_by_pid: &HashMap<u32, String>,
) -> Option<String> {
    let descendants = descendant_pids(&[pane_pid], children_of);
    let mut leaf_candidates: Vec<(usize, String)> = Vec::new();
    let mut fallback_candidates: Vec<(usize, String)> = Vec::new();

    for pid in descendants {
        let Some(args) = args_by_pid.get(&pid) else {
            continue;
        };
        let Some(basename) = process_basename(args) else {
            continue;
        };
        if basename.is_empty() || is_shell_command(basename) {
            continue;
        }
        let candidate = args.trim().to_string();
        let len = candidate.len();
        let is_leaf = children_of
            .get(&pid)
            .map_or(true, |children| children.is_empty());
        if is_leaf {
            leaf_candidates.push((len, candidate));
        } else {
            fallback_candidates.push((len, candidate));
        }
    }

    leaf_candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    if let Some((_, command)) = leaf_candidates.into_iter().next() {
        return Some(command);
    }

    fallback_candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    fallback_candidates
        .into_iter()
        .next()
        .map(|(_, command)| command)
}

fn extract_port(name: &str) -> Option<u16> {
    let trimmed = name.trim();
    let (_, tail) = trimmed.rsplit_once(':')?;
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

fn parse_lsof_listening_ports(lsof_output: &str) -> Vec<(u32, u16)> {
    let mut current_pid: Option<u32> = None;
    let mut out = Vec::new();

    for line in lsof_output.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            current_pid = rest.parse::<u32>().ok();
            continue;
        }
        if let Some(rest) = line.strip_prefix('n')
            && let (Some(pid), Some(port)) = (current_pid, extract_port(rest))
        {
            out.push((pid, port));
        }
    }

    out
}

/// Scan per-pane process state for the provided sessions.
/// The lookup starts from each pane's PID and walks the process tree, so it can
/// pick up child dev servers spawned by an agent shell and detect when the
/// agent process itself has exited.
pub fn scan_session_process_snapshot(sessions: &[SessionInfo]) -> Option<PaneProcessSnapshot> {
    let pane_pids = parse_pane_pids(sessions);
    if pane_pids.is_empty() {
        return None;
    }

    let Some(ps_output) = run_command("ps", &["-eo", "pid=,ppid=,args="]) else {
        return None;
    };
    let (children_of, args_by_pid) = parse_ps_processes(&ps_output);

    let mut pid_to_panes: HashMap<u32, Vec<String>> = HashMap::new();
    let mut live_agent_panes: HashSet<String> = HashSet::new();
    let mut command_by_pane: HashMap<String, String> = HashMap::new();
    for session in sessions {
        for window in &session.windows {
            for pane in &window.panes {
                let Some(&pane_pid) = pane_pids.get(&pane.pane_id) else {
                    continue;
                };
                let descendant_set = descendant_pids(&[pane_pid], &children_of);
                if process_tree_has_agent(&[pane_pid], &children_of, &args_by_pid, &pane.agent) {
                    live_agent_panes.insert(pane.pane_id.clone());
                }
                if let Some(command) = best_command_for_pane(pane_pid, &children_of, &args_by_pid) {
                    command_by_pane.insert(pane.pane_id.clone(), command);
                }
                for pid in descendant_set {
                    pid_to_panes
                        .entry(pid)
                        .or_default()
                        .push(pane.pane_id.clone());
                }
            }
        }
    }

    let Some(lsof_output) = run_command("lsof", &["-iTCP", "-sTCP:LISTEN", "-nP", "-F", "pn"])
    else {
        return None;
    };
    let listening = parse_lsof_listening_ports(&lsof_output);

    let mut ports_by_pane: HashMap<String, BTreeSet<u16>> = HashMap::new();
    for (pid, port) in listening {
        if let Some(panes) = pid_to_panes.get(&pid) {
            for pane_id in panes {
                ports_by_pane
                    .entry(pane_id.clone())
                    .or_default()
                    .insert(port);
            }
        }
    }

    Some(PaneProcessSnapshot {
        ports_by_pane: ports_by_pane
            .into_iter()
            .map(|(pane_id, ports)| (pane_id, ports.into_iter().collect()))
            .collect(),
        command_by_pane,
        live_agent_panes,
    })
}

/// Scan listening TCP ports for panes in the provided sessions.
/// The lookup starts from each pane's PID and walks the process tree, so it can
/// pick up child dev servers spawned by an agent shell.
pub fn scan_session_ports(sessions: &[SessionInfo]) -> HashMap<String, Vec<u16>> {
    scan_session_process_snapshot(sessions)
        .map(|snapshot| snapshot.ports_by_pane)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_port_handles_common_lsof_names() {
        assert_eq!(extract_port("127.0.0.1:3000"), Some(3000));
        assert_eq!(extract_port("*:5173"), Some(5173));
        assert_eq!(extract_port("localhost:http"), None);
    }

    #[test]
    fn parse_lsof_listening_ports_pairs_pid_and_port() {
        let sample = "p123\nn127.0.0.1:3000\np456\nn*:5173\n";
        assert_eq!(
            parse_lsof_listening_ports(sample),
            vec![(123, 3000), (456, 5173)]
        );
    }

    #[test]
    fn best_command_for_pane_prefers_leaf_non_shell_command() {
        let children = HashMap::from([(10, vec![11, 12]), (11, vec![]), (12, vec![])]);
        let args = HashMap::from([
            (10, "zsh".to_string()),
            (11, "/usr/bin/node /tmp/server.js --port 3000".to_string()),
            (12, "/usr/bin/git status".to_string()),
        ]);

        let command = best_command_for_pane(10, &children, &args).unwrap();
        assert_eq!(command, "/usr/bin/node /tmp/server.js --port 3000");
    }

    #[test]
    fn descendant_pids_walks_process_tree() {
        let children = HashMap::from([(1, vec![2, 3]), (2, vec![4]), (4, vec![5])]);
        let seen = descendant_pids(&[1], &children);
        assert!(seen.contains(&1));
        assert!(seen.contains(&2));
        assert!(seen.contains(&3));
        assert!(seen.contains(&4));
        assert!(seen.contains(&5));
    }

    #[test]
    fn process_tree_has_agent_matches_descendant_process_name() {
        let children = HashMap::from([(1, vec![2, 3]), (2, vec![4])]);
        let args = HashMap::from([
            (1, "bash".to_string()),
            (2, "node".to_string()),
            (3, "/opt/homebrew/bin/claude --flag".to_string()),
            (4, "sleep 1".to_string()),
        ]);
        assert!(process_tree_has_agent(
            &[1],
            &children,
            &args,
            &AgentType::Claude
        ));
        assert!(!process_tree_has_agent(
            &[1],
            &children,
            &args,
            &AgentType::Codex
        ));
    }

    #[test]
    fn process_matches_agent_requires_command_name_match() {
        assert!(process_matches_agent("/opt/bin/claude --flag", "claude"));
        assert!(process_matches_agent(
            "\"/usr/local/bin/codex\" run",
            "codex"
        ));
        assert!(!process_matches_agent("bash -lc codex", "codex"));
        assert!(!process_matches_agent("grep claude", "claude"));
    }
}
