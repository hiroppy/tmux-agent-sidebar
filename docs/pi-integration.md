# Pi Agent Integration

The sidebar monitors Pi (pi.dev) using a local Pi extension that calls the
`tmux-agent-sidebar hook` CLI on lifecycle events.

## How It Works

1. A Pi extension subscribes to Pi's internal events via `pi.on(...)`.
2. On each event, it calls `tmux-agent-sidebar hook pi <event> '<json>'` via
   `execSync`.
3. The sidebar's `PiAdapter` normalizes the camelCase JSON payloads into
   internal `AgentEvent` structs.
4. Process-based fallback detects Pi panes that exit without sending a final
   hook event (same stale-cleanup mechanism as Codex and OpenCode).

## Installation

### 1. Create the Pi extension

Save the following to a file Pi can load as an extension (e.g.
`~/.config/pi/extensions/tmux-agent-sidebar.ts`):

```typescript
// Pi extension for tmux-agent-sidebar
// Subscribes to Pi lifecycle/tool events and forwards them to the sidebar.
import { execSync } from "child_process";

const HOOK = "tmux-agent-sidebar";

function hook(event: string, payload: Record<string, unknown>) {
  try {
    execSync(`${HOOK} hook pi ${event} '${JSON.stringify(payload)}'`, {
      stdio: "ignore",
      timeout: 1000,
    });
  } catch {
    // Sidebar not installed or not running — silently ignore
  }
}

// Session lifecycle
pi.on("session_start", (ctx) => {
  hook("session-start", {
    projectPath: ctx.projectPath,
    sessionId: ctx.sessionId,
    source: "startup",
  });
});

pi.on("session_shutdown", (ctx) => {
  hook("session-end", {
    endReason: ctx.reason ?? "unknown",
  });
});

// User prompts
pi.on("before_agent_start", (ctx) => {
  hook("user-prompt-submit", {
    projectPath: ctx.projectPath,
    prompt: ctx.prompt,
    sessionId: ctx.sessionId,
  });
});

// Agent lifecycle
pi.on("agent_end", (ctx) => {
  hook("stop", {
    projectPath: ctx.projectPath,
    lastMessage: ctx.lastMessage ?? "",
    sessionId: ctx.sessionId,
  });
});

pi.on("agent_error", (ctx) => {
  hook("stop-failure", {
    projectPath: ctx.projectPath,
    error: ctx.error ?? "unknown_error",
    sessionId: ctx.sessionId,
  });
});

// Notifications (permission prompts, tool requests, etc.)
pi.on("notification", (ctx) => {
  hook("notification", {
    projectPath: ctx.projectPath,
    notificationType: ctx.type ?? "unknown",
    sessionId: ctx.sessionId,
  });
});

// Tool execution (activity log)
pi.on("tool_execution_start", (ctx) => {
  hook("activity-log", {
    toolName: ctx.toolName,
    toolArgs: ctx.args ?? {},
    sessionId: ctx.sessionId,
  });
});

pi.on("tool_execution_end", (ctx) => {
  hook("activity-log", {
    toolName: ctx.toolName,
    toolArgs: ctx.args ?? {},
    result: ctx.result ?? {},
    sessionId: ctx.sessionId,
  });
});
```

### 2. Load the extension

Follow Pi's extension loading instructions to point Pi at the extension file,
or symlink it into Pi's extension directory.

### 3. Restart Pi

The sidebar will detect your Pi sessions automatically once the extension is
active.

## Supported Events

| Event | Payload Fields | Sidebar Effect |
|---|---|---|
| `session-start` | `projectPath`, `sessionId` | Creates a new agent pane row |
| `session-end` | `endReason` | Stops tracking the pane |
| `user-prompt-submit` | `projectPath`, `prompt`, `sessionId` | Shows prompt text; marks pane `running` |
| `stop` | `projectPath`, `lastMessage`, `sessionId` | Marks pane `idle`; shows response |
| `stop-failure` | `projectPath`, `error`, `sessionId` | Marks pane `error`; shows error |
| `notification` | `projectPath`, `notificationType`, `sessionId` | Marks pane `waiting` with reason |
| `activity-log` | `toolName`, `toolArgs`, `result`, `sessionId` | Appends to activity tab |

## Tool Name Mapping

Pi's camelCase tool names are normalized to the sidebar's internal PascalCase
vocabulary:

| Pi Tool Name | Sidebar Shows As |
|---|---|
| `executeCommand` | Bash |
| `readFile` | Read |
| `writeFile` | Write |
| `editFile` | Edit |
| `globFiles` | Glob |
| `grepFiles` | Grep |
| `searchWeb` | WebSearch |
| `fetchUrl` | WebFetch |
| `askUser` | AskUser |
| *(anything else)* | Passed through as-is |

## Spawning Pi Worktrees

Pi is available in the spawn worktree popup (`n` key in the sidebar). Select
"pi" from the agent list to create a new tmux window running the `pi` CLI.

## Customization

Set Pi's agent color in your `tmux.conf`:

```
set -g @sidebar_color_agent_pi "209"
```

The default is colour 209 (soft orange/peach).
