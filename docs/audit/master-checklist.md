# Master Repair-Readiness Checklist

Last updated: April 15, 2026

## Purpose

This is the single checklist to work through before broad code-repair starts. The goal is to enter the repair phase with one clear system model, one clear session model, a triaged defect list, and an explicit verification plan.

Use this as the gate between "we are still learning how the app behaves" and "we are ready to change behavior on purpose."

## How To Use This Checklist

- Treat this file as the top-level checklist for the audit workspace.
- Update linked dossiers and the defect register as evidence is gathered.
- Only mark an item complete when there is a concrete artifact, repro, test, or doc update behind it.
- Prefer finishing one session family end to end before starting the next one.

## Phase 1: Lock The Current Mental Model

- [x] Reconcile the top-level repo map so `README.md` matches the shipped architecture, shell layout, refresh model, and persistence model.
- [x] Keep [01-system-map.md](01-system-map.md) current enough that a new reviewer can identify the major app subsystems without reverse-engineering the repo.
- [x] Keep [sessions/session-lifecycle.md](sessions/session-lifecycle.md) current enough that launch, runtime, exit, recovery, and cleanup behavior are described in one place.
- [x] Keep [sessions/session-type-matrix.md](sessions/session-type-matrix.md) current enough that each session family has direct and indirect system touchpoints documented.
- [x] Make sure each major segment dossier identifies its authority boundary, main handoffs, and strongest cohesion risks.

## Phase 2: Audit Each Session Family As A System

- [x] Audit the dispatcher session from UI entry to supervisor launch, live runtime, exit, and recovery.
- [x] Audit the worktree-agent session from worktree selection to launch, work-item coupling, cleanup, and recovery.
- [x] Audit the workflow-stage session from workflow dispatch to per-stage launch, persistence, failure handling, and teardown.
- [x] Audit the recovery-and-history session path from failed/interrupted/orphaned state to resume, relaunch, or termination.
- [x] Audit the bootstrap-and-restore-context path from startup state through restore preparation, restart, and post-restore session behavior.
- [x] For each session family, split the dossier into `expected behavior`, `actual behavior`, `confirmed mismatches`, and `open questions`.
- [x] For each session family, document which systems can break it indirectly even when the launch path itself looks correct.

## Phase 3: Capture Evidence Instead Of Assumptions

- [x] Record exact repro steps for each confirmed session defect.
- [x] Capture the expected logs, diagnostics, history rows, and artifacts for each major session path.
- [x] Identify which findings are code-visible only versus reproduced in a running flow.
- [x] Annotate each material issue as `confirmed`, `likely`, or `structural` in [defect-register.md](defect-register.md).
- [x] Group defects by shared root cause instead of treating every symptom as an isolated bug.

## Phase 4: Decide Ownership Before Fixing Behavior

- [x] Decide which frontend layer is authoritative for target routing and active terminal ownership.
- [x] Decide which layer is authoritative for recovery-detail presentation and recovery actions.
- [x] Decide which layer owns refresh follow-up after session launch, exit, workflow progression, and recovery.
- [x] Decide which backend boundary is authoritative for session lifecycle orchestration versus persistence versus diagnostics.
- [x] Decide which policy questions are real correctness requirements before repair starts.
  Policy examples: fail-open versus fail-closed vault audit logging, restore token durability, workflow-start isolation guarantees.

## Phase 5: Close The Highest-Risk Unknowns

- [x] Confirm or disprove `AUD-008` with a targeted workflow-run concurrency test.
- [x] Confirm or harden `AUD-013` so latest-session lookup does not depend on unstated ordering assumptions.
- [x] Trace `AUD-014` and name one authoritative recovery-detail path.
- [x] Break down `AUD-015` into explicit session-system boundaries so later refactors do not stay conceptual.
- [x] Confirm whether `AUD-016` can leave workflow state partially advanced after a crash or interrupted write.
- [x] Review whether restore-path and vault-audit risks should be treated as correctness bugs or accepted operational constraints.

## Phase 6: Define The Repair Waves

- [x] Sequence repairs so authority-boundary fixes land before dependent UI cleanup.
- [x] Separate docs-only corrections, verification expansion, frontend cohesion fixes, backend hardening, and session-behavior fixes into distinct waves.
- [x] Identify which fixes can safely run in parallel without overlapping write ownership.
- [x] Convert structural issues into explicit work items before implementation starts.
- [x] Update [remediation-roadmap.md](remediation-roadmap.md) so the first repair wave is narrow, testable, and ordered by dependency instead of convenience.

## Phase 7: Lock Verification Before Touching Core Paths

- [x] Define the required smoke path for each session family.
- [x] Identify the missing automated coverage for shell routing, workflow-run isolation, restore flow, and recovery/resume behavior.
- [x] Decide which defects require frontend integration coverage, which require Rust integration tests, and which require both.
- [x] Make sure the verification matrix reflects the real gaps before repair work starts.
- [x] Confirm the repo-health, build, and targeted Rust/supervisor checks that will gate each repair wave.

## Ready To Attack Code When

- [x] The current architecture docs are accurate enough to serve as the working spec.
- [x] Every session family has a current dossier with `expected vs actual` behavior.
- [x] The defect register has been triaged into confirmed defects, structural issues, and policy decisions.
- [x] Ownership is explicit for routing, refresh, recovery details, and session lifecycle orchestration.
- [x] The first repair wave has a narrow scope, named verification gates, and a clear rollback point.
- [x] The team can explain not just what is broken, but why it is broken and which boundary must change to fix it.
- [x] The remaining targeted correctness risks (`AUD-008`, `AUD-016`) are either proven by regression tests or scoped tightly enough that implementation will not guess at their behavior.
