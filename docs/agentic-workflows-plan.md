# Agentic Workflows, Workflow Builder, Env Vault — Ultraplan

Status: phased implementation in progress
Last updated: 2026-04-14

## Purpose

Turn Project Commander from a session/mailbox substrate into a true **harness** for long-running agent work, aligned with Anthropic's harness-design guidance. Four coupled pieces:

1. **Role-typed agents** — planner, generator, evaluator as first-class concepts with independent model/provider routing.
2. **Global library of workflows and agent pods** — reusable, versioned primitives that projects adopt into their own scope.
3. **Workflow builder** — data-driven workflows (coding, documentation, planning, data analysis) stored in SQLite, using work items + ADRs + artifacts as the handoff medium.
4. **Env vault** — secure, auditable secret access so agents can reach tools without secrets leaking into context, logs, or diagnostics.

This plan does not touch the A+ guardrails (no new polling, no orchestration monoliths, reuse shared primitives).

## Implementation Snapshot (2026-04-14)

Phase 0/1 is now shipped in the app as the **catalog + adoption foundation**. Later sections in this document still describe the intended end-state, but they are not all implemented yet.

Delivered now:

- Global workflow/pod library seeding and sync under `<app-data>/library/{workflows,pods}`.
- Seeded shipped catalog entries for feature delivery, ADR authoring, documentation, data analysis, and research workflows plus reusable planner/generator/evaluator/reviewer/integrator pods.
- SQLite-backed catalog/adoption state for `workflow_categories`, `library_workflows`, `library_pods`, category assignments, and `project_catalog_adoptions`.
- SQLite-backed workflow execution state for `project_workflow_overrides`, `workflow_runs`, and `workflow_run_stages`, with frozen resolved workflow JSON persisted at run start.
- Tauri commands for library reads plus project adopt / upgrade / detach flows.
- Tauri/supervisor commands to start workflow runs and list project run history.
- A lazy-loaded **Workflows** workspace surface with `Library` / `This Project` scopes, workflow vs. pod tabs, link-status badges, and YAML inspection.
- A project-scoped **Runs** tab with workflow run history, per-stage status/detail, and start-run controls on adopted workflows.
- Linked workflow adoption automatically pulls referenced pods into the project catalog and pins the resolved versions.
- Workflow run resolution now freezes the adopted workflow plus adopted pods, applies the current SQLite-backed override seam, validates generator/evaluator provider separation, and rejects outdated linked adoptions before dispatch.
- Workflow stages and project overrides may now declare explicit vault env bindings; run resolution freezes them, validates them against `needs_secrets` plus the resolved pod's `secret_scopes`, and supervisor stage launch passes them through the existing session-start vault resolver/audit/scrubber path.
- Vault bindings may now choose `delivery=file` for tools that expect a credential file path; the session host materializes those secrets into per-session runtime files and still redacts the raw values from output/log artifacts.
- Workflow stage dispatch now reuses the existing managed worktree + session host + supervisor broker path: each stage launches a fresh session on the managed worktree, sends a directive through the broker, waits on threaded replies, persists stage/run status, and tears the session down before the next stage.
- A lazy-loaded `Settings -> Vault` surface with trusted-UI deposit / rotate / delete flows.
- Stronghold-backed vault value storage plus SQLite metadata/audit tables (`vault_entries`, `vault_audit_events`) and a redacted frontend invoke seam for secret writes.
- Launch-profile `env_json` can now declare vault-backed env bindings that the supervisor resolves at session start with scope checks, audit rows, and PTY/session-output scrubbing.

Not delivered yet:

- Visual workflow builder/editor UI.
- Egress proxying, integration-template execution, and brokered secret delivery beyond env-based session injection.
- Rich template-driven non-env delivery modes beyond the shipped env/file session-launch path.
- Artifact-contract validation, negotiated sprint contracts, and generator/evaluator retry loops.
- Repo-backed override YAML files and an override editor UI; the current implementation ships the override seam as SQLite-backed `project_workflow_overrides` rows.

Rule for future work: build on the shipped catalog/adoption foundation instead of bypassing it with ad hoc per-project configs or a second orchestration store.

## Guiding Principles

From the article:

- **Separate the doer from the judge.** Generator never evaluates its own work.
- **Context resets over compaction.** Each stage starts a fresh session; handoff state lives in artifacts, not conversation history.
- **Contracts before code.** Every sprint begins with a negotiated "done means X" artifact.
- **High-level planning, not granular specs.** Planners state deliverables; generators own technical choices.
- **Harness-level metrics.** Iterations per sprint, eval pass rate, cost-per-completed-sprint — tune the harness against these.
- **Keep the scaffold swappable.** Model capability improves; remove stages that stop being load-bearing.

From PC house rules:

- Reuse mailbox + worktrees + MCP as the transport. Do not build a second message system.
- No new polling loops. Everything event-driven.
- Extend shared primitives; do not fork.

## Target Architecture (one-screen view)

```
 ┌─────────────────────────────────────────────────────────────┐
 │  Workflow Definition (YAML in repo  →  workflow_defs DB)    │
 │    stages: [planner, sprint_contract, generator, evaluator] │
 └──────────────────────┬──────────────────────────────────────┘
                        │ user triggers run on a work item
                        ▼
 ┌─────────────────────────────────────────────────────────────┐
 │  Workflow Run (workflow_runs + workflow_run_stages)         │
 │    orchestrator: supervisor (Rust)                          │
 │    state: stage status, artifacts produced, retries, cost   │
 └────────┬──────────┬──────────┬───────────┬──────────────────┘
          ▼          ▼          ▼           ▼
      Planner   Contract    Generator   Evaluator
      session   artifact    session     session
      (Opus)    (shared)    (Sonnet/    (Codex)
                            Opus/
                            Codex)
          │          │          │           │
          └──────────┴──────────┴───────────┘
                       │
               mailbox + artifacts (SQLite + files)
                       │
                 Env Vault (Stronghold)
                       │
          brokered launch / integration resolver
```

Key rule: **stages never share context windows.** They share *artifacts*.

## Global Library, Agent Pods, and Project Scope

Workflows and agent pods are **global-first**. A project doesn't own them; a project *adopts* them. This mirrors how PC already handles shared UI primitives — one canonical definition, many consumers.

### Agent pods (new first-class primitive)

An **agent pod** is a reusable agent configuration that workflow stages reference by name. A pod bundles:

- `role` — planner / generator / evaluator / …
- `categories` — domain labels (multi-select, user-extensible): `CODING`, `MARKETING`, `SALES`, `DATA`, `DOCUMENTATION`, `RESEARCH`, `DESIGN`, `PRODUCT`, `OPS`, `LEGAL`, …
- `provider` + `model` (or `auto` with a routing policy)
- `prompt_template_ref` — path to a markdown prompt template
- `tool_allowlist` — which MCP tools the pod may call
- `secret_scopes` — which vault scopes it may request secrets from
- `default_policy` — retries, timeouts, token budgets
- `tags` — finer attributes orthogonal to category: `codex_preferred`, `low_cost`, `deep_research`, `untested`, …

**Categories vs. tags.** Categories are the domain-of-work grouping — the top-level filter in the pool picker, what a user reaches for first ("show me CODING pods"). Tags are lightweight attributes used for routing logic and filtering within a category. Keep them separate; one filter mechanism collapses into the other if merged.

**At least one category is required.** Pods without a category are rejected at save time by the validator. When the agent-builder agent can't confidently infer a better fit, it defaults to `META`. This prevents orphan pods that only surface via free-text search.

Stages in a workflow either **inline** their config (quick one-off) or **reference a pod** by name/version (the normal case). A change to a pod propagates to every workflow that references it, subject to version pinning.

Why this is a separate primitive: you want to tune "our standard Codex evaluator" once and have every workflow that uses it benefit. Duplicating the config in every workflow is the anti-pattern the article warns about.

Default shipped pods (examples):

- `planner.opus.standard`
- `generator.sonnet.standard`
- `generator.opus.crosscutting`
- `generator.codex.cli-work`
- `evaluator.codex.strict`
- `evaluator.sonnet.fallback` (used only when Codex is unavailable)
- `reviewer.sonnet.lightweight`
- `researcher.sonnet.standard`
- `integrator.sonnet.merge`

### Two-tier scope: library and project

| Tier        | Lives at                                  | Purpose                                            |
|-------------|-------------------------------------------|----------------------------------------------------|
| **Library** | `<app-data>/library/{workflows,pods}/`    | Global, user-owned catalog of reusable definitions |
| **Project** | `<project>/.project-commander/{workflows,pods}/` | Project-scoped instances, either linked or forked from library entries |

Library content comes from three sources, in order of precedence when names collide:

1. User-authored (the normal case)
2. App-shipped defaults (seeded on first launch; user can override)
3. Shared/imported bundles (future: pull a pack from a git URL)

### Adoption model

When a project wants to use a library workflow or pod, it **adopts** it. Two modes:

- **Linked (default).** Project stores a reference `{name, version_pinned}` to the library entry. No local copy. Upgrades are one-click when upstream bumps. Project cannot edit — edits must happen in the library and bump a version.
- **Forked.** Project detaches, takes a full copy into `.project-commander/`. Edits freely, loses upstream sync. Can be "rebased" later (manual merge against current upstream version).

Linked is the ergonomic default and matches the "keep the scaffold swappable" principle — fix a bug in the global pod, all projects pick it up.

### Project overrides without forking

Projects shouldn't have to fork to make small adjustments. Supported overrides a project can apply on top of a linked entry without detaching:

- Swap a pod used by a specific stage (e.g. "in this project, the `generate` stage uses `generator.opus.crosscutting` instead of the default")
- Override stage secret capability requirements or bindings (to match project-specific integrations without forking the whole workflow)
- Override retry/budget policy per stage
- Set workflow-level parameter defaults

Target end-state: overrides are small YAML files in `.project-commander/overrides/<workflow-name>.yaml`, diff-able and version-controlled with the project.
Current shipped seam: overrides live in SQLite-backed `project_workflow_overrides` rows so run resolution and future UI work can layer changes without forcing a workflow fork yet.

### Resolution at run time

When a project triggers a workflow run, the engine resolves the effective definition in this order:

1. Start with the library workflow at the project's pinned version.
2. Apply project overrides (if any).
3. Resolve each `pod_ref` to the library pod at the pinned version (or the project fork if detached).
4. Apply stage-level inline config (if any).
5. Validate the fully-resolved workflow; fail fast if any reference is broken.

The resolved definition is frozen and stored with the `workflow_run` row, so a library upgrade mid-run never changes an in-flight run.

### UX implications

The Workflows tab gains a scope switcher: **Library** / **This Project**.

- **Library** view shows all global workflows and pods. Edits happen here. Version bumps happen here.
- **This Project** view shows what this project has adopted, with a link-status badge (linked @v3 / forked / outdated @v2, upstream v4). One-click upgrade when upstream is newer. One-click override panel for the small adjustments above.

Adoption flow: from the Library, click a workflow → **Adopt in project** → pick which open project → confirms the pod references it will also pull in. The project's adopted list updates; no file copies unless forking.

### Implications for the earlier sections

- Data model (below) adds separate tables for library vs. project entries and resolution records.
- The workflow YAML format stays the same; the scope determines *where* the file lives and *who* can edit it.
- Templates in the gallery are just library workflows tagged `template: true` — adoption is how you "use" them.

## Data Model (SQLite additions)

Implementation note: the catalog/adoption subset, workflow-run subset, and vault-core subset are now live. Specifically, `workflow_categories`, `library_workflows`, `library_pods`, their category-assignment tables, `project_catalog_adoptions`, `project_workflow_overrides`, `workflow_runs`, `workflow_run_stages`, `vault_entries`, and `vault_audit_events` are implemented. Artifact tables, runtime secret-grant tables, and repo-backed override files remain planned follow-on work.

New tables (names tentative):

- `library_workflows (id, name, kind, version, yaml, description, created_at, source)`
  - `source` ∈ {`user`, `shipped`, `imported`}; `kind` ∈ {`coding`, `documentation`, `planning`, `data_analysis`, `research`, `custom`}
- `library_pods (id, name, version, yaml, description, created_at, source)` — agent pods.
- `pod_categories (id, name, description, is_shipped, created_at)` — category registry; user can add. Shipped seed: `CODING`, `MARKETING`, `SALES`, `DATA`, `DOCUMENTATION`, `RESEARCH`, `DESIGN`, `PRODUCT`, `OPS`, `LEGAL`, `META`.
- `pod_category_assignments (pod_id, category_id)` — many-to-many.
- `workflow_category_assignments (workflow_id, category_id)` — same registry reused for workflows so the Library filters consistently.
- `project_categories (project_id, category_id)` — a project declares which categories apply; drives pod/workflow recommendations in the picker.
- `library_workflow_stages (id, workflow_id, ordinal, name, role, pod_ref, inline_config_json, inputs_json, outputs_json, transitions_json, retry_policy_json)`
  - `role` ∈ {`planner`, `generator`, `evaluator`, `reviewer`, `researcher`, `synthesizer`, `integrator`}
  - Stage either uses `pod_ref` (name + pinned version) or `inline_config_json`; never both.
- `project_adoptions (id, project_id, entity_type, library_id, pinned_version, mode, detached_yaml, created_at)`
  - `entity_type` ∈ {`workflow`, `pod`}; `mode` ∈ {`linked`, `forked`}
  - `detached_yaml` only populated when `forked`.
- `project_overrides (id, project_id, adoption_id, overrides_json, updated_at)` — small stage-level tweaks applied on top of linked entries.
- `workflow_runs (id, project_id, resolved_yaml, workflow_name, workflow_version, root_callsign, status, started_at, completed_at, cost_tokens, cost_usd_cents)`
  - `resolved_yaml` is the fully-resolved definition at dispatch time; runs are immune to mid-flight library upgrades.
- `workflow_run_stages (id, run_id, stage_name, pod_name, pod_version, session_id, status, attempt, input_artifact_ids, output_artifact_ids, eval_score_json, failure_reason, started_at, completed_at)`
- `artifact_types (name, schema_json, description)` — registry so the UI and validator agree
- `artifacts (id, type, callsign, workflow_run_id, stage_id, path, frontmatter_json, body_md, created_at, created_by_session_id, supersedes_id)`
- `secrets (id, name, description, scope_type, scope_id, tags_json, created_at, last_accessed_at)` — planned end-state naming; current Phase 0 implementation uses `vault_entries`, with **values in Stronghold and metadata only in SQLite**
- `secret_access_log (id, secret_id, session_id, workflow_run_id, reason, accessed_at, redacted)` — planned runtime-grant table; current implementation ships `vault_audit_events` as the broker/audit scaffold for settings mutations and launch-time env injection

All new tables get `workflow_run_id` and `stage_id` on their diagnostics events so the existing NDJSON feed can filter.

## Workflow Definition Format

Workflows live as YAML in `workflows/` (version-controlled) and sync into `workflow_definitions` on load. The YAML is the source of truth; the DB row is the denormalized copy used by the runtime.

Sketch:

```yaml
name: feature-dev
kind: coding
version: 1
description: Plan → sprint contract → generate → evaluate → merge

stages:
  - name: plan
    role: planner
    provider: claude_cli
    model: opus-4.6
    prompt_template: prompts/planner.feature-dev.md
    inputs: [work_item]
    outputs: [plan_doc, sprint_list]
    transitions:
      on_complete: sprint_loop

  - name: sprint_loop
    role: orchestrator
    strategy: for_each
    over: sprint_list
    body:
      - name: contract
        role: planner            # negotiates with evaluator persona
        provider: claude_cli
        model: opus-4.6
        inputs: [plan_doc, sprint]
        outputs: [sprint_contract]

      - name: generate
        role: generator
        provider: auto           # see routing below
        inputs: [sprint_contract]
        outputs: [implementation_report, diff_summary]
        worktree: per_sprint

      - name: evaluate
        role: evaluator
        provider: codex_cli
        inputs: [sprint_contract, implementation_report, diff_summary]
        outputs: [eval_report]
        retry_policy:
          on_fail_feedback_to: generate
          max_attempts: 3

    on_success: merge
    on_hard_fail: escalate

  - name: merge
    role: integrator
    provider: claude_cli
    model: sonnet-4.6
    inputs: [eval_report, sprint_contract]
    outputs: [merge_record]
```

Templates shipped in-repo for:

- `feature-dev` (above)
- `adr-authoring` (researcher → planner → writer → evaluator)
- `documentation-pass` (auditor → writer → evaluator)
- `data-analysis` (scoper → analyst → reviewer → report-writer)
- `research-brief` (scoper → researcher → synthesizer → critic)

A **workflow builder UI** (Phase 4) produces/edits these YAML files visually but never owns a separate schema.

## Role → Provider/Model Routing

Default policy (overridable per-workflow-stage):

| Role         | Default provider | Default model     | Notes |
|--------------|------------------|-------------------|-------|
| planner      | claude_cli       | opus-4.6          | Always Opus; planning errors cascade. |
| generator    | `auto`           | sonnet-4.6        | `auto` escalates to opus on `complexity=high` or cross-cutting tag; may route to `codex_cli` for stages tagged `codex_preferred`. |
| evaluator    | codex_cli        | (codex default)   | Independence from generator is the point. Fallback to Claude Sonnet if codex unavailable, but **never the same session family as the generator of that stage**. |
| reviewer     | claude_cli       | sonnet-4.6        | Lightweight pass; not a replacement for evaluator. |
| researcher   | claude_cli       | sonnet-4.6        | Opus if `depth=deep`. |
| synthesizer  | claude_cli       | opus-4.6          | Collapses research into a brief. |
| integrator   | claude_cli       | sonnet-4.6        | Performs merge + housekeeping. |

`auto` resolution is a small deterministic function of work-item frontmatter (`complexity`, `scope`, tags) plus a user-editable policy file (`config/auto-routing.yaml`). No LLM in the routing path.

**Hard rule:** evaluator session's provider MUST differ from the generator session's provider within the same sprint. Enforced at run dispatch.

## Artifact Contracts

Artifacts are the only currency between stages. Each type has:

- a frontmatter schema (validated before handoff)
- a markdown body template
- a retention policy

Core artifact types:

- `plan_doc` — deliverables, non-goals, sprint list (no granular tech prescriptions).
- `sprint_contract` — scope, acceptance criteria, rubric thresholds, out-of-scope list. **Evaluator reads this, not the plan doc.**
- `implementation_report` — what the generator did, files touched, known limitations, self-declared readiness (advisory only).
- `diff_summary` — machine-extracted from git; evaluator cross-checks against claims.
- `eval_report` — per-criterion score, pass/fail per hard threshold, feedback items keyed to generator actions.
- `merge_record` — commit SHAs, worktree cleanup confirmation, downstream invalidation notes.
- `adr`, `research_brief`, `data_analysis_report`, `doc_audit`, `doc_patchset` — for non-coding workflows.

Artifacts persist to both SQLite (indexed metadata) and the filesystem (`artifacts/<callsign>/<type>-<id>.md`) so they are diff-able and human-readable.

**Contract validation** runs at stage boundaries: if the produced artifact fails schema, the stage is marked `produced_invalid_artifact` and kicked back with a targeted error — this is cheap and catches most "agent declared done but didn't actually do it" failures before an evaluator ever runs.

## Sprint Contract Negotiation

Before any generator starts on a sprint:

1. Planner drafts a `sprint_contract` from the plan doc + sprint description.
2. Evaluator persona (same provider as the downstream evaluator, dedicated short session) reviews the contract and either signs off or pushes back on vagueness, missing acceptance criteria, or mis-scoped rubrics.
3. Contract is stamped `negotiated` and written to the DB as immutable.

This is the single cheapest quality improvement the article identifies. It also forces the planner to be concrete without over-specifying implementation.

## Evaluation Loop

- Evaluator runs on a **sibling worktree** or on a read-only view of the generator's worktree; it has its own MCP access to diff the branch.
- Scores each rubric criterion from the contract. Hard thresholds cause sprint fail regardless of aggregate.
- On fail, produces targeted feedback list tied to generator actions. Generator restarts with contract + prior implementation report + eval report; max N attempts (default 3).
- On hard fail after N attempts, sprint escalates to user with full artifact chain attached to the work item.
- On pass, integrator stage runs merge + cleanup.

All eval runs emit an `eval_report` artifact — *including passes*. Pass reports are the training data for harness tuning.

## Env Vault

### Storage

- Values stored in a Stronghold snapshot managed by the backend.
- Metadata (name, description, scope, tags, last-access) in SQLite `vault_entries` rows today; the runtime access-log tables remain later-phase work.
- Values **never** written to SQLite, logs, diagnostics, or crash artifacts.

### Scopes

- `global` — available to any session the user launches.
- `project` — scoped to a PC project (e.g. HAAS Alert creds only in that project).
- `workflow` — scoped to a specific workflow definition (e.g. Snowflake creds for `data_analysis` workflows only).
- `work_item` — short-lived; auto-expires when work item closes.

### Agent Access Model

Agents never receive raw secret bytes through chat context, frontend state hydration, or generic MCP reads. They operate against a brokered capability model:

```
Available capabilities:
  - github:read
  - github:repo
  - snowflake:query
  - anthropic:api
```

The trusted UI can list and deposit secrets. Runtime consumers resolve them only through supervised launch/integration paths. There is intentionally no generic agent-facing `get_secret` primitive in the final design.

### Injection Mode (opt-in)

For tools that read credentials from env or from a file path passed through env, the session host can inject resolved secrets at spawn time based on declared capability tags. The shipped runtime slice supports this through launch-profile `env_json` vault bindings plus workflow-stage / workflow-override `vault_env_bindings` carried into stage launches, with `delivery=env` or `delivery=file`; integration-template routing remains later work. Injected values are masked in all logs.

### Redaction

- Diagnostics, supervisor logs, session output logs, and crash reports all pipe through a redactor that replaces known secret values with a marker like `<vault:NAME>`.
- Redactor registers values when a secret is injected or fetched, not when defined — keeps memory pressure low.
  The shipped runtime slice already redacts launch-injected secrets from PTY/session-output persistence.

### UI

- `Settings → Vault` panel: list, add, edit, delete, scope tags, gate policy, last-accessed column.
- Per-session sidebar shows which secrets that session has touched, with reasons.
  This is planned follow-on work; the shipped slice is the Settings-based vault surface.

## Workflow Builder UX

Two surfaces, both operating on the same YAML files:

1. **YAML-first (Phase 1–3):** workflows are files in `workflows/`; the app hot-reloads on file change. Source of truth.
2. **Visual builder (Phase 4):** in-app graph editor that round-trips to YAML — no hidden state.

### Entry point

New top-level tab **Workflows** (uses the shared `Tabs` primitive). Top-level scope switcher: **Library** / **This Project**. Three sub-tabs:

- **Runs** — live + historical workflow runs for the current project (observability view).
- **Workflows** — workflow definitions at the selected scope.
- **Pods** — agent pod definitions at the selected scope.

In **Library** scope, you author and version global workflows + pods, including the app-shipped templates ("Feature Dev", "ADR", "Documentation Pass", "Data Analysis", "Research Brief"). Each library entry has an **Adopt in project** action.

In **This Project** scope, you see what the project has adopted, each with a badge (`linked @v3`, `forked`, `outdated @v2 → v4 available`). Actions per adoption: **Upgrade to latest**, **Edit overrides**, **Detach (fork)**, **Remove**.

### The canvas

Left-to-right DAG editor. Three regions:

- **Left rail — palette.** Stage types grouped by role: Planner, Generator, Evaluator, Reviewer, Researcher, Synthesizer, Integrator. Plus control nodes: `for_each`, `branch`, `merge`, `human_checkpoint`.
- **Center — graph.** Stage cards connected by typed edges. Edges colored by the artifact type flowing along them. Invalid wiring shows red with a concrete message ("evaluator `evaluate` consumes `sprint_contract` but no upstream stage produces one").
- **Right rail — inspector.** Whatever is selected — stage, edge, or the workflow itself.

### Stage card anatomy

Dropping a `Generator` node renders a compact card:

```
┌──────────────────────────────────────┐
│ generate                      ⚙ ⋯    │
│ role: generator                      │
│ provider: auto  ▸  sonnet-4.6        │
│ worktree: per_sprint                 │
├──────────────────────────────────────┤
│ inputs:  sprint_contract             │
│ outputs: implementation_report       │
│          diff_summary                │
├──────────────────────────────────────┤
│ needs_secrets: github:repo           │
└──────────────────────────────────────┘
```

Clicking opens the inspector with five tabs:

1. **Pod** — either pick a library pod (`generator.sonnet.standard`, `evaluator.codex.strict`, …) with a pinned version, or flip to **Inline config** for a one-off. When a pod is picked, the other tabs show pod-inherited values grayed out; overrides appear in bold.
2. **Basics** — name, role (locked by node type), provider, model, worktree mode. Provider dropdown filtered to providers that support the role; `auto` surfaces a "preview resolution" showing which model would be chosen for the current test work item.
3. **Prompt** — prompt-template picker (markdown files from `prompts/`), live preview with variables substituted from a sample work item. Edit-in-place opens the template file on disk.
4. **Contracts** — input/output artifact types as chips. Adding an output type makes it available to downstream stages. Removing an output that has downstream dependents lights the graph red and the inspector shows which stages depend on it.
5. **Policy** — retries, timeouts, `on_fail_feedback_to`, `needs_secrets` (capability tags and brokered bindings, never raw values).

### Wiring artifacts

Arrows are auto-drawn for the common case: when you add a stage downstream of another, the builder auto-connects any input types it can consume from upstream outputs. Manual wiring only for ambiguity or fan-in. Edges show the artifact type on hover with the full schema.

### Header bar

Above the canvas:

- Workflow name + kind (`coding` | `documentation` | etc.)
- Version badge (bumps on save)
- **Validate** — runs the graph validator (orphan stages, missing inputs, evaluator/generator provider separation, artifact-type mismatches).
- **Dry run** — mocks every stage; logs the artifact handoffs that *would* happen from a sample work item. No tokens spent.
- **Save** — writes YAML to `workflows/<name>.yaml`, bumps version, shows diff modal.
- **Publish** — marks a version as active default for new runs. Older versions remain runnable for in-flight work.

### Running a workflow

From the work-item panel, new button **Run workflow ▾**. Menu lists workflows whose `kind` matches the work item type plus an "All workflows" option. Modal confirms:

- Root work item (pre-filled)
- Any workflow-level parameters the YAML exposes
- Secrets scope that this run will have access to
- Cost budget cap (optional, default from workflow)

### Run view

Same DAG as the builder, nodes carry live state: `pending` / `running` / `passed` / `failed` / `retrying (2/3)`. Clicking a stage opens a drawer with:

- Session ID + link to the live terminal for that stage
- Token usage + cost so far
- Artifacts produced (clickable → artifact viewer)
- Mailbox traffic for this stage, filtered
- Eval report if the stage's evaluator has already run

Top strip: total cost, elapsed, iterations used vs. budget, next expected stage.

### Template gallery

Templates are plain workflow YAML files shipped with the app. "Use template" clones the YAML into `workflows/` as a new file; you edit from there. The builder never produces anything a human couldn't have written by hand.

### Authoring without the builder

Because YAML is the source of truth, advanced users skip the builder entirely. Drop a YAML file in `workflows/`, the app hot-reloads, validates, and it appears in Library. Builder and text editor stay in sync because they operate on the same files.

### Principles driving the UX

- **One surface for build + observe.** Same DAG layout in builder and run view — mental model transfers. Hovering a stage in a live run shows the prompt template that's executing.
- **No hidden state.** Everything the builder does maps to a line of YAML. Git-diff a workflow to review it.
- **Cheap safety nets before tokens.** Validate + Dry run catch most authoring mistakes before any real run spends money.
- **Templates are starting points, not magic.** A template is a YAML file you own after cloning.

### Phase 1 MVP (defer the visual editor)

Skip the full visual builder for Phase 1. Ship:

1. YAML hot-reload from `workflows/`
2. Library list view (read-only) with a "View as graph" rendering
3. Full run view with the live DAG + stage drawers

That delivers the entire experience except in-app editing — which happens in VS Code against the YAML files for the first few weeks. Visual builder lands in Phase 4 once the schema is stable enough that a UI on top is worth building.

## Observability / Harness Metrics

New dashboard: `Settings → Harness` (or a new top-level panel).

Per workflow definition:

- Runs executed, pass rate, avg cost, avg wall time
- Eval pass rate per stage
- Retry distribution per generator stage
- "Context anxiety" proxy: generator sessions that ended with `<N` tool calls and `>M` tokens remaining — surfaces premature completion.

Per active run:

- Stage DAG view with live status and artifact links
- Per-stage session ID, model, token usage, cost estimate
- Mailbox traffic for the run, filtered

All existing diagnostics events gain `workflow_run_id` and `stage_id` fields so current filters keep working.

## Integration Points With Existing PC

- **Mailbox stays the comms channel.** Stage transitions send a typed mailbox message (`workflow_stage_start`, `workflow_stage_done`, `workflow_stage_eval_fail`) to the relevant session.
- **Work items are the run root.** Every workflow run is attached to a WCP callsign; artifacts are linked back to the work item via `wcp_attach`-equivalent.
- **Worktrees.** Each generator stage gets its own worktree. Evaluator reads from the generator's worktree or its own clone. Cleanup happens in the integrator stage, per the existing cleanup-after-merge rule.
- **Supervisor owns the state machine.** Workflow orchestration is a Rust-side state machine in the supervisor, not in the frontend. Frontend observes.
- **Diagnostics pipeline.** Reused as-is; only new event types + new tags.

Files likely touched (planning estimate, not a commit list):

- `src-tauri/src/db.rs` — new tables + migrations
- `src-tauri/src/workflow/` (new module) — engine, loader, validator, router
- `src-tauri/src/vault.rs` (new module) — Stronghold wrapper, metadata, audit, redactor
- `src-tauri/src/supervisor_mcp.rs` — workflow/runtime tools plus brokered vault deposit / execution helpers
- `src-tauri/src/bin/project-commander-supervisor.rs` — workflow dispatch loop
- `src/components/workflow/` (new) — run view, builder UI
- `src/components/AppSettingsPanel.tsx` — Vault settings surface
- `workflows/*.yaml`, `prompts/*.md`, `artifact-types/*.schema.json`
- `A_PLUS_TRACKER.md`, `docs/observability.md`, this doc

## Phased Rollout

Each phase must leave the app green (`npm test`, `npm run build`, `cargo test`) and preserve A+.

**Phase 0 — Foundations (data + vault skeleton).**
- Add new tables + migrations, including `library_workflows`, `library_pods`, `project_adoptions`, `project_overrides`.
- Build env vault storage (Stronghold + metadata) with trusted-UI deposit/edit/delete, audit scaffolding, redaction hooks, and no generic agent-readable secret API.
- No workflow engine yet; vault is usable standalone.

**Phase 1 — Minimal coding workflow, library-scoped, YAML only.**
- Seed library with shipped defaults: `feature-dev` workflow + the default pod set (`planner.opus.standard`, `generator.sonnet.standard`, `evaluator.codex.strict`, etc.).
- Workflow engine in supervisor: resolve library → project adoption → overrides → run.
- Stages: planner (Opus) → sprint_contract → generator (Sonnet/Opus auto) → evaluator (Codex) → integrator.
- Artifact types: `plan_doc`, `sprint_contract`, `implementation_report`, `diff_summary`, `eval_report`, `merge_record`.
- UI: Library (read-only list) + This Project (adopt/upgrade/detach) + read-only run view (DAG + status + artifact links). No visual editor yet.

**Phase 2 — Routing + cross-provider evaluator hardening.**
- Implement `auto` routing policy.
- Enforce generator/evaluator provider-separation rule.
- Codex adapter end-to-end for evaluator role; fallback policy documented.

**Phase 3 — Additional workflow templates.**
- `adr-authoring`, `documentation-pass`, `data-analysis`, `research-brief`.
- Per-workflow artifact-type registry extensions.

**Phase 4 — Visual workflow builder.**
- Graph editor, validation, YAML round-trip, dry-run mode.

**Phase 5 — Harness telemetry dashboard.**
- Per-definition metrics, context-anxiety proxy, cost tracking, eval-pass trends.

## Open Questions

1. **Codex MCP reach.** Can Codex CLI reach PC's MCP server over HTTP/SSE the way Claude CLI does? If not, evaluator needs file-based artifact read — doable but slower. Need to confirm in the agent lab.
2. **Cost budgeting.** Article cites 20x cost. Do we want per-workflow-run hard budgets, with the engine halting on overrun?
3. **Sprint granularity enforcement.** How do we prevent a planner from emitting 1-sprint plans that re-become monolithic builds? Candidate: rubric in the planner's own prompt + auto-rejection of plans with `<2` sprints for `coding` workflows above a complexity threshold.
4. **Secret rotation + staleness.** Do we proactively warn when a secret hasn't been accessed in N days and might be stale, or when one is accessed abnormally often?
5. **Contract immutability vs. replan.** If mid-sprint the generator discovers the contract is infeasible, what's the escape valve? Proposal: `request_contract_amendment` — generator pauses, planner+evaluator re-negotiate, new contract version supersedes the old.
6. **Human-in-the-loop checkpoints.** Should any workflow type require explicit user sign-off between stages by default? Proposal: yes for `adr-authoring` and `data-analysis`; no for `feature-dev` unless flagged.
7. **Multi-run coordination.** If two workflow runs touch the same files via different worktrees, who arbitrates? Likely deferred until we see it break.

## Risks

- **Complexity creep.** Workflow engine + vault + builder is a lot. Phase 0–1 must be usable standalone before Phase 4 is started.
- **Context leakage via artifacts.** If artifact bodies grow too large, handoffs recreate the compaction problem. Enforce size budgets per artifact type.
- **Evaluator provider unavailability.** Codex CLI outage should not stall a run indefinitely — need a documented fallback provider and a "degraded" badge on the run.
- **Secret misuse.** A rogue prompt could request a brokered integration it should not have. Mitigations: capability-tag enforcement, per-session confirmation gates, runtime audit events, and no generic raw-secret fetch primitive.
- **A+ regression risk.** New polling in the engine, new global refresh in the UI — both would regress A+. Engine must be event-driven (mailbox + SQLite `updated_at` watchers already in place).

## Decision Checklist (for our review)

- [ ] Confirm role→provider defaults table.
- [ ] Confirm artifact-type list for Phase 1.
- [ ] Confirm YAML-first over builder-first for Phase 1.
- [x] Confirm env vault storage backend: Stronghold-backed backend storage with SQLite metadata, no generic agent-readable secret API.
- [ ] Confirm workflow templates to ship in Phase 3.
- [ ] Confirm default pod set shipped with the library.
- [ ] Confirm adoption default is `linked` (vs. `forked`) and that overrides are the preferred ergonomic path.
- [ ] Decide whether library entries can be shared between machines (export/import bundle, or future git-backed sync).
- [ ] Confirm shipped category seed list and whether workflows and projects share the same category registry as pods (default: yes, one registry).
- [ ] Decide on per-run cost budgets (Q2 above).
- [ ] Decide on default HITL checkpoints (Q6 above).
