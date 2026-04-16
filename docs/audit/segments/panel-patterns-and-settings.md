# Panel Patterns And Settings

Last updated: April 15, 2026

## Scope

- `src/components/*Panel.tsx`
- `src/components/AppSettingsPanel.tsx`
- `src/components/ui/*`

## Current Responsibilities

- Top-level panels provide the main operator surfaces for backlog, documents, history, configuration, workflows, diagnostics, and overview/tracker editing.
- Shared UI primitives such as `Tabs`, `PanelState`, and `VirtualList` are intended to be the common language for navigation, async states, and large-list rendering.

## Authority Boundary

- Top-level panel components own operator-specific interaction flows.
- Shared UI primitives under `src/components/ui/*` own the common contracts for tabs, async states, and large-list rendering.
- `ConfigurationPanel.tsx` should remain project-scoped, while `AppSettingsPanel.tsx` should remain app-scoped even if the chrome looks similar.

## Main Handoffs

- shell navigation -> stage mount -> panel selectors and actions
- panel mutation intent -> store or invoke path -> refreshed panel state
- shared primitive contract -> panel-specific layout and content

## Findings

### Confirmed

- `WorkItemsPanel.tsx` and `DocumentsPanel.tsx` implement very similar split-pane CRUD flows without a shared shell or shared panel primitive.
- `DocumentsPanel.tsx` now follows the shell’s full-height flex layout instead of a fixed-height outer shell, so the remaining cohesion issue is pattern duplication rather than basic layout mismatch.

### Structural

- `ConfigurationPanel.tsx` and `AppSettingsPanel.tsx` use similar tabbed chrome even though one is project-scoped and the other is app-scoped, which blurs ownership for operators.
- `AppSettingsPanel.tsx` is now a shell plus tab-owned modules, so the remaining panel-cohesion question is whether backlog/documents should share a common split-pane shell.
- Shared UI primitives exist, but not every list or panel pattern actually routes through them.

## Why This Matters

Users perceive "lack of cohesion" most quickly in panels. When similar screens behave differently, the app feels improvised even if the underlying data is correct.

## Immediate Follow-Up

- Extract a shared split-pane CRUD shell if backlog and documents are expected to keep the same interaction model.
