import type { ExtensionAPI } from "@earendil-works/pi-coding-agent"
import { spawn } from "node:child_process"
import { existsSync, realpathSync } from "node:fs"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

// Locate `hook.sh` by walking up from this file. The extension may live in
// `.pi/extensions/` inside the plugin checkout, or be symlinked into
// `~/.pi/agent/extensions/`, so a fixed relative path is not reliable.
const resolveHookScript = (): string | null => {
  // Resolve symlinks first so the walk-up starts at the real file when this
  // extension is symlinked into `~/.pi/agent/extensions/`.
  let self = fileURLToPath(import.meta.url)
  try {
    self = realpathSync(self)
  } catch {
    // keep the unresolved path and fall through to the PATH fallback
  }
  let dir = dirname(self)
  for (let i = 0; i < 6; i += 1) {
    const candidate = resolve(dir, "hook.sh")
    if (existsSync(candidate)) {
      return candidate
    }
    const parent = dirname(dir)
    if (parent === dir) {
      break
    }
    dir = parent
  }
  return null
}

const HOOK_COMMAND = (() => {
  const hookScript = resolveHookScript()
  return hookScript
    ? { cmd: "bash", prefix: [hookScript, "pi"] }
    : { cmd: "tmux-agent-sidebar", prefix: ["hook", "pi"] }
})()

// Fire-and-forget: the sidebar must never add latency to a Pi turn, and a
// missing binary or tmux server should not surface as an extension error.
const hook = (eventName: string, payload: Record<string, unknown>): void => {
  try {
    const child = spawn(HOOK_COMMAND.cmd, [...HOOK_COMMAND.prefix, eventName], {
      stdio: ["pipe", "ignore", "ignore"],
    })
    child.on("error", () => {})
    child.stdin.on("error", () => {})
    child.stdin.end(JSON.stringify(payload))
  } catch {
    // Absence of tmux or the sidebar binary must never break Pi.
  }
}

// Keep tmux option values short and free of newlines.
const preview = (value: unknown, limit = 400): string => {
  if (typeof value !== "string") return ""
  const flat = value.replace(/\s+/g, " ").trim()
  if (!flat) return ""
  return flat.length > limit ? `${flat.slice(0, limit - 3)}...` : flat
}

export default function (pi: ExtensionAPI) {
  const base = (cwd: string, sessionId: string) => ({
    cwd,
    session_id: sessionId,
  })

  pi.on("session_start", async (event, ctx) => {
    hook("session-start", {
      ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
      source: event.reason,
    })
  })

  pi.on("session_shutdown", async (event) => {
    hook("session-end", { end_reason: event.reason })
  })

  pi.on("before_agent_start", async (event, ctx) => {
    const prompt = event.messageType === "user" ? preview(event.prompt) : ""
    hook("user-prompt-submit", {
      ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
      prompt,
    })
  })

  // `agent_start` also fires for retries and continuations where no fresh
  // user prompt exists. An empty prompt keeps the previous preview and only
  // advances the pane status to running.
  pi.on("agent_start", async (_event, ctx) => {
    hook("user-prompt-submit", {
      ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
      prompt: "",
    })
  })

  // Pi is blocked on a user-facing prompt, so the pane needs attention.
  pi.on("ui_prompt_start", async (_event, ctx) => {
    hook("notification", {
      ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
      wait_reason: "ui_prompt",
    })
  })

  pi.on("ui_prompt_end", async (_event, ctx) => {
    hook("user-prompt-submit", {
      ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
      prompt: "",
    })
  })

  pi.on("agent_end", async (_event, ctx) => {
    hook("stop", { ...base(ctx.cwd, ctx.sessionManager.getSessionId()), last_message: "" })
  })

  // Final boundary: no retry, compaction, or queued work will run.
  pi.on("agent_settled", async (_event, ctx) => {
    hook("stop", { ...base(ctx.cwd, ctx.sessionManager.getSessionId()), last_message: "" })
  })

  pi.on("turn_end", async (event, ctx) => {
    if (event.outcome === "error") {
      hook("stop-failure", {
        ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
        error: "turn error",
      })
    }
  })

  pi.on("session_compact_failed", async (event, ctx) => {
    if (!event.aborted) {
      hook("stop-failure", {
        ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
        error: preview(event.errorMessage) || "compaction failed",
      })
    }
  })

  pi.on("tool_execution_start", async (event, ctx) => {
    hook("activity-log", {
      ...base(ctx.cwd, ctx.sessionManager.getSessionId()),
      tool_name: event.toolName,
      tool_input: event.args ?? {},
      tool_response: {},
    })
  })
}
