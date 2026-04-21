import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";

const resolveHookScript = () => {
  let dir = import.meta.dir;
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

const hook = async (eventName, payload) => {
  try {
    const hookScript = resolveHookScript();
    const command = hookScript
      ? ["bash", hookScript, "opencode", eventName]
      : ["tmux-agent-sidebar", "hook", "opencode", eventName];
    const proc = Bun.spawn(command, {
      stdin: new Response(JSON.stringify(payload)),
    });
    await proc.exited;
  } catch {
    // OpenCode should keep running even if the bridge is missing or
    // the sidebar binary is unavailable.
  }
};

const asString = (value) => (typeof value === "string" ? value : "");

const pickFirstString = (value, keys) => {
  for (const key of keys) {
    const candidate = value?.[key];
    if (typeof candidate === "string" && candidate) {
      return candidate;
    }
  }
  return "";
};

const toolPayload = (event) => {
  const properties = event?.properties ?? {};
  const toolName = pickFirstString(properties, [
    "tool_name",
    "toolName",
    "tool",
  ]);
  const toolInput = properties.input ?? properties.args ?? properties.tool_input ?? {};
  const toolResponse = properties.result ?? properties.output ?? properties.tool_response ?? {};
  return {
    tool_name: toolName,
    tool_input: toolInput,
    tool_response: toolResponse,
  };
};

const sessionId = (event) =>
  pickFirstString(event?.properties ?? {}, ["sessionID", "sessionId", "session_id"]);

export const TmuxAgentSidebar = async ({ directory }) => {
  const cwd = asString(directory);

  return {
    event: async ({ event }) => {
      if (!event || !event.type) {
        return;
      }

      if (event.type === "session.created") {
        await hook("session-start", {
          cwd,
          session_id: sessionId(event),
          source: pickFirstString(event.properties ?? {}, ["source"]) || "startup",
        });
        return;
      }

      if (event.type === "session.status") {
        const status = pickFirstString(event.properties ?? {}, ["status"]);
        if (status === "active") {
          await hook("user-prompt-submit", {
            cwd,
            session_id: sessionId(event),
            prompt: pickFirstString(event.properties ?? {}, ["prompt"]),
          });
        } else if (status === "idle") {
          await hook("stop", {
            cwd,
            session_id: sessionId(event),
            last_message: "",
          });
        } else if (status === "error") {
          await hook("stop-failure", {
            cwd,
            session_id: sessionId(event),
            error: "session.status=error",
          });
        }
        return;
      }

      if (event.type === "session.idle") {
        await hook("stop", {
          cwd,
          session_id: sessionId(event),
          last_message: "",
        });
        return;
      }

      if (event.type === "session.error") {
        await hook("stop-failure", {
          cwd,
          session_id: sessionId(event),
          error: pickFirstString(event.properties ?? {}, ["error", "message"]) || "session.error",
        });
        return;
      }

      if (event.type === "permission.asked") {
        await hook("notification", {
          cwd,
          session_id: sessionId(event),
          wait_reason: "permission",
        });
        return;
      }

      if (event.type === "tool.execute.after") {
        await hook("activity-log", {
          cwd,
          session_id: sessionId(event),
          ...toolPayload(event),
        });
      }
    },
  };
};
