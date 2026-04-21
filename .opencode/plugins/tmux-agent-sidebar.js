import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const resolveHookScript = () => {
  let dir = dirname(fileURLToPath(import.meta.url));
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
    ? { cmd: "bash", prefix: [hookScript, "opencode"] }
    : { cmd: "tmux-agent-sidebar", prefix: ["hook", "opencode"] };
})();

// Fire-and-forget: OpenCode dispatches the `event` hook without awaiting
// the returned promise, so serializing subprocess exits would only add
// latency without backpressure.
const hook = (eventName, payload) => {
  try {
    const child = spawn(HOOK_COMMAND.cmd, [...HOOK_COMMAND.prefix, eventName], {
      stdio: ["pipe", "ignore", "ignore"],
    });
    child.on("error", () => {});
    child.stdin.on("error", () => {});
    child.stdin.end(JSON.stringify(payload));
  } catch {
    // OpenCode should keep running even if the bridge is missing or
    // the sidebar binary is unavailable.
  }
};

const pickFirstString = (value, keys) => {
  for (const key of keys) {
    const candidate = value?.[key];
    if (typeof candidate === "string" && candidate) {
      return candidate;
    }
  }
  return "";
};

const errorMessage = (err) => {
  if (!err) return "";
  if (typeof err === "string") return err;
  if (typeof err === "object") {
    return pickFirstString(err, ["message", "name"]) || JSON.stringify(err);
  }
  return String(err);
};

const extractPromptText = (parts) => {
  if (!Array.isArray(parts)) return "";
  const chunks = [];
  for (const part of parts) {
    if (!part || part.type !== "text") continue;
    if (part.synthetic || part.ignored) continue;
    if (typeof part.text === "string" && part.text) {
      chunks.push(part.text);
    }
  }
  return chunks.join("\n");
};

export const TmuxAgentSidebar = async ({ directory }) => {
  const cwd = typeof directory === "string" ? directory : "";

  return {
    "chat.message": async (input, output) => {
      const session_id =
        typeof input?.sessionID === "string" ? input.sessionID : "";
      const prompt = extractPromptText(output?.parts);
      hook("user-prompt-submit", { cwd, session_id, prompt });
    },

    event: async ({ event }) => {
      if (!event || !event.type) return;
      const props = event.properties ?? {};
      const session_id = pickFirstString(props, ["sessionID", "sessionId", "session_id"]);

      switch (event.type) {
        case "session.created":
          hook("session-start", { cwd, session_id, source: "startup" });
          return;

        case "session.status": {
          // Status is a union: { type: "idle" | "busy" | "retry", ... }.
          // `busy` is a secondary status-transition signal — the real prompt
          // text is written via the `chat.message` hook, which fires with
          // the UserMessage parts. When both fire, the empty-prompt call
          // here is a no-op for @pane_prompt (handler guard) but still
          // advances status to "running" for cases where chat.message is
          // delayed or missing (e.g. retry → busy).
          const statusType = props.status?.type;
          if (statusType === "busy") {
            hook("user-prompt-submit", { cwd, session_id, prompt: "" });
          } else if (statusType === "idle") {
            hook("stop", { cwd, session_id, last_message: "" });
          }
          return;
        }

        case "session.idle":
          hook("stop", { cwd, session_id, last_message: "" });
          return;

        case "session.error":
          hook("stop-failure", {
            cwd,
            session_id,
            error: errorMessage(props.error) || "session.error",
          });
          return;

        case "permission.asked":
          hook("notification", { cwd, session_id, wait_reason: "permission" });
      }
    },

    // Dedicated hook for tool execution results. `event` bus does not carry
    // tool.execute.* — those are surfaced only through these trigger hooks.
    "tool.execute.after": async (input, output) => {
      hook("activity-log", {
        cwd,
        session_id: input?.sessionID ?? "",
        tool_name: input?.tool ?? "",
        tool_input: input?.args ?? {},
        tool_response: {
          title: output?.title ?? "",
          output: output?.output ?? "",
          metadata: output?.metadata ?? null,
        },
      });
    },
  };
};
