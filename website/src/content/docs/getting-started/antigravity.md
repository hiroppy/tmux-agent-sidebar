---
title: Antigravity CLI setup
description: Install the native agy plugin for tmux execution status and tool activity.
---

This integration targets **Antigravity CLI (`agy`) running inside tmux**, not the
desktop IDE or remote-control daemon. It uses native command hooks, with no
extra service or JavaScript runtime.

## Install the plugin

Install or build the sidebar binary first. From a source checkout, run
`cargo build --release`. Ensure tmux is using that build before toggling the
sidebar off and on: a separate TPM installation will still use its own binary,
and `bin/tmux-agent-sidebar` takes precedence over `target/release/`.

```sh
agy plugin validate ~/.tmux/plugins/tmux-agent-sidebar/plugins/antigravity
agy plugin install ~/.tmux/plugins/tmux-agent-sidebar/plugins/antigravity
```

Restart `agy` inside a tmux pane. Run `/hooks` to inspect loaded hooks and submit
a prompt. The sidebar registers the pane on its first model invocation, not
when the CLI initially opens. `agy plugin list` shows the installed plugin.

The plugin defaults to the sidebar's TPM location and uses `hook.sh` to locate
the binary. It does not bundle a second binary. The hook commands return harmless
JSON if the binary is missing or the process is outside tmux. They do not change
tool approval policy, inject messages, or force the agent to continue.

## Hooks fire but no agent appears

The hook process and the sidebar TUI must both run a binary with Antigravity
support. Pointing `TMUX_AGENT_SIDEBAR_HOOK` at a new checkout updates the hooks
only; it does not change the binary used to render the sidebar.

Check the configured sidebar binary:

```sh
sidebar_bin="$(tmux show-option -gqv @agent_sidebar_bin)"
"$sidebar_bin" setup agy
```

If this reports an unknown agent, the installed sidebar is too old. Install the
new build at that path (preserving the previous binary as a backup), or configure
tmux to launch the new build. Restart the sidebar afterward; a running process
does not load a replacement executable automatically. On macOS, a copied Cargo
binary must be re-signed with `codesign --force --sign - <binary-path>`.

To distinguish this from a hook failure, inspect pane metadata:

```sh
tmux list-panes -a -F '#{pane_id} #{pane_current_command} #{@pane_agent} #{@pane_status}'
```

An `agy` agent tag and status mean the hook has registered the pane. If it is
still absent from the UI, check the sidebar binary, restart it, and check the
active status/repository filters before reinstalling the Antigravity plugin.

## Custom installation paths

Install the plugin from your actual checkout and export its hook path before
starting `agy`:

```sh
agy plugin install /absolute/path/to/tmux-agent-sidebar/plugins/antigravity
export TMUX_AGENT_SIDEBAR_HOOK="/absolute/path/to/tmux-agent-sidebar/hook.sh"
agy
```

Alternatively, `tmux-agent-sidebar setup agy` prints a native `hooks.json` block
with the resolved absolute hook path. Its root is the named hook set
`tmux-agent-sidebar`, not Claude's `hooks` wrapper. This command only prints JSON;
it never modifies configuration. Avoid installing the same hooks both globally
and in a plugin, which would duplicate activity and notifications.

## Status and activity

| Native hook | Sidebar behavior |
| --- | --- |
| `PreInvocation` | Register `agy` and mark running; preserve the existing run timer across model calls |
| `PostToolUse` | Record completed tool calls, normalizing file, search, shell, and web tools |
| `Stop`, `fullyIdle: false` | Remain running while background tasks are active; no completion notification |
| `Stop`, `fullyIdle: true` | Mark idle and use the existing completion-notification settings |
| `Stop` with an error or step-limit exhaustion | Mark error and display the failure reason |
| CLI process exit | Clear stale pane metadata through process-based cleanup |

Select `agy` in the sidebar's spawn dialog to launch it in a new worktree. Only
the default launch mode is offered; no permission-bypass flags are added.

Customize its color with:

```text
set -g @sidebar_color_agent_agy 150
```

## Limitations

- No prompt/response previews or permission-wait detection: native hook payloads
  do not expose the corresponding data/events.
- No task progress, subagent tree, or native worktree-lifecycle hooks.
- Use one top-level conversation per pane. Concurrent/subagent hook attribution
  is not supported; the documented payload has no parent-conversation identifier.
- Multiple mounted workspaces are grouped using the first `workspacePaths` entry.
- Detached or remote agents that do not inherit the originating `TMUX_PANE` and
  tmux socket environment cannot be attributed to a local pane.

The integration follows the [native hook contract](https://antigravity.google/docs/hooks/)
and [CLI plugin layout](https://antigravity.google/docs/cli/plugins/). Plugin schema
validation and synthetic hook tests do not substitute for a live CLI session
when checking environment inheritance on a particular Antigravity version.
