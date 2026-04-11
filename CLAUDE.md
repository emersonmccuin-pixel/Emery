# Project Commander — House Rules for Claude

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
