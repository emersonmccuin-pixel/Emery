# Debug: "Create a worktree" session trace

**Date:** 2026-04-06
**Goal:** User asked to create a worktree. I needed to show available work items so they could pick one.

---

## Step 1: Fetch tool schemas

**Tool:** `ToolSearch` — fetched schemas for `emery_work_item_list` and `emery_worktree_create`.

No issues here.

---

## Step 2: List backlog work items

**Tool:** `emery_work_item_list(namespace: "EMERY", status: "backlog")`

**Result:** Error — result was **137,232 characters**, exceeding the MCP max token limit. Output was saved to a temp file:
```
C:\Users\emers\.claude\projects\E--Claude-Code-Projects-Personal-EURI\66b05a66-20a2-476c-af12-78ec407a50d1\tool-results\mcp-emery-emery_work_item_list-1775487563036.txt
```

**Problem #1:** The work item list response is enormous because it includes full descriptions, acceptance criteria, and all fields for 52 items. For a "pick a callsign" flow, I only need `callsign` and `title`.

---

## Step 3: Attempts to extract callsign + title from the temp file

### Attempt 3a: `Read` tool with limit 50
**Result:** Error — file is 43,070 tokens even with limit=50, still exceeds 10,000 token cap.

### Attempt 3b: `Read` tool with limit 30
**Result:** Same error — 43,070 tokens. The `limit` param controls lines, but this file is dense JSON so even a few lines blow the token budget.

### Attempt 3c: `Grep` for `"callsign"|"title"`
**Result:** No matches found. This is because the JSON in the file is nested inside a `[{"type":"text","text":"..."}]` wrapper — the callsign/title keys are inside a stringified JSON blob within the `text` field, not top-level keys. Grep couldn't match them because they're escaped inside the string.

### Attempt 3d: `Bash` with `head -c 3000`
**Result:** Success — got the first ~3000 bytes. Confirmed the file structure:
```json
[
  {
    "type": "text",
    "text": "52 work item(s) returned:\n[\n  {\n    \"callsign\": \"EMERY-1.003\",\n    ..."
  }
]
```
This confirmed the data is a JSON array with one element whose `text` field contains a human-readable header line followed by a JSON array of work items.

### Attempt 3e: `Bash` with `grep -o` for callsign pattern
**Result:** Empty output. The `grep -o` pattern `'"callsign":"[^"]*"'` failed because the actual JSON has spaces after colons (`"callsign": "EMERY-1.003"`) and the keys are backslash-escaped inside the outer JSON string (`\"callsign\": \"EMERY-1.003\"`). The literal `"callsign"` pattern doesn't match `\"callsign\"` inside a string.

### Attempt 3f: Python script to parse JSON
**Result:** First attempt failed with `UnicodeDecodeError` — Windows default encoding is cp1252, not UTF-8. File contains Unicode characters (arrows, em-dashes).

### Attempt 3g: Python script with `encoding='utf-8'`
**Result:** Partially succeeded — parsed the JSON and printed items, but then hit `UnicodeEncodeError` on stdout because Windows console uses cp1252 and can't encode `→` characters. However, the output DID appear in stderr before the crash, so I got the full list of callsigns and titles.

---

## Summary of problems encountered

| # | Problem | Root cause |
|---|---------|-----------|
| 1 | MCP response too large for context | `emery_work_item_list` returns full item bodies (descriptions, acceptance criteria) for all 52 items. No `fields` or `summary` parameter to request only callsign+title. |
| 2 | `Read` tool can't handle the temp file | Even with small line limits, token count is computed on the whole file first. The file is a single giant JSON blob so line-based limits don't help. |
| 3 | `Grep` can't find keys | The JSON is double-encoded (JSON inside a JSON string), so literal key names don't appear as plain text. |
| 4 | `grep -o` in Bash fails | Same double-encoding issue — backslash-escaped quotes in the stringified JSON don't match simple patterns. |
| 5 | Python cp1252 encoding | Windows default file encoding isn't UTF-8; file contains Unicode. Need explicit `encoding='utf-8'`. |
| 6 | Python cp1252 stdout | Even after reading the file correctly, printing Unicode to Windows console fails. Need `PYTHONIOENCODING=utf-8` or `.encode()` workaround. |

---

## What would have made this fast

1. **`emery_work_item_list` with a `fields` or `compact` parameter** — return only callsign + title + status. This would keep the response under the token limit and avoid the temp file entirely.
2. **`limit` parameter on the MCP call** — I did not pass `limit`, so it returned all 52 items. A smaller page would have fit in context.
3. **A dedicated "list callsigns" tool** — lightweight tool that returns just the identifiers.

---

## Total tool calls: 10

1. `ToolSearch` — fetch tool schemas (success)
2. `emery_work_item_list` — list backlog items (overflow to temp file)
3. `Bash: head -c 3000` — peek at file structure (success)
4. `Grep` — search for callsign keys (failed: double-encoded JSON)
5. `Read` with limit 50 — read temp file (failed: token limit)
6. `Read` with limit 30 — read temp file (failed: token limit)
7. `Bash: grep -o` — extract callsigns (failed: encoding mismatch)
8. `Bash: python3` — parse JSON (failed: cp1252 read error)
9. `Bash: python3 utf-8` — parse JSON (partial success: printed list, then crashed on Unicode stdout)
10. Final answer assembled from stderr output of attempt 9.

**Time wasted on workarounds: ~8 tool calls that could have been 0 with a compact list response.**
