# Frontend Overhaul Brief

## Goal

Redesign the Project Commander frontend so it feels minimal, fast, and terminal-first.

This is a UI overhaul, not a product or backend rewrite.

The runtime layer is now stable enough that the frontend can be reworked, but the
redesign must preserve the current supervisor, session, MCP, and database behavior.

## Hard Boundaries

The redesign must not change the architecture of:

- the Rust supervisor
- session launch / terminate / reattach behavior
- MCP attachment and tool names
- database schema
- Tauri storage locations
- work-item and document backend contracts

Frontend changes should stay inside:

- `src/App.tsx`
- `src/index.css`
- `src/components/`
- light frontend wiring in `src/types.ts` if needed

Avoid changing Rust or Tauri code unless the redesign is blocked without a small,
clearly justified command or type addition.

## Target Information Hierarchy

The app should only show the minimum necessary structure:

1. Left panel: registered projects
2. Center panel: main tabbed workspace
3. Right panel: live sessions and worktree sessions

Everything else should either be removed, collapsed, or moved behind clear actions.

## Required Layout

### Left Panel

Purpose: project switching only.

Must contain:

- list of registered project names
- selected project state
- `Add project` button

Must not contain:

- project details
- metrics
- document counts
- root paths
- large forms open by default
- extra dashboard widgets

Behavior:

- clicking a project selects it
- selecting a project should switch the center panel to the `Terminal` tab
- if that project has a live session, show the live terminal immediately
- if it does not, show the launch state for that project

### Center Panel

Purpose: primary workspace.

Must be tabbed.

Required tabs:

- `Terminal`
- `Project Overview`
- `Work Items`

Terminal tab behavior:

- default tab when a project is selected
- if the selected project has a live session, show the terminal directly
- if it does not, show a minimal launch state with `Launch terminal`
- terminal should remain the visual priority of the app

Project Overview tab:

- keep this minimal
- use it for concise project-level context only
- do not turn it into a dashboard
- it can include root path, selected launch profile, recent summary info, or other
  essential project context, but only if presented cleanly

Work Items tab:

- work-item list and editing flow
- optimize for fast selection and action
- keep linked context clear, but do not overload the screen

### Right Panel

Purpose: runtime session visibility.

Must contain:

- live sessions at the top
- worktree sessions below

This panel is reserved for session runtime state. Do not turn it into a second
overview/dashboard column.

For the current slice, worktree sessions can be a placeholder or empty-state area
if the real worktree feature is not implemented yet, but the layout should reserve
space for it cleanly.

## Collapsible Panels

All major panels should be collapsible:

- left project rail
- right session rail

Collapsing a side panel should expand the center panel.

The collapse behavior should feel intentional and stable:

- obvious control affordance
- no layout jitter
- no confusing hidden state

## Visual Direction

The interface should feel:

- minimal
- dense but readable
- terminal-native
- calm, not busy
- operational, not “dashboard SaaS”

Use the Emery screenshot as directional inspiration for tone, density, and
terminal-first composition, but do not blindly copy Emery’s exact UI or its
mistakes.

Strong preferences:

- dark terminal-forward presentation
- restrained accent usage
- mono-forward typography where it makes sense
- tight spacing
- flatter, cleaner surfaces
- very low chrome

Avoid:

- oversized cards
- generic marketing-style spacing
- stacked admin dashboard boxes
- showing every possible control at once
- decorative UI that competes with the terminal

## Behavioral Constraints

The redesign must preserve these behaviors:

- selecting a project updates the center workspace to that project
- the terminal tab shows the real live session if one exists
- `Stop session` and `Launch terminal` behavior stays intact
- work-item flows still operate against the current project
- MCP-backed Claude sessions still work without extra UI steps
- installed app and packaged exe flows still work

The redesign should not introduce:

- extra required setup steps
- hidden session state
- ambiguous launch behavior
- duplicate controls for the same action

## Explicit Simplification Rules

The UI should only show what is needed for the current user task.

That means:

- left rail stays project-only
- center stays task/workspace-only
- right rail stays session-only

If a surface does not clearly support one of those three purposes, remove it,
collapse it, or move it behind an action.

## Responsive Behavior

Desktop is the priority.

If space gets tight:

- collapse the right rail first
- then allow the left rail to collapse
- keep the center terminal usable

Do not compromise the terminal tab to preserve decorative layout elements.

## Out Of Scope For This Overhaul

Do not add any of the following as part of the redesign:

- new planning entities
- workflow execution
- worktree backend behavior
- new MCP tools
- new database tables
- supervisor orchestration changes
- agent pod systems
- dashboard analytics

This is a shell and interaction overhaul only.

## Suggested Implementation Order

1. Replace the app shell and layout structure.
2. Make the three-panel layout with collapsible side rails.
3. Make the center panel tabs the primary navigation.
4. Simplify the project list rail to names plus add action only.
5. Simplify the terminal launch state.
6. Rework work items into the new center tab flow.
7. Add the right-side runtime rail for live sessions and worktree sessions.
8. Polish density, spacing, and visual hierarchy.

## Acceptance Criteria

The redesign is acceptable when:

- selecting a project drops the user into `Terminal`
- left rail shows project names and `Add project`, nothing noisy
- center is clearly tabbed with `Terminal`, `Project Overview`, and `Work Items`
- right rail is clearly about live sessions and worktree sessions only
- left and right rails are collapsible
- the interface feels substantially more minimal than the current UI
- the terminal remains the dominant interaction surface
- session launch / stop / reopen behavior still works
- Claude MCP workflow still works after the redesign

## Handoff Prompt

Use this if handing the redesign to another LLM:

```text
Redesign only the React/CSS frontend of Project Commander.

Hard constraints:
- Do not rewrite the Rust supervisor, MCP integration, DB schema, or session architecture.
- Keep backend contracts and behavior intact.
- Prefer changes in src/App.tsx, src/index.css, and src/components/.

Target layout:
- Left collapsible rail: registered project names only, plus Add project.
- Center primary workspace with tabs: Terminal, Project Overview, Work Items.
- Right collapsible rail: live sessions on top, worktree sessions below.

Behavior:
- Clicking a project should switch to the Terminal tab.
- If that project has a live session, show the live terminal immediately.
- If not, show a minimal launch state with Launch terminal.
- Keep the UI minimal, terminal-first, and dense.
- Remove clutter and non-essential surfaces.

Use the Emery screenshot as aesthetic inspiration for tone and density, but do not copy it literally.
```
