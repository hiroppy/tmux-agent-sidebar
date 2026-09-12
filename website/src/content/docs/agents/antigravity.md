---
title: Antigravity CLI
description: Supported Antigravity CLI events and integration boundaries.
---

Antigravity CLI appears as `agy` in the sidebar. Its native plugin reports model
invocations, completed tools, and execution-loop termination.

Supported features include running/idle/error status, elapsed execution time,
tool activity, repository Git information, completion/failure notifications,
and launching `agy` in a sidebar-created worktree.

Unlike a user-prompt event, an invocation can happen many times in one run. The
sidebar preserves the timer across those invocations and waits for
`fullyIdle: true` before treating a successful stop as completion. Background
work stays `running`; there is no separate background-command label.

Prompt/response previews, permission-wait notifications, permission badges,
task progress, concurrent subagent attribution, and native worktree lifecycle
tracking are not supported.

See [Antigravity CLI setup](../../getting-started/antigravity/) for installation,
custom hook paths, configuration, and limitations.
