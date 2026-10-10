import { spawn, spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

const HOOK_SCRIPT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "hook.sh");
const MAX_PREVIEW_CHARS = 4_096;
const MAX_TOOL_NAME_CHARS = 256;

type HandlerContext = {
  readonly cwd: string;
  readonly sessionManager: {
    getSessionId(): string;
  };
};

const boundedText = (value: unknown, limit = MAX_PREVIEW_CHARS): string =>
  typeof value === "string" ? value.slice(0, limit) : "";

const identity = (ctx: HandlerContext) => ({
  cwd: ctx.cwd,
  session_id: ctx.sessionManager.getSessionId(),
});

const assistantPreview = (messages: unknown): string => {
  if (!Array.isArray(messages)) return "";

  for (let messageIndex = messages.length - 1; messageIndex >= 0; messageIndex -= 1) {
    const message = messages[messageIndex];
    if (!message || typeof message !== "object" || !("role" in message) || message.role !== "assistant") {
      continue;
    }

    if (!("content" in message)) continue;
    if (typeof message.content === "string") {
      const preview = boundedText(message.content);
      if (preview) return preview;
      continue;
    }
    if (!Array.isArray(message.content)) continue;

    let preview = "";
    for (const block of message.content) {
      if (
        !block ||
        typeof block !== "object" ||
        !("type" in block) ||
        block.type !== "text" ||
        !("text" in block) ||
        typeof block.text !== "string" ||
        !block.text
      ) {
        continue;
      }

      if (preview && preview.length < MAX_PREVIEW_CHARS) preview += "\n";
      const remaining = MAX_PREVIEW_CHARS - preview.length;
      if (remaining <= 0) break;
      preview += block.text.slice(0, remaining);
    }

    if (preview) return preview;
  }

  return "";
};

// The bridge never awaits a child or observes its output. OMP inherits no
// monitoring failure, while hook.sh retains its own missing-binary fail-open path.
const sendHook = (eventName: string, payload: Record<string, unknown>): void => {
  try {
    const child = spawn("bash", [HOOK_SCRIPT, "omp", eventName], {
      env: process.env,
      stdio: ["pipe", "ignore", "ignore"],
    });
    child.on("error", () => {});
    child.stdin.on("error", () => {});
    child.stdin.end(JSON.stringify(payload));
    child.unref();
  } catch {
    // Monitoring must never interfere with an OMP session.
  }
};

// OMP exits immediately after session_shutdown handlers return. Run only that
// final tombstone synchronously so tmux metadata is cleared before process exit.
const sendShutdownHook = (payload: Record<string, unknown>): void => {
  try {
    spawnSync("bash", [HOOK_SCRIPT, "omp", "session-end"], {
      env: process.env,
      input: JSON.stringify(payload),
      stdio: ["pipe", "ignore", "ignore"],
      timeout: 2_000,
    });
  } catch {
    // Cleanup remains best-effort; process liveness is the fallback.
  }
};

const safely = (observe: () => void): void => {
  try {
    observe();
  } catch {
    // Context access and payload extraction are passive and fail-open too.
  }
};

export default function tmuxAgentSidebar(pi: ExtensionAPI): void {
  pi.on("session_start", (_event, ctx) => {
    safely(() => {
      const current = identity(ctx);
      sendHook("session-start", { ...current, source: "startup" });
    });
  });

  pi.on("session_switch", (event, ctx) => {
    safely(() => {
      sendHook("session-start", { ...identity(ctx), source: event.reason });
    });
  });

  pi.on("session_branch", (_event, ctx) => {
    safely(() => {
      sendHook("session-start", { ...identity(ctx), source: "branch" });
    });
  });

  pi.on("session_shutdown", (_event, ctx) => {
    safely(() => {
      const current = identity(ctx);
      sendShutdownHook({ ...current, end_reason: "shutdown" });
    });
  });

  pi.on("before_agent_start", (event, ctx) => {
    safely(() => {
      const current = identity(ctx);
      sendHook("user-prompt-submit", {
        ...current,
        prompt: boundedText(event.prompt),
      });
    });
  });

  pi.on("agent_end", (event, ctx) => {
    safely(() => {
      const lastMessage = assistantPreview(event.messages);
      sendHook("stop", {
        ...identity(ctx),
        last_message: lastMessage,
      });
    });
  });

  pi.on("tool_execution_start", (event, ctx) => {
    safely(() => {
      const toolName = boundedText(event.toolName, MAX_TOOL_NAME_CHARS);
      if (!toolName) return;
      sendHook("activity-log", {
        ...identity(ctx),
        tool_name: toolName,
        tool_input: {},
      });
    });
  });

  pi.on("tool_approval_requested", (_event, ctx) => {
    safely(() => {
      sendHook("notification", {
        ...identity(ctx),
        wait_reason: "permission",
      });
    });
  });

}
