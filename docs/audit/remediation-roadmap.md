# Remediation Roadmap

Last updated: April 15, 2026

## Goal

Repair the app in a sequence that increases whole-system cohesion instead of producing another layer of local fixes.

Use [master-checklist.md](master-checklist.md) as the pre-repair gate. The roadmap sequences repair work; the checklist defines what must be known before broad code changes begin.

## Wave 0: Finish The Audit Gate

Priority: highest

- Keep `docs/audit/` current as the canonical repair-planning workspace.
- Maintain [defect-repro-playbook.md](defect-repro-playbook.md) and execute the highest-risk smoke paths.
- Use [ownership-decisions.md](ownership-decisions.md) as the authority contract for the first repair wave.
- Convert the remaining structural issues into explicit work items before implementation starts.
- Track the implementation queue under `PJTCMD-76` and children `PJTCMD-76.01` through `PJTCMD-76.08`.

Exit criteria:

- The first repair wave can start without reopening basic ownership arguments.

## Wave 1: Land Authority-Boundary Repairs First

Priority: highest

- Normalize route and target authority around `uiSlice` and `sessionSlice`.
- Collapse recovery-detail behavior onto one recovery workflow contract instead of letting banner, terminal, and history surfaces drift.
- Reduce refresh initiation paths to the documented ownership chain before cleaning up dependent views.
- Clarify backend authority boundaries before changing workflow or restore behavior.

Exit criteria:

- The app has one clear owner for routing, terminal target, recovery contract, refresh follow-up, and backend lifecycle policy.

## Wave 2: Frontend Cohesion Cleanup

Priority: high

- Keep shell/panel cleanup limited to shared-shell extraction work that still materially reduces drift.
- Prefer the existing shared primitives (`Tabs`, `PanelState`, `VirtualList`) over new one-off panel infrastructure.
- Only extract a shared backlog/documents split-pane shell if the two surfaces are expected to keep evolving together.

Exit criteria:

- The shell, major panels, and settings surfaces share one clear interaction model.

## Wave 3: Backend Hardening And Correctness

Priority: high

- Reduce `db.rs` to persistence and narrow helper responsibilities.
- Make workflow-run start isolation explicitly transactional.
- Review session runtime metadata updates for read-then-write race windows.
- Keep launch-time vault access auditing fail-closed.
- Keep session-host launch cleanup guard-based so secret-file and MCP-config cleanup do not drift by branch.

Exit criteria:

- Backend authority boundaries are explicit and concurrency-sensitive paths are easier to reason about.

## Wave 4: Verification Expansion And Release Confidence

Priority: high

- Add frontend integration coverage for shell routing, tabs, and key operator paths.
- Add workflow-run integration coverage on the supervisor path.
- Add restore-path smoke coverage across app restart.
- Add a doc-drift checklist or script for top-level architecture docs.

Exit criteria:

- The docs that describe the app are backed by tests or automated checks often enough to stay true.

## Parallel Workstreams

The work can run in parallel if ownership stays disjoint:

- Workstream A: remaining audit artifacts, repro execution, and work-item conversion
- Workstream B: frontend authority-boundary fixes
- Workstream C: frontend cohesion cleanup after boundary fixes land
- Workstream D: backend lifecycle / restore / workflow correctness hardening
- Workstream E: verification expansion and doc-drift automation

Each workstream should update the defect register and verification matrix as it lands.

## First Narrow Repair Wave

Completed on April 15, 2026 before broader cleanup:

1. Fix target/view authority drift around `activeView` and `selectedTerminalWorktreeId`.
2. Fix recovery-detail authority so one path owns detail loading and action selection.
3. Remove feature-local refresh policy from `WorkflowsPanel` or move it behind the shared refresh gateway.

This wave is intentionally narrow because later UI cleanup depends on these boundaries being stable first.

Next:

1. Clarify backend session authority boundaries before broader runtime changes land.
2. Reduce `db.rs` to persistence and narrow helper responsibilities.
3. Review session runtime metadata updates for any remaining read-then-write race windows.

Rollback point for future frontend cleanup:

- If Wave 1 destabilizes shell routing, active terminal selection, or recovery actions, stop after the authority-layer changes and do not continue into panel cleanup or backend extraction in the same branch.
