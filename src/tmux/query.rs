use std::path::Path;
use std::process::Command;

use super::commands::run_tmux;
use super::options::unset_pane_option;
use super::types::{
    AgentType, CODEX_AGENT, PaneInfo, PaneStatus, PermissionMode, SessionInfo, WindowInfo,
    WorktreeMetadata,
};

// Field indices in `tmux list-panes -F` output. Keep in lock-step with
// the `PANE_FORMAT` format string. When adding a new field, update both
// this module and the format string together.
mod session_line_field {
    pub const SESSION_NAME: usize = 0;
    pub const WINDOW_ID: usize = 1;
    pub const WINDOW_NAME: usize = 3;
    pub const WINDOW_ACTIVE: usize = 4;
    pub const AUTOMATIC_RENAME: usize = 5;
    /// Index where the per-pane field suffix consumed by `parse_pane_line` begins.
    pub const PANE_LINE_OFFSET: usize = 6;
    /// Minimum number of fields a valid `PANE_FORMAT` line must contain.
    pub const MIN_FIELDS: usize = 27;
}

// Indices into the pane-line suffix that `parse_pane_line` operates on.
// Each value = (absolute index in the full format string) - 6, because
// `build_session_hierarchy` strips the leading 6 window-level fields
// before joining the remainder back into `pane_line`.
pub(super) mod pane_line_field {
    pub const PANE_ACTIVE: usize = 0; // absolute 6
    pub const PANE_STATUS: usize = 1; // absolute 7  (@pane_status)
    pub const PANE_ATTENTION: usize = 2; // absolute 8  (@pane_attention)
    pub const AGENT: usize = 3; // absolute 9  (@pane_agent)
    pub const PANE_CURRENT_PATH: usize = 5; // absolute 11 (pane_current_path)
    pub const PANE_CURRENT_COMMAND: usize = 6; // absolute 12
    pub const PANE_ROLE: usize = 7; // absolute 13 (@pane_role)
    pub const PANE_ID: usize = 8; // absolute 14
    pub const PROMPT: usize = 9; // absolute 15 (@pane_prompt)
    pub const PROMPT_SOURCE: usize = 10; // absolute 16 (@pane_prompt_source)
    pub const STARTED_AT: usize = 11; // absolute 17 (@pane_started_at)
    pub const WAIT_REASON: usize = 12; // absolute 18 (@pane_wait_reason)
    pub const PANE_PID: usize = 13; // absolute 19
    pub const SUBAGENTS: usize = 14; // absolute 20 (@pane_subagents)
    pub const PANE_CWD: usize = 15; // absolute 21 (@pane_cwd)
    pub const PERMISSION_MODE: usize = 16; // absolute 22 (@pane_permission_mode)
    pub const WORKTREE_NAME: usize = 17; // absolute 23 (@pane_worktree_name)
    pub const WORKTREE_BRANCH: usize = 18; // absolute 24 (@pane_worktree_branch)
    pub const SESSION_ID: usize = 19; // absolute 25 (@pane_session_id)
    pub const SIDEBAR_SPAWNED: usize = 20; // absolute 26 (@agent-sidebar-spawned)
    /// Minimum number of fields the pane-line suffix must contain.
    /// Equals `session_line_field::MIN_FIELDS - PANE_LINE_OFFSET`.
    pub const MIN_FIELDS: usize = 21;
}

/// tmux `list-panes -F` format used by [`query_sessions`]. Every field is
/// quoted with `#{q:...}` so embedded pipes in user content survive the split.
const PANE_FORMAT: &str = "#{q:session_name}|#{q:window_id}|#{q:window_index}|#{q:window_name}|#{q:window_active}|#{q:automatic-rename}|#{q:pane_active}|#{q:@pane_status}|#{q:@pane_attention}|#{q:@pane_agent}|#{q:@pane_name}|#{q:pane_current_path}|#{q:pane_current_command}|#{q:@pane_role}|#{q:pane_id}|#{q:@pane_prompt}|#{q:@pane_prompt_source}|#{q:@pane_started_at}|#{q:@pane_wait_reason}|#{q:pane_pid}|#{q:@pane_subagents}|#{q:@pane_cwd}|#{q:@pane_permission_mode}|#{q:@pane_worktree_name}|#{q:@pane_worktree_branch}|#{q:@pane_session_id}|#{q:@agent-sidebar-spawned}";

type SessionMap = indexmap::IndexMap<String, indexmap::IndexMap<String, WindowInfo>>;

/// (window_id, pane_index_in_window, pane_pid) — the minimum info needed to
/// later retarget a permission-mode update at the right pane.
type CodexPidEntry = (String, usize, u32);

/// Query all sessions, windows, and panes in a single `tmux list-panes -a` call
/// (plus one optional `ps` call to resolve Codex permission modes), instead of
/// N+1 subprocess invocations.
pub fn query_sessions() -> Vec<SessionInfo> {
    let all_panes_output = match run_tmux(&["list-panes", "-a", "-F", PANE_FORMAT]) {
        Some(s) => s,
        None => return vec![],
    };

    let (mut sessions_map, codex_pids) = build_session_hierarchy(&all_panes_output);
    if !codex_pids.is_empty() {
        resolve_codex_permission_modes(&mut sessions_map, &codex_pids);
    }
    finalize_sessions(sessions_map)
}

/// Parse the raw `tmux list-panes` output into an indexed session→window→pane
/// hierarchy. Also returns every Codex pane's pid so the caller can resolve
/// permission modes in a single `ps` pass.
fn build_session_hierarchy(all_panes_output: &str) -> (SessionMap, Vec<CodexPidEntry>) {
    let mut sessions_map: SessionMap = indexmap::IndexMap::new();
    let mut codex_pids: Vec<CodexPidEntry> = Vec::new();

    for line in all_panes_output.lines() {
        let parts = split_tmux_fields(line, '|');
        if parts.len() < session_line_field::MIN_FIELDS {
            continue;
        }

        let session_name = parts[session_line_field::SESSION_NAME].as_str();
        let window_id = parts[session_line_field::WINDOW_ID].as_str();
        // Pass the unescaped pane fields directly instead of re-joining
        // with `|` and re-splitting, which would turn any literal pipe
        // inside a pane field (cwd, prompt, branch) back into a field
        // separator and shift every downstream index.
        let pane_fields = &parts[session_line_field::PANE_LINE_OFFSET..];

        let sessions_entry = sessions_map.entry(session_name.to_string()).or_default();

        let window = sessions_entry
            .entry(window_id.to_string())
            .or_insert_with(|| WindowInfo {
                window_id: window_id.to_string(),
                window_name: parts[session_line_field::WINDOW_NAME].to_string(),
                window_active: parts[session_line_field::WINDOW_ACTIVE] == "1",
                auto_rename: parts[session_line_field::AUTOMATIC_RENAME] == "1",
                panes: Vec::new(),
            });

        if let Some(pane) = parse_pane_fields(pane_fields) {
            if pane.agent == AgentType::Codex
                && let Some(pid) = pane.pane_pid
            {
                codex_pids.push((window_id.to_string(), window.panes.len(), pid));
            }
            window.panes.push(pane);
        }
    }

    (sessions_map, codex_pids)
}

/// Single `ps` pass that fans out Codex permission mode updates to every
/// Codex pane across every window. No-op if `ps` fails.
fn resolve_codex_permission_modes(sessions_map: &mut SessionMap, codex_pids: &[CodexPidEntry]) {
    let output = match Command::new("ps")
        .args(["-eo", "pid=,ppid=,comm=,args="])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let ps_out = String::from_utf8_lossy(&output.stdout);
    let (children_of, info_by_pid) = parse_ps_processes(&ps_out);

    for windows in sessions_map.values_mut() {
        for (window_id, window) in windows.iter_mut() {
            let window_pids: Vec<(usize, u32)> = codex_pids
                .iter()
                .filter(|(wid, _, _)| wid == window_id)
                .map(|(_, idx, pid)| (*idx, *pid))
                .collect();
            if window_pids.is_empty() {
                continue;
            }
            apply_codex_permission_modes(
                &mut window.panes,
                &window_pids,
                &children_of,
                &info_by_pid,
            );
        }
    }
}

/// Flatten the session→window hierarchy into a `Vec<SessionInfo>`, dropping
/// any windows whose `parse_pane_line` filtering left them empty, and any
/// sessions whose windows are all empty as a result.
fn finalize_sessions(sessions_map: SessionMap) -> Vec<SessionInfo> {
    let mut sessions = Vec::new();
    for (session_name, windows) in sessions_map {
        let windows: Vec<WindowInfo> = windows
            .into_values()
            .filter(|w| !w.panes.is_empty())
            .collect();
        if !windows.is_empty() {
            sessions.push(SessionInfo {
                session_name,
                windows,
            });
        }
    }
    sessions
}

/// Parse a single pane line from `tmux list-panes -F`.
/// Returns None if the line has fewer than 20 fields, is a sidebar, or has no agent.
/// Thin wrapper used by the unit tests, which still construct a raw
/// `|`-joined fixture line. Production callers go through
/// `parse_pane_fields` directly to avoid re-joining and re-splitting
/// fields that may themselves contain literal `|` characters (cwd,
/// prompt, branch) — see `build_session_hierarchy`.
#[cfg(test)]
pub(crate) fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let parts = split_tmux_fields(line, '|');
    parse_pane_fields(&parts)
}

pub(crate) fn parse_pane_fields(parts: &[String]) -> Option<PaneInfo> {
    if parts.len() < pane_line_field::MIN_FIELDS {
        return None;
    }

    if parts[pane_line_field::PANE_ROLE] == "sidebar" {
        return None;
    }

    let agent = AgentType::from_label(&parts[pane_line_field::AGENT])?;
    let current_command = parts[pane_line_field::PANE_CURRENT_COMMAND].as_str();

    // Codex / OpenCode panes can leave stale tmux metadata behind after the
    // agent exits and the pane falls back to the user's shell. Neither
    // agent exposes a reliable "process exit" hook (Codex has no such
    // hook, OpenCode runs under Bun where `process.on("exit")` does not
    // fire our handlers), so the Rust polling side must own teardown:
    // wipe pane options + activity log the first poll after the agent
    // is gone. Subsequent polls short-circuit at the `AgentType::from_label`
    // check above once `@pane_agent` has been cleared. Claude is excluded
    // because its SessionEnd hook drives cleanup instead.
    if matches!(agent, AgentType::Codex | AgentType::OpenCode) && is_shell_command(current_command)
    {
        clear_agent_pane_state(&parts[pane_line_field::PANE_ID]);
        return None;
    }

    let pane_pid: Option<u32> = parts[pane_line_field::PANE_PID].parse().ok();

    // Prefer @pane_cwd (set by hook from agent's cwd) over pane_current_path
    let pane_cwd = &parts[pane_line_field::PANE_CWD];
    let path = if !pane_cwd.is_empty() {
        pane_cwd.to_string()
    } else {
        parts[pane_line_field::PANE_CURRENT_PATH].to_string()
    };

    // Claude: read permission_mode from hook-set tmux variable.
    // Codex / OpenCode: no permission_mode in hooks, keep the default.
    let permission_mode = if agent == AgentType::Claude {
        PermissionMode::from_label(&parts[pane_line_field::PERMISSION_MODE])
    } else {
        PermissionMode::Default
    };

    let prompt_source = &parts[pane_line_field::PROMPT_SOURCE];
    let prompt_is_response = prompt_source == "response";

    // Sanitize prompt: replace pipes/newlines, filter system-injected messages, truncate
    let prompt = sanitize_prompt(&parts[pane_line_field::PROMPT]);

    let session_id = if parts[pane_line_field::SESSION_ID].is_empty() {
        None
    } else {
        Some(parts[pane_line_field::SESSION_ID].to_string())
    };

    Some(PaneInfo {
        pane_active: parts[pane_line_field::PANE_ACTIVE] == "1",
        status: PaneStatus::from_label(&parts[pane_line_field::PANE_STATUS]),
        attention: !parts[pane_line_field::PANE_ATTENTION].is_empty(),
        agent,
        path,
        current_command: parts[pane_line_field::PANE_CURRENT_COMMAND].to_string(),
        pane_id: parts[pane_line_field::PANE_ID].to_string(),
        prompt,
        prompt_is_response,
        started_at: parts[pane_line_field::STARTED_AT].parse().ok(),
        wait_reason: parts[pane_line_field::WAIT_REASON].to_string(),
        permission_mode,
        subagents: parse_subagents(&parts[pane_line_field::SUBAGENTS]),
        pane_pid,
        worktree: WorktreeMetadata {
            name: parts[pane_line_field::WORKTREE_NAME].to_string(),
            branch: parts[pane_line_field::WORKTREE_BRANCH].to_string(),
        },
        session_id,
        session_name: String::new(),
        sidebar_spawned: parts[pane_line_field::SIDEBAR_SPAWNED] == "1",
    })
}

/// Wipe all agent-tracked tmux pane options and the activity log file for
/// `pane_id`. Triggered by `parse_pane_fields` when it detects a Codex or
/// OpenCode pane that has dropped back to the user's shell, since neither
/// CLI fires a reliable process-exit hook. Claude panes are never routed
/// here because Claude has its own SessionEnd hook. The set of keys
/// mirrors `clear_all_meta` + `clear_run_state` + status/attention clears
/// in `src/cli/hook/context.rs`; keep them in sync when a new `@pane_*`
/// key is added.
fn clear_agent_pane_state(pane_id: &str) {
    const KEYS: &[&str] = &[
        "@pane_agent",
        "@pane_prompt",
        "@pane_prompt_source",
        "@pane_subagents",
        "@pane_cwd",
        "@pane_permission_mode",
        "@pane_worktree_name",
        "@pane_worktree_branch",
        "@pane_session_id",
        "@pane_pending_session_end",
        "@pane_pending_worktree_remove",
        "@pane_started_at",
        "@pane_wait_reason",
        "@pane_attention",
        "@pane_status",
    ];
    for key in KEYS {
        unset_pane_option(pane_id, key);
    }
    let log_path = crate::activity::log_file_path(pane_id);
    let _ = std::fs::remove_file(log_path);
}

fn is_shell_command(command: &str) -> bool {
    const SHELL_COMMANDS: &[&str] = &[
        "ash",
        "bash",
        "csh",
        "dash",
        "elvish",
        "fish",
        "ksh",
        "mksh",
        "nu",
        "oksh",
        "pdksh",
        "posh",
        "powershell",
        "powershell.exe",
        "pwsh",
        "sh",
        "tcsh",
        "xonsh",
        "zsh",
    ];

    let Some(token) = command.split_whitespace().next() else {
        return false;
    };
    let executable = Path::new(token)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(token)
        .to_ascii_lowercase();

    SHELL_COMMANDS.contains(&executable.as_str())
}

/// Detect Codex permission mode from process args (--full-auto, --yolo, etc.)
fn detect_codex_permission_mode(args: &str) -> PermissionMode {
    if args.contains("dangerously-bypass-approvals-and-sandbox") || args.contains("--yolo") {
        return PermissionMode::BypassPermissions;
    }
    if args.contains("--full-auto") {
        return PermissionMode::Auto;
    }
    PermissionMode::Default
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    comm: String,
    args: String,
}

fn parse_ps_processes(
    ps_out: &str,
) -> (
    std::collections::HashMap<u32, Vec<u32>>,
    std::collections::HashMap<u32, ProcessInfo>,
) {
    let mut children_of: std::collections::HashMap<u32, Vec<u32>> =
        std::collections::HashMap::new();
    let mut info_by_pid: std::collections::HashMap<u32, ProcessInfo> =
        std::collections::HashMap::new();

    for line in ps_out.lines() {
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

fn descendant_pids(
    seed_pids: &[u32],
    children_of: &std::collections::HashMap<u32, Vec<u32>>,
) -> std::collections::HashSet<u32> {
    let mut seen = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<u32> = seed_pids.iter().copied().collect();

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

fn apply_codex_permission_modes(
    panes: &mut [PaneInfo],
    pids_to_check: &[(usize, u32)],
    children_of: &std::collections::HashMap<u32, Vec<u32>>,
    info_by_pid: &std::collections::HashMap<u32, ProcessInfo>,
) {
    for (idx, pid) in pids_to_check {
        let descendants = descendant_pids(&[*pid], children_of);
        for descendant in descendants {
            let Some(info) = info_by_pid.get(&descendant) else {
                continue;
            };
            if info.comm != CODEX_AGENT {
                continue;
            }
            if let Some(pane) = panes.get_mut(*idx) {
                pane.permission_mode = detect_codex_permission_mode(&info.args);
                if pane.permission_mode != PermissionMode::Default {
                    break;
                }
            }
        }
    }
}

/// Sanitize prompt text from tmux variable so it's safe for display.
fn sanitize_prompt(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    // Filter known system-injected messages. Avoid the old broad angle-bracket
    // check so legitimate prompts containing comparisons or code snippets
    // still render.
    if raw.contains("<task-notification>")
        || raw.contains("<system-reminder>")
        || raw.contains("<task-status>")
    {
        return String::new();
    }
    if raw.chars().count() > 200 {
        raw.chars().take(200).collect()
    } else {
        raw.to_string()
    }
}

/// Parse subagent list from tmux variable.
/// Format: comma-separated "type" entries, e.g. "Explore,Explore,Plan"
/// Parse the comma-separated `@pane_subagents` value into display strings.
///
/// Each entry is either `agent_type` (legacy) or `agent_type:agent_id`
/// (current). When an `agent_id` is present, the entry is rendered as
/// `"agent_type #<id-prefix>"` where `<id-prefix>` is the first 4 characters
/// of the id — stable per instance, so the UI label does not shift when
/// sibling subagents stop. The `#` embedding is recognized by the `#`-based
/// numbering branch in `subagent_rows`, which keeps it verbatim.
fn parse_subagents(raw: &str) -> Vec<String> {
    const ID_PREFIX_LEN: usize = 4;
    if raw.is_empty() {
        return vec![];
    }
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|entry| match entry.split_once(':') {
            Some((ty, id)) if !id.is_empty() => {
                let prefix: String = id.chars().take(ID_PREFIX_LEN).collect();
                format!("{} #{}", ty, prefix)
            }
            _ => entry.to_string(),
        })
        .collect()
}

/// Split a tmux format line while honoring tmux `#{q:...}` backslash escapes.
fn split_tmux_fields(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == delimiter {
            fields.push(current);
            current = String::new();
            continue;
        }

        current.push(ch);
    }

    if escaped {
        current.push('\\');
    }

    fields.push(current);
    fields
}

#[cfg(test)]
mod tests {
    use super::super::options::test_mock;
    use super::*;

    #[test]
    fn detect_codex_permission_mode_variants() {
        assert_eq!(
            detect_codex_permission_mode("codex"),
            PermissionMode::Default
        );
        assert_eq!(
            detect_codex_permission_mode("codex --full-auto"),
            PermissionMode::Auto
        );
        assert_eq!(
            detect_codex_permission_mode("codex --dangerously-bypass-approvals-and-sandbox"),
            PermissionMode::BypassPermissions
        );
        assert_eq!(
            detect_codex_permission_mode("codex --full-auto --yolo"),
            PermissionMode::BypassPermissions
        );
    }

    fn test_pane_codex(id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: id.into(),
            pane_active: false,
            status: PaneStatus::Idle,
            attention: false,
            agent: AgentType::Codex,
            path: "/tmp".into(),
            current_command: String::new(),
            prompt: String::new(),
            prompt_is_response: false,
            started_at: None,
            wait_reason: String::new(),
            permission_mode: PermissionMode::Default,
            subagents: vec![],
            pane_pid: None,
            worktree: WorktreeMetadata::default(),
            session_id: None,
            session_name: String::new(),
            sidebar_spawned: false,
        }
    }

    #[test]
    fn apply_codex_permission_modes_from_ps() {
        let mut panes = vec![test_pane_codex("%1")];
        let pids = vec![(0, 101)];
        let ps_out = "101 1 bash /bin/bash\n102 101 codex /bin/codex --full-auto\n";
        let (children_of, info_by_pid) = parse_ps_processes(ps_out);

        apply_codex_permission_modes(&mut panes, &pids, &children_of, &info_by_pid);
        assert_eq!(panes[0].permission_mode, PermissionMode::Auto);
    }

    #[test]
    fn apply_codex_permission_modes_follows_shell_wrappers() {
        let mut panes = vec![test_pane_codex("%1")];
        let pids = vec![(0, 101)];
        let ps_out = "101 1 bash /bin/bash\n102 101 sh -c wrapper\n103 102 codex /usr/local/bin/codex --yolo\n";
        let (children_of, info_by_pid) = parse_ps_processes(ps_out);

        apply_codex_permission_modes(&mut panes, &pids, &children_of, &info_by_pid);
        assert_eq!(panes[0].permission_mode, PermissionMode::BypassPermissions);
    }

    #[test]
    fn parse_ps_processes_preserves_spaced_args() {
        let (children_of, info_by_pid) = parse_ps_processes(
            "100 1 codex /Applications/Codex App/bin/codex --full-auto\n101 100 sh sh -c wrapper\n",
        );

        assert_eq!(children_of.get(&1).cloned(), Some(vec![100]));
        let info = info_by_pid.get(&100).expect("process info");
        assert_eq!(info.comm, "codex");
        assert_eq!(info.args, "/Applications/Codex App/bin/codex --full-auto");
    }

    // ─── sanitize_prompt tests ──────────────────────────────────────

    #[test]
    fn sanitize_prompt_filters_system_injected() {
        assert_eq!(
            sanitize_prompt("<system-reminder>noise</system-reminder>"),
            ""
        );
        assert_eq!(
            sanitize_prompt("hello <task-notification>abc</task-notification> world"),
            ""
        );
    }

    #[test]
    fn sanitize_prompt_passes_normal_text() {
        assert_eq!(sanitize_prompt("fix the bug"), "fix the bug");
    }

    #[test]
    fn sanitize_prompt_keeps_legitimate_angle_brackets() {
        assert_eq!(sanitize_prompt("1 < 2 and 3 > 1"), "1 < 2 and 3 > 1");
    }

    #[test]
    fn sanitize_prompt_truncates_long_text() {
        let long = "a".repeat(300);
        let result = sanitize_prompt(&long);
        assert_eq!(result.chars().count(), 200);
    }

    #[test]
    fn sanitize_prompt_empty() {
        assert_eq!(sanitize_prompt(""), "");
    }

    // ─── parse_subagents tests ──────────────────────────────────────

    #[test]
    fn parse_subagents_empty() {
        assert_eq!(parse_subagents(""), Vec::<String>::new());
    }

    #[test]
    fn parse_subagents_single() {
        assert_eq!(parse_subagents("Explore"), vec!["Explore"]);
    }

    #[test]
    fn parse_subagents_multiple() {
        assert_eq!(
            parse_subagents("Explore,Plan,Bash"),
            vec!["Explore", "Plan", "Bash"]
        );
    }

    #[test]
    fn parse_subagents_duplicates() {
        assert_eq!(
            parse_subagents("Explore,Explore,Plan"),
            vec!["Explore", "Explore", "Plan"]
        );
    }

    #[test]
    fn parse_subagents_renders_id_prefix() {
        // Current format: `type:id`. The id prefix is used as a stable
        // `#<prefix>` label so surviving siblings do not renumber when
        // another subagent stops.
        assert_eq!(
            parse_subagents("Explore:sub123456,Plan:abc987654"),
            vec!["Explore #sub1", "Plan #abc9"]
        );
    }

    #[test]
    fn parse_subagents_id_prefix_distinguishes_parallel_same_type() {
        // Two subagents of the same type get distinct labels from their ids,
        // which is the whole point of id-based tagging.
        assert_eq!(
            parse_subagents("Explore:aaaa1111,Explore:bbbb2222"),
            vec!["Explore #aaaa", "Explore #bbbb"]
        );
    }

    #[test]
    fn parse_subagents_id_shorter_than_prefix_len_uses_full_id() {
        // Short ids (e.g. test fixtures like "s1") render in full rather
        // than being padded or truncated to nothing.
        assert_eq!(parse_subagents("Plan:s1"), vec!["Plan #s1"]);
    }

    #[test]
    fn parse_subagents_legacy_without_id_renders_type_only() {
        // Stale entry written before id tracking (or by an older build)
        // falls back to the bare type name.
        assert_eq!(
            parse_subagents("Explore,Plan:sub-999"),
            vec!["Explore", "Plan #sub-"]
        );
    }

    // ─── parse_pane_line tests ──────────────────────────────────────

    fn make_pane_line(fields: &[&str]) -> String {
        fields.join("|")
    }

    fn full_fields() -> Vec<&'static str> {
        vec![
            "1",                  // 0: pane_active
            "running",            // 1: @pane_status
            "",                   // 2: @pane_attention
            "claude",             // 3: @pane_agent
            "my-agent",           // 4: @pane_name
            "/home/user/project", // 5: pane_current_path
            "fish",               // 6: pane_current_command
            "",                   // 7: @pane_role
            "%1",                 // 8: pane_id
            "fix the bug",        // 9: @pane_prompt
            "user",               // 10: @pane_prompt_source
            "1700000000",         // 11: @pane_started_at
            "",                   // 12: @pane_wait_reason
            "12345",              // 13: pane_pid
            "Explore,Plan",       // 14: @pane_subagents
            "/custom/cwd",        // 15: @pane_cwd
            "auto",               // 16: @pane_permission_mode
            "",                   // 17: @pane_worktree_name
            "",                   // 18: @pane_worktree_branch
            "",                   // 19: @pane_session_id
            "",                   // 20: @agent-sidebar-spawned
        ]
    }

    #[test]
    fn parse_pane_line_full_fields() {
        let line = make_pane_line(&full_fields());
        let pane = parse_pane_line(&line).expect("should parse 17 fields");
        assert!(pane.pane_active);
        assert_eq!(pane.status, PaneStatus::Running);
        assert_eq!(pane.agent, AgentType::Claude);
        assert_eq!(pane.path, "/custom/cwd"); // pane_cwd preferred
        assert_eq!(pane.current_command, "fish");
        assert_eq!(pane.pane_id, "%1");
        assert_eq!(pane.prompt, "fix the bug");
        assert!(!pane.prompt_is_response);
        assert_eq!(pane.started_at, Some(1700000000));
        assert_eq!(pane.pane_pid, Some(12345));
        assert_eq!(pane.subagents, vec!["Explore", "Plan"]);
        assert_eq!(pane.permission_mode, PermissionMode::Auto);
    }

    #[test]
    fn parse_pane_line_sidebar_spawned_field() {
        let mut fields = full_fields();
        fields[20] = "1";
        let pane = parse_pane_line(&make_pane_line(&fields)).unwrap();
        assert!(pane.sidebar_spawned);

        fields[20] = "";
        let pane = parse_pane_line(&make_pane_line(&fields)).unwrap();
        assert!(!pane.sidebar_spawned);

        fields[20] = "0";
        let pane = parse_pane_line(&make_pane_line(&fields)).unwrap();
        assert!(
            !pane.sidebar_spawned,
            "any value other than `1` is treated as false"
        );
    }

    #[test]
    fn parse_pane_line_response_prompt_source() {
        let mut fields = full_fields();
        fields[10] = "response"; // @pane_prompt_source
        let line = make_pane_line(&fields);
        let pane = parse_pane_line(&line).unwrap();
        assert!(pane.prompt_is_response);
    }

    #[test]
    fn parse_pane_line_rejects_fewer_than_21_fields() {
        // Only 15 fields — should be rejected
        let fields_15 =
            "1|running||claude|name|/path|fish||%1|prompt|1700000000||12345|Explore|/cwd";
        assert!(
            parse_pane_line(fields_15).is_none(),
            "15 fields should be rejected"
        );

        // 20 fields — still rejected (need 21)
        let fields_20 = "1|running||claude|name|/path|fish||%1|prompt|user|1700000000||12345|Explore|/cwd|auto|||";
        assert!(
            parse_pane_line(fields_20).is_none(),
            "20 fields should be rejected"
        );
    }

    #[test]
    fn parse_pane_line_rejects_sidebar_role() {
        let mut fields = full_fields();
        fields[7] = "sidebar";
        let line = make_pane_line(&fields);
        assert!(
            parse_pane_line(&line).is_none(),
            "sidebar role should be filtered out"
        );
    }

    #[test]
    fn parse_pane_line_rejects_unknown_agent() {
        let mut fields = full_fields();
        fields[3] = ""; // no agent type
        let line = make_pane_line(&fields);
        assert!(
            parse_pane_line(&line).is_none(),
            "empty agent should be rejected"
        );
    }

    #[test]
    fn parse_pane_line_falls_back_to_pane_current_path() {
        let mut fields = full_fields();
        fields[15] = ""; // empty pane_cwd
        let line = make_pane_line(&fields);
        let pane = parse_pane_line(&line).unwrap();
        assert_eq!(
            pane.path, "/home/user/project",
            "should fall back to pane_current_path when pane_cwd is empty"
        );
    }

    #[test]
    fn parse_pane_line_preserves_pipe_in_path() {
        let mut fields = full_fields();
        fields[5] = "/home/user/a\\|b";
        fields[15] = "";
        let line = make_pane_line(&fields);
        let pane = parse_pane_line(&line).unwrap();
        assert_eq!(pane.path, "/home/user/a|b");
    }

    #[test]
    fn split_tmux_fields_unescapes_delimiter() {
        let fields = split_tmux_fields("one|two\\|still-two|three", '|');
        assert_eq!(fields, vec!["one", "two|still-two", "three"]);
    }

    #[test]
    fn parse_pane_line_codex_ignores_permission_mode_field() {
        let mut fields = full_fields();
        fields[3] = "codex";
        fields[6] = "node";
        fields[16] = "auto"; // should be ignored for codex
        let line = make_pane_line(&fields);
        let pane = parse_pane_line(&line).unwrap();
        assert_eq!(
            pane.permission_mode,
            PermissionMode::Default,
            "codex should not read permission_mode from tmux variable"
        );
    }

    #[test]
    fn parse_pane_line_rejects_stale_codex_shell_pane() {
        let mut fields = full_fields();
        fields[3] = "codex";
        fields[6] = "zsh";
        let line = make_pane_line(&fields);
        assert!(
            parse_pane_line(&line).is_none(),
            "codex metadata on a shell pane should be treated as stale"
        );
    }

    #[test]
    fn parse_pane_line_rejects_stale_codex_shell_pane_with_path_and_args() {
        let mut fields = full_fields();
        fields[3] = "codex";
        fields[6] = "/usr/local/bin/PwSh -l";
        let line = make_pane_line(&fields);
        assert!(
            parse_pane_line(&line).is_none(),
            "shell detection should handle paths, args, and case differences"
        );
    }

    #[test]
    fn parse_pane_line_does_not_wipe_claude_on_shell_command() {
        // Claude owns its own SessionEnd hook, so Rust must NOT sweep its
        // pane state even when `current_command` is a shell — otherwise
        // we race with the hook and lose legitimate prompt/status.
        let _guard = test_mock::install();
        let pane = "%CLAUDE_SHELL";
        test_mock::set(pane, "@pane_agent", "claude");
        test_mock::set(pane, "@pane_prompt", "keep me");
        test_mock::set(pane, "@pane_status", "running");

        let mut fields = full_fields();
        fields[pane_line_field::PANE_ID] = pane;
        fields[pane_line_field::AGENT] = "claude";
        fields[pane_line_field::PANE_CURRENT_COMMAND] = "bash";
        let _ = parse_pane_line(&make_pane_line(&fields));

        assert!(test_mock::contains(pane, "@pane_agent"));
        assert!(test_mock::contains(pane, "@pane_prompt"));
        assert_eq!(
            test_mock::get(pane, "@pane_prompt").as_deref(),
            Some("keep me"),
            "claude prompt must survive — SessionEnd hook is the clear path",
        );
    }

    #[test]
    fn parse_pane_line_wipes_stale_state_for_codex_shell_pane() {
        // Codex shares the same shell-fallback sweep path as OpenCode —
        // neither fires a reliable process-exit hook, so the Rust poller
        // must clear @pane_* keys and the activity log when the pane
        // reverts to a shell. Mirrors the OpenCode regression test below.
        let _guard = test_mock::install();
        let pane = "%CODEX_STALE";
        test_mock::set(pane, "@pane_agent", "codex");
        test_mock::set(pane, "@pane_prompt", "previous codex prompt");
        test_mock::set(pane, "@pane_prompt_source", "user");
        test_mock::set(pane, "@pane_status", "waiting");
        test_mock::set(pane, "@pane_started_at", "1700000000");
        test_mock::set(pane, "@pane_cwd", "/repo/codex");
        test_mock::set(pane, "@pane_wait_reason", "permission");
        let log = crate::activity::log_file_path(pane);
        let _ = std::fs::create_dir_all(log.parent().unwrap());
        std::fs::write(&log, "1234|Bash|pytest\n").unwrap();

        let mut fields = full_fields();
        fields[pane_line_field::PANE_ID] = pane;
        fields[pane_line_field::AGENT] = "codex";
        fields[pane_line_field::PANE_CURRENT_COMMAND] = "zsh";
        let line = make_pane_line(&fields);

        assert!(parse_pane_line(&line).is_none());
        for key in &[
            "@pane_agent",
            "@pane_prompt",
            "@pane_prompt_source",
            "@pane_status",
            "@pane_started_at",
            "@pane_cwd",
            "@pane_wait_reason",
        ] {
            assert!(
                !test_mock::contains(pane, key),
                "{key} must be cleared when a codex pane falls back to shell"
            );
        }
        assert!(
            !log.exists(),
            "codex activity log must be removed once the agent process is gone"
        );
    }

    #[test]
    fn parse_pane_line_wipes_stale_state_for_opencode_shell_pane() {
        // When an OpenCode pane falls back to the user's shell, the Rust
        // polling side owns teardown because OpenCode has no reliable
        // process-exit hook. The detector must unset every @pane_* key it
        // seeded and remove the activity log so the next launch starts
        // from a clean slate without flashing stale prompt/status.
        let _guard = test_mock::install();
        let pane = "%OPENCODE_STALE";
        test_mock::set(pane, "@pane_agent", "opencode");
        test_mock::set(pane, "@pane_prompt", "previous run");
        test_mock::set(pane, "@pane_prompt_source", "user");
        test_mock::set(pane, "@pane_status", "running");
        test_mock::set(pane, "@pane_started_at", "1700000000");
        test_mock::set(pane, "@pane_cwd", "/repo");
        test_mock::set(pane, "@pane_session_id", "ses-1");
        let log = crate::activity::log_file_path(pane);
        let _ = std::fs::create_dir_all(log.parent().unwrap());
        std::fs::write(&log, "1234|Bash|ls\n").unwrap();

        let mut fields = full_fields();
        fields[pane_line_field::PANE_ID] = pane;
        fields[pane_line_field::AGENT] = "opencode";
        fields[pane_line_field::PANE_CURRENT_COMMAND] = "fish";
        let line = make_pane_line(&fields);

        assert!(parse_pane_line(&line).is_none());
        for key in &[
            "@pane_agent",
            "@pane_prompt",
            "@pane_prompt_source",
            "@pane_status",
            "@pane_started_at",
            "@pane_cwd",
            "@pane_session_id",
        ] {
            assert!(
                !test_mock::contains(pane, key),
                "{key} must be cleared after shell fallback sweep"
            );
        }
        assert!(
            !log.exists(),
            "activity log must be removed when the agent process is gone"
        );
    }

    // ─── finalize_sessions ─────────────────────────────────────────

    #[test]
    fn finalize_sessions_drops_windows_with_no_panes() {
        // Regression: build_session_hierarchy() creates a WindowInfo as
        // soon as it sees a tmux row, but parse_pane_line() may then
        // reject every pane in that window (sidebar / shell / unknown).
        // finalize_sessions must filter out the resulting empty windows
        // so downstream code never has to special-case them.
        let mut sessions_map: SessionMap = indexmap::IndexMap::new();
        let entry = sessions_map.entry("main".to_string()).or_default();
        entry.insert(
            "@1".to_string(),
            WindowInfo {
                window_id: "@1".into(),
                window_name: "with-pane".into(),
                window_active: true,
                auto_rename: false,
                panes: vec![PaneInfo {
                    pane_id: "%1".into(),
                    pane_active: true,
                    status: PaneStatus::Running,
                    attention: false,
                    agent: AgentType::Claude,
                    path: "/repo".into(),
                    current_command: String::new(),
                    prompt: String::new(),
                    prompt_is_response: false,
                    started_at: None,
                    wait_reason: String::new(),
                    permission_mode: PermissionMode::Default,
                    subagents: vec![],
                    pane_pid: None,
                    worktree: WorktreeMetadata::default(),
                    session_id: None,
                    session_name: String::new(),
                    sidebar_spawned: false,
                }],
            },
        );
        entry.insert(
            "@2".to_string(),
            WindowInfo {
                window_id: "@2".into(),
                window_name: "empty".into(),
                window_active: false,
                auto_rename: false,
                panes: vec![],
            },
        );

        let sessions = finalize_sessions(sessions_map);

        assert_eq!(sessions.len(), 1, "session should survive");
        assert_eq!(
            sessions[0].windows.len(),
            1,
            "empty window must be filtered out"
        );
        assert_eq!(sessions[0].windows[0].window_id, "@1");
    }

    #[test]
    fn finalize_sessions_drops_session_when_all_windows_are_empty() {
        let mut sessions_map: SessionMap = indexmap::IndexMap::new();
        let entry = sessions_map.entry("dead".to_string()).or_default();
        entry.insert(
            "@9".to_string(),
            WindowInfo {
                window_id: "@9".into(),
                window_name: "ghost".into(),
                window_active: false,
                auto_rename: false,
                panes: vec![],
            },
        );

        let sessions = finalize_sessions(sessions_map);

        assert!(sessions.is_empty(), "session with no panes must be dropped");
    }
}
