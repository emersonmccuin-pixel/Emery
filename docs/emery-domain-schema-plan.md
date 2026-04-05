# Emery Domain Model And Schema Plan

## Purpose

This document defines the concrete Emery v1 domain model and storage plan across:

- `app.db` for operational state
- `knowledge.db` for the long-lived context graph
- artifact storage on disk for transcripts and other durable session files

This document turns the earlier architecture boundaries into an implementation-ready schema direction.

## Design Rules

The schema follows these locked decisions:

- `app.db` owns operational state
- `knowledge.db` owns the long-lived context graph
- projects are multi-root workspace containers
- work items are first-class planning, execution, and knowledge anchors
- Emery-native docs are canonical DB-backed docs in `knowledge.db`
- raw transcripts stay in artifact storage, not `knowledge.db`
- agents use application services, MCP, and CLI tooling rather than direct DB access

## Database Split

## `app.db`

`app.db` stores:

- products and workspace containers
- project roots
- accounts
- worktrees
- session specs and session records
- session artifact references
- planning assignments
- workspace restore state
- workflow reconciliation proposals

`app.db` does not store:

- canonical work item identity
- canonical authored docs
- extracted knowledge entries
- graph links
- embeddings
- full transcript bodies

## `knowledge.db`

`knowledge.db` stores:

- work items and hierarchy
- canonical Emery-native docs
- session provenance nodes
- extracted knowledge entries
- explicit graph links
- embeddings and retrieval support
- knowledge admission and extraction audit records

## Artifact Storage

Artifact storage on disk stores:

- raw terminal logs
- cleaned transcripts
- context bundle files
- extraction outputs
- review snapshots
- debug and diagnostics artifacts when explicitly enabled

Rule:

- databases index artifacts
- artifacts stay on disk

## Identity Conventions

## IDs

All major records should use UUID-like text IDs in the app layer.

Recommended pattern:

- `project_id`
- `project_root_id`
- `worktree_id`
- `session_spec_id`
- `session_id`
- `artifact_id`
- `proposal_id`

In `knowledge.db`, graph entities may use either UUID text IDs or integer row IDs. The important rule is consistency at the service boundary.

Recommendation:

- use text UUID IDs in new Emery schema for both databases
- keep human-readable callsigns and slugs as separate stable identifiers

## Callsigns

Work item callsigns remain human-facing and lineage-bearing.

Examples:

- `Emery-5`
- `Emery-5.001`
- `Emery-5.001.001`

Rule:

- joins use IDs
- humans use callsigns
- services can resolve by either

## Cross-Database Reference Convention

Because SQLite foreign keys do not span two database files cleanly in the intended product model, cross-database references must be application-enforced.

Recommended convention:

- `app.db` stores `work_item_id` as a text ID from `knowledge.db`
- optional `work_item_callsign` may be denormalized into read models or caches
- all cross-DB integrity is enforced in supervisor services, not by SQLite foreign keys

## `app.db` Schema

## 1. `projects`

Represents a workspace container, not merely one repo.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable project identity |
| `name` | TEXT NOT NULL | display name |
| `slug` | TEXT NOT NULL UNIQUE | stable app-level key |
| `sort_order` | INTEGER NOT NULL | left-sidebar ordering |
| `default_account_id` | TEXT NULL | preferred launch account |
| `settings_json` | TEXT NULL | project-scoped settings |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `archived_at` | INTEGER NULL | hide from active views |

Indexes:

- `idx_projects_sort_order`
- `idx_projects_slug`

## 2. `project_roots`

Represents codebase roots or filesystem roots inside a project.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable root identity |
| `project_id` | TEXT NOT NULL | FK to `projects.id` |
| `label` | TEXT NOT NULL | user-facing label |
| `path` | TEXT NOT NULL | absolute path |
| `git_root_path` | TEXT NULL | resolved git root |
| `remote_url` | TEXT NULL | optional primary remote |
| `root_kind` | TEXT NOT NULL | `repo`, `folder`, `artifact-space`, reserved others |
| `sort_order` | INTEGER NOT NULL | ordering within project |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `archived_at` | INTEGER NULL | soft hide |

Constraints:

- unique on `(project_id, path)`

Indexes:

- `idx_project_roots_project_sort`
- `idx_project_roots_git_root`

## 3. `accounts`

Reusable launch profiles for agent providers.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable account identity |
| `agent_kind` | TEXT NOT NULL | `claude`, `codex`, later others |
| `label` | TEXT NOT NULL | display name |
| `binary_path` | TEXT NULL | optional pinned binary |
| `config_root` | TEXT NULL | account home or config dir |
| `env_preset_ref` | TEXT NULL | points to env preset row |
| `is_default` | INTEGER NOT NULL | boolean |
| `status` | TEXT NOT NULL | `ready`, `missing_binary`, `invalid_config`, `disabled` |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |

Indexes:

- `idx_accounts_agent_kind`
- `idx_accounts_default`

## 4. `env_presets`

Stores reusable launch environment presets without bloating `accounts`.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable preset identity |
| `name` | TEXT NOT NULL | display name |
| `env_json` | TEXT NOT NULL | serialized env map |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |

## 5. `worktrees`

First-class operational entities independent from sessions.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable worktree identity |
| `project_id` | TEXT NOT NULL | FK to `projects.id` |
| `project_root_id` | TEXT NOT NULL | FK to `project_roots.id` |
| `branch_name` | TEXT NOT NULL | local branch |
| `head_commit` | TEXT NULL | current commit when last inspected |
| `base_ref` | TEXT NULL | origin branch or merge target |
| `path` | TEXT NOT NULL UNIQUE | absolute worktree path |
| `status` | TEXT NOT NULL | `active`, `cleaned`, `missing`, `orphaned`, `failed` |
| `created_by_session_id` | TEXT NULL | original creating session |
| `last_used_at` | INTEGER NULL | useful for cleanup hemerystics |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `closed_at` | INTEGER NULL | no longer active |

Indexes:

- `idx_worktrees_project`
- `idx_worktrees_root_status`
- `idx_worktrees_last_used`

Operational rule:

- one worktree can have many historical sessions
- one worktree can have at most one live session at a time

## 6. `session_specs`

Captures declarative launch intent.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable session spec identity |
| `project_id` | TEXT NOT NULL | FK to `projects.id` |
| `project_root_id` | TEXT NULL | target root |
| `worktree_id` | TEXT NULL | optional worktree |
| `work_item_id` | TEXT NULL | cross-db reference into `knowledge.db` |
| `account_id` | TEXT NOT NULL | FK to `accounts.id` |
| `agent_kind` | TEXT NOT NULL | denormalized for convenience |
| `cwd` | TEXT NOT NULL | launch cwd |
| `command` | TEXT NOT NULL | launch binary or resolved command |
| `args_json` | TEXT NOT NULL | serialized args |
| `env_preset_ref` | TEXT NULL | optional |
| `origin_mode` | TEXT NOT NULL | `ad_hoc`, `planning`, `research`, `execution`, `follow_up` |
| `title_policy` | TEXT NOT NULL | e.g. `auto_until_manual` |
| `restore_policy` | TEXT NOT NULL | e.g. `restore_on_launch`, `do_not_restore` |
| `initial_terminal_cols` | INTEGER NOT NULL | initial geometry |
| `initial_terminal_rows` | INTEGER NOT NULL | initial geometry |
| `context_bundle_artifact_id` | TEXT NULL | FK to `session_artifacts.id` once materialized |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |

Indexes:

- `idx_session_specs_project`
- `idx_session_specs_work_item`
- `idx_session_specs_worktree`

## 7. `sessions`

Runtime and historical session record.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable session identity |
| `session_spec_id` | TEXT NOT NULL | FK to `session_specs.id` |
| `project_id` | TEXT NOT NULL | denormalized read-model key |
| `project_root_id` | TEXT NULL | denormalized read-model key |
| `worktree_id` | TEXT NULL | denormalized read-model key |
| `work_item_id` | TEXT NULL | cross-db reference |
| `account_id` | TEXT NOT NULL | FK to `accounts.id` |
| `agent_kind` | TEXT NOT NULL | denormalized |
| `origin_mode` | TEXT NOT NULL | copied from spec for easy filtering |
| `current_mode` | TEXT NOT NULL | mutable workflow mode |
| `title` | TEXT NULL | current display title |
| `title_source` | TEXT NOT NULL | `auto`, `manual` |
| `user_prompt_count` | INTEGER NOT NULL | auto-title cadence input |
| `next_title_refresh_at_prompt_count` | INTEGER NULL | auto-title schedule |
| `runtime_state` | TEXT NOT NULL | `starting`, `running`, `stopping`, `exited`, `failed`, `interrupted` |
| `status` | TEXT NOT NULL | `active`, `archived`, `deleted` |
| `activity_state` | TEXT NOT NULL | `working`, `idle`, `needs_input` |
| `needs_input_reason` | TEXT NULL | optional structured cause |
| `pty_owner_key` | TEXT NULL | live supervisor runtime/session key |
| `cwd` | TEXT NOT NULL | current cwd snapshot |
| `transcript_primary_artifact_id` | TEXT NULL | primary cleaned transcript |
| `raw_log_artifact_id` | TEXT NULL | primary raw terminal log |
| `started_at` | INTEGER NULL | epoch ms |
| `ended_at` | INTEGER NULL | epoch ms |
| `last_output_at` | INTEGER NULL | epoch ms |
| `last_attached_at` | INTEGER NULL | epoch ms |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `archived_at` | INTEGER NULL | archive time |

Indexes:

- `idx_sessions_project_created`
- `idx_sessions_origin_mode`
- `idx_sessions_current_mode`
- `idx_sessions_runtime_state`
- `idx_sessions_status`
- `idx_sessions_work_item`
- `idx_sessions_last_attached`

## 8. `session_artifacts`

Indexes artifacts stored on disk.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable artifact identity |
| `session_id` | TEXT NOT NULL | FK to `sessions.id` |
| `artifact_class` | TEXT NOT NULL | `runtime`, `transcript`, `context`, `extraction`, `review`, `git`, `diagnostic` |
| `artifact_type` | TEXT NOT NULL | concrete type within class |
| `path` | TEXT NOT NULL | absolute path |
| `is_durable` | INTEGER NOT NULL | boolean |
| `is_primary` | INTEGER NOT NULL | boolean |
| `source` | TEXT NULL | `supervisor`, `extractor`, `review`, `user` |
| `generator_ref` | TEXT NULL | model or subsystem identifier |
| `supersedes_artifact_id` | TEXT NULL | replaces prior artifact |
| `created_at` | INTEGER NOT NULL | epoch ms |

Constraints:

- unique on `(session_id, artifact_type, is_primary)` where appropriate in service logic

Indexes:

- `idx_session_artifacts_session_class`
- `idx_session_artifacts_primary`

## 9. `planning_assignments`

Represents scheduling horizons separately from work item lifecycle.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable assignment identity |
| `work_item_id` | TEXT NOT NULL | cross-db reference |
| `cadence_type` | TEXT NOT NULL | `quarter`, `sprint`, `week`, `day` |
| `cadence_key` | TEXT NOT NULL | e.g. `2026-Q2`, `2026-W14`, `2026-04-04` |
| `created_by` | TEXT NOT NULL | `user`, `agent`, `system` |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `removed_at` | INTEGER NULL | soft deletion |

Constraints:

- unique on `(work_item_id, cadence_type, cadence_key, removed_at IS NULL)` in service logic

Indexes:

- `idx_planning_assignments_work_item`
- `idx_planning_assignments_cadence`

## 10. `workspace_state`

Stores restorable workspace layouts and selections.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | usually a singleton record or named workspace |
| `scope` | TEXT NOT NULL | `global`, later named workspaces if needed |
| `payload_json` | TEXT NOT NULL | open resources, tab order, active resource, splits |
| `saved_at` | INTEGER NOT NULL | epoch ms |

Payload rule:

- workspace resources are stored as structured identities, not raw tab labels
- each resource payload should include fields such as `resource_type`, `root_kind`, `root_id`, `relative_path`, `document_id`, and `view_mode` where relevant
- this keeps restore stable across project roots, worktrees, docs, and session artifacts

## 11. `workflow_reconciliation_proposals`

Structured post-session recommendations.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable proposal identity |
| `source_session_id` | TEXT NOT NULL | FK to `sessions.id` |
| `target_entity_type` | TEXT NOT NULL | `work_item`, `planning_assignment`, `document`, others later |
| `target_entity_id` | TEXT NULL | ID if existing |
| `proposal_type` | TEXT NOT NULL | `mark_done`, `mark_blocked`, `create_child`, `replan`, etc. |
| `proposed_change_payload` | TEXT NOT NULL | JSON payload |
| `reason` | TEXT NOT NULL | user-visible rationale |
| `confidence` | REAL NOT NULL | 0 to 1 |
| `status` | TEXT NOT NULL | `pending`, `accepted`, `dismissed`, `applied` |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `resolved_at` | INTEGER NULL | epoch ms |

Indexes:

- `idx_reconciliation_source_session`
- `idx_reconciliation_status`

## `knowledge.db` Schema

## 1. `work_items`

Canonical planning and execution anchors.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable work item identity |
| `project_id` | TEXT NOT NULL | logical project/namespace anchor |
| `parent_id` | TEXT NULL | hierarchy |
| `root_work_item_id` | TEXT NULL | root ancestor for rollups |
| `callsign` | TEXT NOT NULL UNIQUE | human-facing identity |
| `child_sequence` | INTEGER NULL | helps materialize lineage |
| `title` | TEXT NOT NULL | short name |
| `description` | TEXT NOT NULL | markdown body |
| `acceptance_criteria` | TEXT NULL | markdown or structured text |
| `work_item_type` | TEXT NOT NULL | `epic`, `task`, `bug`, `feature`, `research`, `support` |
| `status` | TEXT NOT NULL | `backlog`, `planned`, `in_progress`, `blocked`, `done`, `archived` |
| `priority` | TEXT NULL | `urgent`, `high`, `medium`, `low` |
| `created_by` | TEXT NULL | `user`, `agent`, `system` |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `closed_at` | INTEGER NULL | epoch ms |

Indexes:

- `idx_work_items_project`
- `idx_work_items_parent`
- `idx_work_items_root`
- `idx_work_items_status`
- `idx_work_items_priority`

## 2. `documents`

Canonical Emery-native authored documents.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable doc identity |
| `project_id` | TEXT NOT NULL | logical project anchor |
| `work_item_id` | TEXT NULL | optional linked work item |
| `session_id` | TEXT NULL | optional source session link |
| `doc_type` | TEXT NOT NULL | `architecture`, `adr`, `plan`, `review`, etc. |
| `title` | TEXT NOT NULL | display title |
| `slug` | TEXT NOT NULL | stable project-local slug |
| `status` | TEXT NOT NULL | `draft`, `active`, `archived` |
| `content_markdown` | TEXT NOT NULL | canonical body |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `archived_at` | INTEGER NULL | epoch ms |

Constraints:

- unique on `(project_id, slug)`

Indexes:

- `idx_documents_project`
- `idx_documents_work_item`
- `idx_documents_doc_type`

## 3. `session_nodes`

Knowledge-graph representation of completed sessions for provenance.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | graph-level session node ID |
| `session_id` | TEXT NOT NULL UNIQUE | operational session reference |
| `project_id` | TEXT NOT NULL | graph anchor |
| `work_item_id` | TEXT NULL | optional related work item |
| `title` | TEXT NULL | final session title |
| `summary` | TEXT NULL | admitted high-level summary |
| `agent_kind` | TEXT NOT NULL | claude, codex |
| `completed_at` | INTEGER NULL | epoch ms |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |

Indexes:

- `idx_session_nodes_project`
- `idx_session_nodes_work_item`

## 4. `knowledge_entries`

Structured extracted or curated graph entries.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable entry identity |
| `entry_type` | TEXT NOT NULL | `decision`, `insight`, `activity`, `reason`, `summary` |
| `title` | TEXT NULL | optional title |
| `content` | TEXT NOT NULL | canonical text |
| `summary` | TEXT NULL | shorter surfacing text |
| `source_session_node_id` | TEXT NULL | provenance session node |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |
| `superseded_by_id` | TEXT NULL | later entry that replaces it |
| `admission_status` | TEXT NOT NULL | `admitted`, `rejected`, `superseded` |

Indexes:

- `idx_knowledge_entries_type`
- `idx_knowledge_entries_source_session`
- `idx_knowledge_entries_admission`

## 5. `links`

Explicit graph edges.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable link identity |
| `source_entity_type` | TEXT NOT NULL | `work_item`, `document`, `session_node`, `knowledge_entry` |
| `source_entity_id` | TEXT NOT NULL | source ID |
| `target_entity_type` | TEXT NOT NULL | same entity set |
| `target_entity_id` | TEXT NOT NULL | target ID |
| `link_type` | TEXT NOT NULL | `references`, `supports`, `implements`, `derived_from`, `related_to`, `supersedes`, `decides` |
| `created_at` | INTEGER NOT NULL | epoch ms |

Indexes:

- `idx_links_forward`
- `idx_links_backward`
- `idx_links_type`

## 6. `embeddings`

Stores vector support data for semantic retrieval.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | stable embedding identity |
| `entity_type` | TEXT NOT NULL | embeddable class |
| `entity_id` | TEXT NOT NULL | linked entity |
| `embedding_model` | TEXT NOT NULL | provider/model name |
| `dimensions` | INTEGER NOT NULL | vector size |
| `vector_blob` | BLOB NOT NULL | stored vector |
| `input_hash` | TEXT NULL | dedupe and freshness |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |

Constraints:

- unique on `(entity_type, entity_id, embedding_model)`

## 7. `extraction_runs`

Audit trail for extraction execution.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | run identity |
| `session_id` | TEXT NOT NULL | operational session reference |
| `classifier_result` | TEXT NOT NULL | `skip`, `summary_only`, `extract_structured_knowledge` |
| `model_ref` | TEXT NULL | model used |
| `status` | TEXT NOT NULL | `pending`, `processing`, `completed`, `failed` |
| `input_artifact_id` | TEXT NULL | artifact ref as text |
| `output_artifact_id` | TEXT NULL | artifact ref as text |
| `confidence` | REAL NULL | extraction confidence |
| `failure_reason` | TEXT NULL | error detail |
| `created_at` | INTEGER NOT NULL | epoch ms |
| `updated_at` | INTEGER NOT NULL | epoch ms |

Indexes:

- `idx_extraction_runs_session`
- `idx_extraction_runs_status`

## 8. `admission_decisions`

Captures what actually enters the graph and why.

Suggested columns:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT PK | decision identity |
| `extraction_run_id` | TEXT NOT NULL | links to extraction run |
| `entry_id` | TEXT NULL | knowledge entry if admitted |
| `decision` | TEXT NOT NULL | `admit`, `reject`, `merge`, `supersede` |
| `reason` | TEXT NOT NULL | inspectable explanation |
| `review_status` | TEXT NOT NULL | `automatic`, `reviewed`, `corrected` |
| `created_at` | INTEGER NOT NULL | epoch ms |

Indexes:

- `idx_admission_decisions_run`
- `idx_admission_decisions_decision`

## Integrity And Lifecycle Rules

The schema should be backed by service-enforced invariants, not only raw tables.

Required v1 rules:

- a `session_spec` must exist before a `session` record is created
- a `session` may exist without a `worktree_id`, but an execution-oriented session should gain one before code-changing actions
- a `worktree` may have many historical sessions but at most one live session
- `app.db` may store `work_item_id` references, but only the supervisor validates them against `knowledge.db`
- deleting or archiving a project must be blocked while live sessions still reference it
- `session_artifacts.is_primary` uniqueness is enforced per `(session_id, artifact_type)` in the service layer
- recommendation-style workflow changes become `workflow_reconciliation_proposals`; deterministic runtime updates do not

## Write Path Ownership

Ownership rules matter as much as table shape.

- the supervisor owns all writes to `app.db`
- knowledge services own all writes to `knowledge.db`
- the Tauri client receives read models and action results, not direct table access
- agents interact through RPC, MCP, and CLI surfaces that call the same service layer
- artifact writers update disk first and then register the artifact row in `app.db`

## Initial Migration Priority

The first migration set should land in dependency order.

Recommended `app.db` order:

1. `projects`
2. `project_roots`
3. `env_presets`
4. `accounts`
5. `worktrees`
6. `session_specs`
7. `sessions`
8. `session_artifacts`
9. `planning_assignments`
10. `workspace_state`
11. `workflow_reconciliation_proposals`

Recommended `knowledge.db` order:

1. `work_items`
2. `documents`
3. `session_nodes`
4. `knowledge_entries`
5. `links`
6. `embeddings`
7. `extraction_runs`
8. `admission_decisions`

## Full-Text Search

`knowledge.db` should provide FTS indexes for:

- work items
- documents
- knowledge entries

Recommendation:

- one FTS virtual table per major entity class
- service-layer hybrid retrieval combines FTS + embeddings + graph signals

## Read Models

The client should not assemble complex relationships on its own.

The supervisor should expose read models such as:

- project overview with roots, recent worktrees, and live sessions
- work item detail with linked docs, sessions, and extracted context
- session detail with artifact references and operational state
- daily and weekly planning views
- workspace resource listings

These read models are API outputs, not extra persistence tables unless profiling proves otherwise.

## Migrations

Both databases require explicit versioned migrations.

Recommended approach:

- `app.db` migration table
- `knowledge.db` migration table
- forward-only numbered migrations
- startup migration run owned by the supervisor

Do not use:

- ad hoc `ALTER TABLE IF MISSING` evolution as the primary strategy

## Artifact Layout

Per session artifact directory:

- `sessions/<session-id>/meta.json`
- `sessions/<session-id>/raw-terminal.log`
- `sessions/<session-id>/transcript/cleaned-transcript.json`
- `sessions/<session-id>/context/context-bundle.md`
- `sessions/<session-id>/context/context-bundle.json`
- `sessions/<session-id>/extraction/`
- `sessions/<session-id>/review/`
- `sessions/<session-id>/debug/` only when enabled

Rule:

- `session_artifacts.path` always points into this boring predictable layout

## Service Ownership Rules

The Tauri client must never:

- write directly to `app.db`
- write directly to `knowledge.db`
- infer graph truth from artifacts alone

The supervisor and service layer must:

- enforce cross-db reference validity
- keep denormalized read-model fields in sync
- ensure deterministic operational changes are auto-applied
- ensure recommendation-style changes become proposals instead of silent mutations

## Minimum V1 Scope

The minimum schema scope needed before substantial UI work:

1. `projects`
2. `project_roots`
3. `accounts`
4. `worktrees`
5. `session_specs`
6. `sessions`
7. `session_artifacts`
8. `planning_assignments`
9. `workspace_state`
10. `work_items`
11. `documents`
12. `session_nodes`
13. `knowledge_entries`
14. `links`
15. `embeddings`

`workflow_reconciliation_proposals`, `extraction_runs`, and `admission_decisions` should still be designed now, even if the first implementation lands right after the prototype milestone.

## Final Recommendation

Emery should adopt a clean dual-database design:

- `app.db` for operational truth and restoreable workspace state
- `knowledge.db` for canonical work items, docs, provenance, links, and retrieval
- artifact files for transcript and session history

The most important concrete schema decisions are:

- projects are multi-root containers
- worktrees are first-class operational entities
- session specs are separate from session records
- work item identity stays in `knowledge.db`
- planning assignments stay in `app.db`
- raw transcripts never move into the knowledge graph
