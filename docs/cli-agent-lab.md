# CLI Agent Lab

Last updated: April 13, 2026

## Purpose

This is a standalone prototype for validating the minimum orchestration loop with `Claude Code` and `Codex CLI` before wiring the behavior into the main Project Commander runtime.

The lab is intentionally narrow. It answers these questions:

- can the local CLI be launched from a chosen repo or worktree path
- can Project Commander observe the provider's structured event stream
- can the provider expose a stable session or thread identifier that we can capture
- can we send a follow-up prompt against that same session or thread

It does **not** attempt to prove the full dispatcher architecture yet.

Out of scope for this lab:

- mailbox-driven worker coordination
- in-app xterm session hosting
- Claude mid-turn control
- Codex `app-server` steering
- approval and permission UX parity with the desktop app
- durable storage of runs across lab restarts

## What The Prototype Does

The lab server lives under `scripts/agent-lab/` and serves a small browser UI.

Current behaviors:

- probes local `claude` and `codex` availability
- launches either provider in a selected repo or worktree path
- streams stdout and stderr to the browser
- parses JSONL when available
- captures likely resume tokens:
  - Claude: `session_id`
  - Codex: `thread_id`
- lets the operator submit a follow-up prompt using the captured resume token
- records basic git context for the selected path:
  - whether it is inside a git repo
  - git root
  - current branch

## Run It

```powershell
npm run agent:lab
```

Then open the printed localhost URL in a browser.

Default starting path is the current Project Commander repo. Replace it with a specific disposable worktree path when you want to validate worktree-local behavior.

## Current Command Strategy

### Claude Code

Fresh run:

```powershell
claude --print --verbose --output-format stream-json --include-partial-messages ...
```

Resume run:

```powershell
claude --print --verbose --output-format stream-json --resume <session-id> ...
```

### Codex CLI

Fresh run:

```powershell
codex exec --json ...
```

Resume run:

```powershell
codex exec resume <session-id> --json ...
```

## How To Use It

Recommended first checks:

1. Point the lab at a real repo or git worktree path.
2. Start a minimal prompt like "Inspect the repo root, summarize the top-level folders, and stop."
3. Confirm that:
   - the run completes
   - structured events appear
   - a resume token is captured
4. Send a follow-up prompt like "Now inspect package.json and summarize the scripts."
5. Confirm the second run continues from the same provider session or thread.

## How To Interpret Results

Healthy result:

- launch works in the target worktree path
- the provider emits machine-readable events
- the provider exposes a stable resume token
- follow-up prompts continue the prior session or thread cleanly

Warning signs:

- no structured stream
- resume token missing or unstable
- resumed run forgets prior context
- provider requires interactive approval to do basic work in headless mode

Those warning signs mean Project Commander should not assume the provider is ready for richer dispatcher control yet.
