use std::path::Path;
use std::process::Command;

pub const CLAUDE_AGENT: &str = "claude";
pub const CODEX_AGENT: &str = "codex";

#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub pane_id: String,
    pub pane_active: bool,
    pub status: PaneStatus,
    pub attention: bool,
    pub agent: AgentType,
    pub path: String,
    pub current_command: String,
    pub prompt: String,
    pub prompt_is_response: bool,
    pub started_at: Option<u64>,
    pub wait_reason: String,
    pub permission_mode: PermissionMode,
    pub subagents: Vec<String>,
    pub pane_pid: Option<u32>,
    pub worktree_name: String,
    pub worktree_branch: String,
    pub session_id: Option<String>,
    pub session_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaneStatus {
    Running,
    Waiting,
    Idle,
    Error,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionMode {
    Default,
    Plan,
    AcceptEdits,
    Auto,
    DontAsk,
    BypassPermissions,
}

impl PermissionMode {
    /// Parse the permission-mode label written by agent hooks. Unknown
    /// values fall back to `Default`.
    pub fn from_label(s: &str) -> Self {
        match s {
            "plan" => Self::Plan,
            "acceptEdits" => Self::AcceptEdits,
            "auto" => Self::Auto,
            "dontAsk" => Self::DontAsk,
            "bypassPermissions" => Self::BypassPermissions,
            _ => Self::Default,
        }
    }

    pub fn badge(&self) -> &str {
        match self {
            Self::Default => "",
            Self::Plan => "plan",
            Self::AcceptEdits => "edit",
            Self::Auto => "auto",
            Self::DontAsk => "dontAsk",
            Self::BypassPermissions => "!",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    Claude,
    Codex,
    #[allow(dead_code)]
    Unknown,
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub window_id: String,
    pub window_name: String,
    pub window_active: bool,
    pub auto_rename: bool,
    pub panes: Vec<PaneInfo>,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_name: String,
    pub windows: Vec<WindowInfo>,
}

impl AgentType {
    /// Parse the agent label set by hooks. Returns `None` for unknown
    /// values so callers can skip non-agent panes.
    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            CLAUDE_AGENT => Some(Self::Claude),
            CODEX_AGENT => Some(Self::Codex),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Claude => CLAUDE_AGENT,
            Self::Codex => CODEX_AGENT,
            Self::Unknown => "unknown",
        }
    }

    pub fn label(&self) -> &str {
        self.as_str()
    }
}

impl PaneStatus {
    /// Parse the status label written by agent hooks. Unknown values
    /// map to `Unknown`.
    pub fn from_label(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "waiting" | "notification" => Self::Waiting,
            "idle" => Self::Idle,
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Running => "●",
            Self::Waiting => "◐",
            Self::Idle => "○",
            Self::Error => "✕",
            Self::Unknown => "·",
        }
    }
}

pub fn run_tmux(args: &[&str]) -> Option<String> {
    let output = Command::new("tmux").args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

/// tmux `list-panes -F` format used by [`query_sessions`]. Every field is
/// quoted with `#{q:...}` so embedded pipes in user content survive the split.
const PANE_FORMAT: &str = "#{q:session_name}|#{q:window_id}|#{q:window_index}|#{q:window_name}|#{q:window_active}|#{q:automatic-rename}|#{q:pane_active}|#{q:@pane_status}|#{q:@pane_attention}|#{q:@pane_agent}|#{q:@pane_name}|#{q:pane_current_path}|#{q:pane_current_command}|#{q:@pane_role}|#{q:pane_id}|#{q:@pane_prompt}|#{q:@pane_prompt_source}|#{q:@pane_started_at}|#{q:@pane_wait_reason}|#{q:pane_pid}|#{q:@pane_subagents}|#{q:@pane_cwd}|#{q:@pane_permission_mode}|#{q:@pane_worktree_name}|#{q:@pane_worktree_branch}|#{q:@pane_session_id}";

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
        if parts.len() < 26 {
            continue;
        }

        let session_name = parts[0].as_str();
        let window_id = parts[1].as_str();
        let pane_line = parts[6..].join("|");

        let sessions_entry = sessions_map.entry(session_name.to_string()).or_default();

        let window = sessions_entry
            .entry(window_id.to_string())
            .or_insert_with(|| WindowInfo {
                window_id: window_id.to_string(),
                window_name: parts[3].to_string(),
                window_active: parts[4] == "1",
                auto_rename: parts[5] == "1",
                panes: Vec::new(),
            });

        if let Some(pane) = parse_pane_line(&pane_line) {
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
pub(crate) fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let parts = split_tmux_fields(line, '|');
    if parts.len() < 20 {
        return None;
    }

    if parts[7] == "sidebar" {
        return None;
    }

    let agent = AgentType::from_label(&parts[3])?;
    let current_command = parts[6].as_str();

    // Codex panes can leave stale tmux metadata behind after the agent exits
    // and the pane falls back to the user's shell. In that case, ignore the
    // pane so the sidebar stops displaying a non-existent Codex session.
    if agent == AgentType::Codex && is_shell_command(current_command) {
        return None;
    }

    let pane_pid: Option<u32> = parts[13].parse().ok();

    // Prefer @pane_cwd (set by hook from agent's cwd) over pane_current_path
    let pane_cwd = &parts[15];
    let path = if !pane_cwd.is_empty() {
        pane_cwd.to_string()
    } else {
        parts[5].to_string()
    };

    // Claude: read permission_mode from hook-set tmux variable
    // Codex: no permission_mode in hooks, detect from process args later
    let permission_mode = if agent == AgentType::Claude {
        PermissionMode::from_label(&parts[16])
    } else {
        PermissionMode::Default
    };

    let prompt_source = &parts[10];
    let prompt_is_response = prompt_source == "response";

    // Sanitize prompt: replace pipes/newlines, filter system-injected messages, truncate
    let prompt = sanitize_prompt(&parts[9]);

    let session_id = if parts[19].is_empty() {
        None
    } else {
        Some(parts[19].to_string())
    };

    Some(PaneInfo {
        pane_active: parts[0] == "1",
        status: PaneStatus::from_label(&parts[1]),
        attention: !parts[2].is_empty(),
        agent,
        path,
        current_command: parts[6].to_string(),
        pane_id: parts[8].to_string(),
        prompt,
        prompt_is_response,
        started_at: parts[11].parse().ok(),
        wait_reason: parts[12].to_string(),
        permission_mode,
        subagents: parse_subagents(&parts[14]),
        pane_pid,
        worktree_name: parts[17].to_string(),
        worktree_branch: parts[18].to_string(),
        session_id,
        session_name: String::new(),
    })
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

pub fn get_sidebar_pane_info(tmux_pane: &str) -> (bool, bool, u16, u16) {
    let output = run_tmux(&[
        "display-message",
        "-t",
        tmux_pane,
        "-p",
        "#{pane_active} #{window_active} #{pane_width} #{pane_height}",
    ]);
    match output {
        Some(s) => {
            let parts: Vec<&str> = s.trim().splitn(4, ' ').collect();
            if parts.len() >= 4 {
                (
                    parts[0] == "1",
                    parts[1] == "1",
                    parts[2].parse().unwrap_or(28),
                    parts[3].parse().unwrap_or(24),
                )
            } else {
                (false, false, 28, 24)
            }
        }
        None => (false, false, 28, 24),
    }
}

pub fn get_option(name: &str) -> Option<String> {
    run_tmux(&["show", "-gv", name])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Fetch all global tmux options in a single subprocess call.
/// Returns a map of option name → value.
pub fn get_all_global_options() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(output) = run_tmux(&["show", "-g"]) {
        for line in output.lines() {
            // Format: "option-name value" or "@user_option value"
            if let Some((key, value)) = line.split_once(' ') {
                map.insert(key.to_string(), value.trim_matches('"').to_string());
            }
        }
    }
    map
}

pub fn get_pane_path(pane_id: &str) -> Option<String> {
    run_tmux(&[
        "display-message",
        "-t",
        pane_id,
        "-p",
        "#{pane_current_path}",
    ])
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// Query tmux for all panes in the active window, returning (pane_id, pane_active, path).
/// This queries tmux directly and is NOT filtered by agent type, so it includes
/// all panes (shell, editor, etc.) — not just agent panes.
pub fn query_active_window_panes() -> Vec<(String, bool, String)> {
    // List panes in the current (active) window across all sessions
    let output = match run_tmux(&[
        "list-panes",
        "-F",
        "#{pane_id}|#{pane_active}|#{pane_current_path}",
    ]) {
        Some(s) => s,
        None => return vec![],
    };
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() < 3 {
                return None;
            }
            Some((parts[0].to_string(), parts[1] == "1", parts[2].to_string()))
        })
        .collect()
}

/// Find the focused (non-sidebar) pane ID and path by querying tmux directly.
/// Returns all panes regardless of agent type, so activity/git info can be shown
/// even for non-agent panes.
pub fn find_active_pane(sidebar_pane: &str) -> Option<(String, String)> {
    pick_active_pane(sidebar_pane, &query_active_window_panes())
}

/// Pure logic: pick the active non-sidebar pane from a list.
/// Returns the pane with pane_active=true (excluding sidebar) if one exists.
/// Returns None when the sidebar itself is active or no valid pane is found,
/// so callers can preserve the previously focused pane.
pub(crate) fn pick_active_pane(
    sidebar_pane: &str,
    panes: &[(String, bool, String)],
) -> Option<(String, String)> {
    let valid = |p: &&(String, bool, String)| p.0 != sidebar_pane && !p.2.is_empty();
    panes
        .iter()
        .find(|p| p.1 && valid(p))
        .map(|p| (p.0.clone(), p.2.clone()))
}

/// Find the focused pane's working directory by querying tmux directly.
/// Used by the background git thread which doesn't have access to AppState.
/// Queries all panes (not just agent panes) so git info is available
/// even when the focused pane has no agent running.
pub fn focused_pane_path(sidebar_pane: &str) -> Option<String> {
    find_active_pane(sidebar_pane).map(|(_, path)| path)
}

pub fn set_pane_option(pane: &str, key: &str, value: &str) {
    #[cfg(test)]
    if test_mock::intercept_set(pane, key, value) {
        return;
    }
    let _ = run_tmux(&["set", "-t", pane, "-p", key, value]);
}

pub fn unset_pane_option(pane: &str, key: &str) {
    #[cfg(test)]
    if test_mock::intercept_unset(pane, key) {
        return;
    }
    let _ = run_tmux(&["set", "-t", pane, "-p", "-u", key]);
}

pub fn get_pane_option_value(pane: &str, key: &str) -> String {
    #[cfg(test)]
    if let Some(value) = test_mock::intercept_get(pane, key) {
        return value;
    }
    run_tmux(&["show", "-t", pane, "-pv", key])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Per-thread in-memory tmux pane store used by tests. Activated by
/// installing a mock with [`test_mock::install`]; until then, all
/// `set/unset/get_pane_option*` calls fall through to the real `tmux`
/// command. The whole module is `cfg(test)` so it has zero cost in
/// release builds.
#[cfg(test)]
pub mod test_mock {
    use std::cell::RefCell;
    use std::collections::HashMap;

    type Store = HashMap<(String, String), String>;

    thread_local! {
        static MOCK: RefCell<Option<Store>> = const { RefCell::new(None) };
    }

    /// Install a fresh mock store for the current thread. Returns a guard
    /// that uninstalls the mock on drop so concurrent tests don't leak
    /// state across each other.
    pub fn install() -> MockGuard {
        MOCK.with(|m| *m.borrow_mut() = Some(Store::new()));
        MockGuard
    }

    pub struct MockGuard;

    impl Drop for MockGuard {
        fn drop(&mut self) {
            MOCK.with(|m| *m.borrow_mut() = None);
        }
    }

    /// Pre-populate a pane option in the mock store. Call after `install`.
    pub fn set(pane: &str, key: &str, value: &str) {
        MOCK.with(|m| {
            if let Some(store) = m.borrow_mut().as_mut() {
                store.insert((pane.to_string(), key.to_string()), value.to_string());
            }
        });
    }

    /// Read a pane option from the mock store. Returns `None` if no mock
    /// is installed (so production code paths still hit real tmux).
    pub fn get(pane: &str, key: &str) -> Option<String> {
        MOCK.with(|m| {
            m.borrow().as_ref().map(|store| {
                store
                    .get(&(pane.to_string(), key.to_string()))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    }

    /// Returns true if a key exists in the mock store. Useful for
    /// asserting that a teardown DID NOT remove a key.
    pub fn contains(pane: &str, key: &str) -> bool {
        MOCK.with(|m| {
            m.borrow()
                .as_ref()
                .is_some_and(|store| store.contains_key(&(pane.to_string(), key.to_string())))
        })
    }

    pub(super) fn intercept_set(pane: &str, key: &str, value: &str) -> bool {
        MOCK.with(|m| {
            if let Some(store) = m.borrow_mut().as_mut() {
                store.insert((pane.to_string(), key.to_string()), value.to_string());
                true
            } else {
                false
            }
        })
    }

    pub(super) fn intercept_unset(pane: &str, key: &str) -> bool {
        MOCK.with(|m| {
            if let Some(store) = m.borrow_mut().as_mut() {
                store.remove(&(pane.to_string(), key.to_string()));
                true
            } else {
                false
            }
        })
    }

    pub(super) fn intercept_get(pane: &str, key: &str) -> Option<String> {
        MOCK.with(|m| {
            m.borrow().as_ref().map(|store| {
                store
                    .get(&(pane.to_string(), key.to_string()))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    }
}

pub fn display_message(target: &str, format: &str) -> String {
    run_tmux(&["display-message", "-t", target, "-p", format])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn select_pane(pane_id: &str) {
    // Find the session containing this pane and switch to it first
    if let Some(session_id) = run_tmux(&["display-message", "-t", pane_id, "-p", "#{session_id}"]) {
        let session_id = session_id.trim();
        if !session_id.is_empty() {
            let _ = run_tmux(&["switch-client", "-t", session_id]);
        }
    }
    // Then switch to the correct window
    if let Some(window_id) = run_tmux(&["display-message", "-t", pane_id, "-p", "#{window_id}"]) {
        let window_id = window_id.trim();
        if !window_id.is_empty() {
            let _ = run_tmux(&["select-window", "-t", window_id]);
        }
    }
    let _ = run_tmux(&["select-pane", "-t", pane_id]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_status_from_str_all_variants() {
        assert_eq!(PaneStatus::from_label("running"), PaneStatus::Running);
        assert_eq!(PaneStatus::from_label("waiting"), PaneStatus::Waiting);
        assert_eq!(PaneStatus::from_label("notification"), PaneStatus::Waiting);
        assert_eq!(PaneStatus::from_label("idle"), PaneStatus::Idle);
        assert_eq!(PaneStatus::from_label("error"), PaneStatus::Error);
        assert_eq!(PaneStatus::from_label("anything"), PaneStatus::Unknown);
        assert_eq!(PaneStatus::from_label(""), PaneStatus::Unknown);
    }

    #[test]
    fn pane_status_icon_all_variants() {
        assert_eq!(PaneStatus::Running.icon(), "●");
        assert_eq!(PaneStatus::Waiting.icon(), "◐");
        assert_eq!(PaneStatus::Idle.icon(), "○");
        assert_eq!(PaneStatus::Error.icon(), "✕");
        assert_eq!(PaneStatus::Unknown.icon(), "·");
    }

    #[test]
    fn agent_type_from_str_all() {
        assert_eq!(AgentType::from_label("claude"), Some(AgentType::Claude));
        assert_eq!(AgentType::from_label("codex"), Some(AgentType::Codex));
        assert_eq!(AgentType::from_label("unknown"), None);
        assert_eq!(AgentType::from_label(""), None);
    }

    #[test]
    fn agent_type_label() {
        assert_eq!(AgentType::Claude.label(), "claude");
        assert_eq!(AgentType::Codex.label(), "codex");
        assert_eq!(AgentType::Unknown.label(), "unknown");
    }

    #[test]
    fn permission_mode_from_str_all() {
        assert_eq!(
            PermissionMode::from_label("default"),
            PermissionMode::Default
        );
        assert_eq!(PermissionMode::from_label("plan"), PermissionMode::Plan);
        assert_eq!(
            PermissionMode::from_label("acceptEdits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(PermissionMode::from_label("auto"), PermissionMode::Auto);
        assert_eq!(
            PermissionMode::from_label("dontAsk"),
            PermissionMode::DontAsk
        );
        assert_eq!(
            PermissionMode::from_label("bypassPermissions"),
            PermissionMode::BypassPermissions
        );
        assert_eq!(PermissionMode::from_label(""), PermissionMode::Default);
        assert_eq!(
            PermissionMode::from_label("unknown"),
            PermissionMode::Default
        );
    }

    #[test]
    fn permission_mode_badge() {
        assert_eq!(PermissionMode::Default.badge(), "");
        assert_eq!(PermissionMode::Plan.badge(), "plan");
        assert_eq!(PermissionMode::AcceptEdits.badge(), "edit");
        assert_eq!(PermissionMode::Auto.badge(), "auto");
        assert_eq!(PermissionMode::DontAsk.badge(), "dontAsk");
        assert_eq!(PermissionMode::BypassPermissions.badge(), "!");
    }

    #[test]
    fn dont_ask_is_distinct_from_auto() {
        // Upstream docs list "auto" and "dontAsk" as separate permission_mode
        // values (code.claude.com/docs/en/hooks "Common input fields"), so
        // they must not collapse to the same variant.
        let auto = PermissionMode::from_label("auto");
        let dont_ask = PermissionMode::from_label("dontAsk");
        assert_eq!(auto, PermissionMode::Auto);
        assert_eq!(dont_ask, PermissionMode::DontAsk);
        assert_ne!(auto, dont_ask);
        assert_ne!(auto.badge(), dont_ask.badge());
    }

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

    #[test]
    fn apply_codex_permission_modes_from_ps() {
        let mut panes = vec![PaneInfo {
            pane_id: "%1".into(),
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
            worktree_name: String::new(),
            worktree_branch: String::new(),
            session_id: None,
            session_name: String::new(),
        }];
        let pids = vec![(0, 101)];
        let ps_out = "101 1 bash /bin/bash\n102 101 codex /bin/codex --full-auto\n";
        let (children_of, info_by_pid) = parse_ps_processes(ps_out);

        apply_codex_permission_modes(&mut panes, &pids, &children_of, &info_by_pid);
        assert_eq!(panes[0].permission_mode, PermissionMode::Auto);
    }

    #[test]
    fn apply_codex_permission_modes_follows_shell_wrappers() {
        let mut panes = vec![PaneInfo {
            pane_id: "%1".into(),
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
            worktree_name: String::new(),
            worktree_branch: String::new(),
            session_id: None,
            session_name: String::new(),
        }];
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
    fn parse_pane_line_response_prompt_source() {
        let mut fields = full_fields();
        fields[10] = "response"; // @pane_prompt_source
        let line = make_pane_line(&fields);
        let pane = parse_pane_line(&line).unwrap();
        assert!(pane.prompt_is_response);
    }

    #[test]
    fn parse_pane_line_rejects_fewer_than_20_fields() {
        // Only 15 fields — should be rejected
        let fields_15 =
            "1|running||claude|name|/path|fish||%1|prompt|1700000000||12345|Explore|/cwd";
        assert!(
            parse_pane_line(fields_15).is_none(),
            "15 fields should be rejected"
        );

        // 19 fields — still rejected (need 20)
        let fields_19 = "1|running||claude|name|/path|fish||%1|prompt|user|1700000000||12345|Explore|/cwd|auto||";
        assert!(
            parse_pane_line(fields_19).is_none(),
            "19 fields should be rejected"
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

    // ─── pick_active_pane tests ───────────────────────────────────

    fn pane_tuple(id: &str, active: bool, path: &str) -> (String, bool, String) {
        (id.to_string(), active, path.to_string())
    }

    #[test]
    fn pick_active_pane_selects_active_non_sidebar() {
        let panes = vec![
            pane_tuple("%1", false, "/home/user/a"),
            pane_tuple("%2", true, "/home/user/b"),
            pane_tuple("%99", false, "/home/user/sidebar"),
        ];
        let result = pick_active_pane("%99", &panes);
        assert_eq!(result, Some(("%2".into(), "/home/user/b".into())));
    }

    #[test]
    fn pick_active_pane_skips_sidebar_even_if_active() {
        let panes = vec![
            pane_tuple("%1", false, "/home/user/a"),
            pane_tuple("%99", true, "/home/user/sidebar"),
        ];
        let result = pick_active_pane("%99", &panes);
        assert_eq!(
            result, None,
            "sidebar is active, no other pane is active → None (preserve previous focus)"
        );
    }

    #[test]
    fn pick_active_pane_none_when_no_pane_is_active() {
        let panes = vec![
            pane_tuple("%99", false, "/sidebar"),
            pane_tuple("%1", false, "/home/user/a"),
            pane_tuple("%2", false, "/home/user/b"),
        ];
        let result = pick_active_pane("%99", &panes);
        assert_eq!(
            result, None,
            "no pane is active → None (preserve previous focus)"
        );
    }

    #[test]
    fn pick_active_pane_none_when_only_sidebar() {
        let panes = vec![pane_tuple("%99", true, "/sidebar")];
        let result = pick_active_pane("%99", &panes);
        assert_eq!(result, None);
    }

    #[test]
    fn pick_active_pane_none_when_empty() {
        let result = pick_active_pane("%99", &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn pick_active_pane_skips_empty_path() {
        let panes = vec![
            pane_tuple("%1", true, ""),
            pane_tuple("%2", false, "/home/user/b"),
        ];
        let result = pick_active_pane("%99", &panes);
        // %1 is active but has empty path → skip, no other active pane → None
        assert_eq!(
            result, None,
            "active pane with empty path is invalid → None"
        );
    }

    #[test]
    fn pick_active_pane_sidebar_active_with_multiple_panes() {
        // Simulates: sidebar has tmux focus, pane1 and pane2 exist but neither is active.
        // This is the key scenario: when user moves to sidebar, all other panes
        // have pane_active=false (only sidebar has true, but it's excluded).
        let panes = vec![
            pane_tuple("%99", true, "/sidebar"),
            pane_tuple("%1", false, "/home/user/project-a"),
            pane_tuple("%2", false, "/home/user/project-b"),
        ];
        let result = pick_active_pane("%99", &panes);
        assert_eq!(
            result, None,
            "sidebar has focus → None so caller preserves previous focused pane"
        );
    }

    #[test]
    fn pick_active_pane_returns_pane_when_non_sidebar_is_active() {
        // When user focuses pane2 (not sidebar), pick_active_pane returns pane2.
        let panes = vec![
            pane_tuple("%99", false, "/sidebar"),
            pane_tuple("%1", false, "/home/user/project-a"),
            pane_tuple("%2", true, "/home/user/project-b"),
        ];
        let result = pick_active_pane("%99", &panes);
        assert_eq!(
            result,
            Some(("%2".into(), "/home/user/project-b".into())),
            "pane2 is active → returned"
        );
    }

    // ─── parse_pane_line tests ──────────────────────────────────────

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
                    worktree_name: String::new(),
                    worktree_branch: String::new(),
                    session_id: None,
                    session_name: String::new(),
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
