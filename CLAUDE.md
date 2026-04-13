# Project Commander — House Rules for Claude

## Start Here

Before making non-trivial changes, read:

1. `A_PLUS_TRACKER.md`
2. `AGENTS.md`
3. `docs/a-plus-delivery.md`
4. `docs/refresh-architecture.md` if the change touches refresh or runtime invalidation
5. `docs/observability.md` if the change touches supervisor/runtime behavior

The app is currently at an `A+` baseline. Claude work should preserve that bar, not just make the requested change pass locally.

## A+ Guardrails

- Do not reintroduce broad polling, global refresh fan-out, or avoidable store churn.
- Do not collapse the modular workspace-shell structure back into large orchestration components.
- Do not bypass the shared async-state patterns in `src/components/ui/panel-state.tsx`.
- Do not remove or weaken existing performance budgets, smoke coverage, repo-health checks, or observability on hot paths.
- Keep `A_PLUS_TRACKER.md` and the relevant docs in sync when the quality contract changes.

## UI Conventions

### Tabs

Any tabbed surface **must** use the shared `Tabs` primitive at
`src/components/ui/tabs.tsx`. Do not hand-roll tab buttons, `activeView`
conditionals styled with `workspace-tab` classes, or ad-hoc button rows that
visually imitate tabs.

Required API:

```tsx
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'

<Tabs value={view} onValueChange={setView}>
  <TabsList>
    <TabsTrigger value="a">A</TabsTrigger>
    <TabsTrigger value="b">B</TabsTrigger>
  </TabsList>
  <TabsContent value="a">…</TabsContent>
  <TabsContent value="b">…</TabsContent>
</Tabs>
```

Visual styling lives in `src/index.css` under `.workspace-tab` /
`.workspace-tab--active`. `TabsTrigger` applies those classes automatically —
don't duplicate them on consumers.

If a surface has a tabbed nav plus right-side status chrome (e.g. project
name + live indicator), wrap everything in `<Tabs>` using
`className="contents"` so the Tabs provider doesn't impose its own layout,
and place `<TabsList>` alongside the status in a plain `<nav>`. See
`src/components/WorkspaceShell.tsx` for the canonical example.

**Do not** introduce another tabs component, another styling system for
tabs, or inline "fake tabs" built from `Button`s. If the shared primitive is
missing a capability you need, extend it — don't fork.

## Bug Logging

We are building Project Commander itself. Any bug, unexpected behavior, or
workaround encountered during development **must** be logged:

- Call `list_work_items` first to avoid duplicates
- Then `create_work_item(itemType: 'bug')` with clear repro steps
- Do this before continuing work — don't silently paper over issues
