# Data Persistence And Backend Services

Last updated: April 15, 2026

## Scope

- `src-tauri/src/db.rs`
- `src-tauri/src/db/app_state_core_records.rs`
- `src-tauri/src/db/app_state_feature_services.rs`
- `src-tauri/src/db/app_state_session_coordination.rs`
- `src-tauri/src/agent_message_broker.rs`
- `src-tauri/src/agent_message_store.rs`
- `src-tauri/src/agent_signal_store.rs`
- `src-tauri/src/app_settings_store.rs`
- `src-tauri/src/document_store.rs`
- `src-tauri/src/launch_profile_store.rs`
- `src-tauri/src/project_store.rs`
- `src-tauri/src/session_reconciliation.rs`
- `src-tauri/src/session_store.rs`
- `src-tauri/src/work_item_store.rs`
- `src-tauri/src/worktree_store.rs`
- `src-tauri/src/embeddings.rs`
- supporting persistence helpers in related backend modules

## Current Responsibilities

- `db.rs` manages schema, migration, bootstrap, and shared data-integrity helpers.
- `db/app_state_core_records.rs` owns the `AppState` project/work-item/document/worktree/profile/settings coordination surface on top of the extracted stores.
- `db/app_state_feature_services.rs` owns the `AppState` workflow/vault/settings service surface on top of workflow/vault helpers and extracted settings storage.
- `db/app_state_session_coordination.rs` owns the `AppState` session/event/message/signal coordination surface on top of the extracted session/message/signal stores.
- `agent_message_broker.rs` owns the in-process agent-message wait/notify broker used by supervisor message-polling routes.
- `agent_message_store.rs` owns agent-message persistence queries, thread-id inheritance, inbox filtering, and read-state updates.
- `agent_signal_store.rs` owns agent-signal persistence queries, status updates, and pending-count lookups used by worktree state.
- `app_settings_store.rs` owns app-settings row/query/update behavior, default-launch-profile validation, and the clean-shutdown setting.
- `document_store.rs` owns document row/query/update behavior plus project/work-item link validation for documents.
- `launch_profile_store.rs` owns launch-profile row/query/update behavior, env JSON normalization, and default-setting cleanup when a profile is deleted.
- `project_store.rs` owns project registration/query/update behavior, root-path identity rules, project-prefix generation, and tracker bootstrap/backfill logic.
- `session_reconciliation.rs` owns startup policy for durable session records that were still marked `running` when the supervisor restarts.
- `session_store.rs` owns session-record and session-event persistence queries and updates.
- `work_item_store.rs` owns work-item row/query/update behavior, hierarchy rules, identifier assignment, and identifier reconciliation/backfill logic.
- `worktree_store.rs` owns worktree row/query/update behavior plus derived worktree-state enrichment such as dirty state, pending signals, unmerged-branch checks, and session-summary shaping.
- `embeddings.rs` adds semantic-search behavior on top of the same app-state/persistence boundary.

## Authority Boundary

- `db.rs` is authoritative for durable records, migrations, bootstrap, and shared data-integrity helpers.
- `db/app_state_core_records.rs` is authoritative for the `AppState` coordination surface over projects, launch profiles, settings, work items, documents, and worktrees.
- `db/app_state_feature_services.rs` is authoritative for the `AppState` coordination surface over workflow and vault service calls, plus read access to app settings.
- `db/app_state_session_coordination.rs` is authoritative for the `AppState` coordination surface over session rows/events, agent signals, and broker messages.
- `agent_message_broker.rs` is authoritative for in-process agent-message sequence/change notification.
- `agent_message_store.rs` is authoritative for agent-message row/query/update shape.
- `agent_signal_store.rs` is authoritative for agent-signal row/query/update shape.
- `app_settings_store.rs` is authoritative for app-settings row/query/update shape and launch-profile-backed settings validation.
- `document_store.rs` is authoritative for document row/query/update shape and linked-work-item validation on documents.
- `launch_profile_store.rs` is authoritative for launch-profile row/query/update shape and profile-local config normalization.
- `project_store.rs` is authoritative for project-row/query/update shape and project registration identity helpers.
- `session_reconciliation.rs` is authoritative for startup running-session reconciliation policy.
- `session_store.rs` is authoritative for session-row and session-event query/update shape.
- `work_item_store.rs` is authoritative for work-item row/query/update shape and hierarchy/identifier integrity helpers.
- `worktree_store.rs` is authoritative for durable worktree-row/query/update shape and derived worktree-state persistence helpers.
- Live PTY state, in-flight session output, and process liveness are not authoritative in the DB until runtime code persists them.
- `embeddings.rs` extends search behavior on top of durable state, but does not own the session/runtime contract itself.

## Main Handoffs

- session/runtime services -> `AppState` core-record coordination in `db/app_state_core_records.rs`; `AppState` feature-service coordination in `db/app_state_feature_services.rs`; `AppState` session/event/message/signal coordination in `db/app_state_session_coordination.rs`; durable session rows and events in `session_store.rs`; startup running-session reconciliation in `session_reconciliation.rs`; in-process message wait/notify coordination in `agent_message_broker.rs`; durable broker messages in `agent_message_store.rs`; durable signal rows and pending-count queries in `agent_signal_store.rs`; durable app settings in `app_settings_store.rs`; durable launch-profile rows in `launch_profile_store.rs`; durable project rows and registration identity helpers in `project_store.rs`; durable work-item rows and hierarchy/identifier helpers in `work_item_store.rs`; durable document rows and link validation in `document_store.rs`; durable worktree rows and derived worktree-state shaping in `worktree_store.rs`; broader workflow metadata in `db.rs`
- workflow/vault services -> `db/app_state_feature_services.rs` plus shared persistence/bootstrap helpers behind `db.rs`
- history/recovery/docs panels -> DB-backed records read through supervisor/desktop APIs

## Findings

### Confirmed

- `db.rs` is much closer to a true persistence/bootstrap boundary now. The broad `AppState` coordination surface has been split across `db/app_state_core_records.rs`, `db/app_state_feature_services.rs`, and `db/app_state_session_coordination.rs`, while durable row/query behavior lives in the feature stores.
- The main remaining weight in `db.rs` is schema/bootstrap/test bulk, not the cross-cutting app-service authority that originally made it a structural defect.

### Likely

- The audit pass flagged at least one read-then-write update path in backend session/runtime metadata that should be reviewed for transactional isolation.

### Structural

- A large portion of the backend still depends on understanding the persistence schema and migrations in `db.rs`.
- The remaining improvement opportunity is keeping future feature additions out of `db.rs` unless they are truly schema/bootstrap/integrity concerns.

## Why This Matters

This used to be the backend equivalent of an oversized app shell. It is now closer to the right boundary, but it still needs discipline so future feature work does not slide authority back into `db.rs`.

## Immediate Follow-Up

- Keep session persistence in `session_store.rs` instead of sliding it back into `db.rs`.
- Keep startup running-session reconciliation in `session_reconciliation.rs` instead of sliding it back into `db.rs`.
- Keep `AppState` workflow/vault/settings service coordination in `db/app_state_feature_services.rs` instead of sliding it back into `db.rs`.
- Keep `AppState` session/message/signal coordination in `db/app_state_session_coordination.rs` instead of sliding it back into `db.rs`.
- Keep `AppState` project/work-item/document/worktree/profile/settings coordination in `db/app_state_core_records.rs` instead of sliding it back into `db.rs`.
- Keep the in-process agent-message broker in `agent_message_broker.rs` instead of sliding it back into `db.rs`.
- Keep agent-message persistence in `agent_message_store.rs` instead of sliding it back into `db.rs`.
- Keep agent-signal persistence in `agent_signal_store.rs` instead of sliding it back into `db.rs`.
- Keep app-settings persistence and validation in `app_settings_store.rs` instead of sliding them back into `db.rs`.
- Keep launch-profile persistence and normalization in `launch_profile_store.rs` instead of sliding it back into `db.rs`.
- Keep project registration/query/update logic in `project_store.rs` instead of sliding it back into `db.rs`.
- Keep work-item hierarchy and identifier logic in `work_item_store.rs` instead of sliding it back into `db.rs`.
- Keep document persistence and work-item-link validation in `document_store.rs` instead of sliding them back into `db.rs`.
- Keep worktree persistence and derived worktree-state shaping in `worktree_store.rs` instead of sliding them back into `db.rs`.
- Define what `db.rs` should remain authoritative for versus what should be delegated to feature services.
- Treat `db.rs` as schema/bootstrap/shared integrity territory unless there is a compelling reason not to.
- Add concurrency-focused tests for the highest-risk state transitions.
