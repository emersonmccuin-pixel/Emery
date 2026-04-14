# CLI Agent Orchestration Notes

Last updated: April 14, 2026

## Goal

Define a model-agnostic dispatcher/agent orchestration design that works with CLI-based agents, not API/SDK-native agents.

Primary target:

- dispatcher may be Claude, but worker agents may be Claude Code, Codex CLI, or Gemini CLI
- Project Commander remains the source of truth for broker state, session state, and recovery
- agents should be observable, resumable, and diagnosable
- user should not need to rely on a fragile visible terminal input box as the primary orchestration mechanism

## Current Project Commander Baseline

Project Commander already has a durable internal broker:

- canonical message storage: `agent_messages`
- MCP tools exposed to agents:
  - `send_message`
  - `get_messages`
  - `list_messages`
  - `wait_for_messages`
  - `reconcile_inbox`

Current code seams:

- DB-backed broker message and inbox queries:
  - `src-tauri/src/db.rs`
- MCP transport for agent broker access:
  - `src-tauri/src/supervisor_mcp.rs`
- supervisor HTTP routes for message send/list/inbox/wait/ack:
  - `src-tauri/src/bin/project-commander-supervisor.rs`
- session wrapper / PTY-root host:
  - `src-tauri/src/bin/project-commander-session-wrapper.rs`

Important conclusion:

- the broker is already model-agnostic at the data layer
- the portable contract is `agent_messages` plus `thread_id` / `reply_to_message_id` and blocking broker waits, not provider-native teammate/mailbox features

Current implementation status:

- Claude teammate mailbox delivery has been removed from the active runtime path.
- Claude CLI dispatcher sessions and SDK-backed workers now share the same Project Commander broker.
- Claude-backed runtimes now bind Project Commander MCP over the supervisor's local HTTP `/mcp` endpoint instead of asking Claude to spawn a local stdio helper.
- Codex SDK worker hosts now use the same broker contract and attach Project Commander MCP through the supervisor's `mcp-stdio` helper command.
- SDK worker hosts wait on `/message/wait` instead of polling the inbox on a timer.
- `/message/wait` is broker-notified inside the supervisor process, so waiting sessions wake on actual broker traffic rather than fixed sleep intervals.
- SDK worker hosts can now publish `ready` / `busy` / `idle` / `completed` / `failed` lifecycle markers through supervisor-backed `publish_status`, `heartbeat`, and `mark_done` tools instead of relying only on terminal text.
- Claude Agent SDK launches now also record which auth seam they used (`dedicated_config_dir` vs `default_home`) so the personal-Claude path is visible in diagnostics and crash recovery.
- Future providers should implement the same broker contract instead of adding a second message system.

## Core Constraint

For CLI-only orchestration, a running agent does not automatically "listen" for new messages unless one of these is true:

1. the CLI exposes a real remote control or structured session input channel
2. the agent actively polls or blocks on mailbox tools
3. Project Commander injects a new turn into the running session host
4. the agent is relaunched with queued directives in startup context

There is no universal cross-CLI native inbox.

## Working Architecture

Build around a Project Commander session host, not around provider-specific terminal behavior.

### 1. Source Of Truth

Project Commander owns:

- broker persistence
- queue ordering
- session state
- busy / idle / completed / failed state
- recovery and crash evidence

### 2. Universal Agent Protocol

All agents should use the same Project Commander MCP contract.

The broker wait and worker-lifecycle primitives are now in place:

- `wait_for_messages(timeoutMs)`
- `publish_status(state, detail?)`
- `heartbeat()`
- `mark_done(summary, contextJson?)`

Desired agent contract:

- on startup: reconcile inbox, load instructions, publish `ready`
- during work: publish status transitions
- when blocked: send `blocked`
- when complete: send `complete` and publish `done`

### 3. Session Host

Each CLI process should run behind a Project Commander host/wrapper.

The host is responsible for:

- spawning the CLI
- capturing stdout/stderr or PTY output
- detecting prompt-idle vs busy when possible
- queueing inbound dispatcher directives
- deciding when queued messages can be delivered safely
- handling crashes without losing session context

### 4. Delivery Policy

Use a tiered strategy, not one mechanism for every case.

- if provider supports structured remote/session input: use that
- else if agent is idle and host has high confidence: inject the directive as a new turn
- else leave queued and block on `wait_for_messages`
- else relaunch/resume with pending directives in startup context

## Provider Snapshot

This section is a planning snapshot, not a final capability matrix.

### Claude Code

Known promising areas to leverage:

- MCP support
- hooks / notification events
- `--remote-control`
- print mode with structured streaming formats

Likely role in architecture:

- strongest CLI candidate for richer session-host integration
- may support a better-than-PTY last-mile delivery path

### Gemini CLI

Known promising areas to leverage:

- MCP support
- non-interactive/headless JSONL event output
- structured event stream may help with busy/done detection

Likely role in architecture:

- strong observability candidate
- may still need relaunch or checkpoint-based delivery for queued directives

### Codex CLI

Known confirmed areas:

- MCP support
- AGENTS.md steering
- SDK-backed worker hosting is now viable in Project Commander through `@openai/codex-sdk` plus the supervisor-owned broker contract.
- Because the current Codex SDK thread API does not expose a separate system/developer instruction channel, Project Commander worker hosts must prepend the bridge instructions to each dispatcher-driven turn instead of burning an autonomous bootstrap turn before inbox wait.

Still needs deeper confirmation:

- whether there is an official structured event stream for a long-lived session host
- whether there is any remote-control or append-turn capability
- whether busy/done detection can be made reliable without prompt heuristics

Likely role in architecture today:

- assume broker + checkpoints + host-managed lifecycle unless stronger official capabilities are confirmed

## Preferred End State

Project Commander should expose a provider-neutral orchestration layer:

- `BrokerStore`
- `SessionHost`
- `DeliveryAdapter`
- `ProviderCapabilities`

Suggested adapter split:

- `ClaudeAdapter`
- `CodexAdapter`
- `GeminiAdapter`

Each adapter should answer:

- can receive push while live?
- can expose structured busy/done states?
- can expose permission-needed states?
- can resume same conversation?
- can accept queued directives only at prompt?

## State Machine

Supervisor should own a real session state machine, independent of UI wording.

Suggested states:

- `launching`
- `ready`
- `busy`
- `waiting_for_tool`
- `waiting_for_permission`
- `idle`
- `completed`
- `failed`
- `shell_fallback`

Queued-message policy:

- flush immediately only in `ready` or `idle`
- never auto-inject during `busy`
- on `completed`, either deliver next directive or terminate session based on policy
- on `failed`, keep queue intact and offer recovery path

## What To Research Next

### Capability Verification

- Claude Code:
  - exact behavior of `--remote-control`
  - whether it can append user turns to a live local session
  - which hooks fire on done / waiting / permission / question
  - whether hook output can safely drive Project Commander state transitions
- Gemini CLI:
  - exact headless/session semantics
  - whether headless mode can be kept long-lived for multi-turn orchestration
  - whether queued directives can be fed without full relaunch
- Codex CLI:
  - official docs for long-lived session hosting
  - any structured event stream
  - any remote-control capability
  - how to determine done / waiting / blocked without brittle terminal parsing

### Host Design

- do we keep PTY for all providers, or support non-PTY host modes per provider?
- do we model a hidden terminal plus structured viewer, or fully move to event-driven rendering?
- how do we persist host-level state transitions in diagnostics?

### Queue Semantics

- should directives stack or collapse?
- should only one active directive per agent exist at a time?
- should dispatcher be able to mark a queued message as urgent/preemptive?
- when an agent reports `complete`, should the next directive auto-dispatch?

### User Experience

- should users see raw terminals, a structured transcript view, or both?
- should injected directives appear as synthetic user messages in the transcript?
- how should the app explain queued vs delivered vs acknowledged?

## Recommended Build Order

1. Formalize provider capability matrix in code and docs.
2. Add host-owned session state machine and diagnostics events.
3. Add mailbox primitives:
   - `wait_for_messages`
   - `publish_status`
   - `heartbeat`
4. Move delivery behind provider adapters.
5. Make Claude adapter richer if `remote-control` or hooks support it cleanly.
6. Make Codex/Gemini work reliably with queue + checkpoint + relaunch, even if push remains weaker.
7. Only after that, revisit hidden-terminal or fully structured UI.

## Immediate Design Guidance

- do not tie orchestration correctness to Claude teammate mailbox or any provider-native inbox
- do not assume mid-turn push exists across providers
- do not assume PTY prompt parsing alone is reliable enough to be the only state signal
- prefer explicit agent MCP signals over output heuristics wherever possible
- keep Project Commander as the durable broker even when provider-native features exist

## External Docs To Revisit

- Claude Code CLI docs
- Claude Code hooks docs
- Claude Code remote-control docs
- OpenAI Codex MCP / CLI docs
- Gemini CLI docs
- Gemini CLI headless mode docs

This document is intentionally a handoff note for a future implementation session, not a final architecture decision.
