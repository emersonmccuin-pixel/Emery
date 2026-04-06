# Debug: Worktree Creation — `project_not_found` Error

**Date:** 2026-04-06  
**Branch:** `dev`  
**Goal:** User asked to spin up a worktree for manual work. Dispatcher could not resolve the `project_id` required by `emery_worktree_create`.

---

## Problem Summary

All worktree/session/merge-queue MCP tools require a `project_id` parameter. There is no `emery_project_list` or `emery_project_get` MCP tool exposed, so the dispatcher has no way to discover valid project IDs. Both `"emery"` and `"EURI"` were rejected with `project_not_found`.

---

## Tool Calls Attempted (in order)

### 1. `mcp__wcp__wcp_namespaces` — SUCCESS
- **Purpose:** Discover available namespaces per the dispatcher protocol.
- **Result:** Returned 27 namespaces. `EURI` namespace exists with 157 items and 90 documents.

### 2. `mcp__emery__emery_worktree_list(project_id: "emery")` — FAILED
- **Error:** `project_not_found: project emery was not found`
- **Note:** This was the first guess at project_id. The CLAUDE.md examples used generic `project_id` placeholders without specifying actual values.

### 3. `mcp__wcp__wcp_list(namespace: "EURI", status: "open")` — SUCCESS (0 results)
- **Purpose:** Find open work items to identify what to create a worktree for.
- **Result:** No items with status `open`.

### 4. `mcp__wcp__wcp_list(namespace: "EURI", status: "in_progress")` — SUCCESS
- **Result:** 5 items in progress:
  - EURI-183: Scratch — ad-hoc testing (spike)
  - EURI-182: Namespace-based work item scoping — replace project_id with namespace (feature, high)
  - EURI-152: Emery v1 polish (feature, high)
  - EURI-1: EURI v1 foundation (feature, urgent)
  - EURI-118: UX Pod — full UI audit (spike, high)

### 5. Attempted to query `app.db` via sqlite3 — FAILED
- **Purpose:** Look up project IDs directly in Emery's database.
- **Commands tried:**
  - `sqlite3 "$LOCALAPPDATA/Emery/app.db" ".tables"` — sqlite3 not found on PATH
  - PowerShell `Get-Command sqlite3` — not found
  - `find` for sqlite3.exe — not found
- **Note:** No sqlite3 binary available in this environment.

### 6. Grep for `project_not_found` in supervisor source — no matches
- **Pattern:** `project_not_found|project.get|project.list`
- **Path:** `crates/supervisor-core/src/**/*.rs`
- **Result:** No matches (the error string is likely constructed dynamically or in a different crate).

### 7. Grep for `"project"` in supervisor source — found project CRUD functions
- **Path:** `crates/supervisor-core/src/supervisor.rs`
- **Key findings:**
  - `list_projects()` at line 111
  - `get_project(project_id)` at line 119
  - `create_project(request)` at line 123
  - `list_project_roots(project_id)` at line 139
- **Path:** `crates/supervisor-core/src/store.rs`
  - `list_projects()` at line 129
  - `get_project(project_id)` at line 180
  - `project_slug_exists()` at line 109

### 8. Grep for RPC method names — found `project.list` and `project.get`
- **Pattern:** `"project\.(list|get)"`
- **Results:**
  - `crates/supervisor-ipc/src/protocol.rs:102` — `Self::ProjectList => "project.list"`
  - `crates/supervisor-ipc/src/protocol.rs:103` — `Self::ProjectGet => "project.get"`
  - MCP tools `instructions.rs` and `session.rs` call `rpc.call("project.get", ...)` internally

### 9. `ListMcpResourcesTool(server: "emery")` — no resources
- **Purpose:** Check if Emery exposes project info as MCP resources.
- **Result:** No resources found.

### 10. `ToolSearch(query: "+emery project")` — no project list/get tool
- **Result:** Returned 10 emery tools. None are `emery_project_list` or `emery_project_get`. The tools that exist all *require* `project_id` as input but none *return* project IDs.

### 11. `mcp__emery__emery_worktree_list(project_id: "EURI")` — FAILED
- **Error:** `project_not_found: project EURI was not found`
- **Note:** Tried the namespace key as project_id since EURI-182 is about replacing project_id with namespace. Still failed.

---

## Key Observations

1. **No discovery tool:** The MCP tool surface has no way to list or look up projects. The RPC layer has `project.list` and `project.get` methods (see `supervisor-ipc/src/protocol.rs:102-103`), but these are not exposed as MCP tools in `crates/emery-mcp/src/tools/`.

2. **EURI-182 is relevant:** Work item EURI-182 ("Namespace-based work item scoping — replace project_id with namespace") is `in_progress`. The updated CLAUDE.md (received mid-session) now shows `emery_work_item_list` and `emery_document_list` accept `namespace` as an alternative to `project_id`. But worktree/session tools still require `project_id`.

3. **The project_id value is opaque:** It's stored in `app.db` (SQLite), created via the Emery UI or `create_project` RPC. Could be a UUID, a slug, or something else. Without sqlite3 or an MCP tool to list projects, the dispatcher cannot discover it.

4. **knowledge.db vs app.db:** The user hinted that knowledge.db should have namespace-to-project linkage. The `emery_work_item_*` and `emery_document_*` tools accept `namespace` and internally may resolve to a project. But the worktree tools don't accept `namespace` — they only accept `project_id`.

---

## Critical Finding: knowledge.db Has the project_id, but app.db Doesn't

After the user pointed out that knowledge.db is accessible via `emery_work_item_list`, I queried it:

### `emery_work_item_list(namespace: "EURI", limit: 5)` — SUCCESS
- Returned 1 item: EURI-1 (a test item with title "test", status "backlog")
- **project_id on the work item:** `proj_3b62da6746904e9191887cb2f4568dd9`

### `emery_worktree_list(project_id: "proj_3b62da6746904e9191887cb2f4568dd9")` — FAILED
- **Error:** `project_not_found: project proj_3b62da6746904e9191887cb2f4568dd9 was not found`

### Conclusion
The project_id `proj_3b62da6746904e9191887cb2f4568dd9` exists in **knowledge.db** (attached to work items) but is **not registered in app.db** (where the supervisor looks up projects for worktree operations). The two databases have divergent project registries.

This means either:
1. The project was created in knowledge.db but never synced to app.db
2. The project existed in app.db but was deleted/recreated and the IDs drifted
3. The project registration in app.db uses a different ID scheme

### Also Notable
- WCP (`wcp_list`) returned 5 in-progress EURI items, but Emery's knowledge.db (`emery_work_item_list`) returned only 1 item (a test item). These are completely separate data stores.
- The `emery_work_item_list` tool accepts `namespace` directly (bypassing project_id), which works. But worktree tools only accept `project_id` and resolve it against app.db.

---

## Suggested Fix Directions

1. **Expose `emery_project_list` MCP tool** — wrap the existing `project.list` RPC method so dispatchers can discover project IDs.

2. **Accept `namespace` on worktree/session tools** — consistent with the EURI-182 migration direction. Resolve namespace → project_id internally in the MCP layer.

3. **Both** — expose project list for visibility, but also allow namespace as the primary identifier everywhere (the EURI-182 goal).

---

## Environment Notes

- Platform: Windows 11 Pro (win32)
- No `sqlite3` binary on PATH
- Emery prod data: `%LOCALAPPDATA%\Emery` (app.db, knowledge.db, worktrees/, sessions/)
- Emery dev data: `%LOCALAPPDATA%\Emery-Dev`
- MCP server: `emery` — exposes worktree, session, merge queue, vault, document, work item, and instruction tools
- MCP server: `wcp` — separate WCP server with its own namespace/work-item tools
