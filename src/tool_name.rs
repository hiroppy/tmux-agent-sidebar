/// Canonical tool-name vocabulary used across agents. Claude and Codex emit
/// these PascalCase names natively; OpenCode's lowercase IDs are normalised to
/// this vocabulary in `src/adapter/opencode.rs`. Keeping the list as an enum
/// means typos in adapters or the strategy table become compile errors rather
/// than silently unmatched tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalTool {
    Bash,
    Read,
    Edit,
    Write,
    NotebookEdit,
    PowerShell,
    Monitor,
    PushNotification,
    Glob,
    Grep,
    WebFetch,
    WebSearch,
    ToolSearch,
    Skill,
    SendMessage,
    TeamCreate,
    Lsp,
    CronCreate,
    CronDelete,
    EnterWorktree,
    ExitWorktree,
    Agent,
    TaskCreate,
    TaskUpdate,
    TaskGet,
    TaskStop,
    TaskOutput,
    AskUserQuestion,
    TodoWrite,
}

impl CanonicalTool {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "Bash",
            Self::Read => "Read",
            Self::Edit => "Edit",
            Self::Write => "Write",
            Self::NotebookEdit => "NotebookEdit",
            Self::PowerShell => "PowerShell",
            Self::Monitor => "Monitor",
            Self::PushNotification => "PushNotification",
            Self::Glob => "Glob",
            Self::Grep => "Grep",
            Self::WebFetch => "WebFetch",
            Self::WebSearch => "WebSearch",
            Self::ToolSearch => "ToolSearch",
            Self::Skill => "Skill",
            Self::SendMessage => "SendMessage",
            Self::TeamCreate => "TeamCreate",
            Self::Lsp => "LSP",
            Self::CronCreate => "CronCreate",
            Self::CronDelete => "CronDelete",
            Self::EnterWorktree => "EnterWorktree",
            Self::ExitWorktree => "ExitWorktree",
            Self::Agent => "Agent",
            Self::TaskCreate => "TaskCreate",
            Self::TaskUpdate => "TaskUpdate",
            Self::TaskGet => "TaskGet",
            Self::TaskStop => "TaskStop",
            Self::TaskOutput => "TaskOutput",
            Self::AskUserQuestion => "AskUserQuestion",
            Self::TodoWrite => "TodoWrite",
        }
    }
}
