use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::process::Command;

use crate::tmux::{AgentType, SessionInfo};

#[derive(Debug, Default, Clone)]
pub struct PaneProcessSnapshot {
    pub ports_by_pane: HashMap<String, Vec<u16>>,
    pub command_by_pane: HashMap<String, String>,
    pub live_agent_panes: HashSet<String>,
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    comm: String,
    args: String,
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

fn parse_ps_processes(ps_output: &str) -> (HashMap<u32, Vec<u32>>, HashMap<u32, ProcessInfo>) {
    let mut children_of: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut info_by_pid: HashMap<u32, ProcessInfo> = HashMap::new();
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
        let Some(comm) = parts.next() else {
            continue;
        };
        children_of.entry(ppid).or_default().push(pid);
        info_by_pid.insert(
            pid,
            ProcessInfo {
                comm: comm.to_string(),
                args: parts.collect::<Vec<_>>().join(" "),
            },
        );
    }
    (children_of, info_by_pid)
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
    info_by_pid: &HashMap<u32, ProcessInfo>,
    agent: &AgentType,
) -> bool {
    let agent_name = agent.label();
    let descendants = descendant_pids(seed_pids, children_of);
    descendants.into_iter().any(|pid| {
        info_by_pid
            .get(&pid)
            .map(|info| process_matches_agent(info, agent_name))
            .unwrap_or(false)
    })
}

fn process_matches_agent(info: &ProcessInfo, agent_name: &str) -> bool {
    if info.comm == agent_name {
        return true;
    }

    let Some(command) = info.args.split_whitespace().next() else {
        return false;
    };
    let command = command.trim_matches('"');
    let basename = command.rsplit('/').next().unwrap_or(command);
    basename == agent_name
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
    info_by_pid: &HashMap<u32, ProcessInfo>,
) -> Option<String> {
    let descendants = descendant_pids(&[pane_pid], children_of);
    let mut leaf_candidates: Vec<(usize, String)> = Vec::new();
    let mut fallback_candidates: Vec<(usize, String)> = Vec::new();

    for pid in descendants {
        let Some(info) = info_by_pid.get(&pid) else {
            continue;
        };
        let basename = info.comm.as_str();
        if basename.is_empty() || is_shell_command(basename) {
            continue;
        }
        let candidate = if info.args.is_empty() {
            info.comm.clone()
        } else {
            info.args.trim().to_string()
        };
        let len = candidate.len();
        let is_leaf = children_of
            .get(&pid)
            .is_none_or(|children| children.is_empty());
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

    let ps_output = run_command("ps", &["-eo", "pid=,ppid=,comm=,args="])?;
    let (children_of, info_by_pid) = parse_ps_processes(&ps_output);

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
                if process_tree_has_agent(&[pane_pid], &children_of, &info_by_pid, &pane.agent) {
                    live_agent_panes.insert(pane.pane_id.clone());
                }
                if let Some(command) = best_command_for_pane(pane_pid, &children_of, &info_by_pid) {
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

    let lsof_output = run_command("lsof", &["-iTCP", "-sTCP:LISTEN", "-nP", "-F", "pn"])?;
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
        let info = HashMap::from([
            (
                10,
                ProcessInfo {
                    comm: "zsh".to_string(),
                    args: "zsh".to_string(),
                },
            ),
            (
                11,
                ProcessInfo {
                    comm: "node".to_string(),
                    args: "/usr/bin/node /tmp/server.js --port 3000".to_string(),
                },
            ),
            (
                12,
                ProcessInfo {
                    comm: "git".to_string(),
                    args: "/usr/bin/git status".to_string(),
                },
            ),
        ]);

        let command = best_command_for_pane(10, &children, &info).unwrap();
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
        let info = HashMap::from([
            (
                1,
                ProcessInfo {
                    comm: "bash".to_string(),
                    args: "bash".to_string(),
                },
            ),
            (
                2,
                ProcessInfo {
                    comm: "node".to_string(),
                    args: "node".to_string(),
                },
            ),
            (
                3,
                ProcessInfo {
                    comm: "claude".to_string(),
                    args: "/opt/homebrew/bin/claude --flag".to_string(),
                },
            ),
            (
                4,
                ProcessInfo {
                    comm: "sleep".to_string(),
                    args: "sleep 1".to_string(),
                },
            ),
        ]);
        assert!(process_tree_has_agent(
            &[1],
            &children,
            &info,
            &AgentType::Claude
        ));
        assert!(!process_tree_has_agent(
            &[1],
            &children,
            &info,
            &AgentType::Codex
        ));
    }

    #[test]
    fn process_matches_agent_requires_command_name_match() {
        assert!(process_matches_agent(
            &ProcessInfo {
                comm: "claude".to_string(),
                args: "/opt/bin/claude --flag".to_string(),
            },
            "claude"
        ));
        assert!(process_matches_agent(
            &ProcessInfo {
                comm: "codex".to_string(),
                args: "\"/usr/local/bin/codex\" run".to_string(),
            },
            "codex"
        ));
        assert!(!process_matches_agent(
            &ProcessInfo {
                comm: "bash".to_string(),
                args: "bash -lc codex".to_string(),
            },
            "codex"
        ));
        assert!(!process_matches_agent(
            &ProcessInfo {
                comm: "grep".to_string(),
                args: "grep claude".to_string(),
            },
            "claude"
        ));
    }

    #[test]
    fn parse_ps_processes_preserves_spaced_args() {
        let (children, info_by_pid) = parse_ps_processes(
            "100 1 codex /Applications/Codex App/bin/codex --full-auto\n101 100 sh sh -c wrapper\n",
        );

        assert_eq!(children.get(&1).cloned(), Some(vec![100]));
        let info = info_by_pid.get(&100).expect("process info");
        assert_eq!(info.comm, "codex");
        assert_eq!(info.args, "/Applications/Codex App/bin/codex --full-auto");
    }
}
