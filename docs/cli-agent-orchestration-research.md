# CLI Agent Orchestration — Provider Research

April 13, 2026

Raw capability findings for Claude Code, Gemini CLI, and Codex CLI as orchestration targets.

---

## Claude Code

### Fire-and-Forget Execution

```bash
claude -p "prompt" --output-format stream-json
```

JSONL on stdout. Events: `stream_event` (text deltas, tool use), `system` (retries), `hook_event`. Exit code = final status.

Other output formats: `text` (default), `json` (single object with `session_id`, `result`, `usage`).

Structured output: `--json-schema '{...}'` constrains response shape.

```bash
# auto-approve specific tools
claude -p "Run tests" --allowedTools "Bash,Read"

# bare mode — skip hooks, MCP discovery, CLAUDE.md, auto-memory
claude --bare -p "prompt"

# budget caps
claude -p "Refactor" --max-turns 3 --max-budget-usd 2.00

# disable session persistence
claude -p "One-off task" --no-session-persistence
```

### Session Resume

```bash
claude --continue              # most recent session in cwd
claude -c -p "Follow up"      # headless continuation
claude --resume "session-id"   # by ID
claude --resume "name"         # by name
claude --resume "id" --fork-session  # branch into new session
claude --session-id "uuid"     # assign custom ID
```

Full conversation history restored. MCP state carried forward.

### MCP Integration

```bash
claude --mcp-config ./mcp.json           # per-session
claude --strict-mcp-config --mcp-config ./mcp.json  # ignore all other MCP configs
```

Config scopes: `.mcp.json` (project), `~/.claude/.mcp.json` (user), `--mcp-config` (session override).

Tool namespace: `mcp__<server>__<tool>`.

Elicitation protocol: MCP servers can request user input during tool calls. Hook events `Elicitation` and `ElicitationResult` fire.

### Hooks (26 Events)

Types: `command` (shell), `http` (POST), `prompt` (single-turn LLM), `agent` (multi-turn subagent).

All synchronous and blocking. Multiple matching hooks run in parallel. Most restrictive decision wins.

**Orchestration-relevant hooks:**

| Hook | Fires When |
|---|---|
| `Stop` | Claude finishes a turn (every turn, not just final) |
| `Notification` | Claude needs input or permission |
| `TeammateIdle` | Agent teammate about to idle |
| `PreToolUse` / `PostToolUse` | Before/after tool execution |
| `SubagentStart` / `SubagentStop` | Subagent lifecycle |
| `SessionEnd` | Session terminates |
| `PermissionRequest` | Permission dialog appears |

Hook stdin receives JSON with `session_id`, `hook_event_name`, tool details, etc.

Hook output options:
- Exit code 0 = allow, exit code 2 = block (stderr = reason)
- JSON with `additionalContext` to inject text into Claude's context
- JSON with `permissionDecision`: `deny`, `allow`, `ask`, `defer`

Timeout: 10 min default, configurable per hook.

### Remote Control

```bash
claude --remote-control "Name"   # interactive + remote
claude remote-control            # server mode, multiple connections
```

Routes through Anthropic API over TLS. Controllable from claude.ai/code or mobile app. **Not locally controllable by an orchestrator** — messages route through Anthropic infrastructure, not a local protocol.

Server mode flags: `--spawn worktree|session`, `--capacity N`.

### Mid-Session Injection

**Not possible.** No local API to append a user turn to a running interactive session. The only paths are:
- Hooks injecting `additionalContext` on existing events
- MCP tools writing to files/DB that the agent checks
- Kill and resume with `-c -p "next directive"`

### Other Flags

```bash
claude --system-prompt "..."           # override system prompt
claude --append-system-prompt "..."    # append to system prompt
claude --worktree feature-auth         # git worktree isolation
claude --tools "Bash,Edit,Read"        # restrict available tools
claude --permission-mode auto          # auto-approve everything
```

Permission modes: `default`, `acceptEdits`, `plan`, `auto`, `dontAsk`, `bypassPermissions`.

---

## Gemini CLI

### Fire-and-Forget Execution

```bash
gemini -p "prompt" --output-format stream-json
```

JSONL events:

| Event | Schema |
|---|---|
| `init` | `{type, timestamp, session_id, model}` |
| `message` | `{type, timestamp, role, content, delta}` |
| `tool_use` | `{type, timestamp, tool_name, tool_id, parameters}` |
| `tool_result` | `{type, timestamp, tool_id, status, output/error}` |
| `error` | `{type, timestamp, severity, message}` |
| `result` | `{type, timestamp, status, stats{total_tokens, input_tokens, output_tokens, ...}}` |

Exit codes: 0=success, 1=error, 42=input error, 53=turn limit.

Other output formats: `text` (default), `json` (single object).

```bash
# auto-approve all tool calls
gemini -p "prompt" --approval-mode yolo

# headless auto-detection
CI=true gemini -p "prompt"
```

Headless mode auto-denies the `ask_user` tool — agent can't block on user input.

### Session Resume

```bash
gemini --resume              # most recent
gemini -r latest
gemini -r 1                  # by index
gemini -r <uuid>             # by ID
gemini -r <id> -p "next"    # headless continuation
```

Sessions persist to `~/.gemini/tmp/<project_hash>/chats/`.

Named checkpoints within sessions:
```
/resume save decision-point
/resume list
/resume resume decision-point
```

Retention config in `settings.json`: `sessionRetention.maxAge`, `sessionRetention.maxCount`.

File-level checkpointing (optional): shadow git commits before modifications, `/restore` to revert.

### MCP Integration

Config in `~/.gemini/settings.json` or `<project>/.gemini/settings.json`:

```json
{
  "mcpServers": {
    "myServer": {
      "command": "node",
      "args": ["./server.js"],
      "env": { "KEY": "$ENV_VAR" },
      "timeout": 30000,
      "trust": false
    }
  }
}
```

Transports: stdio, SSE, streamable HTTP.

CLI management: `gemini mcp add/remove/list`.

Per-server: `includeTools`/`excludeTools` allowlists, `trust: true` bypasses confirmation.

MCP Resources supported: `@server://resource/path` syntax in prompts.

Runtime restriction: `--allowed-mcp-server-names` limits active servers per invocation.

### ACP Mode (Bidirectional Control)

```bash
gemini --acp
```

**JSON-RPC 2.0 over stdio.** This is the primary orchestration interface.

Methods:
- `initialize` — handshake, orchestrator can expose its own MCP server to Gemini
- `newSession` — start fresh
- `loadSession` — resume prior session
- `prompt` — send a user turn
- `cancel` — cancel in-flight turn
- `setSessionMode` — switch modes
- `unstable_setSessionModel` — switch model mid-session

Bidirectional MCP: during `initialize`, the ACP client (orchestrator) can register an MCP server that Gemini can call back into. Agent calls orchestrator tools during execution.

This is the correct integration point for multi-turn programmatic control. Headless `-p` is for single-prompt fire-and-forget.

### Other Features

```bash
gemini --model pro|flash|auto|gemini-2.5-pro  # model selection
gemini --worktree                              # git worktree (experimental)
gemini --sandbox                               # sandboxed execution
gemini --debug                                 # verbose stderr logging
```

Instructions file: `GEMINI.md` (equivalent to CLAUDE.md).

Turn limits: `model.maxSessionTurns` in settings.json.

Extensions: `gemini extensions install <source>` — plugins from git repos or local paths.

Policy engine: TOML-based tool approval policies, `--policy` flag.

---

## Codex CLI

### Fire-and-Forget Execution

```bash
codex exec "prompt" --json
```

JSONL events:

| Event | Description |
|---|---|
| `thread.started` | First event, contains `thread_id` |
| `turn.started` | Model processing begins |
| `turn.completed` | Turn done, includes `usage` (input/cached/output tokens) |
| `turn.failed` | Turn failed with `error.message` |
| `item.started` | New item in progress |
| `item.updated` | Item state changed |
| `item.completed` | Item reached terminal state |
| `error` | Unrecoverable stream error |

Item types: `agent_message`, `reasoning`, `command_execution` (command, output, exit_code, status), `file_change` (path, add/delete/update), `mcp_tool_call` (server, tool, arguments, result), `collab_tool_call`, `web_search`, `todo_list`, `error`.

```bash
codex exec "prompt" --full-auto          # sandbox + auto-approve
codex exec "prompt" --yolo               # no sandbox, no approvals
codex exec "prompt" --ephemeral          # no session persistence
codex exec "prompt" --output-schema f.json  # constrained output
codex exec "prompt" -o result.md         # write final message to file
```

Default: only final message to stdout. `--json`: all events to stdout, everything else to stderr.

### Session Resume

```bash
codex resume <session-id>        # interactive TUI
codex resume --last              # most recent
codex exec resume <id>           # headless resume
codex exec resume --last         # headless, most recent
codex fork <id>                  # branch into new session
```

Sessions persist to `~/.codex/sessions/`. `--ephemeral` disables.

Via app-server API: `thread/start`, `thread/resume`, `thread/fork`, `thread/list`, `thread/read`, `thread/archive`, `thread/rollback` (drop last N turns), `thread/compact/start` (compress history).

### MCP Integration

**Client and server.**

As client, config in `~/.codex/config.toml`:

```toml
[mcp_servers.myserver]
command = "node"
args = ["./server.js"]
env = { KEY = "..." }

[mcp_servers.myserver.tools.search]
approval_mode = "approve"
```

Supports stdio and streamable HTTP. Per-tool approval overrides. `enabled_tools`/`disabled_tools` allowlists. OAuth support.

CLI: `codex mcp add/remove/list`.

**As MCP server:** `codex mcp-server` runs Codex as a stdio MCP server. Exposes `codex` (start thread + turn) and `codex-reply` (continue) tools. External MCP clients can drive full sessions.

### App-Server (Bidirectional Control)

```bash
codex app-server --listen stdio://       # JSON-RPC over stdin/stdout
codex app-server --listen ws://127.0.0.1:PORT  # websocket
```

**JSON-RPC v2.** Key methods:

| Method | Description |
|---|---|
| `turn/start` | Send a new user turn |
| `turn/steer` | **Inject into a running turn** |
| `turn/interrupt` | Cancel in-flight turn |
| `thread/start` | Create new session |
| `thread/resume` | Resume prior session |
| `thread/fork` | Branch session |
| `thread/inject_items` | Append raw items to history without triggering a turn |
| `thread/rollback` | Drop last N turns |
| `thread/compact/start` | Compress conversation history |

`turn/steer` is unique — true mid-execution injection. Not just between turns, during active work.

`thread/inject_items` plants context silently — useful for orchestrator-provided updates.

Auth: capability tokens or signed bearer tokens.

**SDKs:**
- TypeScript: `@openai/codex-sdk` — spawns `codex exec --json`, JSONL over stdin/stdout
- Python: `codex-app-server-sdk` — spawns `codex app-server`, drives via JSON-RPC

### AGENTS.md

Hierarchical instruction injection. Files at any directory level, deeper overrides shallower. Always lower priority than explicit system/developer prompts.

### Built-In Multi-Agent

Collab tools: `spawn_agent`, `send_input`, `wait`, `close_agent`. Max 6 threads default. Agents fork context from parent. Configurable depth limits.

### Other Features

```bash
codex exec "prompt" -m <model>           # model selection
codex exec "prompt" --sandbox workspace-write  # sandboxed writes
codex exec "prompt" --profile <name>     # config profile from config.toml
codex exec "prompt" --cd <dir>           # working directory
codex exec "prompt" --add-dir <dir>      # additional writable dirs
codex exec "prompt" --skip-git-repo-check
```

Hooks: `session-start.command` with JSON input/output schemas. Notify hook fires on turn completion.

Sandbox: Seatbelt (macOS), Landlock (Linux), restricted token (Windows).

Feature flags: `--enable <FEATURE>` / `--disable <FEATURE>`.

Environment: `CODEX_API_KEY`, `CODEX_HOME`, `CODEX_SQLITE_HOME`, `LOG_FORMAT=json`, `RUST_LOG`.

---

## Comparative Matrix

| Capability | Claude Code | Gemini CLI | Codex CLI |
|---|---|---|---|
| Fire-and-forget | `claude -p` + stream-json | `gemini -p` + stream-json | `codex exec --json` |
| Session resume | `--resume <id>` | `-r <id>` | `exec resume <id>` |
| Headless continuation | `claude -c -p "next"` | `gemini -r <id> -p "next"` | `codex exec resume <id>` |
| Bidirectional control | **No** | **Yes** — ACP (JSON-RPC/stdio) | **Yes** — app-server (JSON-RPC/stdio+ws) |
| Mid-turn injection | **No** | **No** (cancel + re-prompt) | **Yes** — `turn/steer` |
| History injection | **No** | **No** | **Yes** — `thread/inject_items` |
| Cancel in-flight | **No** | **Yes** — `cancel` | **Yes** — `turn/interrupt` |
| MCP client | Yes | Yes | Yes |
| MCP server | No | No | **Yes** — `codex mcp-server` |
| Structured JSONL events | Yes | Yes (typed) | Yes (rich item types) |
| Hook/callback system | **26 events, 4 hook types** | Policy engine | session-start + notify |
| Auto-approval | `--permission-mode` | `--approval-mode yolo` | `--full-auto` / `--yolo` |
| Instructions file | CLAUDE.md | GEMINI.md | AGENTS.md |
| Worktree isolation | `--worktree` | `--worktree` (experimental) | sandbox modes |
| Built-in multi-agent | Subagents + teammates | No | Collab tools |
| System prompt override | `--system-prompt` | No | No (AGENTS.md only) |
| Structured output schema | `--json-schema` | No | `--output-schema` |
| Budget/turn caps | `--max-turns`, `--max-budget-usd` | `maxSessionTurns` | No explicit cap |
