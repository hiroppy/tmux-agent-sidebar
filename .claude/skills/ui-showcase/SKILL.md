---
name: ui-showcase
description: Display all agent pane UI elements simultaneously in the sidebar for visual verification. Trigger on "UI check", "showcase", "UI demo", "show all patterns", "verify panel display", "UIを確認したい", "showcaseして". Use after UI changes to visually confirm rendering.
---

# UI Showcase — Display All Agent Pane Elements Simultaneously

Trigger all UI elements within the current Claude Code session so the sidebar's agent pane displays everything at once for visual verification.

## UI Elements and How to Trigger Them

Execute in **this order**. Once all steps complete, every element is visible in the sidebar simultaneously.

### 1. Port Display

Start a background HTTP server via Bash tool. The sidebar detects it through port scanning.

```bash
python3 -m http.server 9876 &
```

### 2. Task Progress Display

Create 3 tasks with TaskCreate, then set each to a different status to show all icon types (✔◼◻):

- Task 1: create → TaskUpdate to completed (✔)
- Task 2: create → TaskUpdate to in_progress (◼)
- Task 3: create only → stays pending (◻)

### 3. Subagent Display

Launch 2 subagents in parallel via Agent tool with `run_in_background: true`.
The sidebar shows ├ / └ tree visualization while they run.

Give each subagent a time-consuming task. For example:
- subagent 1: Read all files in `src/ui/` and list every public function/struct
- subagent 2: Read all files in `tests/` and list every test function name

### 4. Ask User to Verify

Once all elements are active, ask the user to check the sidebar.
Subagents remain visible until they complete, providing enough time for inspection.

## Verification Checklist

After execution, confirm the following are all visible in the sidebar:

- [ ] Status icon (● Running)
- [ ] Agent name (claude)
- [ ] Elapsed time (XmXs on the right)
- [ ] Branch name (auto-displayed when inside a git repo)
- [ ] Port number (:9876)
- [ ] Task progress (✔◼◻ X/Y)
- [ ] Subagent tree (├ / └)
- [ ] Prompt text
- [ ] Activity log (Bash, TaskCreate, Agent etc. in the Activity tab)

## Elements That Cannot Be Shown Simultaneously

These are mutually exclusive states, so they are out of scope for this skill:

- **Wait reason** (permission required, etc.) — not shown while Running
- **Response display** (▶ arrow) — only shown in Idle state
- **Error status** (✕) — mutually exclusive with Running
- **Permission mode badge** — depends on Claude Code launch options (no badge in default mode)
- **Worktree indicator** (+ name: branch) — requires launching Claude Code inside a git worktree

## Cleanup

Stop the HTTP server after verification:

```bash
kill $(lsof -ti:9876) 2>/dev/null
```
