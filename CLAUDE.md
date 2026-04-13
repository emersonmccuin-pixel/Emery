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

## Diagnostics Workflow

Project Commander now has an always-on diagnostics pipeline while the desktop
app process is running. Claude should use it by default when debugging
slowdowns, refresh churn, crashes, launch failures, or “something feels off”
reports.

### Rebuild / Run

- Verify the repo with:
  `npm test`
  `npm run build`
  `cargo test --manifest-path src-tauri/Cargo.toml`
- Run the current local desktop build with:
  `npm run tauri:dev`
- Build and run a release build with:
  `npm run prod:build`
  `npm run prod:run`
- Diagnostics start automatically with the app; open `Settings -> Diagnostics`
  after launch.

### Use The In-App Diagnostics Console First

- Open `Settings -> Diagnostics`
- Use `Live Feed` to inspect:
  - frontend perf spans
  - Tauri command exchanges and durations
  - correlation IDs, app-run IDs, and captured selection context
  - targeted refresh reasons
  - terminal event traffic
  - unhandled frontend errors / promise rejections
  - streamed supervisor log lines
- Use `Supervisor Log` to inspect the live `supervisor.log` tail and resolved
  runtime/log/crash-artifact paths
- Use the console’s filters and copy/export actions before asking the user to restart the
  app if the issue is currently reproduced

### Persisted Artifacts To Inspect

When the issue must be debugged after the fact, use the persisted runtime
artifacts documented in `docs/observability.md`:

- `<app-data>/db/logs/diagnostics.ndjson`
- `<app-data>/db/logs/diagnostics.prev.ndjson`
- `<app-data>/db/logs/supervisor.log`
- `<app-data>/db/logs/supervisor.prev.log`
- `<app-data>/crash-reports/<session-id>.json`
- `<app-data>/session-output/<session-id>.log`

### History Behavior

- The live diagnostics feed is still bounded in memory for UI cost control
- Recent diagnostics history is also persisted to rolling NDJSON logs, so
  prior-run context survives app restarts
- The in-app feed hydrates from that persisted recent history on startup
- Use `appRunId` and project/source filters to isolate the session you care
  about before drawing conclusions
- For deep history beyond the current in-app window, inspect the raw
  `diagnostics*.ndjson` files directly

### Debugging Expectation

When the user asks for help debugging:

- check the in-app diagnostics console if the app is still running
- filter by run, project, source, or `slow only` before scanning raw logs
- inspect persisted supervisor/crash/session-output artifacts for prior runs
- prefer exporting a diagnostics bundle when the user needs to preserve a bad
  session for later analysis
- correlate slow or failed UX with the recorded Tauri invokes, refresh events,
  and supervisor log lines before guessing
