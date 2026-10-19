// @ts-nocheck
// tmux-agent-sidebar bridge for oh-my-pi (omp).
//
// omp has no native hooks-JSON config like Claude Code / Codex, so this
// extension listens on omp's extension event bus and shells out to hook.sh,
// the same fire-and-forget pattern used by the OpenCode plugin bridge.
//
// Install: symlink this file (not the directory) into omp's global
// extensions directory so it auto-loads for every project:
//
//   mkdir -p ~/.omp/agent/extensions
//   ln -sf ~/.tmux/plugins/tmux-agent-sidebar/.omp/extensions/tmux-agent-sidebar.ts \
//     ~/.omp/agent/extensions/tmux-agent-sidebar.ts
//
// omp has no "agent settled" event, so `agent_end` is debounced into `stop`
// (and retryable provider errors are held until omp's own auto-retry has had
// a chance to start a new turn).
import { spawn } from "node:child_process";
import { existsSync, realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// omp loads extensions through a dynamic `import()` of the discovered path,
// which for the documented global install is the symlink itself, so
// `import.meta.url` reflects the symlink rather than the real file. Without
// realpath-ing first, the walk-up below would start in
// `~/.omp/agent/extensions/` and never find `hook.sh`.
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
    ? { cmd: "bash", prefix: [hookScript, "omp"] }
    : { cmd: "tmux-agent-sidebar", prefix: ["hook", "omp"] };
})();

// Fire-and-forget: omp does not await extension event handlers before
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
    // omp should keep running even if the bridge is missing or the
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

const lastAssistantMessage = (messages) => {
  if (!Array.isArray(messages)) return undefined;
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i]?.role === "assistant") return messages[i];
  }
  return undefined;
};

// Same classification herdr-omp-agent-state.ts uses to decide whether an
// errored turn will be retried by omp itself.
const retryableErrorPattern =
  /overloaded|provider.?returned.?error|rate.?limit|too many requests|429|500|502|503|504|service.?unavailable|server.?error|internal.?error|network.?error|connection.?error|connection.?refused|connection.?lost|websocket.?closed|websocket.?error|other side closed|fetch failed|upstream.?connect|reset before headers|socket hang up|ended without|http2 request did not get a response|timed? out|timeout|terminated|retry delay/i;

const IDLE_DEBOUNCE_MS = 250;
const RETRY_GRACE_MS = 2500;

// Only the interactive root agent may drive the pane's state. Subagent
// sessions rebind this factory and inherit the parent's $TMUX_PANE, so
// letting them emit would clobber the parent's prompt/status.
const isRoot = (ctx) => ctx?.hasUI === true && ctx?.agent?.kind !== "sub";

export default function (pi) {
  if (!process.env.TMUX_PANE) {
    return;
  }

  let pendingTimer;

  const clearPending = () => {
    if (pendingTimer) {
      clearTimeout(pendingTimer);
      pendingTimer = undefined;
    }
  };

  const base = (ctx) => ({ cwd: ctx.cwd, session_id: getSessionId(ctx) });

  pi.on("session_start", (_event, ctx) => {
    if (!isRoot(ctx)) return;
    hook("session-start", { ...base(ctx), source: "startup" });
  });

  pi.on("session_switch", (_event, ctx) => {
    if (!isRoot(ctx)) return;
    clearPending();
    hook("session-start", { ...base(ctx), source: "resume" });
  });

  pi.on("before_agent_start", (event, ctx) => {
    if (!isRoot(ctx)) return;
    clearPending();
    hook("user-prompt-submit", { ...base(ctx), prompt: event?.prompt ?? "" });
  });

  // Status-only: covers queued / steered batches that never pass through
  // `before_agent_start`, and cancels any pending idle / retry timer.
  pi.on("agent_start", (_event, ctx) => {
    if (!isRoot(ctx)) return;
    clearPending();
    hook("user-prompt-submit", { ...base(ctx), prompt: "" });
  });

  pi.on("tool_approval_requested", (_event, ctx) => {
    if (!isRoot(ctx)) return;
    hook("notification", { ...base(ctx), wait_reason: "permission" });
  });

  pi.on("tool_execution_start", (event, ctx) => {
    if (event?.toolName !== "ask") return;
    if (!isRoot(ctx)) return;
    hook("notification", { ...base(ctx), wait_reason: "elicitation_dialog" });
  });

  // The next activity-log event moves the pane waiting -> running, so no
  // explicit "resolved" hook is needed for approvals or `ask` dialogs.
  pi.on("tool_result", (event, ctx) => {
    if (!event?.toolName) return;
    if (!isRoot(ctx)) return;
    hook("activity-log", {
      ...base(ctx),
      tool_name: event.toolName,
      tool_input: event.input ?? event.args ?? {},
      tool_response: {
        content: event.content ?? [],
        details: event.details ?? {},
        isError: Boolean(event.isError),
      },
    });
  });

  pi.on("agent_end", (event, ctx) => {
    if (!isRoot(ctx)) return;
    clearPending();
    const payload = base(ctx);
    const last = lastAssistantMessage(event?.messages);

    if (last?.stopReason === "error") {
      const error = String(last.errorMessage ?? "");
      if (retryableErrorPattern.test(error)) {
        // omp will auto-retry; only report failure if no new turn starts.
        pendingTimer = setTimeout(() => {
          pendingTimer = undefined;
          hook("stop-failure", { ...payload, error });
        }, RETRY_GRACE_MS);
        pendingTimer.unref?.();
      } else {
        hook("stop-failure", { ...payload, error });
      }
      return;
    }

    const lastMessage = extractAssistantText(event?.messages);
    pendingTimer = setTimeout(() => {
      pendingTimer = undefined;
      hook("stop", { ...payload, last_message: lastMessage });
    }, IDLE_DEBOUNCE_MS);
    pendingTimer.unref?.();
  });

  pi.on("session_shutdown", (event, ctx) => {
    if (!isRoot(ctx)) return;
    clearPending();
    hook("session-end", { ...base(ctx), end_reason: event?.reason ?? "" });
  });
}
