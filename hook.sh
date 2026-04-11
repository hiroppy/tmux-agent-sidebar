#!/usr/bin/env bash
# Thin wrapper: delegates to the Rust binary. Called by Claude Code /
# Codex hooks (settings.json).
#
# Why this file exists even though `tmux-agent-sidebar doctor` can emit
# absolute binary paths:
#
# 1. Late binding. settings.json only needs to know where `hook.sh`
#    lives. The actual binary is resolved fresh on every hook fire, so
#    the user can move or rebuild the binary (bin/ ↔ target/release/,
#    relocate the plugin dir, swap install methods) without having to
#    regenerate their agent config. Without this indirection, any
#    doctor-generated path becomes a stale snapshot the moment the
#    binary moves.
#
# 2. Graceful absence. If the binary is missing — during a rebuild,
#    mid-uninstall, or on a fresh clone before `cargo build` — this
#    script exits 0 silently, so the agent session never sees a hook
#    failure. A direct binary invocation would surface "no such file"
#    errors into the user's workflow.
#
# Keep this wrapper small and side-effect-free. Any logic that needs to
# know event semantics belongs in the Rust `hook` subcommand.
PLUGIN_DIR="$(cd "$(dirname "$0")" && pwd -P)"
if command -v tmux-agent-sidebar &>/dev/null; then
  BIN="tmux-agent-sidebar"
elif [ -x "$PLUGIN_DIR/bin/tmux-agent-sidebar" ]; then
  BIN="$PLUGIN_DIR/bin/tmux-agent-sidebar"
elif [ -x "$PLUGIN_DIR/target/release/tmux-agent-sidebar" ]; then
  BIN="$PLUGIN_DIR/target/release/tmux-agent-sidebar"
else
  exit 0
fi
exec "$BIN" hook "$@"
