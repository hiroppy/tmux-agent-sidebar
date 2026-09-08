// tmux-agent-sidebar bridge for the pi coding agent.
//
// pi has no native hooks-JSON config like Claude Code / Codex, so this
// extension listens on pi's own extension event bus
// (https://github.com/earendil-works/pi-coding-agent — see docs/extensions.md)
// and shells out to hook.sh, the same fire-and-forget pattern used by
// OpenCode's plugin bridge (.opencode/plugins/tmux-agent-sidebar.js).
//
// Install: symlink this file (not the directory) into pi's global
// extensions directory so it auto-loads for every project without
// needing per-project trust:
//
//   mkdir -p ~/.pi/agent/extensions
//   ln -sf ~/.tmux/plugins/tmux-agent-sidebar/.pi/extensions/tmux-agent-sidebar.ts \
//     ~/.pi/agent/extensions/tmux-agent-sidebar.ts
//
// Extensions are plain untyped JS/TS run through jiti, so this file makes
// no use of pi's ExtensionAPI types — it only needs `.ts` naming to match
// pi's auto-discovery glob (`~/.pi/agent/extensions/*.ts`, `.pi/extensions/*.ts`).
import { spawn } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// pi loads extensions (via jiti) through a dynamic `import()` of whatever
// path discovery found — for the documented global install that path is
// the `~/.pi/agent/extensions/tmux-agent-sidebar.ts` symlink itself, and
// `import.meta.url` reflects that symlink path verbatim rather than the
// real file it points at (unlike a plain `node <path>` CLI invocation,
// which does resolve symlinks for its entry module). Without realpath-ing
// first, the walk-up below starts from the wrong directory tree entirely
// (`~/.pi/agent/extensions/` instead of the plugin/repo root) and can
// never find `hook.sh`, silently falling back to a bare `tmux-agent-sidebar`
// command that usually isn't on PATH.
const resolveSelfPath = () => {
  const raw = fileURLToPath(import.meta.url);
  try {
    return realpathSync(raw);
  } catch {
    return raw;
  }
};

const resolveHookScript = () => {
  let dir = dirname(resolveSelfPath());
  for (let i = 0; i < 4; i += 1) {
    const candidate = resolve(dir, "hook.sh");
    if (existsSync(candidate)) {
      return candidate;
    }
    const parent = dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return null;
};

const HOOK_COMMAND = (() => {
  const hookScript = resolveHookScript();
  return hookScript
    ? { cmd: "bash", prefix: [hookScript, "pi"] }
    : { cmd: "tmux-agent-sidebar", prefix: ["hook", "pi"] };
})();

// Fire-and-forget: pi does not await extension event handlers before
// continuing its own event loop, so serializing subprocess exits would
// only add latency without backpressure.
const hook = (eventName, payload) => {
  try {
    const child = spawn(HOOK_COMMAND.cmd, [...HOOK_COMMAND.prefix, eventName], {
      stdio: ["pipe", "ignore", "ignore"],
    });
    child.on("error", () => {});
    child.stdin.on("error", () => {});
    child.stdin.end(JSON.stringify(payload));
  } catch {
    // pi should keep running even if the bridge is missing or the
    // sidebar binary is unavailable.
  }
};

const getSessionId = (ctx) => {
  try {
    return ctx?.sessionManager?.getSessionId?.() ?? "";
  } catch {
    return "";
  }
};

// "resume" and "fork" both continue a prior conversation from the user's
// point of view; every other session_start reason ("startup", "reload",
// "new") is treated as a fresh session.
const mapSource = (reason) => (reason === "resume" || reason === "fork" ? "resume" : "");

const extractAssistantText = (messages) => {
  if (!Array.isArray(messages)) return "";
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (!message || message.role !== "assistant") continue;
    const content = Array.isArray(message.content) ? message.content : [];
    const chunks = content
      .filter((part) => part && part.type === "text" && typeof part.text === "string")
      .map((part) => part.text);
    if (chunks.length > 0) return chunks.join("\n");
  }
  return "";
};

export default function (pi) {
  // Last assistant reply seen this run, flushed into the `stop` hook once
  // pi truly settles. `agent_end` can still be followed by an automatic
  // retry/compaction retry/queued follow-up, so the actual "done" signal
  // is `agent_settled`, not `agent_end`.
  let lastAssistantText = "";

  pi.on("session_start", async (event, ctx) => {
    hook("session-start", {
      cwd: ctx.cwd,
      session_id: getSessionId(ctx),
      source: mapSource(event.reason),
    });
  });

  pi.on("session_shutdown", async (event, ctx) => {
    hook("session-end", { end_reason: event.reason ?? "" });
  });

  pi.on("before_agent_start", async (event, ctx) => {
    hook("user-prompt-submit", {
      cwd: ctx.cwd,
      session_id: getSessionId(ctx),
      prompt: event.prompt ?? "",
    });
  });

  pi.on("agent_end", async (event) => {
    const text = extractAssistantText(event.messages);
    if (text) {
      lastAssistantText = text;
    }
  });

  pi.on("agent_settled", async (_event, ctx) => {
    hook("stop", {
      cwd: ctx.cwd,
      session_id: getSessionId(ctx),
      last_message: lastAssistantText,
    });
    lastAssistantText = "";
  });

  // `tool_result` (rather than `tool_execution_end`) is the one lifecycle
  // event that carries both the tool's input args and its finalized
  // result in the same payload.
  pi.on("tool_result", async (event, ctx) => {
    if (!event?.toolName) return;
    hook("activity-log", {
      cwd: ctx.cwd,
      session_id: getSessionId(ctx),
      tool_name: event.toolName,
      tool_input: event.input ?? {},
      tool_response: {
        content: event.content ?? [],
        details: event.details ?? {},
        isError: Boolean(event.isError),
      },
    });
  });
}
