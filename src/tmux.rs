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
    pub fn from_str(s: &str) -> Self {
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
    pub fn from_str(s: &str) -> Option<Self> {
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
    pub fn from_str(s: &str) -> Self {
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

/// Query all sessions, windows, and panes in a single `tmux list-panes -a` call
/// plus one `list-sessions` call, instead of N+1 subprocess invocations.
pub fn query_sessions() -> Vec<SessionInfo> {
    // 1. Get all panes across all sessions in one call
    let pane_format = "#{q:session_name}|#{q:window_id}|#{q:window_index}|#{q:window_name}|#{q:window_active}|#{q:automatic-rename}|#{q:pane_active}|#{q:@pane_status}|#{q:@pane_attention}|#{q:@pane_agent}|#{q:@pane_name}|#{q:pane_current_path}|#{q:pane_current_command}|#{q:@pane_role}|#{q:pane_id}|#{q:@pane_prompt}|#{q:@pane_prompt_source}|#{q:@pane_started_at}|#{q:@pane_wait_reason}|#{q:pane_pid}|#{q:@pane_subagents}|#{q:@pane_cwd}|#{q:@pane_permission_mode}|#{q:@pane_worktree_name}|#{q:@pane_worktree_branch}";
    let all_panes_output = match run_tmux(&["list-panes", "-a", "-F", pane_format]) {
        Some(s) => s,
        None => return vec![],
    };

    // 2. Build the session→window→pane hierarchy
    use indexmap::IndexMap;
    let mut sessions_map: IndexMap<String, IndexMap<String, WindowInfo>> = IndexMap::new();
    // Track (window_id, pane_index_in_window, pid) for codex permission mode resolution
    let mut codex_pids: Vec<(String, usize, u32)> = Vec::new();

    for line in all_panes_output.lines() {
        let parts = split_tmux_fields(line, '|');
        if parts.len() < 25 {
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
            if pane.agent == AgentType::Codex {
                if let Some(pid) = pane.pane_pid {
                    codex_pids.push((window_id.to_string(), window.panes.len(), pid));
                }
            }
            window.panes.push(pane);
        }
    }

    // 3. Single `ps` call for all Codex panes across all windows
    if !codex_pids.is_empty() {
        if let Ok(output) = Command::new("ps").args(["-eo", "ppid,args"]).output() {
            if output.status.success() {
                let ps_out = String::from_utf8_lossy(&output.stdout);
                for (_session_name, windows) in &mut sessions_map {
                    for (window_id, window) in windows.iter_mut() {
                        let window_pids: Vec<(usize, u32)> = codex_pids
                            .iter()
                            .filter(|(wid, _, _)| wid == window_id)
                            .map(|(_, idx, pid)| (*idx, *pid))
                            .collect();
                        if !window_pids.is_empty() {
                            apply_codex_permission_modes(&mut window.panes, &window_pids, &ps_out);
                        }
                    }
                }
            }
        }
    }

    // 4. Assemble final Vec<SessionInfo>
    let mut sessions = Vec::new();
    for (session_name, windows) in sessions_map {
        let windows: Vec<WindowInfo> = windows.into_values().collect();
        if windows.iter().any(|w| !w.panes.is_empty()) {
            sessions.push(SessionInfo {
                session_name,
                windows,
            });
        }
    }

    sessions
}

/// Parse a single pane line from `tmux list-panes -F`.
/// Returns None if the line has fewer than 19 fields, is a sidebar, or has no agent.
pub(crate) fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let parts = split_tmux_fields(line, '|');
    if parts.len() < 19 {
        return None;
    }

    if parts[7] == "sidebar" {
        return None;
    }

    let agent = AgentType::from_str(&parts[3])?;

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
        PermissionMode::from_str(&parts[16])
    } else {
        PermissionMode::Default
    };

    let prompt_source = &parts[10];
    let prompt_is_response = prompt_source == "response";

    // Sanitize prompt: replace pipes/newlines, filter system-injected messages, truncate
    let prompt = sanitize_prompt(&parts[9]);

    Some(PaneInfo {
        pane_active: parts[0] == "1",
        status: PaneStatus::from_str(&parts[1]),
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
    })
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

fn apply_codex_permission_modes(
    panes: &mut [PaneInfo],
    pids_to_check: &[(usize, u32)],
    ps_out: &str,
) {
    for (idx, pid) in pids_to_check {
        let pid_str = pid.to_string();
        for line in ps_out.lines() {
            let trimmed = line.trim();
            if let Some((ppid_str, args)) = trimmed.split_once(char::is_whitespace) {
                if ppid_str.trim() != pid_str {
                    continue;
                }
                if let Some(pane) = panes.get_mut(*idx) {
                    pane.permission_mode = detect_codex_permission_mode(args);
                    if pane.permission_mode != PermissionMode::Default {
                        break;
                    }
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
    // Filter system-injected messages (e.g. <task-notification>, <system-reminder>)
    if raw.contains('<') && raw.contains('>') {
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
    let _ = run_tmux(&["set", "-t", pane, "-p", key, value]);
}

pub fn unset_pane_option(pane: &str, key: &str) {
    let _ = run_tmux(&["set", "-t", pane, "-p", "-u", key]);
}

pub fn get_pane_option_value(pane: &str, key: &str) -> String {
    run_tmux(&["show", "-t", pane, "-pv", key])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
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
        assert_eq!(PaneStatus::from_str("running"), PaneStatus::Running);
        assert_eq!(PaneStatus::from_str("waiting"), PaneStatus::Waiting);
        assert_eq!(PaneStatus::from_str("notification"), PaneStatus::Waiting);
        assert_eq!(PaneStatus::from_str("idle"), PaneStatus::Idle);
        assert_eq!(PaneStatus::from_str("error"), PaneStatus::Error);
        assert_eq!(PaneStatus::from_str("anything"), PaneStatus::Unknown);
        assert_eq!(PaneStatus::from_str(""), PaneStatus::Unknown);
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
        assert_eq!(AgentType::from_str("claude"), Some(AgentType::Claude));
        assert_eq!(AgentType::from_str("codex"), Some(AgentType::Codex));
        assert_eq!(AgentType::from_str("unknown"), None);
        assert_eq!(AgentType::from_str(""), None);
    }

    #[test]
    fn agent_type_label() {
        assert_eq!(AgentType::Claude.label(), "claude");
        assert_eq!(AgentType::Codex.label(), "codex");
        assert_eq!(AgentType::Unknown.label(), "unknown");
    }

    #[test]
    fn permission_mode_from_str_all() {
        assert_eq!(PermissionMode::from_str("default"), PermissionMode::Default);
        assert_eq!(PermissionMode::from_str("plan"), PermissionMode::Plan);
        assert_eq!(
            PermissionMode::from_str("acceptEdits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(PermissionMode::from_str("auto"), PermissionMode::Auto);
        assert_eq!(PermissionMode::from_str("dontAsk"), PermissionMode::DontAsk);
        assert_eq!(
            PermissionMode::from_str("bypassPermissions"),
            PermissionMode::BypassPermissions
        );
        assert_eq!(PermissionMode::from_str(""), PermissionMode::Default);
        assert_eq!(PermissionMode::from_str("unknown"), PermissionMode::Default);
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
        let auto = PermissionMode::from_str("auto");
        let dont_ask = PermissionMode::from_str("dontAsk");
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
        }];
        let pids = vec![(0, 101)];
        let ps_out = " 101 /bin/codex --full-auto\n";

        apply_codex_permission_modes(&mut panes, &pids, ps_out);
        assert_eq!(panes[0].permission_mode, PermissionMode::Auto);
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
    fn parse_pane_line_rejects_fewer_than_19_fields() {
        // Only 15 fields — should be rejected
        let fields_15 =
            "1|running||claude|name|/path|fish||%1|prompt|1700000000||12345|Explore|/cwd";
        assert!(
            parse_pane_line(fields_15).is_none(),
            "15 fields should be rejected"
        );

        // 18 fields — still rejected
        let fields_18 = "1|running||claude|name|/path|fish||%1|prompt|user|1700000000||12345|Explore|/cwd|auto|";
        assert!(
            parse_pane_line(fields_18).is_none(),
            "18 fields should be rejected"
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
        fields[15] = "auto"; // should be ignored for codex
        let line = make_pane_line(&fields);
        let pane = parse_pane_line(&line).unwrap();
        assert_eq!(
            pane.permission_mode,
            PermissionMode::Default,
            "codex should not read permission_mode from tmux variable"
        );
    }
}
