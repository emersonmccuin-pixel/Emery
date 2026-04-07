# Librarian End-to-End Smoke Test Playbook

**Audience:** a fresh Claude Code session with no prior context on EMERY-226.
**Purpose:** validate that the high-bar session librarian (capture, gardener, feedback, metrics, tuning knobs) works end-to-end against a freshly-built supervisor binary.

This document is self-contained. Read it top to bottom before running anything.

---

## 1. What you are validating

EMERY-226 added a session librarian to Emery: it watches session activity, runs a triage → extract → critic → reconcile pipeline, writes durable memories with provenance, and offers a propose-only gardener for retiring stale memories. It exposes a feedback signal (`memory.flag`) where `noise` retires the memory immediately but never deletes the row — the audit trail is the point. Per-namespace tuning knobs (`triage_min_score`, `max_grains_per_run`, `gardener_cap_percent`, `gardener_cooldown_h`) live in `librarian_config` and are read at the start of every capture run.

The work landed across three commits on branch `emery/emery-226.001`:

- `16a5805` — EMERY-226.001 capture loop wiring
- `afbb99a` — EMERY-226.002 gardener + digest
- `5e15966` — EMERY-226.002 RPC + MCP wiring
- `ec826e7` — EMERY-226.003 feedback + metrics + tuning knobs

Worktree: `C:\Users\emers\AppData\Local\Emery\worktrees\emery-emery-226.001`
Production app data dir: `%LOCALAPPDATA%\Emery\` (knowledge.db lives there).

---

## 2. Preflight (do not skip)

```bash
cd "C:/Users/emers/AppData/Local/Emery/worktrees/emery-emery-226.001"
git log --oneline -5     # confirm ec826e7 is at the tip
cargo check -p supervisor-core -p supervisor-ipc -p emery-mcp -p emery-supervisor
```

If any check fails, stop and report — do NOT try to "fix" things. The branch was green when this doc was written.

---

## 3. In-process validation (highest leverage, do this first)

The librarian has full unit + integration coverage in `supervisor-core`. Run the librarian-scoped tests first — if these fail, the live smoke is moot.

```bash
cargo test -p supervisor-core --lib librarian
cargo test -p supervisor-core --lib service::tests::gardener
cargo test -p supervisor-core --lib service::tests::librarian
```

Expected: ~30+ tests pass, 0 fail. Key tests to verify by name in the output:

- `librarian::orchestrator::tests::full_pipeline_writes_one_memory`
- `librarian::orchestrator::tests::capture_loop_respects_triage_min_score`
- `librarian::orchestrator::tests::capture_loop_respects_max_grains_per_run`
- `librarian::feedback::tests::flag_noise_retires_memory_immediately`
- `librarian::feedback::tests::flag_does_not_delete_memory_row`
- `librarian::feedback::tests::metrics_breaks_down_by_prompt_version`
- `librarian::config::tests::validate_rejects_out_of_range_values`
- `service::tests::gardener_run_returns_existing_run_within_rate_limit`
- `service::tests::gardener_decide_refuses_double_decision`

If everything is green, the pipeline + curation + feedback layers are correct in isolation. Move on to step 4 to verify the binary actually serves these over RPC.

---

## 4. Live smoke against the worktree binary

The user is restarting the Tauri app to pick up the new supervisor. After the restart:

### 4a. Confirm the running supervisor is the new build

```bash
# Find the running emery-supervisor.exe and check its modified time.
# It should match the worktree target/debug build, not the old install.
```

The Tauri app bundles the supervisor; after the user restarts, the spawned `emery-supervisor.exe` should reflect the rebuilt binary. If you are unsure, ask the user to confirm rather than guessing.

### 4b. Sanity-check the schema migration ran

The new migration `0007_librarian_feedback.sql` adds two tables: `librarian_config` and `memory_feedback`.

```bash
# knowledge.db lives at %LOCALAPPDATA%\Emery\knowledge.db
sqlite3 "$LOCALAPPDATA/Emery/knowledge.db" ".tables" | grep -E "librarian_config|memory_feedback"
sqlite3 "$LOCALAPPDATA/Emery/knowledge.db" "SELECT version FROM knowledge_schema_versions ORDER BY version DESC LIMIT 5;"
```

You should see `librarian_config` and `memory_feedback` in the table list, and version `7` (or higher) at the top of the migration table. If migration 0007 did not run, the supervisor failed to bootstrap with the new code — stop and report.

### 4c. Exercise the new RPC surface via MCP tools

The MCP tool surface for the librarian is exposed by `emery-mcp` (already wired in `crates/emery-mcp/src/tools/librarian.rs` and `tools/mod.rs`). The Tauri client launches `emery-mcp.exe` as Claude Desktop's MCP server, so once the user has restarted, **the new tools will appear in the MCP tool list available to you in this very conversation** under the `mcp__emery__` prefix:

- `mcp__emery__emery_librarian_config_get`
- `mcp__emery__emery_librarian_config_set`
- `mcp__emery__emery_librarian_metrics`
- `mcp__emery__emery_librarian_digest`
- `mcp__emery__emery_memory_flag`
- `mcp__emery__emery_gardener_run`
- `mcp__emery__emery_gardener_review`
- `mcp__emery__emery_gardener_decide`

Run them in this order:

1. **Config defaults** — `mcp__emery__emery_librarian_config_get` with `namespace: "EMERY"`. Expect the four code defaults: `triage_min_score=1, max_grains_per_run=5, gardener_cap_percent=20, gardener_cooldown_h=24`.

2. **Config write + read-modify-write** — `mcp__emery__emery_librarian_config_set` with `namespace: "EMERY"`, `max_grains_per_run: 8`. Then read it back. Verify only `max_grains_per_run` changed; the other three fields are still defaults. This proves the supervisor-side read-modify-write in `rpc.rs` works.

3. **Metrics on a window** — `mcp__emery__emery_librarian_metrics` with `namespace: "EMERY"`, `since_days: 7`. Expect a JSON object with counts and ratios. Some ratios may be `null` if denominators are zero (no runs/feedback yet) — that's correct, not a bug. Verify `prompt_version_health` is an array (possibly empty).

4. **Digest** — `mcp__emery__emery_librarian_digest` with `namespace: "EMERY"`, `since_days: 7`, `include_dropped: true`. Expect kept-grain counts grouped by type plus dropped-candidate bodies. If you've never run a session under the new librarian, this will be empty — that's fine.

5. **Trigger a real capture** — ask the user to run an Emery session that does something memory-worthy (touch a file, make a decision). After the session ends, re-run the digest. You should now see at least one entry in the kept-grain counts. If nothing appears, check `%LOCALAPPDATA%\Emery\logs\` for `librarian.*` lines.

6. **Flag a memory as noise** — pick a `mem_…` ID from the digest output (or query `SELECT id FROM memories WHERE ns='EMERY' ORDER BY created_at DESC LIMIT 1;`). Call `mcp__emery__emery_memory_flag` with `memory_id: "<that id>"`, `signal: "noise"`, `note: "smoke test"`. Verify with SQL that `memories.valid_to` is now set on that row AND `memory_feedback` has a new row referencing it:

   ```sql
   SELECT id, valid_to FROM memories WHERE id = '<mem_id>';
   SELECT id, memory_id, signal, note FROM memory_feedback WHERE memory_id = '<mem_id>';
   ```

   Both must be true. The `memories` row must still exist (not deleted) — that's the audit guarantee.

7. **Gardener round-trip** — `mcp__emery__emery_gardener_run` with `namespace: "EMERY"`. It returns `{ run, proposals }`. Then `mcp__emery__emery_gardener_review` with the same namespace; the same proposals should appear in pending state. Pick one `gprp_…` ID and call `mcp__emery__emery_gardener_decide` with `decision: "reject"`. Re-run review — the proposal should no longer appear in the pending list. Verify SQL:

   ```sql
   SELECT id, decision FROM gardener_proposals WHERE id = '<gprp_id>';
   ```

   `decision` must be `'reject'`. Then call decide again on the same ID — it should return an error (idempotency guard).

8. **Re-check metrics** — `librarian.metrics` again. After the noise flag and the gardener round-trip, the counts should reflect them: `noise_flags >= 1`, `gardener_proposals_total >= 1`, `gardener_proposals_approved` matches your decisions.

If steps 1–8 all pass, the librarian is functional end-to-end.

---

## 5. Verification SQL cheat sheet

These are the queries to keep in your back pocket while running the live test. All against `%LOCALAPPDATA%\Emery\knowledge.db`.

```sql
-- All librarian-written memories in EMERY, newest first
SELECT id, type, substr(body, 1, 60), created_at, valid_to
FROM memories
WHERE ns = 'EMERY'
ORDER BY created_at DESC
LIMIT 20;

-- Capture runs (one per session activity batch)
SELECT id, namespace, status, prompt_versions, started_at, finished_at
FROM librarian_runs
ORDER BY started_at DESC
LIMIT 10;

-- Critic-dropped candidates
SELECT lc.id, lc.run_id, lc.kept, lc.drop_reason, substr(lc.body, 1, 60)
FROM librarian_candidates lc
JOIN librarian_runs lr ON lr.id = lc.run_id
WHERE lr.namespace = 'EMERY'
ORDER BY lc.id DESC
LIMIT 20;

-- Feedback rows (audit log; never deleted)
SELECT id, memory_id, signal, note, created_at
FROM memory_feedback
ORDER BY created_at DESC
LIMIT 20;

-- Gardener proposals
SELECT id, run_id, memory_id, decision, reason
FROM gardener_proposals
ORDER BY id DESC
LIMIT 20;

-- Per-namespace config
SELECT * FROM librarian_config;
```

---

## 6. Failure modes and what they mean

| Symptom | Likely cause | What to do |
|---|---|---|
| `mcp__emery__emery_librarian_*` tools missing from your MCP tool list | The Tauri client is still running the old `emery-mcp.exe` | Ask the user to fully quit and relaunch Emery |
| Migration 0007 not applied | Supervisor failed bootstrap; check `logs/supervisor.*.log` | Stop and report — do not manually patch the schema |
| `librarian_runs` table empty after a session | Session activity isn't being routed into the capture loop | Check `runtime.rs` activity hooks; do not "fix" by writing fake data |
| `memory.flag` returns `memory not found` | The memory_id was wrong, or it's in a different namespace | Re-query `memories` to confirm the ID |
| `gardener.run` returns the same proposals on a second call within 24h | Rate limit working as designed | Not a bug |
| `memories.valid_to` set on a flagged row but `memory_feedback` empty | Bug — feedback insert failed silently; investigate `feedback::record_feedback` |
| `memories` row is missing entirely after a noise flag | Bug — the audit guarantee is broken; investigate `expire_memory` |

The last two are the only ones that should be treated as code bugs. The rest are environmental and should be reported, not patched.

---

## 7. When you are done

If everything in section 4 passes, mark the smoke test complete and merge the branch via the merge queue. There is a pending task `#16` ("End-to-end smoke test the librarian") that should be set to completed.

If anything fails, do **not** start refactoring. Capture the failure (logs, SQL output, MCP tool response), set the task back to in_progress, and ask the user how to proceed. The librarian is the foundation for a lot of downstream work — it's worth being careful before declaring it done.
