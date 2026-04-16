# Ownership Decisions

Last updated: April 15, 2026

## Purpose

This document turns the audit from description into authority. It names which layer should own each cross-cutting session concern before repair work starts.

## Decision Rules

- One concern gets one authoritative layer.
- Presentation surfaces can request actions, but they should not invent their own policy.
- Aggregation helpers may exist, but they should not become second authorities.
- If a concern crosses frontend and backend, the ownership handoff must be explicit.

## Frontend Decisions

| Concern | Current split | Authoritative layer going forward | Supporting layers | Must not own |
| --- | --- | --- | --- | --- |
| Route state and active view | `WorkspaceNav`, `WorkspaceShell`, `uiSlice`, `App.tsx` all participate indirectly | `uiSlice` owns state; `WorkspaceNav` and `WorkspaceShell` are the only interactive writers | `WorkspaceActiveStage` reads and renders | `App.tsx` must not own route policy |
| Active terminal target and target-focus transitions | `sessionSlice`, session/history actions, worktree/work-item flows, shell shortcuts | `sessionSlice` | `SessionRail`, `TerminalStage`, history/recovery actions call `sessionSlice` actions | Other slices and UI surfaces must not write `selectedTerminalWorktreeId` directly |
| Recovery detail loading and action contract | `historySlice`, `recoverySlice`, `HistoryStage`, `TerminalStage`, `RecoveryBannerHost` | `historySlice` owns the recovery workflow contract | `recoverySlice` caches detail payloads; `SessionRecoveryInspector` renders them | `TerminalStage` and `RecoveryBannerHost` must not invent separate recovery policy |
| Refresh follow-up after launch, exit, workflow progression, and recovery | `App.tsx`, `uiSlice`, mutation slices, `WorkflowsPanel` | Mutating slice decides refresh intent; `uiSlice.refreshSelectedProjectData` is the shared aggregation gateway | `App.tsx` adapts external runtime events into targeted refresh requests | Feature panels must not own durable refresh policy |

## Frontend Boundary Notes

- `App.tsx` can remain the bridge for app-level event listeners, but it should translate runtime events into store actions instead of carrying its own refresh policy.
- `WorkspaceNav` should reflect route state and normalize invalid values, not act as a second route model.
- `TerminalStage`, `HistoryStage`, and `RecoveryBannerHost` should become presenters of one recovery contract rather than peers with overlapping policy.

## Backend Decisions

| Concern | Current split | Authoritative layer going forward | Supporting layers | Must not own |
| --- | --- | --- | --- | --- |
| Tauri bootstrap and command edge | `lib.rs` mixes wiring, command forwarding, diagnostics, and some orchestration glue | `lib.rs` stays the Tauri facade only | diagnostics helpers, command timing helpers | `lib.rs` must not own business policy or recovery decisions |
| Supervisor transport, polling, and runtime discovery | `session.rs` already owns most of this | `session.rs` | `lib.rs` forwards commands into it | `session.rs` must not decide persistence or lifecycle semantics |
| Live session lifecycle | split across `session_host.rs`, `db.rs`, supervisor startup code | `session_host.rs` through `SessionRegistry` | supervisor routes call into it; DB persists what it decides | `db.rs` must not be the lifecycle-policy authority |
| Persistence and query shape | `db.rs` plus scattered policy assumptions elsewhere | `db.rs` | higher layers compose its operations | `db.rs` must not encode recovery or orchestration policy beyond data integrity rules |
| Startup reconciliation and route orchestration | supervisor bin mixes routing with lifecycle policy | `project-commander-supervisor.rs` as orchestrator only | delegates lifecycle policy to `SessionRegistry` / app state services | the supervisor route layer must not duplicate lifecycle rules inline |
| Shared transport contract | `session_api.rs` | `session_api.rs` | all transport callers | `session_api.rs` must remain behavior-free |

## Backend Boundary Notes

- Recovery and restore policy are currently too distributed. Repairs should move toward one startup/reconciliation policy chain instead of repeating decisions in `lib.rs`, `db.rs`, `backup.rs`, and the supervisor binary.
- `db.rs` can keep data-integrity helpers, but lifecycle-state transitions should be initiated by the runtime/session authority, not by ad hoc callers.

## Policy Decisions Before Repair

| Question | Decision | Why it matters | Resulting priority |
| --- | --- | --- | --- |
| Should restore authorization survive app restart until commit or cancel? | Yes. This is a correctness requirement, not an optional convenience. | Restore is explicitly a staged multi-step operator flow. Losing authorization while leaving staged data on disk is a broken contract. | Treat `AUD-009` as a correctness bug |
| Must restore apply target the same DB path the runtime later opens? | Yes. This is a correctness requirement. | A restore that writes to a different SQLite file does not restore the live app state. | Treat `AUD-017` as a correctness bug |
| Should workflow-run start enforce isolation transactionally? | Yes. This is a correctness requirement. | Operators should not be able to create overlapping active runs through race windows. | Treat `AUD-008` as correctness, not just hardening |
| Should workflow stage/run progression be crash-safe as one logical transaction boundary? | Yes. | Partial advancement corrupts the meaning of run status and stage history. | Treat `AUD-016` as correctness, not just resilience |
| Should vault access auditing be allowed to fail open on successful secret delivery? | No for launch-time access auditing. Successful secret delivery without durable audit evidence is not acceptable as the long-term contract. | Vault access is part of the operator trust model. | Implemented on April 15, 2026: launch-time audit now fails closed before child-process spawn |
| Should recovery details have one authoritative user-facing control plane? | Yes. | Operators need one place that decides what can be resumed, relaunched, or terminated. | Treat `AUD-014` as a structural issue that blocks clean repair planning |

## Immediate Consequences

- Future repair waves should change ownership first, then clean up dependent UI or persistence code.
- New fixes should route through these authorities even if the underlying storage shape does not change yet.
- Any new test plan should assert the boundary, not just the symptom.
