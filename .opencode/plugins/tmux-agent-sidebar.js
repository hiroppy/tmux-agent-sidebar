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

// Fire-and-forget: OpenCode dispatches hooks without awaiting the returned
// promise, so serializing subprocess exits would only add latency without
// backpressure.
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

const sessionIdFrom = (props) =>
  pickFirstString(props, ["sessionID", "sessionId", "session_id"]);

// Shared server-event switch, used by both the V1 `server()` hook object and
// the V2 `ctx.event.subscribe()` loop below.
const handleServerEvent = (cwd, event) => {
  if (!event || !event.type) return;
  const props = event.properties ?? {};
  const session_id = sessionIdFrom(props);

  switch (event.type) {
    case "session.created":
      hook("session-start", { cwd, session_id, source: "startup" });
      return;

    case "session.status": {
      // Status is a union: { type: "idle" | "busy" | "retry", ... }.
      // `busy` is a secondary status-transition signal — the real prompt
      // text is written via the prompt hook, which fires with the admitted
      // user input. When both fire, the empty-prompt call here is a no-op
      // for @pane_prompt (handler guard) but still advances status to
      // "running" for cases where the prompt hook is delayed or missing
      // (e.g. retry → busy).
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
};

// V1 factory (OpenCode 1.x): returns the hook object. Kept as the named
// export it always was; V2 never calls it.
export const TmuxAgentSidebar = async ({ directory }) => {
  const cwd = typeof directory === "string" ? directory : "";

  return {
    "chat.message": async (input, output) => {
      const session_id =
        typeof input?.sessionID === "string" ? input.sessionID : "";
      const prompt = extractPromptText(output?.parts);
      hook("user-prompt-submit", { cwd, session_id, prompt });
    },

    event: async (payload) => handleServerEvent(cwd, payload?.event),

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

// ---------------------------------------------------------------------------
// V2 entrypoint (OpenCode 2.x): default export with `id` and `setup(ctx)`.
// Hooks are registered on the context instead of being returned.
// ---------------------------------------------------------------------------
export default {
  id: "tmux-agent-sidebar",

  async setup(ctx) {
    const cwd =
      typeof ctx.location?.directory === "string" ? ctx.location.directory : "";

    // V1 `chat.message` → V2 prompt admission hook. Runs once per user
    // prompt (synthetic/shell/compaction messages do not run it, matching
    // the old parts-based filtering for synthetic/ignored parts).
    await ctx.session.hook("prompt", (event) => {
      const session_id =
        typeof event.sessionID === "string" ? event.sessionID : "";
      const prompt =
        typeof event.prompt?.text === "string" ? event.prompt.text : "";
      hook("user-prompt-submit", { cwd, session_id, prompt });
    });

    // V1 `event` hook → public server event stream subscription. Abort it
    // from the cleanup function returned by setup.
    const controller = new AbortController();
    void (async () => {
      try {
        for await (const event of ctx.event.subscribe({
          signal: controller.signal,
        })) {
          handleServerEvent(cwd, event);
        }
      } catch {
        // Aborted on unload; nothing to report.
      }
    })();

    // V1 `tool.execute.after` → V2 tool hook. The `event` bus does not
    // carry tool execution results — only this hook does.
    await ctx.tool.hook("execute.after", (event) => {
      const result = event.status === "completed" ? event.result : undefined;
      const failure = event.status === "error" ? event.error : undefined;
      hook("activity-log", {
        cwd,
        session_id:
          typeof event.sessionID === "string" ? event.sessionID : "",
        tool_name: typeof event.tool === "string" ? event.tool : "",
        tool_input: event.input ?? {},
        tool_response: {
          title: result?.title ?? "",
          output: result?.output ?? (failure ? errorMessage(failure) : ""),
          metadata: result?.metadata ?? null,
        },
      });
    });

    return () => controller.abort();
  },

  // V1 entrypoint (OpenCode 1.18.29+). Ignored by V2; kept so a downgrade
  // to 1.x still loads. V1 calls `server()` and uses the returned hooks.
  server: TmuxAgentSidebar,
};
