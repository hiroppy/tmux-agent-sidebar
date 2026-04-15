use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub timestamp: String,
    pub tool: String,
    pub label: String,
}

impl ActivityEntry {
    pub fn tool_color_index(&self) -> u8 {
        match self.tool.as_str() {
            "Edit" | "Write" => 180,         // soft yellow
            "Bash" | "PowerShell" => 114,    // soft green
            "Read" | "Glob" | "Grep" => 110, // soft blue
            "Agent" => 181,                  // soft pink
            "WebFetch" | "WebSearch" => 117, // soft cyan
            "Skill" => 218,                  // soft magenta
            "TaskCreate" | "TaskUpdate" | "TaskGet" | "TaskList" | "TaskStop" | "TaskOutput" => 223, // soft gold
            "SendMessage" | "TeamCreate" | "TeamDelete" => 182, // soft lavender
            "LSP" => 146,                                       // soft teal
            "NotebookEdit" => 180,                              // soft yellow (like Edit)
            "AskUserQuestion" => 216,                           // soft orange
            "CronCreate" | "CronDelete" | "CronList" | "RemoteTrigger" => 151, // soft mint
            "EnterPlanMode" | "ExitPlanMode" => 189,            // soft periwinkle
            "EnterWorktree" | "ExitWorktree" => 179,            // soft bronze
            "ToolSearch" => 250,                                // light gray
            _ => 244,
        }
    }
}

pub fn log_file_path(pane_id: &str) -> PathBuf {
    let encoded = pane_id.replace('%', "_");
    PathBuf::from(format!("/tmp/tmux-agent-activity{encoded}.log"))
}

fn parse_entry(line: &str) -> Option<ActivityEntry> {
    let mut parts = line.splitn(3, '|');
    let timestamp = parts.next()?.to_string();
    let tool = parts.next()?.to_string();
    let label = parts.next().unwrap_or("").to_string();
    Some(ActivityEntry {
        timestamp,
        tool,
        label,
    })
}

pub fn read_activity_log(pane_id: &str, max_entries: usize) -> Vec<ActivityEntry> {
    let path = log_file_path(pane_id);
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    if max_entries > 0 {
        // Only parse the last N lines (avoid allocating the full Vec)
        let entries: Vec<ActivityEntry> = content
            .rsplit('\n')
            .filter(|l| !l.is_empty())
            .take(max_entries)
            .filter_map(parse_entry)
            .collect();
        // rsplit yields newest-first, which is the desired order (reverse chronological)
        entries
    } else {
        // Parse all entries in reverse order
        content
            .rsplit('\n')
            .filter(|l| !l.is_empty())
            .filter_map(parse_entry)
            .collect()
    }
}

/// Per-task status extracted from the activity log.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

/// Summary of task progress for a single pane.
#[derive(Debug, Clone, Default)]
pub struct TaskProgress {
    pub tasks: Vec<(String, TaskStatus)>, // (subject, status)
}

impl TaskProgress {
    pub fn completed_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|(_, s)| *s == TaskStatus::Completed)
            .count()
    }

    pub fn in_progress_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|(_, s)| *s == TaskStatus::InProgress)
            .count()
    }

    pub fn total(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn all_completed(&self) -> bool {
        !self.tasks.is_empty() && self.tasks.iter().all(|(_, s)| *s == TaskStatus::Completed)
    }
}

/// Sentinel tool name written to the activity log at assistant-turn boundaries.
/// Acts as a hard reset barrier for task progress parsing so state from a
/// previous run cannot leak into a new one.
pub const TASK_RESET_MARKER: &str = "__task_reset__";

/// Parse task progress from activity log entries.
/// Entries are in reverse chronological order (newest first), so we reverse to process in order.
pub fn parse_task_progress(entries: &[ActivityEntry]) -> TaskProgress {
    let mut tasks: Vec<(String, String, TaskStatus)> = Vec::new(); // (id, subject, status)

    // Process in chronological order (entries are newest-first)
    for entry in entries.iter().rev() {
        match entry.tool.as_str() {
            TASK_RESET_MARKER => {
                tasks.clear();
            }
            "TaskCreate" => {
                // Label format: "#1 subject" or just "subject"
                let (id, subject) = if entry.label.starts_with('#') {
                    let rest = &entry.label[1..];
                    match rest.find(' ') {
                        Some(pos) => (rest[..pos].to_string(), rest[pos + 1..].to_string()),
                        None => (rest.to_string(), String::new()),
                    }
                } else {
                    (String::new(), entry.label.clone())
                };
                // Reset when a new task set starts:
                // - ID already exists (restarted session reusing IDs), or
                // - all existing tasks are completed (new batch)
                let all_done =
                    !tasks.is_empty() && tasks.iter().all(|(_, _, s)| *s == TaskStatus::Completed);
                let id_exists =
                    !id.is_empty() && tasks.iter().any(|(existing_id, _, _)| existing_id == &id);
                if id_exists || all_done {
                    tasks.clear();
                }
                tasks.push((id, subject, TaskStatus::Pending));
            }
            "TaskUpdate" => {
                // Label format: "completed #1" or "in_progress #2"
                let parts: Vec<&str> = entry.label.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let status_str = parts[0];
                    let id = parts[1].trim_start_matches('#');
                    let new_status = match status_str {
                        "completed" => TaskStatus::Completed,
                        "in_progress" => TaskStatus::InProgress,
                        "deleted" => {
                            // Remove deleted tasks
                            tasks.retain(|(tid, _, _)| tid != id);
                            continue;
                        }
                        _ => TaskStatus::Pending,
                    };
                    // Find and update matching task
                    if let Some(task) = tasks.iter_mut().find(|(tid, _, _)| tid == id) {
                        task.2 = new_status;
                    }
                }
            }
            _ => {}
        }
    }

    TaskProgress {
        tasks: tasks
            .into_iter()
            .map(|(_, subject, status)| (subject, status))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_activity_log() {
        let pane_id = "%99_test";
        let path = log_file_path(pane_id);
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "10:30|Read|package.json").unwrap();
        writeln!(f, "10:31|Edit|sidebar.sh").unwrap();
        writeln!(f, "10:32|Bash|cargo build").unwrap();
        drop(f);

        let entries = read_activity_log(pane_id, 50);

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tool, "Bash");
        assert_eq!(entries[0].label, "cargo build");
        assert_eq!(entries[2].tool, "Read");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_tool_color() {
        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "Edit".into(),
            label: "test".into(),
        };
        assert_eq!(entry.tool_color_index(), 180);

        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "Bash".into(),
            label: "test".into(),
        };
        assert_eq!(entry.tool_color_index(), 114);

        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "WebFetch".into(),
            label: "example.com".into(),
        };
        assert_eq!(entry.tool_color_index(), 117);

        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "WebSearch".into(),
            label: "rust tutorial".into(),
        };
        assert_eq!(entry.tool_color_index(), 117);

        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "ToolSearch".into(),
            label: "".into(),
        };
        assert_eq!(entry.tool_color_index(), 250);

        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "PowerShell".into(),
            label: "Get-Process".into(),
        };
        assert_eq!(entry.tool_color_index(), 114);

        let entry = ActivityEntry {
            timestamp: "10:00".into(),
            tool: "UnknownTool".into(),
            label: "".into(),
        };
        assert_eq!(entry.tool_color_index(), 244);
    }

    #[test]
    fn test_log_file_path() {
        let path = log_file_path("%5");
        assert_eq!(path.to_str().unwrap(), "/tmp/tmux-agent-activity_5.log");
    }

    #[test]
    fn test_read_activity_log_max_entries() {
        let pane_id = "%TEST_MAX";
        let path = log_file_path(pane_id);
        let mut f = fs::File::create(&path).unwrap();
        for i in 0..10 {
            writeln!(f, "10:{:02}|Read|file{i}.rs", i).unwrap();
        }
        drop(f);

        let entries = read_activity_log(pane_id, 3);
        assert_eq!(entries.len(), 3);
        // Should be last 3, reversed (newest first)
        assert_eq!(entries[0].label, "file9.rs");
        assert_eq!(entries[1].label, "file8.rs");
        assert_eq!(entries[2].label, "file7.rs");

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_parse_web_tools() {
        let pane_id = "%99_web_test";
        let path = log_file_path(pane_id);
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "11:00|WebFetch|example.com").unwrap();
        writeln!(f, "11:01|WebSearch|rust tutorial").unwrap();
        writeln!(f, "11:02|ToolSearch|").unwrap();
        drop(f);

        let entries = read_activity_log(pane_id, 50);
        assert_eq!(entries.len(), 3);
        // Reversed (newest first)
        assert_eq!(entries[0].tool, "ToolSearch");
        assert_eq!(entries[0].label, "");
        assert_eq!(entries[1].tool, "WebSearch");
        assert_eq!(entries[1].label, "rust tutorial");
        assert_eq!(entries[2].tool, "WebFetch");
        assert_eq!(entries[2].label, "example.com");

        fs::remove_file(&path).unwrap();
    }

    fn task_entry(tool: &str, label: &str) -> ActivityEntry {
        ActivityEntry {
            timestamp: "10:00".into(),
            tool: tool.into(),
            label: label.into(),
        }
    }

    #[test]
    fn test_parse_task_progress_basic() {
        let entries = vec![
            task_entry("TaskUpdate", "completed #1"),
            task_entry("TaskUpdate", "in_progress #2"),
            task_entry("TaskUpdate", "in_progress #1"),
            task_entry("TaskCreate", "#2 Wire RepoGroup"),
            task_entry("TaskCreate", "#1 Add RepoGroup"),
        ];
        let progress = parse_task_progress(&entries);
        assert_eq!(progress.total(), 2);
        assert_eq!(progress.completed_count(), 1);
        assert_eq!(progress.in_progress_count(), 1);
        assert_eq!(progress.tasks[0].0, "Add RepoGroup");
        assert_eq!(progress.tasks[0].1, TaskStatus::Completed);
        assert_eq!(progress.tasks[1].0, "Wire RepoGroup");
        assert_eq!(progress.tasks[1].1, TaskStatus::InProgress);
    }

    #[test]
    fn test_parse_task_progress_empty() {
        let entries = vec![
            task_entry("Read", "file.rs"),
            task_entry("Bash", "cargo build"),
        ];
        let progress = parse_task_progress(&entries);
        assert!(progress.is_empty());
    }

    #[test]
    fn test_parse_task_progress_deleted() {
        let entries = vec![
            task_entry("TaskUpdate", "deleted #3"),
            task_entry("TaskUpdate", "in_progress #3"),
            task_entry("TaskUpdate", "in_progress #2"),
            task_entry("TaskUpdate", "completed #1"),
            task_entry("TaskCreate", "#3 Temp task"),
            task_entry("TaskCreate", "#2 Real task B"),
            task_entry("TaskCreate", "#1 Real task A"),
        ];
        let progress = parse_task_progress(&entries);
        // #1 completed, #2 in_progress, #3 deleted → 2 tasks remain (not all completed)
        assert_eq!(progress.total(), 2);
        assert_eq!(progress.tasks[0].0, "Real task A");
        assert_eq!(progress.tasks[1].0, "Real task B");
    }

    #[test]
    fn test_parse_task_progress_resets_when_all_completed() {
        // First batch: 2 tasks, both completed. Then new batch starts.
        let entries = vec![
            task_entry("TaskUpdate", "in_progress #3"),
            task_entry("TaskCreate", "#3 New batch task"),
            task_entry("TaskUpdate", "completed #2"),
            task_entry("TaskUpdate", "completed #1"),
            task_entry("TaskUpdate", "in_progress #2"),
            task_entry("TaskUpdate", "in_progress #1"),
            task_entry("TaskCreate", "#2 Old task B"),
            task_entry("TaskCreate", "#1 Old task A"),
        ];
        let progress = parse_task_progress(&entries);
        assert_eq!(progress.total(), 1);
        assert_eq!(progress.tasks[0].0, "New batch task");
        assert_eq!(progress.tasks[0].1, TaskStatus::InProgress);
    }

    #[test]
    fn test_parse_task_progress_no_reset_while_in_progress() {
        // First batch still has in_progress task, new task added
        let entries = vec![
            task_entry("TaskCreate", "#3 Extra task"),
            task_entry("TaskUpdate", "completed #1"),
            task_entry("TaskUpdate", "in_progress #2"),
            task_entry("TaskUpdate", "in_progress #1"),
            task_entry("TaskCreate", "#2 Task B"),
            task_entry("TaskCreate", "#1 Task A"),
        ];
        let progress = parse_task_progress(&entries);
        // Should keep all 3 since task #2 is still in_progress
        assert_eq!(progress.total(), 3);
    }

    #[test]
    fn test_parse_task_progress_resets_on_marker() {
        // Previous run left #1 completed and #2 in_progress.
        // A task-reset marker is written at turn boundary, then new run creates #3.
        // The marker must drop everything before it.
        let entries = vec![
            task_entry("TaskUpdate", "in_progress #3"),
            task_entry("TaskCreate", "#3 Fresh task"),
            task_entry(TASK_RESET_MARKER, ""),
            task_entry("TaskUpdate", "in_progress #2"),
            task_entry("TaskUpdate", "completed #1"),
            task_entry("TaskUpdate", "in_progress #1"),
            task_entry("TaskCreate", "#2 Stale task B"),
            task_entry("TaskCreate", "#1 Stale task A"),
        ];
        let progress = parse_task_progress(&entries);
        assert_eq!(progress.total(), 1);
        assert_eq!(progress.tasks[0].0, "Fresh task");
        assert_eq!(progress.tasks[0].1, TaskStatus::InProgress);
    }

    #[test]
    fn test_parse_task_progress_resets_on_new_session() {
        // Simulates: old session had 3 tasks, new session starts with TaskCreate #1
        let entries = vec![
            task_entry("TaskUpdate", "in_progress #1"),
            task_entry("TaskCreate", "#1 New task"),
            task_entry("Read", "file.rs"),
            task_entry("TaskUpdate", "completed #3"),
            task_entry("TaskUpdate", "completed #2"),
            task_entry("TaskUpdate", "completed #1"),
            task_entry("TaskCreate", "#3 Old task C"),
            task_entry("TaskCreate", "#2 Old task B"),
            task_entry("TaskCreate", "#1 Old task A"),
        ];
        let progress = parse_task_progress(&entries);
        // Should only have the new session's task
        assert_eq!(progress.total(), 1);
        assert_eq!(progress.tasks[0].0, "New task");
        assert_eq!(progress.tasks[0].1, TaskStatus::InProgress);
    }
}
