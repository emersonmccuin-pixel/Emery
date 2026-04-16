# Workflows, Vault, Backup, And Restore

Last updated: April 14, 2026

## Scope

- `src/components/workflow/WorkflowsPanel.tsx`
- `src-tauri/src/workflow.rs`
- `src-tauri/src/vault.rs`
- `src-tauri/src/backup.rs`
- `src-tauri/src/embeddings.rs`

## Current Responsibilities

- Workflow library sync, project adoption, override resolution, and run persistence
- Secret storage and scoped launch delivery
- Manual and scheduled backup
- Staged restore on next boot
- Semantic search embeddings and backfill

## Authority Boundary

- `WorkflowsPanel.tsx` is a workflow control plane and observability surface, not the owner of generic terminal or recovery behavior.
- `workflow.rs` is authoritative for workflow catalog/adoption/run semantics.
- `vault.rs` is authoritative for durable secret storage and launch binding materialization.
- `backup.rs` is authoritative for backup archives, restore preparation, and staged restore markers.
- Session lifecycle, terminal polling, and recovery UX remain owned by the session stack and session audit docs.

## Main Handoffs

- workflow UI intent -> workflow start/adoption APIs -> workflow runtime state
- workflow stage launch -> session runtime/session host -> stage completion or failure persistence
- vault binding request -> launch path in `session_host.rs`
- restore prepare/commit -> startup/bootstrap path before later sessions launch

## Findings

### Confirmed

- `backup.rs` stores restore authorization tokens in a process-local in-memory map.
- `WorkflowsPanel.tsx` is effectively its own control plane with local effects, local run selection, and a local terminal-exit refresh listener.

### Likely

- The workflow-run start path appears to check for active runs before full transactional isolation, which creates a possible race window.

### Structural

- `workflow.rs` couples catalog schema, adoption mode, override semantics, secret-scope policy, and run-state transitions in one module.
- The workflow, vault, backup, and embeddings features all share the same runtime and persistence substrate, but there is no single shipped architecture note that explains how they fit together.

## Why This Matters

This is where the repo grew fastest. It is also where "works locally but not cohesively" failures are most likely to emerge because each feature is individually sophisticated and they all depend on the same runtime core.

## Immediate Follow-Up

- Add a dedicated architecture note for the workflow/vault/backup stack after the audit stabilizes.
- Add a targeted concurrency test for workflow-run start.
- Decide whether restore authorization needs to survive app restarts more explicitly than it does today.
