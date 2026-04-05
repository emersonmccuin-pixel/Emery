# Emery UX Component Audit
_Generated 2026-04-05 against branch `dev`_

---

## 1. Component Inventory

| File | Type | One-Line Description |
|------|------|----------------------|
| `views/home-view.tsx` | View | Dashboard of all projects; focus card grid + full project list |
| `views/agent-view.tsx` | View | Full-screen terminal for a running or ended agent session |
| `views/work-item-view.tsx` | View | Detail page for a single work item |
| `views/project-command-view.tsx` | View | Project hub: fleet strip, merge queue, work items, planning, documents |
| `views/project-settings-view.tsx` | View | Project name, roots, agent templates, danger zone |
| `views/settings-view.tsx` | View | Global settings: accounts, appearance, agent defaults, GitHub |
| `views/inbox-view.tsx` | View | Inbox entries per project with filter pills |
| `views/document-view.tsx` | View | New-document creation form + existing document editor |
| `components/fleet-strip.tsx` | Component | Horizontal list of all live sessions in a project |
| `components/fleet-row.tsx` | Component | Single row in fleet strip: indicator, callsign, title, branch, model, duration |
| `components/merge-queue-section.tsx` | Component | Collapsible merge queue with active and merged sublists |
| `components/merge-queue-entry.tsx` | Component | Single merge queue entry: header + expandable diff viewer |
| `components/merge-diff-viewer.tsx` | Component | Raw diff display (pre-formatted text) |
| `components/diff-stats.tsx` | Component | Files changed / insertions / deletions badge |
| `components/work-items-section.tsx` | Component | Status-grouped work item list with inline create form |
| `components/work-item-row.tsx` | Component | Single work item row: callsign, title, status, plan picker, dispatch button |
| `components/work-item-form.tsx` | Component | Create/edit form for work items |
| `components/planning-section.tsx` | Component | View-mode switcher + renders DailyAgenda or WeeklyPlanner |
| `components/daily-agenda.tsx` | Component | Today's planned items sorted by priority |
| `components/weekly-planner.tsx` | Component | 7-column week grid with per-day and unscheduled items |
| `components/plan-picker.tsx` | Component | Dropdown to assign a work item to today/tomorrow/this week/next week |
| `components/docs-section.tsx` | Component | Document list grouped by doc_type |
| `components/agent-controls.tsx` | Component | Interrupt/Terminate/Detach buttons for a live session |
| `components/agent-info-bar.tsx` | Component | Session metadata bar: callsign, title, branch, model, duration, state |
| `components/agent-terminal.tsx` | Component | Wrapper around TerminalSurface |
| `components/btw-panel.tsx` | Component | Placeholder stub for a future "status queries" panel |
| `components/status-badge.tsx` | Component | Colored status pill (string → CSS class) |
| `components/status-leds.tsx` | Component | Four-LED git health indicator (remote / backed-up / clean / up-to-date) |
| `components/live-indicator.tsx` | Component | Animated dot conveying session runtime state |
| `components/model-badge.tsx` | Component | Short label (Opus / Sonnet / Haiku) derived from agent_kind string |
| `components/duration-display.tsx` | Component | Live clock counting up from session start |
| `dispatch-sheet.tsx` | Overlay | Modal sheet for single or batch session dispatch |
| `topbar.tsx` | Shell | App-level header: branding, connection status, live count, inbox, settings |
| `breadcrumb.tsx` | Shell | Navigation trail that reflects nav-store state |
| `layer-router.tsx` | Shell | Routes nav-store layer to the correct view |
| `workbench.tsx` | Shell | Root layout with topbar, breadcrumb, and layer router |

---

## 2. View-by-View Audit

### 2.1 HomeView (`views/home-view.tsx`)

**Data displayed**
- Count of globally live sessions.
- Focus card grid: project name, agent-dot indicators (up to 5, then "+N"), pending merge count, status badge (active / queued / idle), compact git LED strip.
- All-projects list below focus cards: name, live count, pin button.

**Actions available**
- Open project (focus card or all-projects row click).
- Unpin from focus (★ button on focus card).
- Reorder focus cards (drag-and-drop).
- Pin project to focus (☆ on all-projects row).
- Create new project (button in stats bar → `ProjectCreateForm`).

**States handled**
- Empty (no projects): "No projects yet. Create one to get started."
- Focus slots full: error appears in global `appStore.error` when pinning beyond `maxFocusSlots`.
- Form submitting: submit button shows "Creating…".
- Focus cards: 3 status variants (active / queued / idle) reflected in CSS class.

**Issues and gaps**
- No loading state while `bootstrapShell` runs. The view renders `projects.length === 0 && !showCreateForm` → "No projects yet" which briefly shows on every cold boot before data arrives.
- Keyboard shortcut labels (Ctrl+1 … Ctrl+3) on focus cards are not actually wired to any event listener; they are purely decorative text.
- `maxFocusSlots` (default 3, clamped 1–5) has no UI to change it. The setting is in the store but is inaccessible.
- Project type (coding / research / general) is visible in the create form but never shown anywhere on the project card or list once created.
- Archived projects are omitted by the backend response but there is no "show archived" toggle.
- The all-projects section has no empty state of its own when all projects are pinned. It silently disappears.
- The "pin full" error writes to `appStore.error` which displays as a global banner — feels heavy for what is essentially a local constraint.
- `project_type` is present in `ProjectSummary` and `ProjectDetail` but is never displayed in any existing view after creation.

---

### 2.2 ProjectCommandView (`views/project-command-view.tsx`)

**Data displayed**
- Git health LEDs (StatusLEDs) and Settings button in a top bar.
- Operations zone: FleetStrip (live sessions) + MergeQueueSection.
- Planning zone: PlanningSection (All / Today / Week view) + WorkItemsSection + DocsSection.

**Actions available**
- Navigate to project settings.
- Open an agent (FleetRow click).
- Merge, park, check, load diff on merge queue entries.
- Switch planning view mode (All / Today / Week).
- Navigate work items, dispatch from work item row.
- Select multiple work items → multi-dispatch.
- Create new work item (+ button).
- Create new document (+ button in DocsSection).
- Open existing document.
- Pin/schedule work items via PlanPicker.

**States handled**
- Project not found → "Project not found."
- No live sessions → FleetStrip shows "No agents running. Dispatch from a work item below."
- No work items → "No work items yet."
- No documents → "No documents yet."
- Merge queue empty → MergeQueueSection returns null (section entirely absent).

**Issues and gaps**
- There is no loading indicator while `loadProjectReads` runs. The work items / documents sections appear empty until data resolves. A user sees "No work items yet." followed by items appearing, which is confusing.
- The `selectedProjectId` in store is set separately from navigation; the view assumes they are in sync but there is no guard against them diverging.
- The inbox navigation in Topbar depends on `selectedProjectId` but from the project view that is set elsewhere; if the selected project differs from the navigation project, the inbox button opens the wrong project's inbox.
- Done work items are in a `<details>` collapse; archived items are not shown at all — no indication they exist.
- The multi-dispatch bar appears only after 2+ items are selected, but there is no "select all" shortcut.
- DocsSection does not show document status (draft / active / archived). Archived documents are mixed in with active ones.
- No reconciliation proposal UI anywhere in this view despite the backend having the full `WorkflowReconciliationProposalSummary` model and `reconciliationByWorkItem` state.

---

### 2.3 AgentView (`views/agent-view.tsx`)

**Data displayed**
- AgentInfoBar: callsign, title, branch, model badge, duration, runtime state, activity state, needs-input reason.
- AgentTerminal: live or replay terminal output.
- BtwPanel: stub placeholder (`?` icon, "coming soon").
- AgentControls: Interrupt / Terminate / Detach buttons.

**Actions available**
- Interrupt (sends SIGINT while running).
- Terminate (with confirmation step).
- Detach (navigates back via `navStore.goBack()`).

**States handled**
- Session not found → "Session not found."
- Live session: full controls, ticking duration clock, pulsing indicator.
- Ended session: "Session ended" label, "Back to project" button only, terminal is read-only.
- `waiting_for_input` activity state: "needs input" badge in FleetRow.
- All runtime states (running / starting / stopping / failed / interrupted / exited) reflected in color coding.

**Issues and gaps**
- `endedAt` is received as a prop in `AgentInfoBar` but the parameter is never used (assigned to `_endedAt`). There is no display of "ended N minutes ago" or total session duration for finished sessions.
- No link from the agent view back to its associated work item. The callsign is shown as text but is not clickable.
- No link to the merge queue entry that the session created.
- When `needs_input_reason` is shown in the info bar, there is no action button to answer it from the UI. The user must type in the terminal manually — but there is no hint to do so.
- BtwPanel (`btw-panel.tsx`) is entirely a stub that renders a `?` icon. It takes up real layout space on every session.
- The `cwd`, `dispatch_group`, `worktree_id`, and other session metadata exist in `SessionSummary` but are not surfaced in this view.
- Interrupt is disabled when `runtimeState !== "running"` but the button remains visible and grayed — no tooltip explains why.

---

### 2.4 WorkItemView (`views/work-item-view.tsx`)

**Data displayed**
- Header: callsign, title, status badge, priority badge, type.
- Actions bar (when not editing): Dispatch Agent, New Document, status-transition button.
- Description section (if present), rendered via a basic markdown function.
- Acceptance Criteria section (if present).
- Planning section (PlanPicker with today/week cadences).
- Running Sessions section (live sessions).
- Session History section (past sessions, collapsed if > 5).
- Linked Documents section.
- Children section with "+ Add Child" form.
- Footer "Edit" button.

**Actions available**
- Dispatch Agent (opens dispatch sheet).
- New Document (navigates to new-document view pre-linked to this work item).
- Status transition buttons: backlog → in_progress (Start), in_progress → done (Done), done → in_progress (Reopen).
- Toggle plan assignment.
- Navigate to linked session.
- Navigate to linked document.
- Navigate to child work item.
- Add child work item (inline form).
- Edit work item (inline edit form replaces header).

**States handled**
- Loading: "Loading work item…"
- Not found: "Work item not found."
- No linked sessions → "No sessions linked."
- No linked docs → "No documents linked."
- No children → "No child items."
- Session history > 5: collapse/expand toggle.

**Issues and gaps**
- The "blocked" status has no transition button. It is listed as a valid status in the constants but there is no "Block" or "Unblock" action anywhere.
- `created_by` field on the work item model has no UI surface.
- `closed_at` has no UI surface (no "closed on" display).
- `root_work_item_id` and `child_sequence` have no UI surface (ancestry/epic chain not visualized).
- The markdown renderer (`renderMarkdownBasic`) in this view is a simpler version than in `document-view.tsx`. It does not handle headings, bullet lists, or code blocks — just bold and inline code. The document view has a better renderer.
- Session history shows `status` column (which is the business status field, e.g., "open"), not the exit result. Users likely want to know if the session exited successfully or errored.
- When editing, the header title disappears while the form is open, so the user loses context about which item they are editing.
- No reconciliation proposals UI. The store has `reconciliationByWorkItem` and `ensureReconciliationProposals()` but this data is never loaded or displayed in any view.
- If the work item has a parent, there is no breadcrumb or link to that parent from the detail view.

---

### 2.5 ProjectSettingsView (`views/project-settings-view.tsx`)

**Data displayed**
- General section: project name input, slug display (read-only).
- Project Roots section: list of active roots, each showing label, path, git badge, remote URL, and available actions.
- Agent Templates section: list of templates (key, label, mode, model).
- Danger Zone: archive button.

**Actions available**
- Rename project (inline save with Enter or button).
- Add root (folder picker dialog).
- Remove root (confirmation step required).
- Git Init root (if not a git repo).
- Edit / add remote URL for a git root.
- Add / Edit / Archive agent templates.
- Archive project.

**States handled**
- Loading: "Loading project settings…"
- No roots: "No roots configured."
- No templates: "No agent templates configured."
- Templates loading: "Loading templates…"
- Save name: button cycles through Saving… / Saved (2s) / Save.
- Adding root: "Adding root…" inline.
- Remove root / archive project: two-click confirmation pattern (button text changes to "Confirm?").

**Issues and gaps**
- `project_type` is not displayed or editable in settings. There is no way to change an existing project from "general" to "coding" after creation.
- `default_account_id` (which account is used when dispatching agents) is not shown or editable in project settings. It is only implicitly set during dispatch.
- `sort_order` for the project has no UI.
- `settings_json` and `instructions_md` (backend fields in `ProjectDetail`) have no UI. The instructions are managed via the MCP tools only.
- `agent_safety_overrides_json` exists in the backend model (`ProjectDetail`) but is absent from `types.ts` and has no UI.
- Template edit form does not expose `instructions_md` or `stop_rules_json` fields, which are the most important template fields for the dispatcher workflow.
- Template edit form does not expose `template_key` (read-only once set, but it's not even shown in edit mode, only in the display row).
- Root label cannot be edited after creation. There is no "edit label" for a root, only "remove" and git/remote actions.
- Root `root_kind` is always hardcoded to `"project_root"` in the create call. Multiple root kinds (if they exist) cannot be selected.
- Template errors on save are silently swallowed (`catch {}`), leaving the form open with no user feedback.
- Remove root errors are displayed via the global `appStore.error` banner, inconsistently with the confirmation-dialog pattern.

---

### 2.6 SettingsView (`views/settings-view.tsx`)

**Data displayed**
- Accounts tab: list of configured accounts with label, binary path, agent kind, default badge.
- Appearance tab: theme selection cards (Vaporwave / Neutral Dark).
- Agent Defaults tab: per-account model and safety-mode dropdowns.
- GitHub tab: PAT input with show/hide toggle.

**Actions available**
- Create account (label + binary path form).
- Edit account label.
- Set default account.
- Select theme (applied immediately, persisted to localStorage).
- Update per-account default model and safety mode.
- Save / clear GitHub PAT.

**States handled**
- No accounts: "No accounts configured."
- Save account label: inline edit-mode with Enter to commit.
- Creating account: button shows "Creating…".
- Save model/safety: button shows "Saving…" then "Saved" (2s).
- GitHub token configured: shows "Token configured." notice.

**Issues and gaps**
- Account creation only exposes `label` and `binary_path`. The backend `CreateAccountRequest` also accepts `agent_kind`, `config_root`, `env_preset_ref`, `is_default`, `status`, `default_safety_mode`, `default_launch_args`, and `default_model`. A new user must create an account and then separately configure defaults in a different tab.
- No way to delete an account. The backend `UpdateAccountRequest` presumably supports archiving but there is no UI for it.
- `config_root` is stored per-account but has no UI anywhere.
- `env_preset_ref` has no UI.
- `default_launch_args_json` has no UI.
- `status` field on accounts has no UI (accounts can have non-active statuses but there is no indicator or toggle).
- Safety mode options in Agent Defaults are `full / permissive / none` but the dispatch sheet shows `cautious / yolo`. These are inconsistent option sets.
- Theme is saved to `localStorage` only, not to the workspace state. It is lost if the user clears browser storage.
- No account-level `config_root` or `binary_path` editing in the Accounts section (only label). Binary path is shown (if set) but is not editable after creation.
- The GitHub PAT is stored in `localStorage` via `appStore.loadGithubToken()` / `appStore.saveGithubToken()`. Its purpose or how it is used is never explained in the UI.

---

### 2.7 InboxView (`views/inbox-view.tsx`)

**Data displayed**
- Header: "Inbox", total entry count.
- Filter pills: All / Success / Needs Review.
- Per entry (expandable): title, status badge, work item callsign, timestamp.
- Expanded body: summary text, branch name, session title.

**Actions available**
- Filter by status.
- Expand/collapse individual entry.
- Mark as read.
- Approve entry.
- Dismiss entry.
- Open associated work item.

**States handled**
- Loading: "Loading…"
- Empty (filtered): "No inbox entries [with status …]."
- Unread: blue dot, "unread" CSS class.

**Issues and gaps**
- Entries are sorted by `created_at` descending, but there is no sort control.
- The `entry_type` field is never displayed. Users cannot see whether an entry is a session completion report, a blocker notification, etc.
- `diff_stat_json` exists in `InboxEntrySummary` but is not used or shown.
- `metadata_json` exists but is not shown.
- `worktree_id` is available but there is no link to a worktree or merge queue entry from the inbox.
- There is no "mark all as read" action.
- "Approve" and "Dismiss" call separate store actions (`handleApproveInboxEntry` and `handleDismissInboxEntry`) but both resolve to status changes. The distinction between the two actions and their outcomes is not explained.
- The "resolved" status has no visual treatment distinct from "success" in the collapsed row — only the badge color differs.
- There is no pagination. All entries for a project are loaded and rendered at once.
- The `session_id` on an inbox entry is not used to link to the agent view.

---

### 2.8 DocumentView (`views/document-view.tsx`)

**Modes**: Creation (`documentId === "new"`) and editing of an existing document.

**Data displayed (creation)**
- Title input, Type input (datalist-backed), Status select, Work Item select, content textarea.

**Data displayed (existing)**
- Header with title, type chip, status badge, unsaved dot.
- Metadata toggle button (⋯) → overlay panel with same fields as creation.
- Edit / Preview mode toggle.
- Content textarea (edit) or rendered markdown (preview).
- Footer link back to project.

**Actions available**
- Create document (Ctrl+S also triggers).
- Toggle metadata panel.
- Save content (Ctrl+S also triggers, disabled when not dirty).
- Save metadata separately.
- Switch edit/preview mode.

**States handled**
- Creating: button shows "Creating…", error shown inline if title is empty.
- Loading existing: "Loading document…"
- Not found: "Document not found."
- Unsaved changes: dot indicator + Save button enabled.
- Saving: "Saving…" inline status; "Saved" (2s) on completion.

**Issues and gaps**
- `session_id` on documents is not editable; a document linked to a session cannot be re-linked to a different session or detached.
- `slug` field is not editable in the creation form (absent). The backend auto-generates it.
- `slug` is not editable in the metadata panel of an existing document either, though `UpdateDocumentRequest` accepts it.
- `archived_at` (document archiving) has no UI. There is no way to archive a document.
- The document create form initializes `docType` as `"note"` and the datalist provides suggestions, but free-text input means any value is accepted with no validation.
- Error on create is shown inline only for the "title is required" case. Network/backend errors write to `appStore.error` (global banner), inconsistently.
- The markdown preview renderer handles `ul` items as bare `<li>` tags without wrapping them in `<ul>`, so bullet lists render as disconnected items.
- No delete action for a document anywhere in the UI.
- `content_markdown` is synced to state only when `doc?.id` changes (not on every update), which means if the backend updates the document from another source, the editor will not show the new content until navigation away and back.

---

## 3. Form Audit

### 3.1 ProjectCreateForm (in `home-view.tsx`)

| Field | Type | Required | Validation | Default | Notes |
|-------|------|----------|------------|---------|-------|
| Project name | text | yes | `name.trim().length > 0` | (folder name if picked) | Auto-filled from folder path |
| Folder path | text (read-only) | yes | `folderPath.trim().length > 0` | — | Must use Browse button |
| Project type | radio (General / Coding / Research) | no | none | General (`""`) | Hint text shown on select |
| Initialize git | checkbox | no | none | unchecked | — |

**Submit path**: `appStore.handleCreateProject()` → creates project, creates root, optionally git init, then rebootstraps and navigates.

**Error path**: `appStore.error` is set; the form stays open but does not show the error inline — the global banner appears.

**Missing fields**:
- `slug` — auto-generated from name by the backend, but the user cannot set it at creation time.
- `default_account_id` — not settable at creation. The user must go to project settings after creation.
- `sort_order` — always assigned by backend.
- `instructions_md` — not settable at creation.

**Issues**:
- The folder path input has `readOnly` and requires the Browse button. On platforms where folder picker may fail or return null, there is no keyboard fallback to type a path manually.
- Submit button is disabled but there is no inline validation message explaining what is missing.
- Cancel does not give feedback if the name was typed (no dirty-state check; just closes).

---

### 3.2 WorkItemForm (`components/work-item-form.tsx`)

Used in both create (WorkItemsSection) and edit (WorkItemView) modes.

| Field | Type | Required | Validation | Default |
|-------|------|----------|------------|---------|
| Title | text | yes | `trim().length > 0` | `""` |
| Type | pill buttons | no | none | `"task"` |
| Priority | pill buttons (toggle) | no | none | `""` (unset) |
| Status | select | no | none | `"backlog"` |
| Parent ID | text | no | none | `""` |
| Description | textarea | no | none | `""` |
| Acceptance Criteria | textarea | no | none | `""` |

**Submit path**: `onSubmit(data)` → caller invokes `appStore.handleCreateWorkItem()` or `appStore.handleUpdateWorkItem()`. No inline success feedback; form closes on success.

**Error path**: `appStore.error` global banner.

**Missing fields vs. backend `CreateWorkItemRequest`**:
- `created_by` — not settable. Backend accepts it; it would allow tagging the human creator.

**Issues**:
- Parent ID is a raw text field expecting an internal UUID. There is no search or dropdown. This field is effectively unusable without prior knowledge of the ID. The work item view's "Add Child" form passes the parent ID automatically, but the standalone create form in the project view does not.
- Status defaults to `"backlog"` but the store's `workItemCreateForm` state also defaults to `"backlog"`, so the form's internal state and the store state are duplicated and could diverge.
- The `"archived"` status is a valid enum value in `WORK_ITEM_STATUSES` and appears in the status select, but creating a work item as archived is likely unintentional.
- Submit button uses the class `secondary-button` in both create and edit mode, which is visually identical to Cancel — no visual hierarchy between destructive-ish and confirmatory actions.

---

### 3.3 New Document Form (`views/document-view.tsx` → `NewDocumentView`)

| Field | Type | Required | Validation | Default |
|-------|------|----------|------------|---------|
| Title | text | yes | `title.trim()` (shown inline) | `""` |
| Type | text + datalist | no | none | `"note"` |
| Status | select | no | none | `"draft"` |
| Work item | select | no | none | `""` (none) |
| Content | textarea | no | none | `""` |

**Submit path**: `appStore.handleCreateDocumentWithParams()` → navigates to the new document on success.

**Error path**: inline for title; global banner for backend errors.

**Missing fields vs. `CreateDocumentRequest`**:
- `slug` — auto-generated only. Not settable.
- `session_id` — not settable from UI.

**Issues**:
- Ctrl+S shortcut fires the create action, which is fine, but there is no visual indication that this shortcut exists.
- "Content (optional)" placeholder but no character limit or size hint.

---

### 3.4 Document Metadata Panel (`views/document-view.tsx` → `ExistingDocumentView`)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| Title | text | no | Optional edit |
| Type | text + datalist | no | Accepts free text |
| Status | select | no | draft / active / archived |
| Work item | select | no | Dropdown of all project work items |

**Submit path**: "Save metadata" button → `appStore.handleUpdateDocument()`. Separate from content save.

**Issues**:
- Two separate save buttons (content and metadata) creates a state mismatch risk: the user can save content while the metadata panel has unsaved changes. Both saves pass `content_markdown: content` so they can overwrite each other if interleaved.
- `metaSaveStatus` and `saveStatus` are independent state machines. If content was saved after metadata, the metadata save will overwrite the content with the version at metadata-save time. This is a data-loss risk.
- The metadata panel has no way to detach a document from a work item (clear the work item field to "— none —"), but visually the "— none —" option is present in the dropdown, so it is possible to set work_item_id to empty — it works but may surprise users.

---

### 3.5 AddTemplateForm / TemplateRow Edit (in `project-settings-view.tsx`)

**AddTemplateForm fields**:

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| Template key | text | yes | e.g. `implementer` |
| Label | text | yes | Display name |
| Origin mode | select (code / chat) | no | Defaults to "code" |
| Model | text | no | e.g. `claude-sonnet-4-5` |

**TemplateRow edit fields**: Label, Model, Origin mode (same subset).

**Missing fields vs. `AgentTemplateSummary`**:
- `instructions_md` — the system prompt for the agent. Absent from both create and edit forms. This is the most functionally important field.
- `stop_rules_json` — also absent.
- `template_key` — cannot be changed after creation (not in the edit form). Not labelled as immutable.

**Issues**:
- Errors on save are silently swallowed in `catch {}` in both `handleSave()` functions. The user sees no feedback if the RPC call fails.
- `template_key` must be unique per project, but the form provides no validation or feedback for duplicates (error will come back from the server but is swallowed).

---

### 3.6 Account Create Form (in `settings-view.tsx` → `AccountsSection`)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| Label | text | yes | Display name |
| Binary path | text | no | Path to agent binary |

**Missing fields vs. `CreateAccountRequest`**:
- `agent_kind` — hardcoded to whatever the backend defaults to. Not selectable.
- `config_root` — not settable at creation.
- `env_preset_ref` — not settable.
- `is_default` — not settable at creation. Must use "Set default" separately.
- `status` — not settable.
- `default_safety_mode` — must configure separately in Agent Defaults tab.
- `default_model` — must configure separately in Agent Defaults tab.

---

### 3.7 DispatchSheet — SingleDispatchSheet (`dispatch-sheet.tsx`)

Not a traditional form, but contains user-settable fields:

| Field | Type | Notes |
|-------|------|-------|
| Safety Mode | select | Default ({account default}) / YOLO / Cautious |
| Model | select | Default (sonnet or opus by mode) / Opus / Sonnet |

**Displayed info** (read-only): work item callsign + title, branch name, location (worktree vs. root), account, base.

**Issues**:
- Safety mode options here ("YOLO" / "Cautious") differ from the Agent Defaults section ("full" / "permissive" / "none"). These are different vocabulary sets for the same concept.
- No way to select a different account than the project default. If the project has no default account set, the first account in the global list is used silently.
- No way to add a prompt or title override from this sheet. The `title` and `prompt` parameters available in `createSession` are not exposed.
- Stop rules are shown as read-only text for `planning`, `research`, and `execution` modes; `execution` mode shows no stop rules at all (returns empty array).
- Clicking the overlay backdrop closes the sheet with no confirmation, even if the user has changed the safety mode or model.
- The sheet hardcodes `originMode: "execution"` for single dispatch from a work item. There is no way to dispatch a planning, research, or follow-up session from the UI.

---

### 3.8 DispatchSheet — MultiDispatchSheet (`dispatch-sheet.tsx`)

| Field | Type | Notes |
|-------|------|-------|
| Per-item account | select | One per work item |
| Safety Mode | select (shared) | Same vocabulary mismatch as single dispatch |
| Model | select (shared) | Same as single |

**Issues**:
- All items share a single safety mode and model selection. Per-item overrides for model/safety are not possible.
- Conflict warnings are shown (overlapping files) but there is no mechanism to exclude or reorder items based on the warnings — the user can only cancel.

---

## 4. Missing UI Surfaces

These backend capabilities are implemented but have no frontend exposure.

| Backend Feature | Backend Location | What's Missing |
|-----------------|-----------------|---------------|
| `instructions_md` on Project | `ProjectDetail`, `UpdateProjectRequest` | No field in project settings |
| `agent_safety_overrides_json` on Project | `ProjectDetail` | Not even in `types.ts`; entirely absent |
| `settings_json` on Project | `ProjectDetail` | No field in project settings |
| `instructions_md` on Agent Template | `AgentTemplateSummary` | Neither create nor edit form exposes it |
| `stop_rules_json` on Agent Template | `AgentTemplateSummary` | Neither create nor edit form exposes it |
| Worktrees (`WorktreeSummary`) | `models.rs`, `lib.ts` (no list call) | No worktree list, no worktree detail, no worktree management UI |
| WorkflowReconciliationProposals | `store.ts`, `lib.ts` | State and API exist; never loaded or displayed |
| `account.config_root` | `AccountSummary` | Display-only in account list; not editable |
| `account.env_preset_ref` | `AccountSummary` | Not shown |
| `account.status` | `AccountSummary` | Not shown or editable |
| `account.default_launch_args_json` | `AccountSummary` | Not shown |
| Document `slug` | `DocumentSummary`, `CreateDocumentRequest` | Not settable or displayed |
| Document archiving | `archived_at` field | No archive action; archived documents still appear in list |
| Work item `created_by` | `WorkItemSummary` | Not shown |
| Work item `closed_at` | `WorkItemSummary` | Not shown |
| Session `dispatch_group` | `SessionSummary` | Not shown anywhere |
| Session `cwd` | `SessionSummary` | Not shown in agent view |
| Inbox `entry_type` | `InboxEntrySummary` | Not shown |
| Inbox `diff_stat_json` | `InboxEntrySummary` | Not shown |
| Inbox `metadata_json` | `InboxEntrySummary` | Not shown |
| Merge queue reorder | `mergeQueueReorder` in `lib.ts` | RPC exists; no drag-to-reorder UI |
| `mergeQueueReorder` (RPC) | `lib.ts` line 368 | Imported into `lib.ts` but not imported or called in `store.ts` |
| Project `project_type` after creation | `ProjectDetail` | Shown in create form; not shown or editable after creation |
| Project `default_account_id` | `ProjectDetail` | Not editable from any UI |
| `listProjectRoots` | `lib.ts` line 464 | RPC exposed but not used (project detail already includes roots) |

---

## 5. State Handling Gaps

### 5.1 Missing Loading States

| Scenario | Expected Loading State | Actual Behavior |
|----------|----------------------|-----------------|
| App cold boot (bootstrapShell) | Spinner or skeleton | Renders "No projects yet." until data arrives |
| Project data load (loadProjectReads) | Loading indicator in work items / docs sections | Sections show empty state text immediately |
| Work item detail load (in WorkItemView) | "Loading work item…" only shown when `!workItem && loading` | If work item is already in cache (stale), no refresh indicator |
| Inbox load | "Loading…" spinner | Works correctly |
| Merge queue operations | Individual `loadingKeys[merge:id]` → button disabled | Works correctly |
| Git status load | No indicator | LEDs just show "off" state (indistinguishable from "not a git repo") |
| Session attach (terminal) | No loading indicator while terminal initializes | Terminal area appears blank |

### 5.2 Missing Error States

| Scenario | Error Handling |
|----------|---------------|
| Agent template save fails | Error swallowed (`catch {}`). Form stays open, no message |
| Document metadata save fails | Global banner (inconsistent with inline title error) |
| Project create fails | Global banner only, form stays open |
| Session dispatch fails | Global banner; dispatch sheet closes immediately |
| Git init fails during project create | Swallowed silently; project still created |
| Merge queue load fails | Not attempted; `mergeQueueByProject` stays empty; no error shown |
| Inbox count load fails | No error shown |

### 5.3 Missing Empty States

| Location | What Happens | Should Show |
|----------|-------------|-------------|
| ProjectCommandView when no merge queue entries | Section not rendered at all | Could be useful to show "No pending merges" to confirm |
| DocsSection when documents have no type variety | Groups not shown (single type falls through) | Works, but no indication of type |
| Session History section when count is exactly 0 | Renders "No sessions linked." — works | OK |
| InboxView resolved filter | No filter option for "resolved" exists | Should add |

---

## 6. Consistency Issues

### 6.1 Confirmation Patterns

Three different patterns are used for destructive actions with no consistent approach:

1. **Two-click "Confirm?" pattern** — used for "Remove root", "Archive project", "Archive template". The button text changes to "Confirm?" and the second click confirms. Lost if user clicks away (`onBlur` resets).
2. **Inline expand** — used for "Terminate session" in AgentControls: expands to "Terminate this agent?" text with Yes/Cancel buttons.
3. **No confirmation at all** — "Park" in MergeQueueEntry, "Dismiss" and "Approve" in InboxView, "Interrupt" session.

### 6.2 Error Display

Two inconsistent patterns:

1. **Global banner** — `appStore.error` string displayed somewhere in the app shell. Used by most operations.
2. **Inline** — only `NewDocumentView` shows a local `error` state for the "title required" case.

Templates silently swallow errors with `catch {}`, a third (worst) pattern.

### 6.3 Save Feedback

Three save feedback patterns:

1. **Button label cycling** — Save → "Saving…" → "Saved" (2 s) → Save. Used in project name, agent defaults.
2. **Inline status text** — "Saving…" / "Saved" as a separate span next to the button. Used in document editor, document metadata.
3. **Form closes on success** — used for work item create/edit, document create. No explicit "Saved" confirmation.

### 6.4 Safety Mode Vocabulary

- Dispatch sheet options: `""` (Default), `"yolo"`, `"cautious"`
- Agent Defaults options: `""` (Default), `"full"`, `"permissive"`, `"none"`

These are different string values for what appears to be the same backend parameter (`safety_mode`). A user setting "Cautious" in Agent Defaults would write `"cautious"` but the dispatch sheet sets `"cautious"` too — however the agent defaults are writing "full" / "permissive" / "none". These do not align.

### 6.5 Navigation After Action

- After creating a project: navigates to the new project view. (Good)
- After archiving a project: navigates home. (Good)
- After creating a work item: form closes, item appears in list. No navigation.
- After creating a document: navigates to the document view. (Good)
- After dispatching a session: dispatch sheet closes. User stays on project view. No navigation to agent view.
- After merging a queue entry: entry status updates in place. No navigation.
- After terminating a session: user stays in agent view (ended state). Must manually click "Back to project".

### 6.6 Markdown Rendering

Two different markdown renderers exist:

- `renderMarkdownBasic()` in `work-item-view.tsx` (line 8): handles only bold and inline code. No headings, no lists, no code blocks.
- `renderMarkdown()` in `document-view.tsx` (line 20): handles headings, bullets, ordered lists, code blocks, bold, italic, inline code.

Work item descriptions and acceptance criteria are rendered with the weaker renderer.

### 6.7 "Done" Items in Lists

- Work items grouped as "done" are in a `<details>` collapse in `WorkItemsSection`.
- Completed agenda items are in a `<details>` collapse in `DailyAgenda`.
- Merged queue entries are in a `<details>` collapse in `MergeQueueSection`.
- Archived documents have no collapse — they appear in the flat list without distinction.

---

## 7. Critical Gaps

These issues would confuse or block a real user attempting to use the application end-to-end.

### 7.1 No Way to Set Project Instructions or Account for a Project

`instructions_md` and `default_account_id` are the two most operationally important fields on a project for the dispatcher workflow. Neither is editable from the UI. The dispatcher agent reads `instructions_md` via MCP tools. A human cannot set or view it without going to the CLI.

### 7.2 Agent Template Instructions Are Inaccessible

`instructions_md` on agent templates controls what an agent does. The field is in the model and in the RPC layer, but both the create and edit forms omit it entirely. A user can create a template called "implementer" with a model, but cannot write the actual instructions that make it behave as an implementer. Templates are essentially non-functional from the UI.

### 7.3 No Confirmation Before Closing Dispatch Sheet

The dispatch overlay closes immediately when the user clicks the backdrop. If a user accidentally clicks outside while configuring a dispatch (e.g., choosing model or safety mode), the entire dispatch is cancelled with no warning. This is particularly bad for multi-dispatch where the user may have assigned different accounts to each item.

### 7.4 Reconciliation Proposals Are Entirely Invisible

The `WorkflowReconciliationProposalSummary` model, the `listWorkflowReconciliationProposals` RPC, and `reconciliationByWorkItem` state all exist. `ensureReconciliationProposals()` is defined in the store. But nothing ever calls it, and no view renders proposals. Agents can propose status changes and other workflow updates that simply pile up silently.

### 7.5 Parent ID Field Is Unusable

The work item create form in the project view exposes "Parent ID" as a text input that expects a UUID. Users have no way to discover work item UUIDs from the UI. The field is effectively a developer debug input presented as a user field.

### 7.6 No Way to Know Why a Session Needs Input

When a session enters `waiting_for_input`, the `needs_input_reason` is shown in the info bar and as a badge in the fleet row. But there is no action button or guidance for what to type. The user must click through to the agent view, read the terminal output, and type an appropriate response manually. The reason string itself can be `null`, leaving no hint at all.

### 7.7 Merge Queue Reorder Has No UI

`mergeQueueReorder` is implemented in `lib.ts` but never called from the store or rendered in the UI. Queue position is displayed (via `a.position`) but is not interactive. Users cannot control merge order.

### 7.8 BtwPanel Is a Stub Occupying Real Layout Space

`btw-panel.tsx` renders a `?` icon with a title tooltip "coming soon". It occupies a fixed column in the agent view on every session, consuming horizontal real estate from the terminal which is the primary interaction surface.

### 7.9 Cold Boot Shows False Empty State

On startup, before `bootstrapShell` completes, the home view renders `projects.length === 0 && !showCreateForm` → "No projects yet. Create one to get started." This is incorrect — the user may have 10 projects. The false empty state briefly appears before projects load, which may cause a user to click "+ New Project" unnecessarily.

### 7.10 Two Safety Mode Vocabularies Cannot Both Be Correct

The dispatch sheet uses `"yolo"` and `"cautious"`, and Agent Defaults uses `"full"`, `"permissive"`, `"none"`. If these map to the same backend field, one or both UIs are sending incorrect values. This likely causes silent no-ops or unexpected behavior at session launch.

---

_End of audit. 39 files examined. Key files: `apps/emery-client/src/views/*.tsx`, `apps/emery-client/src/components/*.tsx`, `apps/emery-client/src/store.ts`, `apps/emery-client/src/lib.ts`, `apps/emery-client/src/types.ts`, `crates/supervisor-core/src/models.rs`._
