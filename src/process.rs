use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::process::Command;

use crate::tmux::AgentType;

#[derive(Debug, Clone)]
pub(crate) struct ProcessInfo {
    pub(crate) comm: String,
    pub(crate) args: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessSnapshot {
    pub(crate) children_of: HashMap<u32, Vec<u32>>,
    pub(crate) info_by_pid: HashMap<u32, ProcessInfo>,
}

impl ProcessSnapshot {
    pub(crate) fn scan() -> Option<Self> {
        let output = Command::new("ps")
            .args(["-eo", "pid=,ppid=,comm=,args="])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(Self::from_ps_output(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    pub(crate) fn from_ps_output(ps_output: &str) -> Self {
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

        Self {
            children_of,
            info_by_pid,
        }
    }

    pub(crate) fn descendants(&self, seed_pids: &[u32]) -> HashSet<u32> {
        let mut seen = HashSet::new();
        let mut queue: VecDeque<u32> = seed_pids.iter().copied().collect();

        while let Some(pid) = queue.pop_front() {
            if !seen.insert(pid) {
                continue;
            }
            if let Some(children) = self.children_of.get(&pid) {
                for &child in children {
                    if !seen.contains(&child) {
                        queue.push_back(child);
                    }
                }
            }
        }

        seen
    }

    pub(crate) fn tree_has_agent(&self, seed_pids: &[u32], agent: &AgentType) -> bool {
        let agent_name = agent.as_str();
        self.descendants(seed_pids).into_iter().any(|pid| {
            self.info_by_pid
                .get(&pid)
                .map(|info| process_matches_agent(info, agent_name))
                .unwrap_or(false)
        })
    }

    pub(crate) fn command_lines_for_tree(&self, seed_pids: &[u32]) -> Vec<String> {
        self.descendants(seed_pids)
            .into_iter()
            .filter_map(|pid| self.info_by_pid.get(&pid))
            .map(|info| {
                if info.args.is_empty() {
                    info.comm.clone()
                } else {
                    info.args.trim().to_string()
                }
            })
            .collect()
    }
}

pub(crate) fn command_basename(command: &str) -> &str {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
}

pub(crate) fn process_matches_agent(info: &ProcessInfo, agent_name: &str) -> bool {
    if command_basename(&info.comm) == agent_name {
        return true;
    }

    let Some(command) = info.args.split_whitespace().next() else {
        return false;
    };
    command_basename(command.trim_matches('"')) == agent_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descendants_walks_process_tree() {
        let snapshot = ProcessSnapshot {
            children_of: HashMap::from([(1, vec![2, 3]), (2, vec![4]), (4, vec![5])]),
            info_by_pid: HashMap::new(),
        };
        let seen = snapshot.descendants(&[1]);
        assert!(seen.contains(&1));
        assert!(seen.contains(&2));
        assert!(seen.contains(&3));
        assert!(seen.contains(&4));
        assert!(seen.contains(&5));
    }

    #[test]
    fn parse_ps_processes_preserves_spaced_args() {
        let snapshot = ProcessSnapshot::from_ps_output(
            "100 1 codex /Applications/Codex App/bin/codex --full-auto\n101 100 sh sh -c wrapper\n",
        );

        assert_eq!(snapshot.children_of.get(&1).cloned(), Some(vec![100]));
        let info = snapshot.info_by_pid.get(&100).expect("process info");
        assert_eq!(info.comm, "codex");
        assert_eq!(info.args, "/Applications/Codex App/bin/codex --full-auto");
    }

    #[test]
    fn tree_has_agent_matches_descendant_process_name() {
        let snapshot = ProcessSnapshot::from_ps_output(
            "100 1 fish fish -c opencode\n101 100 opencode opencode\n",
        );

        assert!(snapshot.tree_has_agent(&[100], &AgentType::OpenCode));
        assert!(!snapshot.tree_has_agent(&[100], &AgentType::Codex));
    }

    #[test]
    fn process_matches_agent_requires_command_name_match() {
        assert!(process_matches_agent(
            &ProcessInfo {
                comm: "claude".to_string(),
                args: "/opt/homebrew/bin/claude --flag".to_string(),
            },
            "claude",
        ));
        assert!(process_matches_agent(
            &ProcessInfo {
                comm: "node".to_string(),
                args: "/usr/local/bin/opencode".to_string(),
            },
            "opencode",
        ));
        assert!(!process_matches_agent(
            &ProcessInfo {
                comm: "not-opencode".to_string(),
                args: "/usr/local/bin/not-opencode".to_string(),
            },
            "opencode",
        ));
    }
}
