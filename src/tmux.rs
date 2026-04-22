mod commands;
mod options;
mod panes;
mod query;
mod types;

pub use commands::{
    display_message, kill_window, new_window, pane_session_name, run_tmux, run_tmux_capture,
    select_pane, send_command, set_window_option,
};
pub use options::{
    get_all_global_options, get_option, get_pane_option_value, set_pane_option, unset_pane_option,
};
pub use panes::{
    find_active_pane, focused_pane_path, get_pane_path, get_sidebar_pane_info,
    query_active_window_panes,
};
pub use query::query_sessions;
pub use types::{
    AgentType, CLAUDE_AGENT, CODEX_AGENT, OPENCODE_AGENT, PaneInfo, PaneStatus, PermissionMode,
    SessionInfo, WindowInfo, WorktreeMetadata,
};

#[cfg(test)]
pub use options::test_mock;
