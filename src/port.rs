use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::process::Command;

use crate::tmux::SessionInfo;

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

fn parse_ps_children(ps_output: &str) -> HashMap<u32, Vec<u32>> {
    let mut children_of: HashMap<u32, Vec<u32>> = HashMap::new();
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
    }
    children_of
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

/// Scan listening TCP ports for panes in the provided sessions.
/// The lookup starts from each pane's PID and walks the process tree, so it can
/// pick up child dev servers spawned by an agent shell.
pub fn scan_session_ports(sessions: &[SessionInfo]) -> HashMap<String, Vec<u16>> {
    let pane_pids = parse_pane_pids(sessions);
    if pane_pids.is_empty() {
        return HashMap::new();
    }

    let Some(ps_output) = run_command("ps", &["-eo", "pid=,ppid="]) else {
        return HashMap::new();
    };
    let children_of = parse_ps_children(&ps_output);

    let mut pid_to_panes: HashMap<u32, Vec<String>> = HashMap::new();
    for session in sessions {
        for window in &session.windows {
            for pane in &window.panes {
                let Some(&pane_pid) = pane_pids.get(&pane.pane_id) else {
                    continue;
                };
                let descendant_set = descendant_pids(&[pane_pid], &children_of);
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
        return HashMap::new();
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

    ports_by_pane
        .into_iter()
        .map(|(pane_id, ports)| (pane_id, ports.into_iter().collect()))
        .collect()
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
    fn descendant_pids_walks_process_tree() {
        let children = HashMap::from([(1, vec![2, 3]), (2, vec![4]), (4, vec![5])]);
        let seen = descendant_pids(&[1], &children);
        assert!(seen.contains(&1));
        assert!(seen.contains(&2));
        assert!(seen.contains(&3));
        assert!(seen.contains(&4));
        assert!(seen.contains(&5));
    }
}
