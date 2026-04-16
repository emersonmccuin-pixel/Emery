# Audit Index

Last updated: April 15, 2026

## Purpose

This folder is the audit workspace for Project Commander. It exists to answer four questions in one place:

- What are the current app segments?
- Where is cohesion weak?
- Which problems are confirmed defects versus structural risks?
- What should be repaired first so the app works as one system again?

## How To Use This Folder

1. Start with [master-checklist.md](master-checklist.md).
2. Read [01-system-map.md](01-system-map.md).
3. Read the relevant segment or session dossier under [segments/](segments/) or [sessions/](sessions/).
4. Promote confirmed issues into [defect-register.md](defect-register.md).
5. Use [verification-matrix.md](verification-matrix.md) to see what is actually covered.
6. Sequence repair work from [remediation-roadmap.md](remediation-roadmap.md).

## Current Snapshot

- The shell is more modular than the old monolith, but `src/App.tsx` is still a central orchestration hub.
- Refresh ownership is documented, but operationally split across `App.tsx`, store slices, and feature panels.
- Backend boundaries have widened as workflows, vault, backup, and embeddings were added on top of the same runtime.
- Documentation drift is now concentrated more in restore/runtime notes and ongoing audit maintenance than in the top-level repo map, which has been refreshed.
- Verification is strongest around core Rust recovery/bootstrap paths and weaker around frontend orchestration and newer feature slices.

## Core Artifacts

- [master-checklist.md](master-checklist.md)
- [defect-repro-playbook.md](defect-repro-playbook.md)
- [ownership-decisions.md](ownership-decisions.md)
- [session-smoke-playbook.md](session-smoke-playbook.md)
- [01-system-map.md](01-system-map.md)
- [defect-register.md](defect-register.md)
- [verification-matrix.md](verification-matrix.md)
- [remediation-roadmap.md](remediation-roadmap.md)

## Session Audits

- [sessions/00-index.md](sessions/00-index.md)
- [sessions/session-lifecycle.md](sessions/session-lifecycle.md)
- [sessions/session-type-matrix.md](sessions/session-type-matrix.md)
- [sessions/session-audit-checklist.md](sessions/session-audit-checklist.md)
- [sessions/dispatcher.md](sessions/dispatcher.md)
- [sessions/worktree-agent.md](sessions/worktree-agent.md)
- [sessions/workflow-stage.md](sessions/workflow-stage.md)
- [sessions/recovery-and-history.md](sessions/recovery-and-history.md)
- [sessions/bootstrap-and-restore-context.md](sessions/bootstrap-and-restore-context.md)

## Segment Dossiers

- [segments/frontend-shell-and-navigation.md](segments/frontend-shell-and-navigation.md)
- [segments/panel-patterns-and-settings.md](segments/panel-patterns-and-settings.md)
- [segments/state-and-refresh-ownership.md](segments/state-and-refresh-ownership.md)
- [segments/supervisor-runtime-and-session-host.md](segments/supervisor-runtime-and-session-host.md)
- [segments/data-persistence-and-backend-services.md](segments/data-persistence-and-backend-services.md)
- [segments/workflows-vault-backup-and-restore.md](segments/workflows-vault-backup-and-restore.md)
- [segments/observability-verification-and-doc-drift.md](segments/observability-verification-and-doc-drift.md)

## Working Rules

- Keep segment dossiers focused on current behavior and current risks.
- Put every cross-cutting or repair-worthy issue in the defect register.
- Mark findings as `confirmed`, `likely`, or `structural` instead of overstating certainty.
- Update the roadmap when issue priority or sequencing changes.
