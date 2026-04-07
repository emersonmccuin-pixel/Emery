# Emery Layout Redesign Spec

**Date:** 2026-04-05
**Branch:** `dev`
**Status:** Design spec — ready for implementation planning

---

## Table of Contents

1. [App Shell Layout](#1-app-shell-layout)
2. [Sidebar Spec](#2-sidebar-spec)
3. [Modal vs Peek Decision Matrix](#3-modal-vs-peek-decision-matrix)
4. [Main Content Max-Width](#4-main-content-max-width)
5. [Quick Chat Feature Spec](#5-quick-chat-feature-spec)
6. [Session Launch Flow Redesign](#6-session-launch-flow-redesign)
7. [Naming Fixes](#7-naming-fixes)
8. [Navigation Model Changes](#8-navigation-model-changes)
9. [Implementation Plan](#9-implementation-plan)

---

## 1. App Shell Layout

### Current layout

```
+--------------------------------------------------+
| Topbar (full width)                               |
+--------------------------------------------------+
| Breadcrumb bar (full width)                       |
+--------------------------------------------------+
|                                                   |
|           LayerRouter (full width, flex: 1)       |
|           each view stretches edge to edge        |
|                                                   |
+--------------------------------------------------+
| DispatchSheet (overlay when active)               |
| ToastStack (fixed position)                       |
+--------------------------------------------------+
```

**Problems:** Every view occupies the full viewport width. Clicking anything replaces the entire content area. No persistent orientation — users lose spatial context. Content stretches uncomfortably on wide monitors.

### New layout: three-zone architecture

```
+---------------------------------------------------------------+
| Topbar (full width, data-tauri-drag-region)                   |
+----------+------------------------------------+---------------+
|          |                                    |               |
| Sidebar  |       Main Content                 |  Peek Panel   |
| 240px    |       (flex: 1, max-width)         |  (400px)      |
| collaps. |                                    |  conditional  |
|          |                                    |               |
|          |                                    |               |
|          |                                    |               |
|          |                                    |               |
+----------+------------------------------------+---------------+
```

### CSS layout sketch

```css
.app-shell {
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.topbar {
  /* unchanged — stays at top, full width */
  flex-shrink: 0;
}

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.sidebar {
  width: var(--sidebar-width, 240px);
  min-width: 0;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-surface);
  border-right: 1px solid var(--border-subtle);
  overflow-y: auto;
  overflow-x: hidden;
  transition: width 150ms ease, opacity 100ms ease;
}

.sidebar.collapsed {
  width: var(--sidebar-collapsed-width, 48px);
}

.main-content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.main-content-inner {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-4) var(--space-5);
}

/* Content constraint — prevents edge-to-edge stretching */
.content-frame {
  max-width: 960px;
  margin: 0 auto;
  width: 100%;
}

/* Wide variant for views that need more room (agent terminal, weekly planner) */
.content-frame.content-frame-wide {
  max-width: 1200px;
}

/* Full-bleed variant for agent terminal view */
.content-frame.content-frame-full {
  max-width: none;
}

.peek-panel {
  width: 420px;
  flex-shrink: 0;
  border-left: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: width 150ms ease;
}

.peek-panel.hidden {
  width: 0;
  border-left: none;
}
```

### Component hierarchy (new App.tsx)

```tsx
<div className="app-shell">
  <Topbar />
  <div className="app-body">
    <Sidebar />
    <div className="main-content">
      <Breadcrumb />
      {/* error/connection banners */}
      <div className="main-content-inner">
        <LayerRouter />
      </div>
    </div>
    <PeekPanel />
  </div>
  <DispatchSheet />  {/* overlay — above everything */}
  <ToastStack />
</div>
```

### Dimensions and responsive rules

| Element | Default | Collapsed/Hidden | Min viewport |
|---------|---------|------------------|--------------|
| Sidebar | 240px | 48px (icon rail) | Always visible |
| Main content | flex: 1 | flex: 1 | 480px minimum effective |
| Peek panel | 420px | 0px (hidden) | Only shows when viewport > 1100px and content triggers it |
| Topbar | 44px height | 44px | Always |
| Breadcrumb | 32px height | 32px | Moves inside main-content |

**Responsive behavior:**
- Below 1100px viewport width: peek panel auto-closes; peek triggers open a center modal instead.
- Below 800px viewport width: sidebar collapses to icon rail automatically.
- Sidebar collapse state persists in localStorage (user choice overrides auto-collapse when viewport is wide enough).

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+B` | Toggle sidebar collapse |
| `Ctrl+\` | Toggle peek panel (close if open, or no-op if nothing to peek) |
| `Escape` | Close peek panel if open; otherwise navigate back |
| `Ctrl+`` ` | Go home |
| `Ctrl+,` | Open settings (in peek panel if sidebar visible, else navigate) |
| `Ctrl+1` through `Ctrl+5` | Jump to focus project 1-5 |
| `Ctrl+Shift+N` | Open Quick Chat |
| `Double-Escape` | Exit agent terminal view (unchanged) |

---

## 2. Sidebar Spec

The sidebar is the persistent navigation anchor. It provides spatial context at all times — the user always knows where they are and what else is happening.

### Sections (top to bottom)

```
+---------------------------+
| [Logo/Home]  [Collapse]   |  <- Header
+---------------------------+
| FOCUS PROJECTS            |  <- Section 1: Project navigator
|  > Project A    ●●○       |
|    Project B              |
|    Project C    ●         |
+---------------------------+
| FLEET          3 running  |  <- Section 2: Session fleet
|  ● Emery-58 implementing   |
|  ● Emery-61 planning       |
|  ○ Emery-55 completed      |
+---------------------------+
|                           |
|                           |  <- Flex spacer
+---------------------------+
| [+ Quick Chat]            |  <- Section 3: Quick actions
| [Inbox]          (3)      |
| [Vault]                   |
| [Settings]                |
+---------------------------+
```

### Section 1: Project Navigator

**Purpose:** Show the user's focus projects and let them switch context with one click.

**Content:**
- List of pinned/focus projects (up to `maxFocusSlots`, default 3).
- Each row shows: project name, live agent dot indicators (up to 3 dots, then "+N"), pending merge count badge.
- Active project row is highlighted (matches `selectedProjectId`).
- Clicking a project row navigates to that project's command view.
- Below focus projects: a "All projects" link that navigates to home view.
- "+ New Project" link below the project list.

**Collapsed state:** Show only the first letter of each project name (or a colored circle). Active project gets a left-edge accent bar.

**Interaction:**
- Click project name -> `navStore.goToProject(id)`
- Right-click project -> context menu: "Open in peek", "Unpin", "Settings"
- Drag to reorder (preserve existing focus reorder behavior)

### Section 2: Session Fleet (Global)

**Purpose:** Show all running sessions across all projects, so the user can monitor without navigating away.

**Content:**
- Header: "Fleet" + count of running sessions.
- Each row: live indicator dot, callsign or title (truncated), compact duration.
- Rows grouped by project (project name as small subheader, only if multiple projects have sessions).
- Clicking a session row navigates to that agent view.
- Recently completed sessions (within last 5 minutes) show as dimmed rows with a checkmark or X icon, then fade out.

**Collapsed state:** Show only the fleet count badge on a terminal/agent icon.

**Empty state:** Section header still visible ("Fleet -- 0 running") but no rows. Section takes minimal space.

### Section 3: Quick Actions (bottom-anchored)

**Purpose:** Persistent access to cross-cutting features.

**Items:**
- **Quick Chat** button — launches an unscoped Claude session (see section 5).
- **Inbox** button — shows aggregate unread count badge across all projects. Click navigates to a global inbox view or the current project's inbox.
- **Vault** button — navigates to vault.
- **Settings** button — navigates to settings view.

**Collapsed state:** Show as icon buttons in a vertical strip.

### Sidebar data requirements

The sidebar needs:
- `bootstrap.projects` (filtered to focus set)
- `sessions` (all, for fleet strip and per-project indicators)
- `inboxUnreadCountByProject` (aggregated for inbox badge)
- `selectedProjectId` (for active highlight)
- `focusProjectIds` (for ordering)

All of this already exists in `appStore`. The sidebar is a pure read from existing state — no new data loading required.

### Sidebar collapse behavior

**Toggle:** `Ctrl+B` or clicking the collapse chevron in the sidebar header.

**Collapsed mode (48px wide):**
- Logo reduces to icon only.
- Project names become single-letter circles.
- Fleet section becomes a count badge on an icon.
- Quick action labels hide, leaving only icons.
- Hover on any collapsed item shows a tooltip with the full label.

**Transition:** `width` animates over 150ms with `ease`. Content fades at 120ms. Icons snap to center alignment when collapsed.

**Persistence:** Collapse state saved to `localStorage` key `emery:sidebar-collapsed`. Restored on boot.

---

## 3. Modal vs Peek Decision Matrix

Three presentation modes for content:

1. **Main content** — replaces the center area. For primary workflow views the user dwells in.
2. **Peek panel** — slides in from the right. For reference/detail views the user glances at while keeping context.
3. **Center modal** — overlays the center. For confirmatory actions, forms, configuration that blocks the workflow.

### Decision matrix

| View/Action | Current | Proposed | Why |
|---|---|---|---|
| Home (project list) | Main content | **Main content** | Primary landing — needs full space |
| Project command view | Main content | **Main content** | Primary work surface — needs full space |
| Agent terminal | Main content (full takeover) | **Main content (full-bleed)** | Terminal needs maximum space; breadcrumb provides back-path |
| Work item detail | Main content (full takeover) | **Peek panel** | User wants to see work item while looking at project. Open in peek from project view. |
| Work item detail (from sidebar/direct nav) | Main content | **Main content** | When navigating directly (not from project context), use main |
| Document editor | Main content | **Main content** | Needs focus — editing is a primary activity |
| Document viewer (read-only) | Main content | **Peek panel** | Quick reference while working on project |
| Inbox | Main content (full takeover) | **Peek panel** | User checks inbox without losing project context |
| Project settings | Main content (full takeover) | **Center modal** | Configuration form — complete and dismiss. Not a place to dwell. |
| Global settings | Main content (full takeover) | **Main content** | Has multiple tabs, needs space — or peek if simple |
| Vault | Main content (full takeover) | **Main content** | Credential management needs focus |
| Dispatch (single) | Overlay sheet | **Center modal** | Confirmatory action with form — modal is correct |
| Dispatch (multi) | Overlay sheet | **Center modal** | Same — modal is correct |
| Work item create | Inline form in project view | **Center modal** | Inline form gets lost in the list. Modal focuses attention. |
| Work item edit | Inline form in work item view | **Peek panel** or **inline** | If already in peek, edit inline within peek. If in main, modal. |
| Project create | Inline form in home view | **Center modal** | Focuses attention on the creation flow |
| Session detail (non-terminal) | N/A (doesn't exist) | **Peek panel** | Summary of a completed session — peek while reviewing merge queue |
| Merge queue diff | Inline expand | **Peek panel** | View diff in peek while merge queue list stays in main |
| Quick Chat | N/A (doesn't exist) | **Main content** or **peek panel** | See section 5 |

### Peek panel behavior

**Opening:** Any action that targets the peek panel calls `peekStore.open(content)`. If the peek panel is already showing something, it replaces the content (no stacking).

**Closing:**
- Click the X button in the peek header.
- Press `Escape` (if peek is open, Escape closes peek; if peek is closed, Escape navigates back in main).
- Press `Ctrl+\`.
- Navigate to a different main view (peek auto-closes on main navigation change to avoid stale context).

**Promotion:** If the user wants to see a peeked view at full size, a "Pop out" button in the peek header moves the content to main content (`navStore.goToWorkItem(...)` etc.). This replaces the current main content.

**Width:** Fixed at 420px. Not resizable in v1. Content inside scrolls vertically.

**Header:** Every peek panel view gets a standard header: `[Title] [Pop Out] [X Close]`.

### Center modal behavior

**Opening:** `modalStore.open(content)`. Renders above everything with a backdrop overlay.

**Closing:** Click backdrop, press Escape, or click Cancel/X in the modal.

**Sizing:** Max-width 640px, max-height 80vh, centered. Scrollable content area.

**Stacking:** No stacking. Opening a new modal replaces the current one (with a warning if the current one has unsaved state).

---

## 4. Main Content Max-Width

### The problem

Every view currently stretches to `100%` of the viewport width. On a 2560px monitor with the sidebar open, that leaves ~2300px of content width. Form fields, work item titles, and action buttons end up on opposite edges of the screen. The eye has to travel too far.

### Solution: Content frame with adaptive max-width

Every view wraps its content in a `<div className="content-frame">` that centers and constrains:

| View | Frame variant | Max-width | Rationale |
|------|--------------|-----------|-----------|
| Home | `content-frame` | 960px | Card grid and project list are comfortable here |
| Project command view | `content-frame` | 960px | Work items, planning, fleet strip all fit |
| Work item detail (in main) | `content-frame` | 960px | Form fields and descriptions |
| Document editor | `content-frame` | 800px | Prose content — narrower is better for readability |
| Settings | `content-frame` | 800px | Form-heavy — narrow is better |
| Agent terminal | `content-frame-full` | none (full bleed) | Terminal needs all horizontal space |
| Weekly planner | `content-frame-wide` | 1200px | 7-column grid needs width |
| Inbox | `content-frame` | 960px | List view |
| Vault | `content-frame` | 800px | Credential forms |

### Implementation

Each view component wraps its return in the frame:

```tsx
// In ProjectCommandView:
return (
  <div className="content-frame">
    {/* existing content */}
  </div>
);

// In AgentView:
return (
  <div className="content-frame content-frame-full">
    {/* terminal fills available space */}
  </div>
);
```

The `content-frame` class:

```css
.content-frame {
  max-width: 960px;
  margin-left: auto;
  margin-right: auto;
  width: 100%;
  padding: 0 var(--space-4);
}

.content-frame-wide {
  max-width: 1200px;
}

.content-frame-full {
  max-width: none;
  padding: 0;
}

.content-frame-narrow {
  max-width: 800px;
}
```

When peek panel is open, the main content area shrinks by 420px. The `content-frame` max-width automatically ensures content stays well-constrained even without the peek panel. No special handling needed.

---

## 5. Quick Chat Feature Spec

### What it is

A Claude agent session not tied to any project. For general questions, brainstorming, or tasks that span projects. The user said: "I should be able to easily pop open a Quick Chat."

### Where it lives

**Sidebar:** A "Quick Chat" button in the bottom quick-actions section. Always visible.

**Keyboard:** `Ctrl+Shift+N` opens a new quick chat.

### What happens when you click it

1. If no default account is set, show a small inline dialog: "Select an account" with a dropdown of available accounts and a "Set as default" checkbox. This is a one-time setup — once a default is set, future Quick Chats skip this step.
2. A new session is created via the existing `createSession` RPC with:
   - `project_id`: a special "scratch" or "general" project auto-created on first Quick Chat use.
   - `worktree_id`: the scratch project's default worktree.
   - `account_id`: the user's default account.
   - `origin_mode`: `"ad_hoc"`.
   - `title`: `"Quick Chat"` with a timestamp or incrementing number.
   - No work item association.
3. The app navigates to the agent view for the new session: `navStore.goToAgent(scratchProjectId, sessionId)`.
4. The terminal is live and ready for input.

### Account selection details

- The account picker is a small popover anchored to the Quick Chat button.
- Shows: account label, agent_kind badge.
- "Set as default" checkbox: when checked, saves the selected account ID to `localStorage` key `emery:quick-chat-default-account`. Future Quick Chats use this account automatically.
- If only one account exists, skip the picker and use it directly.

### Where transcripts go

- Quick Chat sessions belong to the "scratch" project.
- They appear in the Fleet section of the sidebar like any other session.
- Session history is accessible from the scratch project's command view (reachable via sidebar "All projects" or by navigating to the scratch project).
- Quick Chat sessions are visually distinguished in the fleet by a chat bubble icon instead of the standard agent dot.

### Scratch project

- Auto-created on first Quick Chat launch if it does not exist.
- Named "Quick Chats" (or "Scratch" — user-renameable).
- Folder path: a dedicated directory under the app's data directory (e.g., `~/.emery/scratch/`).
- Does not appear in the focus project slots by default (but can be pinned).
- Listed in "All projects" on the home view.

### Relationship to projects

Quick Chat is intentionally unscoped. If the user wants a project-scoped chat, they should dispatch from within the project using an appropriate scoped mode such as `research` or `execution`. Quick Chat is for "I have a question that isn't about any specific project."

---

## 6. Session Launch Flow Redesign

### Current problems

1. "Start" button on work items changes status to `in_progress` but does not launch anything. Users expect a terminal.
2. "Dispatch Agent" launches a session but the DispatchSheet is the only UI before it happens — then the session starts in the background with no obvious feedback if multi-dispatching.
3. After single dispatch, the app navigates to AgentView, but the user did not choose to go there — it just happens.

### New flow

#### Work item status transitions

**Rename the buttons:**

| Current label | New label | Action |
|---|---|---|
| "Start" (backlog -> in_progress) | "Move to Active" | Changes status only. No session launch. |
| "Done" (in_progress -> done) | "Mark Complete" | Changes status only. |
| "Reopen" (done -> in_progress) | "Reopen" | Unchanged — clear enough. |

**Rationale:** Separating status changes from session launches eliminates the confusion. "Start" implies starting something (an agent). "Move to Active" is clearly a status transition.

#### Launching a session

The primary CTA for launching a session is the "Launch Agent" button (renamed from "Dispatch Agent").

**From work item row in project command view:**
1. User clicks the rocket/play icon on a work item row.
2. **Dispatch modal** opens (center modal, not full overlay sheet).
3. User configures: account, safety mode, model, origin mode.
4. User clicks "Launch".
5. Session creates. A **toast notification** appears: "Agent launched for Emery-58" with a "View" action button.
6. The user stays on the project command view. The new session appears in the Fleet section (sidebar + project view fleet strip).
7. If the user clicks "View" on the toast, they navigate to the agent terminal.

**From work item detail view:**
1. Same flow — "Launch Agent" button opens the dispatch modal.
2. Toast + stay in place.

**From multi-select:**
1. User selects 2+ work items, clicks "Launch N Agents".
2. Multi-dispatch modal opens.
3. User configures and clicks "Launch All".
4. Toast: "3 agents launched" with "View Fleet" action.
5. User stays in project view. Fleet strip updates in real-time.

#### After launch — what the user sees

The key change: **launching does not auto-navigate.** The user stays where they are. The session appears in two places immediately:

1. **Sidebar fleet section** — new row with pulsing dot, title, duration counting up.
2. **Project command view fleet strip** — same.

To watch a session, the user clicks the session row in either location. This is an explicit choice, not a forced navigation.

#### Session completion notification

When a session completes (while the user is elsewhere):

1. **Toast notification:** "Emery-58 completed" (or "Emery-58 errored") with action buttons: "View Terminal", "View Merge Queue" (if applicable).
2. **Sidebar fleet section:** The session row changes from pulsing green to a static checkmark (completed) or X (errored). It stays visible for 5 minutes, then fades.
3. **Audio cue:** Existing ding/chime sounds continue.
4. **Inbox entry:** Created as before for async review.

#### Getting back to the project from a session

From AgentView, the user can:
- Click the project name in the breadcrumb.
- Click "Back to project" (ended session) or "Detach" (live session) — unchanged.
- Click a different project in the sidebar.
- Double-Escape — unchanged but add a visual hint.

The sidebar always shows the project navigator, so the user never loses orientation.

---

## 7. Naming Fixes

### "Dispatch Agent" -> "Launch Agent"

"Dispatch" is internal jargon from the orchestration model. "Launch" is what users understand. The button, modal title, and toast messages should all say "Launch."

- Button label: "Launch Agent"
- Modal title: "Launch Agent Session"
- Multi-dispatch: "Launch N Agents"
- Toast: "Agent launched for Emery-58"

**Note:** The backend terminology (dispatch, sessions, fleet) stays as-is. This is a UI label change only.

### Safety modes

Current labels are confusing. "Full" could mean "fully restricted" or "fully permissive."

| Current | New label | Description shown in UI |
|---|---|---|
| `full` | "Autonomous" | Agent can read, write, and execute without confirmation |
| `normal` | "Supervised" | Agent asks before destructive operations |
| `restricted` | "Read Only" | Agent can read files but cannot write or execute |

Show a one-line description below the dropdown when each option is selected. Do not rely on the label alone.

### Work item status transitions

| Current | New | Context |
|---|---|---|
| "Start" | "Move to Active" | backlog -> in_progress |
| "Done" | "Mark Complete" | in_progress -> done |
| "Reopen" | "Reopen" | done -> in_progress (fine as-is) |
| "Dispatch Agent" | "Launch Agent" | Opens dispatch modal |

### Model selection

Current: freeform text input.
New: **dropdown** populated from a known list of models.

The dropdown should contain:
- Models from the accounts' `agent_kind` metadata (claude-sonnet, claude-opus, claude-haiku, etc.)
- A "Custom" option that reveals a text field for arbitrary model strings.

Default selection comes from the account's configured default model (already in `agentDefaults`).

---

## 8. Navigation Model Changes

### Current model

`nav-store.ts` has a flat `NavigationLayer` union type. Every navigation replaces the entire content area. History is a linear stack.

### New model: Main + Peek + Modal

The navigation state splits into three independent channels:

```typescript
// nav-store.ts (revised)

export type MainLayer =
  | { layer: "home" }
  | { layer: "project"; projectId: string }
  | { layer: "agent"; projectId: string; sessionId: string }
  | { layer: "document"; projectId: string; documentId: string }
  | { layer: "new-document"; projectId: string; workItemId?: string }
  | { layer: "work_item"; projectId: string; workItemId: string }
  | { layer: "settings" }
  | { layer: "vault" };

export type PeekLayer =
  | null  // closed
  | { peek: "work_item"; projectId: string; workItemId: string }
  | { peek: "inbox"; projectId: string }
  | { peek: "document"; projectId: string; documentId: string }
  | { peek: "session_detail"; projectId: string; sessionId: string }
  | { peek: "merge_diff"; projectId: string; entryId: string }
  | { peek: "project_settings"; projectId: string };

export type ModalLayer =
  | null  // closed
  | { modal: "dispatch_single"; projectId: string; workItemId: string; originMode: string }
  | { modal: "dispatch_multi"; projectId: string; workItemIds: string[] }
  | { modal: "create_work_item"; projectId: string; parentId?: string }
  | { modal: "create_project" }
  | { modal: "confirm"; title: string; message: string; onConfirm: () => void };

type NavState = {
  main: MainLayer;
  peek: PeekLayer;
  modal: ModalLayer;
  mainHistory: MainLayer[];
};
```

### Navigation rules

1. **Main navigation** always pushes to `mainHistory`. Unchanged from current behavior.
2. **Peek navigation** does not affect main history. Opening a peek is independent.
3. **Modal navigation** does not affect any history. Modals are ephemeral.
4. **Main navigation change auto-closes peek.** When the user navigates to a different main view, the peek panel closes. This prevents stale peek content from a different project/context.
5. **Escape priority:** Modal (if open) -> Peek (if open) -> Main (go back).
6. **Back/forward** operates on main history only. Peek and modal are not in the back stack.

### Breadcrumb changes

The breadcrumb reflects only the **main** layer. Peek content gets its own mini-breadcrumb inside the peek panel header. Modals have no breadcrumb.

Breadcrumb additions:
- Settings: show "Emery > Settings" (currently missing from breadcrumb).
- Inbox (if opened in main rather than peek): show "Emery > {Project} > Inbox".

### navStore API changes

```typescript
export const navStore = {
  // Main navigation (existing, renamed internally)
  goHome() { ... },
  goToProject(projectId: string) { ... },
  goToAgent(projectId: string, sessionId: string) { ... },
  goToWorkItem(projectId: string, workItemId: string) { ... },
  goToDocument(projectId: string, documentId: string) { ... },
  goToNewDocument(projectId: string, workItemId?: string) { ... },
  goToSettings() { ... },
  goToVault() { ... },
  goBack() { ... },
  restore(layer: MainLayer) { ... },

  // Peek (new)
  openPeek(peek: PeekLayer) { ... },
  closePeek() { ... },

  // Modal (new)
  openModal(modal: ModalLayer) { ... },
  closeModal() { ... },

  // Existing helpers (updated)
  breadcrumbs() { ... },  // reflects main only
};
```

### Workspace persistence changes

The `WorkspacePayloadV2` already stores `navigation`. It needs to become `WorkspacePayloadV3`:

```typescript
type WorkspacePayloadV3 = {
  version: 3;
  main_navigation: MainLayer;
  // peek is NOT persisted — always starts closed
  // modal is NOT persisted — always starts closed
  focus_project_ids: string[];
  planning_view_mode: string;
  sidebar_collapsed: boolean;
};
```

Migration from V2: `navigation` becomes `main_navigation`. Everything else maps directly.

---

## 9. Implementation Plan

### Phase 0: Foundation (must ship together)

These changes are structural — they cannot be done incrementally without creating an inconsistent state.

**Work item: Emery-xxx — App shell three-zone layout**

Scope:
1. New `app-body` flex container wrapping sidebar placeholder + main content + peek placeholder.
2. Move breadcrumb inside `main-content`.
3. Add `content-frame` wrapper to every view.
4. Sidebar renders as an empty 240px column (placeholder content: just the collapse button and "Sidebar coming soon").
5. Peek panel renders as a hidden 0px column (no triggers yet).
6. All existing navigation continues to work — just visually constrained within the new layout.

**Estimate:** 1 builder session. Low risk — purely additive CSS/structure.

**Acceptance criteria:**
- App renders with three-zone layout.
- All existing views work within `content-frame`.
- Agent terminal view uses `content-frame-full`.
- Sidebar placeholder is visible and collapsible.
- No peek panel visible yet.

### Phase 1: Sidebar (ship after Phase 0)

**Work item: Emery-xxx — Sidebar project navigator + fleet**

Scope:
1. New `Sidebar` component with three sections.
2. Project navigator reads from `focusProjectIds` + `bootstrap.projects`.
3. Fleet section reads from `sessions` (global).
4. Quick actions section: Quick Chat button (placeholder — just the button, no session creation yet), Inbox, Vault, Settings links.
5. Collapse/expand with `Ctrl+B`, persisted to localStorage.
6. Active project highlighting.

**Estimate:** 1 builder session. Medium complexity — new component but reads from existing state.

**Acceptance criteria:**
- Sidebar shows focus projects with live agent indicators.
- Clicking a project navigates to it.
- Fleet section shows all running sessions globally.
- Clicking a session navigates to its agent view.
- Collapse toggle works with keyboard shortcut.
- Quick action buttons navigate to their respective views.

### Phase 2: Navigation model split (ship after Phase 1)

**Work item: Emery-xxx — Split nav-store into main/peek/modal channels**

Scope:
1. Refactor `NavigationLayer` into `MainLayer`, `PeekLayer`, `ModalLayer`.
2. Update `navStore` with `openPeek`, `closePeek`, `openModal`, `closeModal`.
3. Update `LayerRouter` to render based on `main` only.
4. Add `PeekRouter` component that renders peek content.
5. Add `ModalRouter` component that renders modal content.
6. Update Escape handling: modal -> peek -> main back.
7. Update workspace persistence to V3.

**Estimate:** 1-2 builder sessions. Higher risk — touches navigation everywhere. Needs careful testing.

**Acceptance criteria:**
- All existing main navigation works identically.
- `openPeek` renders content in the right panel.
- `openModal` renders content as a centered overlay.
- Escape closes in the correct priority order.
- Workspace persistence saves/restores main layer correctly.
- Peek auto-closes on main navigation change.

### Phase 3: Peek panel integration (ship after Phase 2)

**Work item: Emery-xxx — Wire peek panel to work items, inbox, diffs**

Scope:
1. Work item row click in project command view -> `openPeek({ peek: "work_item", ... })` instead of `navStore.goToWorkItem(...)`.
2. Inbox button -> `openPeek({ peek: "inbox", ... })` instead of full navigation.
3. Merge queue diff expand -> `openPeek({ peek: "merge_diff", ... })` instead of inline expand.
4. Peek panel "Pop Out" button promotes peek to main navigation.
5. Peek panel header with title, pop-out, close.

**Estimate:** 1-2 builder sessions. Medium complexity.

### Phase 4: Modal integration (ship after Phase 2)

**Work item: Emery-xxx — Move dispatch + create forms to center modal**

Scope:
1. Replace `DispatchSheet` overlay with modal-based dispatch.
2. Move `ProjectCreateForm` from inline in HomeView to a modal.
3. Move `WorkItemForm` (create mode) from inline to a modal.
4. Update `ProjectSettingsView` to render as a modal when opened from project command view.

**Estimate:** 1 builder session. Mostly moving existing components into modal wrappers.

### Phase 5: Session launch flow (ship after Phase 4)

**Work item: Emery-xxx — Session launch flow redesign**

Scope:
1. Remove auto-navigation after single dispatch (stay in place, show toast).
2. Add toast notification with "View" action after launch.
3. Add session completion toast notifications.
4. Implement "recently completed" rows in sidebar fleet (5-minute fade).
5. Rename "Dispatch Agent" to "Launch Agent" throughout.
6. Rename status transition buttons.

**Estimate:** 1 builder session.

### Phase 6: Naming and polish (ship after Phase 5)

**Work item: Emery-xxx — Naming fixes, safety mode labels, model dropdown**

Scope:
1. Safety mode label changes + descriptions.
2. Model selection from freeform to dropdown with "Custom" escape hatch.
3. Add agent view hint for double-Escape.
4. Add work item callsign as clickable link in AgentInfoBar.

**Estimate:** 1 builder session. Low risk — pure UI label/control changes.

### Phase 7: Quick Chat (ship after Phase 1)

**Work item: Emery-xxx — Quick Chat feature**

Scope:
1. Auto-create scratch project on first Quick Chat.
2. Account selection popover with "set as default" persistence.
3. Session creation with `origin_mode: "ad_hoc"`.
4. Navigation to agent view after creation.
5. Chat bubble icon distinction in sidebar fleet.

**Estimate:** 1 builder session. Moderate — needs scratch project auto-creation.

### Dependency graph

```
Phase 0 (shell layout)
  |
  v
Phase 1 (sidebar)
  |
  +---> Phase 7 (quick chat, independent of 2-6)
  |
  v
Phase 2 (nav model split)
  |
  +---> Phase 3 (peek integration)
  |
  +---> Phase 4 (modal integration)
  |         |
  |         v
  |     Phase 5 (launch flow)
  |         |
  |         v
  |     Phase 6 (naming/polish)
  v
(complete)
```

### What NOT to change yet

- **Agent terminal view** — stays as full-bleed main content. No split terminal views in v1.
- **Document editor** — stays as main content. No peek editing.
- **Planning section** — stays in project command view. No sidebar planning widget.
- **Multi-session monitoring** — no split/tiled terminal view. One session at a time. Fleet indicators are the monitoring surface.
- **Drag-and-drop for sidebar sections** — no reordering sections in v1.

---

## Appendix A: State Management Additions

### New stores

**`sidebar-store.ts`**

```typescript
type SidebarState = {
  collapsed: boolean;
  fleetFilter: "running" | "recent" | "all";
};
```

Simple store. Persisted to localStorage. Read by `Sidebar` component.

**`peek-store.ts`**

Integrated into `nav-store.ts` as the `peek` field. Not a separate store — peek state is navigation state.

**`modal-store.ts`**

Integrated into `nav-store.ts` as the `modal` field. Same rationale.

### Existing store changes

**`appStore`** — no changes needed. Sidebar reads from existing fields.

**`navStore`** — expanded as described in section 8. The `NavigationLayer` type splits into three types. All existing `goTo*` methods continue to work on the main channel.

**`toastStore`** — needs a small addition: toast actions should support navigation callbacks (e.g., "View" button that calls `navStore.goToAgent(...)`). The existing `action: { label, onClick }` pattern already supports this.

---

## Appendix B: Accessibility Notes

- Sidebar collapse should set `aria-expanded` on the toggle button.
- Peek panel should use `role="complementary"` with `aria-label="Detail panel"`.
- Modal should use `role="dialog"` with `aria-modal="true"` and trap focus.
- Sidebar navigation items should be keyboard-navigable (arrow keys to move, Enter to activate).
- `Ctrl+B` shortcut should not conflict with browser bold (not applicable in Tauri desktop app).
- All new interactive elements need visible focus indicators (already established in the existing CSS token system).

---

## Appendix C: Visual Reference — Component Tree

```
App
 +-- Topbar
 |    +-- BrandBlock (logo + connection dot)
 |    +-- StatusStrip (connection, live count, debug)
 |
 +-- AppBody
 |    +-- Sidebar
 |    |    +-- SidebarHeader (logo icon + collapse toggle)
 |    |    +-- ProjectNavigator
 |    |    |    +-- FocusProjectRow (per project)
 |    |    |    +-- AllProjectsLink
 |    |    +-- FleetSection
 |    |    |    +-- FleetSidebarRow (per live session)
 |    |    |    +-- RecentlyCompletedRow (per recent completion)
 |    |    +-- QuickActions
 |    |         +-- QuickChatButton
 |    |         +-- InboxButton (with badge)
 |    |         +-- VaultButton
 |    |         +-- SettingsButton
 |    |
 |    +-- MainContent
 |    |    +-- Breadcrumb
 |    |    +-- ErrorBanner / ConnectionBanner
 |    |    +-- MainContentInner
 |    |         +-- LayerRouter (content-frame wrapped)
 |    |
 |    +-- PeekPanel
 |         +-- PeekHeader (title + pop-out + close)
 |         +-- PeekRouter (renders peek content)
 |
 +-- ModalOverlay (when modal is open)
 |    +-- ModalRouter (renders modal content)
 |
 +-- ToastStack
```
