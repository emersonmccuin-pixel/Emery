# EURI UX Flow Analysis

**Date:** 2026-04-05
**Scope:** Full app audit — navigation, journeys, friction, dead ends, missing transitions

---

## 1. Navigation Map

### Screens and their routes

```
[home]
  ├── → [project]          click FocusCard or AllProjectsList row; Ctrl+1–5
  ├── → [settings]         "settings" button in topbar; Ctrl+,
  └── → [project] (create) "+ New Project" inline form → auto-navigates on success

[project]  (ProjectCommandView)
  ├── → [agent]            FleetStrip row click; WorkItemRow "Dispatch" → DispatchSheet → auto-navigate
  ├── → [project-settings] "⚙ Settings" button (top-left of view)
  ├── → [work_item]        WorkItemRow click (anywhere except Dispatch button)
  ├── → [document]         DocsSection row click
  ├── → [new-document]     DocsSection "+ New" button
  ├── → [inbox]            topbar "inbox" button (only when project is selected)
  └── ← [home]             breadcrumb "EURI" or Escape or Ctrl+`

[project-settings]  (ProjectSettingsView)
  ├── → [project]          "← Back" button
  └── ← [project]          Escape

[agent]  (AgentView)
  ├── → [project]          "Back to project" (ended) or "Detach" (live)
  └── ← [project]          Double-Escape or breadcrumb project link

[work_item]  (WorkItemView)
  ├── → [agent]            "Dispatch Agent" button; running session card click; past session row click
  ├── → [new-document]     "New Document" button
  ├── → [document]         linked doc row click
  ├── → [work_item]        child item row click
  └── ← [project]          breadcrumb project link; Escape

[document]  (ExistingDocumentView)
  ├── → [project]          "← {ProjectName}" footer link
  └── ← [project]          Escape

[new-document]  (NewDocumentView)
  ├── → [document]         auto-navigate on successful create
  ├── → [project]          "Cancel" button; "← {ProjectName}" footer link
  └── ← [project]          Escape

[inbox]  (InboxView)
  ├── → [work_item]        entry "open item" button (only if entry has work_item_id)
  ├── → [home]             topbar "inbox" button (toggles — click again to go home)
  └── ← [project]          Escape or breadcrumb (note: breadcrumb does NOT include "inbox")

[settings]  (SettingsView)
  ├── ← any                "← Back" button (goBack); Escape; Ctrl+,
  └── tabs: Accounts, Appearance, Agent Defaults, GitHub (in-view only, no nav change)
```

### Persistent chrome (always visible)

**Topbar** (`topbar.tsx`): EURI brand + connection dot | inbox button (project-scoped) | connection status chip | "N live" chip | optional "debug" chip | "settings" button

**Breadcrumb bar** (`breadcrumb.tsx`): EURI › [project name] › [agent|document|work_item|"new document"]
- Note: `settings`, `project-settings`, and `inbox` are NOT in the breadcrumb trail.

---

## 2. Journey Walkthroughs

### Journey 1: First Launch

1. App loads. Bootstrap call begins. User sees: `"Connecting to the supervisor…"` full-screen text (no spinner, no branding beyond CSS class).
2. Bootstrap succeeds. `navStore.restore({ layer: "home" })` fires. User lands on **HomeView**.
3. HomeView shows: "0 agents running" stat, "+ New Project" button, and empty state text "No projects yet. Create one to get started."
4. User clicks "+ New Project". An inline `ProjectCreateForm` slides in above the (empty) card grid.
5. Form fields: Project name (text), Folder path (read-only, filled by Browse), Project type (General/Coding/Research radio), Initialize git checkbox.
6. User must click "Browse" to pick a folder — cannot type a path directly (field is `readOnly`).
7. Browse picker fires `pickFolder()` (native OS dialog). If they pick a folder, name auto-fills from folder name if name was empty.
8. User fills name, picks folder, optionally picks type/git, clicks "Create Project". State: `submitting=true`, button says "Creating...".
9. On success: `handleProjectCreated(projectId)` → `navStore.goToProject(projectId)` → user lands on **ProjectCommandView**.
10. **Accounts not set up yet.** ProjectCommandView shows all sections but "Dispatch Agent" on any work item will fail silently or show the DispatchSheet with no account (account defaults to `null`). There is no prompt to configure accounts.
11. To set up an account: user must notice the "settings" button in topbar, click it, navigate to Accounts tab, click "+", fill Label and optional binary path, click Create. Then navigate back to the project.

**Stuck point:** No onboarding flow guides the user from project creation to account setup. A user could sit at an empty ProjectCommandView wondering why dispatching doesn't work.

---

### Journey 2: Daily Driver — Start Work

1. App opens. Workspace state is restored from persistence. If user was last in a project, they land directly in **ProjectCommandView** for that project.
2. They see:
   - **FleetStrip**: running agents (or "No agents running" empty state).
   - **MergeQueueSection**: only visible if entries exist; otherwise absent.
   - **PlanningSection**: All/Today/Week segmented control + DailyAgenda or WeeklyPlanner depending on mode.
   - **WorkItemsSection**: grouped by status (In Progress → Blocked → Planned → Backlog → Done[collapsed]).
   - **DocsSection**: document list.
3. To see what needs attention: user checks FleetStrip for running agents, MergeQueueSection for pending merges, and WorkItemsSection for items in backlog/planned.
4. **Inbox**: unread count badge appears on topbar inbox button only when a project is selected. User clicks topbar "inbox" to go to InboxView.
5. Inbox shows entries with status filter (All/Success/Needs Review). Each entry can be expanded to see summary, branch, session. Actions: mark read, approve, dismiss, open work item.
6. Back to project: user clicks inbox button again (toggles to home) or hits Escape or uses breadcrumb — but **breadcrumb does not show "inbox"** so Escape is the only shortcut.
7. To pick up a work item: click it in WorkItemsSection → WorkItemView. Actions bar shows: "Dispatch Agent", "New Document", status transition button.
8. To launch a new session: click "Dispatch Agent" → DispatchSheet appears as an overlay.

---

### Journey 3: Agent Lifecycle

#### Dispatching

From WorkItemRow (ProjectCommandView):
1. Click "Dispatch" button (or click the row → WorkItemView → "Dispatch Agent").
2. **DispatchSheet** overlays the screen (modal).
3. Single-dispatch sheet shows: Work Item, Branch (for execution mode), Location, Account, Base, Safety Mode (select), Model (select), Stop Rules.
4. User clicks "Launch on Branch" (execution) or "Launch on Main" (other modes).
5. `confirmDispatch` fires → session is created → `navStore.goToAgent(projectId, sessionId)` → user lands on **AgentView**.

From WorkItemView:
1. "Dispatch Agent" button → same DispatchSheet flow.

#### Monitoring

In **AgentView**:
- **AgentInfoBar**: callsign, title, branch, agent kind, start time, runtime state, activity state, needs-input reason, live status.
- **AgentTerminal**: live xterm.js terminal showing agent output.
- **BtwPanel**: a placeholder "?" icon (no function yet — "coming soon").
- **AgentControls**: Interrupt, Terminate (confirm), Detach buttons.

**Completion notification**: When a session changes to `completed` runtime state, an audio ding plays (C-E-G major chord ascending). When it errors, a triangle-wave chime plays. **No visual toast or notification badge appears** in the current view to indicate completion if the user has navigated away from the AgentView.

When user is on **ProjectCommandView**, the FleetStrip updates reactively. When a session ends, it disappears from FleetStrip. There is no "session just finished" notification in the project view.

#### After completion

If user is watching in AgentView:
1. Terminal shows final output. AgentControls changes to show "Session ended" label + "Back to project" button.
2. User clicks "Back to project" → `navStore.goBack()` → ProjectCommandView.
3. They must now check MergeQueueSection for the merge entry.

If user was not watching (navigated away):
1. No visual indicator in ProjectCommandView that a session just finished.
2. Inbox entry is created (if configured). Unread badge appears on topbar inbox button.
3. User must click inbox to see the completion entry.

#### Reviewing output

User sees agent output only through AgentView (terminal replay). There is no summarized output view outside the terminal.

#### Merge queue

From ProjectCommandView, MergeQueueSection (only visible when there are entries):
1. Each `MergeQueueEntry` shows: callsign, session title, branch name, status badge, diff stats, uncommitted-changes warning.
2. Actions: expand/collapse (loads diff on first expand), "Check" (conflict check), "Merge" (only when status=ready), "Park".
3. User clicks expand → diff loads → `MergeDiffViewer` shows colored diff inline.
4. User clicks "Check" → status updates to `ready` if clean.
5. User clicks "Merge" → merge fires → entry moves to "merged" collapsed section.

---

### Journey 4: Work Item Management

#### Create

From ProjectCommandView → WorkItemsSection:
1. Click "+" button in "Work Items" section header.
2. `WorkItemForm` (create mode) expands inline.
3. Fields: Title (required), Type (pill buttons: epic/task/bug/feature/research/support), Priority (pill buttons), Status (select), Parent ID (text), Description (textarea), Acceptance Criteria (textarea).
4. Click "Create" → item appears in list grouped by status.
5. **After create: user stays on ProjectCommandView.** No navigation to the new item.

#### Assign to planning cycle

From WorkItemRow (in ProjectCommandView):
- `PlanPicker` component is rendered inline in each row. Allows toggling day/week assignment.

From WorkItemView:
- Full `PlanPicker` in "Planning" section.

#### Dispatch agent to work item

From WorkItemRow: "Dispatch" button (right side of row).
From WorkItemView: "Dispatch Agent" button (top actions bar).

Both open DispatchSheet. After confirm, navigates to AgentView.

#### Track progress

From ProjectCommandView: WorkItemRow shows status + priority inline.
From WorkItemView: shows running sessions (cards) and session history (list).

#### Mark done

From WorkItemView: status transition button shows "Done" when `status === "in_progress"`. Clicking fires `handleUpdateWorkItem` with `status: "done"`. UI updates in place.

Alternatively, from ProjectCommandView, work items in "Done" group are collapsed in a `<details>` element.

---

### Journey 5: Project Setup

#### Create project

See Journey 1 steps 4–9. Fields: name (required), folder path (required via Browse), project type (optional), init git (optional checkbox).

After creation, lands on ProjectCommandView. Project has 0 work items, 0 documents, no sessions.

#### Add project roots

From ProjectCommandView → "⚙ Settings" button → ProjectSettingsView → "Project Roots" section → "+" button → `pickFolder()` OS dialog → root added immediately with label from folder name.

#### Configure git

In ProjectSettingsView → RootRow for each root:
- If no git: "Git Init" button → `gitInitProjectRoot`.
- If git, no remote: "Add remote" → inline text field → save.
- If git with remote: "Edit remote" → same flow.

#### Configure name/slug

In ProjectSettingsView → "General" section → name input + "Save" button. Slug is read-only/display only.

#### Configure accounts (global, not per-project)

Settings (topbar) → Accounts tab → "+" → label + optional binary path → Create.

#### Configure per-project default account

ProjectSettingsView does NOT expose a "default account" field. The default account for dispatch comes from `project.default_account_id` (visible in DispatchSheet as pre-selected account), but it cannot be changed from the UI.

#### Agent templates

ProjectSettingsView → "Agent Templates" section → "+" → form with: template key, label, mode (code/chat), model → "Add Template".

---

### Journey 6: Multi-Agent Orchestration

#### Plan a batch

From ProjectCommandView → WorkItemsSection:
1. Check checkboxes on 2+ work items. A `multi-dispatch-bar` appears showing count.
2. Click "Dispatch N in parallel".
3. `MultiDispatchSheet` overlays: lists all selected items with per-item account selector, plus global Safety Mode and Model selects.
4. Conflict warning shown if `dispatchConflicts` has overlapping files between items.
5. User clicks "Dispatch All (N sessions)" → all sessions created.

**After multi-dispatch: no navigation occurs.** User returns to ProjectCommandView. The DispatchSheet closes, but there is no confirmation that all sessions launched.

#### Monitor the fleet

Back in ProjectCommandView, FleetStrip shows all running sessions. Each FleetRow shows: activity indicator, callsign/mode, title, branch, model badge, duration. Clicking a row navigates to AgentView for that session.

There is no multi-session view — only one agent terminal can be watched at a time.

#### Handle completion

Sessions disappear from FleetStrip as they finish. Merge entries appear in MergeQueueSection. Inbox entries appear with unread badge.

#### Conflicts

MergeQueueEntry shows conflict files if `conflict_files` is populated. "Check" button re-runs conflict detection. "Park" removes from active queue. Conflict resolution must happen outside the app (in a terminal/editor).

---

### Journey 7: Settings & Configuration

#### Access settings

- Topbar "settings" button (always visible, top-right).
- Keyboard shortcut Ctrl+,.
- No keyboard hint displayed on the topbar button (though `title="Settings (Ctrl+,)"` tooltip exists).

#### Accounts tab

Add account: "+" → label + binary path → Create.
Edit label: "Edit" inline → input + Save/Cancel.
Set default: "Set default" button.
Account shows: label, default badge, binary path, agent_kind.
**No delete account option exists.**

#### Appearance tab

Two theme cards: Vaporwave, Neutral Dark. Click to apply instantly. Theme persisted to `localStorage`.

#### Agent Defaults tab

Per-account model and safety mode. Save button per account. "Saved" confirmation for 2 seconds.

#### GitHub tab

PAT input. Show/Hide toggle. Save persists to localStorage. Clear button.

---

## 3. Friction Log

Ranked by severity (Critical → High → Medium → Low).

### Critical

**F1: No onboarding or account-setup prompt**
First-time users create a project and land on ProjectCommandView with no accounts configured. Dispatching from here goes to DispatchSheet that shows "—" for account. The session will fail. There is no call-to-action to set up an account first. Source: `dispatch-sheet.tsx` line 24 (`accounts[0] ?? null`), no empty-state guard in DispatchSheet.

**F2: Folder path field is read-only — no keyboard entry**
In `ProjectCreateForm`, the folder path input has `readOnly` on it (`home-view.tsx` line 171). Users cannot type a path; they must use the OS Browse dialog every time. Power users who know their paths are forced to navigate a file picker. Critical for users without GUI file access or remote environments.

### High

**F3: No post-dispatch navigation for multi-dispatch**
After dispatching multiple sessions via `MultiDispatchSheet`, the sheet closes and the user is left on ProjectCommandView with no confirmation that the sessions launched. They must look at FleetStrip to verify. Source: `store.ts` `confirmMultiDispatch` — does not call `navStore.goToAgent` unlike single dispatch.

**F4: No visual completion notification when not watching**
When an agent finishes and the user is elsewhere in the app, the only signal is the audio ding (which may not be heard) and the inbox unread count badge. There is no toast, banner, or highlighted state on the FleetStrip entry (it simply disappears). The user must know to check the inbox. Source: `store.ts` `applySessionStateChange`.

**F5: Inbox navigation is a toggle, not a push**
The topbar inbox button toggles between inbox and home (`goHome()`), not between inbox and the previous view. If user was on a work item or document and clicks inbox, they lose their place. Coming back from inbox goes to home, not back to where they were. Source: `topbar.tsx` lines 19–26 (`navStore.goHome()`).

**F6: Inbox missing from breadcrumb**
When in InboxView, the breadcrumb bar shows only "EURI" (home) — inbox is not in the breadcrumb chain. The back route from inbox is either clicking the inbox button again (which goes home, not back) or Escape. There is no obvious way to return to the project. Source: `breadcrumb.tsx` — `buildCrumbs` has no case for `inbox` layer.

**F7: No default account setting from project UI**
`project.default_account_id` exists in the data model and is used in DispatchSheet to pre-select an account, but ProjectSettingsView has no UI to set it. Users cannot configure which account a project uses without directly editing data. Source: `project-settings-view.tsx` — no `default_account_id` field.

**F8: Work item creation has no post-create navigation**
After creating a work item via WorkItemsSection, the user stays in ProjectCommandView. They have to find and click the new item to open it. This breaks the natural flow of "create → configure → dispatch". Source: `work-items-section.tsx` `handleFormSubmit` — no navigation call after `handleCreateWorkItem`.

### Medium

**F9: BtwPanel is a placeholder with no function**
AgentView includes a side panel labeled "?" with tooltip "Status queries — coming soon". This occupies real estate in the most-used view and provides no value. Source: `btw-panel.tsx`.

**F10: Breadcrumb uses IDs not names for agent/document/work_item in loading state**
If data hasn't loaded yet, the breadcrumb shows the raw UUID instead of a human-readable name. Source: `breadcrumb.tsx` `resolveLabel` fallback to `crumb.label` which is the ID.

**F11: Project settings back button uses goToProject, not goBack**
ProjectSettingsView "← Back" button calls `navStore.goToProject(projectId)` (hard-coded destination) rather than `navStore.goBack()`. This means if the user navigated to settings from somewhere other than the project view, they can't return to where they were. Source: `project-settings-view.tsx` line 106.

**F12: Session ended — no link to the work item from AgentView**
When viewing a completed session in AgentView, there is no direct link to the work item the session was for. The breadcrumb shows the session ID (or callsign), but there is no "View Work Item" action button. Users must go back to project, find the work item, and navigate to it. Source: `agent-view.tsx` — no work item navigation in view or controls.

**F13: No way to navigate to session from inbox entry**
InboxView entries have an "open work item" link, but no "open session" / "view terminal" link. If the user wants to see what the agent actually did, they must go to the project, find the session in history. Source: `inbox-view.tsx` — `InboxEntryRow` has `handleOpenWorkItem` but no session navigation.

**F14: Double-Escape to exit AgentView is undiscoverable**
Pressing Escape once while in AgentView passes through to the terminal (by design — terminal captures single Escape). Double-Escape within 400ms navigates back. This is documented nowhere in the UI. Source: `App.tsx` lines 310–320.

**F15: Work item form "Parent ID" field is a raw text input**
The parent_id field in WorkItemForm is a free-text input that expects a UUID. There is no dropdown, search, or autocomplete from existing work items. Source: `work-item-form.tsx` lines 122–130.

**F16: No way to delete an account**
Settings → Accounts shows Edit and Set Default actions but no Delete. Source: `settings-view.tsx` `AccountRow` — no delete handler.

**F17: Creating a work item from WorkItemView child section uses same form**
The child WorkItemForm in WorkItemView pre-fills `parent_id` but the form still shows the "Parent ID" text field, which is misleading — the parent is already set. Source: `work-item-view.tsx` line 371.

### Low

**F18: Planning section count shows "…" when not in that mode**
PlanningSection segmented control shows "Today (…)" and "Week (…)" when those modes aren't active. The count only resolves when the mode is selected. A user can't tell at a glance how many items are planned for today without clicking. Source: `planning-section.tsx` lines 64–68.

**F19: DocsSection not examined (component not read), but no navigation from inbox to document**
InboxView can link to a work item but not to a document. If an inbox entry references a doc, there is no shortcut.

**F20: Connection loading state has no spinner**
On boot, `"Connecting to the supervisor…"` is plain text with no visual indicator of activity. Source: `App.tsx` line 357.

**F21: "Archive project" danger zone gives no way to restore**
Archiving a project hides it from the home view with no visible way to unarchive from the UI. Source: `project-settings-view.tsx` — no unarchive button.

---

## 4. Dead Ends

### DE1: AgentView with no work item link
User finishes reviewing agent output in AgentView. Controls offer "Back to project" and "Detach". There is no path to the work item, no path to the merge queue entry, no path to related documents. They must navigate back to the project and find things manually.

### DE2: Inbox with no path back to the current project
From InboxView, if the user got there by clicking the topbar inbox button (which was displaying the project), and they want to go back to that project view, their options are: Escape (goes back in nav history), or click the inbox button again (which `goHome()`s). If they arrived at inbox from home (no prior project view in history), Escape goes home, not back to the project.

### DE3: Project settings — no navigation to project roots git config
After setting up a root in ProjectSettingsView, there is no link to see the git health status from within settings. The StatusLEDs only appear in ProjectCommandView. User must go back to the project to see the git state.

### DE4: WorkItemView with no path to project sessions list
From WorkItemView, the user can see sessions linked to that work item. But they cannot see or navigate to sessions for the whole project (the FleetStrip only exists in ProjectCommandView). If they want to check what else is running, they must go back to the project.

### DE5: Multi-dispatch completion — nowhere to see all launched sessions
After a batch dispatch, the user is on ProjectCommandView. They can see the FleetStrip, but all sessions are mixed together. There is no "this batch" grouping. If they dispatched 5 sessions, they have 5 unlabeled fleet rows.

---

## 5. Missing Transitions

### MT1: After creating a work item → should navigate to the new work item
Currently: stays in ProjectCommandView. Should: `navStore.goToWorkItem(projectId, newItemId)` after successful create, or at minimum scroll to/highlight the new row.

### MT2: After multi-dispatch → should show a confirmation or navigate to fleet
Currently: DispatchSheet closes, user left on ProjectCommandView with no feedback. Should: either briefly show a "N sessions launched" toast, or navigate to the project view with the FleetStrip scrolled into view.

### MT3: Agent session completed → visual indicator at project level
Currently: no visual change in ProjectCommandView when a session finishes (beyond it leaving FleetStrip). Should: completed session entries should briefly highlight in the project view, or a notification banner should appear.

### MT4: From inbox entry → to session terminal
Currently: inbox entry can open the work item, but not the session. Should: add "View session" link that calls `navStore.goToAgent(projectId, sessionId)` when `entry.session_id` is available.

### MT5: From AgentView → to work item
Currently: no navigation exists from AgentView to the associated work item. Should: AgentInfoBar (which already shows `callsign`) should make the callsign a clickable link to `navStore.goToWorkItem(projectId, workItemId)`.

### MT6: Inbox → back to project (not home)
Currently: clicking inbox button again calls `navStore.goHome()`. Should: call `navStore.goToProject(selectedProjectId)` to return to the project that owns the inbox.

### MT7: After project archive → should navigate to home
Currently: no navigation after archive in `handleArchiveProject`. If the user successfully archives, they're left on ProjectSettingsView for a project that no longer appears in the project list. Should: navigate to home after archive completes.

### MT8: Settings → Accounts → create first account → prompt to return to project
Currently: after creating an account in Settings, user has no guided path back to their project. Should: after first-ever account creation, show a "Ready to dispatch agents. Go to your project →" nudge.

---

## 6. Quick Wins

Low effort, high impact changes that could be shipped in an hour or less each.

### QW1: Make folder path input writable in ProjectCreateForm
Remove `readOnly` from the folder path input (`home-view.tsx` line 171). Users can still use Browse, but can also type/paste. One line change.

### QW2: Fix inbox back-navigation to go to project, not home
In `topbar.tsx` `handleInboxClick`, change `navStore.goHome()` to `navStore.goToProject(selectedProjectId)`. Makes inbox a true modal layer over the project.

### QW3: Add inbox to breadcrumb
In `breadcrumb.tsx` `buildCrumbs`, add a case for `inbox` layer: push `{ label: "inbox", layer: c }` after the project crumb. Makes it obvious where you are and that you can click "project" to go back.

### QW4: Make callsign in AgentInfoBar a link to the work item
`agent-view.tsx` passes `callsign` and implicitly has `sessionSummary.work_item_id`. Wrap the callsign display in AgentInfoBar in a button that calls `navStore.goToWorkItem(projectId, sessionSummary.work_item_id)`. One clickable element added.

### QW5: Fix Double-Escape hint — add a tooltip to AgentControls
In `AgentControls`, add a small tooltip or note next to "Detach" saying "or press Esc twice". Zero behavior change, pure discoverability.

### QW6: Post-create work item navigation
In `work-items-section.tsx`, after `handleCreateWorkItem()` resolves, call `navStore.goToWorkItem(projectId, newItemId)`. Requires `handleCreateWorkItem` to return the new item ID (check store).

### QW7: Remove BtwPanel or replace with something useful
Delete the "?" panel from AgentView until the feature is ready. It takes up horizontal space in the terminal area with no benefit. `agent-view.tsx` line 64.

### QW8: Add "View session" link to inbox entries
In `InboxEntryRow`, if `entry.session_id` exists, add a button alongside "open item" that calls `navStore.goToAgent(projectId, entry.session_id)`. One button, big discoverability win.

### QW9: Project settings back button should use goBack, not goToProject
`project-settings-view.tsx` line 106: change `navStore.goToProject(projectId)` to `navStore.goBack()`. Preserves the user's nav history correctly.

### QW10: Connection loading — add a CSS spinner
Wrap `"Connecting to the supervisor…"` in a container with a CSS spinner animation. One CSS class addition. Makes boot feel intentional rather than stuck.

---

## 7. Recommended Flow Changes

Higher-effort improvements worth planning as work items.

### RFC1: Onboarding / first-run wizard
**Severity:** Critical — without accounts, the app is unusable.
When no accounts exist, show a guided setup step before the user can fully use the app. Sequence:
1. Home screen shows "No accounts configured" banner with CTA → Settings → Accounts.
2. After first account is created, Settings shows "Account ready. Return to home →" CTA.
3. Alternatively, the ProjectCreateForm checks account existence and warns if zero accounts.
**Files:** `home-view.tsx`, `project-command-view.tsx`, `dispatch-sheet.tsx`

### RFC2: Dispatch confirmation / batch launch feedback
Currently there is no post-dispatch confirmation. After single dispatch the app navigates away. After multi-dispatch nothing happens visually.
- Single dispatch: current behavior (navigate to AgentView) is fine.
- Multi-dispatch: after `confirmMultiDispatch`, navigate to the project view and briefly highlight the FleetStrip, OR show a 3-second "N sessions launched" banner.
**Files:** `store.ts` `confirmMultiDispatch`, `App.tsx` for banner

### RFC3: Session completion notification system
When a session transitions to `completed` or `errored` from any nav state, surface a visible notification:
- If in AgentView: already visible in terminal + controls.
- If in project view: show a non-blocking notification dot or toast that links to the session.
- If in any other view: show a persistent topbar indicator (beyond the existing live count chip) showing "N finished" with a click-to-review action.
**Files:** `store.ts` `applySessionStateChange`, `App.tsx` or new `notification-store.ts`, `topbar.tsx`

### RFC4: Add "default account" picker to ProjectSettingsView
The data model supports `default_account_id` on projects. Expose a dropdown in ProjectSettingsView → General section to pick the default account. This removes the current situation where the default account is silently determined by `accounts[0]`.
**Files:** `project-settings-view.tsx`, `store.ts` `handleUpdateProjectName` (or new `handleUpdateProject`)

### RFC5: Parent ID autocomplete in WorkItemForm
Replace the raw text input for `parent_id` with a searchable dropdown that shows existing work items (callsign + title). Work items are already in `workItemsByProject` in the store.
**Files:** `work-item-form.tsx` — replace input with a filtered select or combobox

### RFC6: Batch session monitoring view
For multi-agent orchestration, add a "batch view" or enhance ProjectCommandView to show sessions grouped by dispatch round or by batch. Currently all live sessions in FleetStrip are ungrouped. Even just showing which work item each session is for (already partially done via `workItem.callsign` in FleetRow) is a step up.
**Files:** `fleet-strip.tsx`, `fleet-row.tsx`

### RFC7: "Archive / restore" symmetry for projects
Currently you can archive a project but there is no UI to restore it. Add a filter to HomeView to show archived projects, and an "Unarchive" action in ProjectSettingsView.
**Files:** `home-view.tsx`, `project-settings-view.tsx`, `store.ts`

### RFC8: Inbox → session terminal shortcut
Inbox entries should surface a direct "View terminal" link when they have a `session_id`. This closes the feedback loop: agent finishes → inbox entry → user reviews terminal output → user acts on merge queue. Currently this path requires 4+ clicks and two navigations.
**Files:** `inbox-view.tsx`, `lib.ts` (ensure `InboxEntrySummary` exposes session_id)

### RFC9: Document editor — unsaved changes guard
The document editor tracks `isDirty` state and shows an unsaved dot in the header. However, navigating away (Escape, breadcrumb, back button) immediately closes the view without warning about unsaved changes. A `beforeNavigate` guard or a "You have unsaved changes" confirmation dialog would prevent accidental data loss.
**Files:** `document-view.tsx` — add a `useEffect` cleanup or nav intercept

### RFC10: Project command view — "nothing to do" empty state
When a new project has zero work items, zero sessions, zero merge queue entries, and zero documents, the ProjectCommandView shows a series of empty-state messages scattered across sections. A unified "Get started" panel for new projects with direct CTAs ("+ Add work item", "Configure settings") would help new users understand the app structure.
**Files:** `project-command-view.tsx`
