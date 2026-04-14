# Vault — High-Level Plan

Status: phased implementation in progress.

## Goal

Let users store secrets (API tokens, RSA keys, etc.) in Project Commander and make them usable by agents, CLI tools, and MCP servers — without any worker agent ever seeing the raw value. Built to be safe by default in a distributed product where users bring their own tools.

## Implementation snapshot

The currently shipped slice is the narrowest useful slice of this design:

- trusted settings-surface deposit/edit/delete
- backend-only Stronghold storage plus SQLite metadata
- append-only audit scaffolding for deposit/update/delete activity
- frontend invoke redaction for secret write calls
- launch-profile env bindings resolved by the supervisor at session start with scope checks and audit rows
- workflow-stage vault env bindings resolved at stage-session start through the same session-host path
- vault bindings may now declare `delivery=file`, which materializes an ephemeral per-session secret file and injects its path through the configured env var
- built-in HTTP-broker integration templates with supervisor-owned HTTPS requests, template-bound secret slots, and allowlisted egress domains
- persisted integration installs/bindings plus a Settings surface for configuring brokered templates without exposing values
- Project Commander MCP tooling for brokered HTTP integration calls so agent traffic can stay on the existing supervisor/broker path
- native vault permission prompts for `confirm_session` / `confirm_each_use`, with per-session approval caching for interactive launches and brokered integration calls
- PTY/session-output scrubbing for injected secret values before terminal buffers and output logs are updated
- no generic agent-readable `get_secret` API
- no generic forward proxy for arbitrary child-process egress or non-HTTP template execution yet

That slice is intentionally chosen so the app can start storing secrets safely before the remaining generic egress controls, richer template execution, and permission layers land.

Current launch contract:
- launch profiles may keep literal env vars in `env_json`
- any env var may instead use a vault binding object, for example `{"OPENAI_API_KEY":{"source":"vault","vault":"OpenAI Key","scopeTags":["openai:api"]}}`
- the supervisor resolves that binding at session start, injects the value into the child env, records an audit row, and scrubs that value from terminal/output artifacts
- bindings may add `"delivery":"file"` when the tool expects a temp file path in the env var rather than the raw secret value
- workflow stages and project overrides may also declare explicit vault env bindings; those bindings are frozen into the resolved run, validated against the stage/pod scope contract, merged into the launch request, and resolved/audited/scrubbed through the same session-start path
- bindings now also honor each vault entry's `gate_policy`: `auto` approves immediately, `confirm_session` prompts once per `(session, secret)` in interactive runs, `confirm_each_use` prompts every time, and `PC_VAULT_MODE=ci` auto-approves headless/test flows

## Threat model

In scope:
- Prompt injection from untrusted content (web pages, docs, issue bodies, CLI output).
- Accidental leakage into logs, diagnostics, crash dumps, conversation transcripts.
- Misbehaving or sloppy tools that echo credentials.

Out of scope:
- Local malware with filesystem or process-memory access while the app runs.
- Untrusted MCP servers or third-party agents (users won't install these).
- OS-level compromise.

## Core design decisions

1. **Backend: Stronghold only.** Single code path, cross-platform, portable blob. No OS keychain. No master password in v1 — at-rest protection is app-install-derived key. Document the trade-off.
2. **Default gate: confirm once per session.** Power users may downgrade to `auto` per-entry or upgrade to `confirm each use` for high-value secrets.
3. **Scope model: capability tags** (e.g. `jira:read`, `snowflake:query`). Brokered tools and integrations declare required tags. Vault entries grant tags.
4. **No template substitution to disk via app logic.** Secrets live in Stronghold, in supervisor memory during a call, or in child-process env / ephemeral-home temp file. Nowhere else.
5. **Rotation: manual in v1.** Metadata schema includes rotation hint fields so automation can be added later without migration.
6. **Headless/CI: `PC_VAULT_MODE=ci`** opts into auto-approve with full audit. Default is interactive, fails closed.

## Architecture

### Three layers in the supervisor (Rust, privileged)

- **Storage** — Stronghold-backed `VaultBackend` trait. Raw bytes in Stronghold; metadata (name, kind, scope tags, timestamps) in SQLite. `zeroize` on transient buffers.
- **Resolver** — only code path that touches raw values. Enforces scope tags. Releases values to: brokered tool calls, child-process env, ephemeral-home temp files. Never returns values over Tauri IPC to the frontend or to any agent.
- **Audit** — every release writes an audit row (`vault_id`, consumer, session, correlation ID, gate result). A dedicated diagnostics source can mirror that later; no value is ever stored in the audit payload.

### Three session-level defenses (all run every time)

1. **Scope check at spawn.** Session only receives secrets whose scope tags match the session's integrations. Cross-provider leakage impossible.
2. **stdio scrubber.** Child process stdout/stderr filtered for known vault values before hitting xterm, diagnostics, or crash artifacts. Replaces with `<vault:name>`.
3. **Egress allowlist (localhost HTTPS forward proxy).** Child processes get `HTTPS_PROXY` env pointing at a supervisor-owned proxy. Per-session domain allowlist derived from the session's active integrations. Default-deny. Load-bearing control.

## Integration framework (users bring their own tools)

Generic primitive: **integration template**. Declarative YAML/JSON describing:
- `kind`: `cli` | `mcp` | `http-broker`
- `secret_slots`: named secrets the integration needs, with delivery method (env var, stdin, ephemeral file path)
- `delivery`: env injection, stdin pipe, or ephemeral-home config file with placeholder substitution
- `egress`: domain allowlist

Three template sources:
- **Built-in catalog** (signed, bundled) — Snowflake CLI, gh, aws, common MCP servers.
- **Custom** — user-authored for internal tools, local-only.
- **Imported** (URL/file) — unverified; full permissions diff shown before install; no auto-update.

One supervisor primitive does all the work: `spawn_with_secrets(binary, args, vault_ids, scope_tags)` — handles env injection, temp-dir config files, stdio scrubber, cleanup on exit, proxy injection.

Current shipped brokered-template subset:
- built-in templates currently cover supervisor-owned `http-broker` requests rather than arbitrary `cli` or `mcp` launch templates
- template secret slots are injected only into supervisor-built request headers today
- requests are constrained to the template's configured HTTPS domains and redirects are disabled
- vault use is still audited through the existing append-only audit table with integration/slot identifiers only

## Deposit flow (the "dispatcher" pattern)

Dispatcher is the orchestrator agent — LLM-backed. It coordinates deposits but never receives secret values.

Flow:
- User asks dispatcher to add a secret.
- Dispatcher calls `open_vault_deposit({name, kind, scope_tags, integration})` tool.
- Supervisor opens a **native UI deposit surface** (modal / file-drop zone) outside the chat.
- User pastes or drops. Value flows UI → Tauri command → supervisor → Stronghold. Zeroed from transient memory.
- Dispatcher receives only `{status, vault_id}`. Its context window never holds the value.

Same deposit surface serves three entry points:
- User-initiated via Vault settings panel.
- Dispatcher-initiated via `open_vault_deposit` tool.
- Import flows (`.env` file, file drop).

Rate-limit dispatcher deposit requests per session to block modal-fatigue attacks.

## UX surface

- **Settings → Vault** panel: list, add, edit, delete, test-connection, rotate.
- **Integrations panel**: browse catalog, add custom, import; shows each integration's required secrets, scope tags, egress domains.
  Current status: a brokered integration section now lives under `Settings -> Vault` for built-in HTTP templates; custom/imported template management is still later work.
- **Deposit modal**: paste or file-drop; shows exact integration and scope the secret will be bound to. Never re-displays value after save.
- **Diagnostics**: new `vault` source; scrubber always on; audit events filterable.
- **Permission prompts**: "session X wants to use secret Y for integration Z" — Allow once / Allow for session / Deny. Default gate = per-session.
  Current status: shipped for interactive Windows launches and brokered HTTP integration calls; denied prompts fail closed and append audit breadcrumbs, while `PC_VAULT_MODE=ci` still auto-approves headless/test flows.

## What we explicitly won't build in v1

- Master password on Stronghold (design leaves it as an opt-in addition).
- Programmatic rotation (metadata fields reserved; no provider handlers).
- Template substitution that writes resolved secrets to persistent files.
- Community template registry with reviews (URL imports only, caveat emptor).
- Clipboard auto-clear, chat-input secret-pattern warnings (v1.1).

## Build order

1. **Vault core**: Stronghold storage, metadata model, resolver with scope tags, audit events, Settings UI for CRUD.
   Current status: foundation shipped.
2. **Ephemeral-home launcher**: generic `spawn_with_secrets` primitive with stdio scrubber.
   Current status: partially shipped through the existing session host for launch-profile env bindings, workflow-stage env bindings, ephemeral file-path delivery, and PTY scrubbing.
3. **Egress proxy**: localhost HTTPS forward proxy with per-session allowlist. Non-negotiable for v1 because users bring arbitrary tools.
   Current status: partially shipped through the supervisor-owned HTTP broker path for built-in templates; the generic child-process `HTTPS_PROXY` layer is still pending.
4. **Integration template schema + built-in catalog** (~5 templates: Snowflake CLI, gh, aws, one HTTP-broker example, one MCP example).
   Current status: partially shipped with a built-in HTTP-broker catalog plus persisted installs/bindings in `Settings -> Vault`; `cli`, `mcp`, custom, and imported templates are still pending.
5. **Dispatcher deposit flow**: `open_vault_deposit` tool + native deposit modal wired through the same Tauri command as the Settings UI.
6. **Custom integration UI**: user-authored templates with network-access prompt as the key security question.
7. **Template import** (URL/file) with diff review. Last, highest-risk surface.

## Open questions before building

- Exact tag vocabulary — start with `<provider>:<read|write|admin>`, expand as needed.
- `spawn_with_secrets` API signature and how it interfaces with the existing session host.
- Proxy implementation: pure Rust (`hyper`) vs. embedded existing crate. TLS handling: CONNECT-only (no MITM) is simpler and sufficient for allowlisting by SNI/host.
- Where the integration template catalog lives in the repo and how it's signed/bundled.
- Whether the Vault panel lives alongside or inside the existing Settings surface.

## Non-goals (for clarity)

- Not a password manager. Not for user-visible credentials they need to see again.
- Not a secret-distribution system across machines. Stronghold blob is per-install; users re-deposit on new machines (or copy the blob manually).
- Not an OAuth broker. If a tool needs OAuth refresh-token flow, we store the refresh token as a generic secret; the integration template handles the rest.
