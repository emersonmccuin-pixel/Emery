use crate::vault::VaultAccessBindingRequest;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

const LIBRARY_DIR_NAME: &str = "library";
const WORKFLOW_DIR_NAME: &str = "workflows";
const POD_DIR_NAME: &str = "pods";
const PROJECT_COMMANDER_DIR_NAME: &str = ".project-commander";
const PROJECT_OVERRIDE_DIR_NAME: &str = "overrides";

const SHIPPED_CATEGORIES: &[(&str, &str)] = &[
    (
        "CODING",
        "Software delivery, debugging, and implementation workflows.",
    ),
    (
        "MARKETING",
        "Campaign, messaging, and outbound marketing workflows.",
    ),
    (
        "SALES",
        "Pipeline, account, and sales-enablement workflows.",
    ),
    (
        "DATA",
        "Analysis, reporting, and data-processing workflows.",
    ),
    (
        "DOCUMENTATION",
        "Docs authoring, audits, and reference-material workflows.",
    ),
    (
        "RESEARCH",
        "Research collection, synthesis, and briefing workflows.",
    ),
    (
        "DESIGN",
        "Design-system, UX, and creative-production workflows.",
    ),
    (
        "PRODUCT",
        "Planning, scoping, and product-delivery workflows.",
    ),
    ("OPS", "Operational, infrastructure, and support workflows."),
    ("LEGAL", "Policy, compliance, and legal-review workflows."),
    ("META", "Meta or uncategorized workflow primitives."),
];

const SHIPPED_POD_FILES: &[(&str, &str)] = &[
    (
        "planner.opus.standard.yaml",
        include_str!("workflow_defaults/pods/planner.opus.standard.yaml"),
    ),
    (
        "generator.sonnet.standard.yaml",
        include_str!("workflow_defaults/pods/generator.sonnet.standard.yaml"),
    ),
    (
        "generator.opus.crosscutting.yaml",
        include_str!("workflow_defaults/pods/generator.opus.crosscutting.yaml"),
    ),
    (
        "generator.codex.cli-work.yaml",
        include_str!("workflow_defaults/pods/generator.codex.cli-work.yaml"),
    ),
    (
        "evaluator.codex.strict.yaml",
        include_str!("workflow_defaults/pods/evaluator.codex.strict.yaml"),
    ),
    (
        "evaluator.sonnet.fallback.yaml",
        include_str!("workflow_defaults/pods/evaluator.sonnet.fallback.yaml"),
    ),
    (
        "reviewer.sonnet.lightweight.yaml",
        include_str!("workflow_defaults/pods/reviewer.sonnet.lightweight.yaml"),
    ),
    (
        "researcher.sonnet.standard.yaml",
        include_str!("workflow_defaults/pods/researcher.sonnet.standard.yaml"),
    ),
    (
        "integrator.sonnet.merge.yaml",
        include_str!("workflow_defaults/pods/integrator.sonnet.merge.yaml"),
    ),
];

const SHIPPED_WORKFLOW_FILES: &[(&str, &str)] = &[
    (
        "feature-dev.yaml",
        include_str!("workflow_defaults/workflows/feature-dev.yaml"),
    ),
    (
        "adr-authoring.yaml",
        include_str!("workflow_defaults/workflows/adr-authoring.yaml"),
    ),
    (
        "documentation-pass.yaml",
        include_str!("workflow_defaults/workflows/documentation-pass.yaml"),
    ),
    (
        "data-analysis.yaml",
        include_str!("workflow_defaults/workflows/data-analysis.yaml"),
    ),
    (
        "research-brief.yaml",
        include_str!("workflow_defaults/workflows/research-brief.yaml"),
    ),
];

const WORKFLOW_RUNTIME_INPUTS: &[&str] = &["work_item", "project_tracker"];

const BUILTIN_ARTIFACT_CONTRACTS: &[(&str, &str, &str, &[&str], &[&str])] = &[
    (
        "plan_doc",
        "Plan Doc",
        "High-level plan with scoped deliverables and acceptance criteria.",
        &["deliverables", "acceptanceCriteria"],
        &["## Scope", "## Acceptance Criteria"],
    ),
    (
        "sprint_list",
        "Sprint List",
        "Sequenced sprint breakdown for the root work item.",
        &["sprints"],
        &["## Sprint Breakdown"],
    ),
    (
        "sprint_contract",
        "Sprint Contract",
        "Negotiated implementation contract for a generator/evaluator loop.",
        &["acceptanceCriteria", "outOfScope"],
        &["## Scope", "## Out Of Scope"],
    ),
    (
        "implementation_report",
        "Implementation Report",
        "Generator handoff describing changes, verification, and open risks.",
        &["filesTouched", "verification"],
        &["## Changes", "## Verification"],
    ),
    (
        "diff_summary",
        "Diff Summary",
        "Concise summary of the current worktree diff.",
        &["filesTouched"],
        &["## Diff Highlights"],
    ),
    (
        "eval_report",
        "Evaluation Report",
        "Independent evaluation verdict and required follow-up actions.",
        &["decision", "score"],
        &["## Findings", "## Verification"],
    ),
    (
        "merge_record",
        "Merge Record",
        "Integrator handoff for merge readiness, cleanup, and follow-up.",
        &["verification"],
        &["## Merge Readiness", "## Cleanup Notes"],
    ),
    (
        "research_brief",
        "Research Brief",
        "Condensed research findings and recommendation framing.",
        &["sources"],
        &["## Findings", "## Recommendations"],
    ),
    (
        "adr_outline",
        "ADR Outline",
        "Structured ADR outline before drafting.",
        &["decision", "alternatives"],
        &["## Context", "## Options"],
    ),
    (
        "adr",
        "ADR",
        "Final architecture decision record draft.",
        &["decision", "status"],
        &["## Context", "## Decision"],
    ),
    (
        "review_notes",
        "Review Notes",
        "Review feedback with concrete findings and follow-up.",
        &["decision"],
        &["## Findings"],
    ),
    (
        "analysis_contract",
        "Analysis Contract",
        "Scope and evaluation contract for analysis work.",
        &["questions"],
        &["## Questions", "## Acceptance Criteria"],
    ),
    (
        "data_analysis_report",
        "Data Analysis Report",
        "Analysis output with methods, findings, and caveats.",
        &["queriesRun", "datasets"],
        &["## Method", "## Findings"],
    ),
    (
        "doc_audit",
        "Doc Audit",
        "Audit of documentation gaps, inconsistencies, and actions.",
        &["gaps"],
        &["## Findings", "## Recommended Changes"],
    ),
    (
        "doc_patchset",
        "Doc Patchset",
        "Drafted documentation update set for review.",
        &["filesTouched"],
        &["## Changes", "## Verification"],
    ),
    (
        "research_scope",
        "Research Scope",
        "Research framing and questions to answer.",
        &["questions"],
        &["## Scope", "## Questions"],
    ),
    (
        "source_notes",
        "Source Notes",
        "Collected notes and citations from source review.",
        &["sources"],
        &["## Source Notes"],
    ),
    (
        "critique_notes",
        "Critique Notes",
        "Critique findings against a synthesized research brief.",
        &["decision"],
        &["## Findings", "## Gaps"],
    ),
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCategoryRecord {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_shipped: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStageRetryPolicyRecord {
    pub max_attempts: i64,
    pub on_fail_feedback_to: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowArtifactContractRecord {
    pub artifact_type: String,
    pub label: String,
    pub description: String,
    pub required_frontmatter_fields: Vec<String>,
    pub required_markdown_sections: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProducedArtifactRecord {
    #[serde(rename = "type", alias = "artifactType")]
    pub artifact_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default, alias = "body", alias = "bodyMarkdown")]
    pub body_markdown: Option<String>,
    #[serde(default)]
    pub frontmatter: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStageRecord {
    pub name: String,
    pub role: String,
    pub pod_ref: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_template_ref: Option<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub input_contracts: Vec<WorkflowArtifactContractRecord>,
    #[serde(default)]
    pub output_contracts: Vec<WorkflowArtifactContractRecord>,
    pub needs_secrets: Vec<String>,
    pub vault_env_bindings: Vec<VaultAccessBindingRequest>,
    pub retry_policy: Option<WorkflowStageRetryPolicyRecord>,
    pub retry_summary: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecord {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub version: i64,
    pub description: String,
    pub source: String,
    pub template: bool,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub stages: Vec<WorkflowStageRecord>,
    pub pod_refs: Vec<String>,
    pub yaml: String,
    pub file_path: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodRecord {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub role: String,
    pub version: i64,
    pub description: String,
    pub provider: String,
    pub model: Option<String>,
    pub prompt_template_ref: Option<String>,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub tool_allowlist: Vec<String>,
    pub secret_scopes: Vec<String>,
    pub default_policy_json: String,
    pub yaml: String,
    pub source: String,
    pub file_path: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLibrarySnapshot {
    pub library_root: String,
    pub workflow_dir: String,
    pub pod_dir: String,
    pub categories: Vec<WorkflowCategoryRecord>,
    pub workflows: Vec<WorkflowRecord>,
    pub pods: Vec<PodRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptionRecord {
    pub slug: String,
    pub pinned_version: i64,
    pub latest_version: Option<i64>,
    pub mode: String,
    pub is_outdated: bool,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowCatalog {
    pub project_id: i64,
    pub workflows: Vec<ProjectWorkflowRecord>,
    pub pods: Vec<ProjectPodRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkflowStageRecord {
    pub ordinal: i64,
    pub name: String,
    pub role: String,
    pub pod_slug: Option<String>,
    pub pod_version: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub prompt_template_ref: Option<String>,
    pub tool_allowlist: Vec<String>,
    pub secret_scopes: Vec<String>,
    pub default_policy_json: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub input_contracts: Vec<WorkflowArtifactContractRecord>,
    #[serde(default)]
    pub output_contracts: Vec<WorkflowArtifactContractRecord>,
    pub needs_secrets: Vec<String>,
    pub vault_env_bindings: Vec<VaultAccessBindingRequest>,
    pub retry_policy: Option<WorkflowStageRetryPolicyRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkflowRecord {
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub version: i64,
    pub description: String,
    pub source: String,
    pub template: bool,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub adoption_mode: String,
    pub has_overrides: bool,
    pub stages: Vec<ResolvedWorkflowStageRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunStageRecord {
    pub id: i64,
    pub run_id: i64,
    pub stage_ordinal: i64,
    pub stage_name: String,
    pub stage_role: String,
    pub pod_slug: Option<String>,
    pub pod_version: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub worktree_id: Option<i64>,
    pub session_id: Option<i64>,
    pub agent_name: Option<String>,
    pub thread_id: Option<String>,
    pub directive_message_id: Option<i64>,
    pub response_message_id: Option<i64>,
    pub status: String,
    pub attempt: i64,
    pub completion_message_type: Option<String>,
    pub completion_summary: Option<String>,
    pub completion_context_json: String,
    pub produced_artifacts: Vec<WorkflowProducedArtifactRecord>,
    pub artifact_validation_status: Option<String>,
    pub artifact_validation_error: Option<String>,
    pub retry_source_stage_name: Option<String>,
    pub retry_feedback_summary: Option<String>,
    pub retry_feedback_context_json: String,
    pub retry_requested_at: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
    pub resolved_stage: ResolvedWorkflowStageRecord,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunRecord {
    pub id: i64,
    pub project_id: i64,
    pub workflow_slug: String,
    pub workflow_name: String,
    pub workflow_kind: String,
    pub workflow_version: i64,
    pub root_work_item_id: i64,
    pub root_work_item_call_sign: String,
    pub root_worktree_id: Option<i64>,
    pub source_adoption_mode: String,
    pub status: String,
    pub has_overrides: bool,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub updated_at: String,
    pub resolved_workflow: ResolvedWorkflowRecord,
    pub stages: Vec<WorkflowRunStageRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowRunSnapshot {
    pub project_id: i64,
    pub runs: Vec<WorkflowRunRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowRecord {
    pub adoption: AdoptionRecord,
    #[serde(flatten)]
    pub workflow: WorkflowRecord,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPodRecord {
    pub adoption: AdoptionRecord,
    #[serde(flatten)]
    pub pod: PodRecord,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptCatalogEntryInput {
    pub project_id: i64,
    pub entity_type: String,
    pub slug: String,
    pub mode: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAdoptionTarget {
    pub project_id: i64,
    pub entity_type: String,
    pub slug: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartWorkflowRunInput {
    pub project_id: i64,
    pub workflow_slug: String,
    pub root_work_item_id: i64,
    pub root_worktree_id: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkWorkflowStageDispatchedInput {
    pub project_id: i64,
    pub run_id: i64,
    pub stage_name: String,
    pub worktree_id: Option<i64>,
    pub session_id: Option<i64>,
    pub agent_name: Option<String>,
    pub thread_id: Option<String>,
    pub directive_message_id: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordWorkflowStageResultInput {
    pub project_id: i64,
    pub run_id: i64,
    pub stage_name: String,
    pub response_message_id: Option<i64>,
    pub completion_message_type: String,
    pub completion_summary: Option<String>,
    pub completion_context_json: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStageRetryDispatchRecord {
    pub source_stage_name: String,
    pub target_stage_name: String,
    pub next_attempt: i64,
    pub max_attempts: i64,
    pub feedback_summary: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordWorkflowStageResultOutput {
    pub run: WorkflowRunRecord,
    pub retry: Option<WorkflowStageRetryDispatchRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailWorkflowRunInput {
    pub project_id: i64,
    pub run_id: i64,
    pub stage_name: Option<String>,
    pub failure_reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowDefinitionYaml {
    #[serde(default)]
    slug: Option<String>,
    name: String,
    kind: String,
    version: i64,
    #[serde(default)]
    description: String,
    #[serde(default)]
    template: bool,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    stages: Vec<WorkflowStageYaml>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowStageYaml {
    name: String,
    role: String,
    #[serde(default, alias = "pod_ref", alias = "podRef")]
    pod_ref: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "prompt_template", alias = "promptTemplate")]
    prompt_template_ref: Option<String>,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    needs_secrets: Vec<String>,
    #[serde(
        default,
        alias = "vaultEnvBindings",
        alias = "secret_bindings",
        alias = "secretBindings"
    )]
    vault_env_bindings: Vec<VaultAccessBindingRequest>,
    #[serde(default, alias = "retry_policy", alias = "retryPolicy")]
    retry_policy: Option<RetryPolicyYaml>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RetryPolicyYaml {
    #[serde(default)]
    max_attempts: Option<i64>,
    #[serde(default, alias = "on_fail_feedback_to", alias = "onFailFeedbackTo")]
    on_fail_feedback_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PodDefinitionYaml {
    name: String,
    role: String,
    version: i64,
    #[serde(default)]
    description: String,
    categories: Vec<String>,
    provider: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "prompt_template_ref", alias = "promptTemplateRef")]
    prompt_template_ref: Option<String>,
    #[serde(default)]
    tool_allowlist: Vec<String>,
    #[serde(default)]
    secret_scopes: Vec<String>,
    #[serde(default)]
    default_policy: serde_yaml::Value,
    #[serde(default)]
    tags: Vec<String>,
}

struct ParsedWorkflowDefinition {
    slug: String,
    name: String,
    kind: String,
    version: i64,
    description: String,
    template: bool,
    categories: Vec<String>,
    tags: Vec<String>,
    stages: Vec<WorkflowStageRecord>,
    pod_refs: Vec<String>,
    yaml: String,
    source: String,
    file_path: String,
}

struct ParsedPodDefinition {
    slug: String,
    name: String,
    role: String,
    version: i64,
    description: String,
    provider: String,
    model: Option<String>,
    prompt_template_ref: Option<String>,
    categories: Vec<String>,
    tags: Vec<String>,
    tool_allowlist: Vec<String>,
    secret_scopes: Vec<String>,
    default_policy_json: String,
    yaml: String,
    source: String,
    file_path: String,
}

#[derive(Clone)]
struct CatalogAdoptionRow {
    entity_type: String,
    entity_slug: String,
    pinned_version: i64,
    mode: String,
    detached_yaml: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowStageOverrideRecord {
    #[serde(alias = "stage_name")]
    stage_name: String,
    #[serde(default, alias = "pod_ref", alias = "podRef")]
    pod_ref: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "prompt_template_ref", alias = "promptTemplateRef")]
    prompt_template_ref: Option<String>,
    #[serde(default, alias = "needs_secrets")]
    needs_secrets: Option<Vec<String>>,
    #[serde(
        default,
        alias = "vault_env_bindings",
        alias = "secret_bindings",
        alias = "secretBindings"
    )]
    vault_env_bindings: Option<Vec<VaultAccessBindingRequest>>,
    #[serde(default, alias = "retry_policy", alias = "retryPolicy")]
    retry_policy: Option<WorkflowStageRetryPolicyRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowOverrideSetRecord {
    #[serde(default, alias = "stage_overrides")]
    stage_overrides: Vec<WorkflowStageOverrideRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowOverrideDocument {
    pub project_id: i64,
    pub workflow_slug: String,
    pub file_path: String,
    pub exists: bool,
    pub source: String,
    pub yaml: String,
    pub has_overrides: bool,
    pub stage_override_count: i64,
    pub validation_error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectWorkflowOverrideInput {
    pub project_id: i64,
    pub workflow_slug: String,
    pub yaml: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryWorkflowStageInput {
    pub name: String,
    pub role: String,
    pub pod_ref: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_template_ref: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub needs_secrets: Vec<String>,
    #[serde(default)]
    pub vault_env_bindings: Vec<VaultAccessBindingRequest>,
    #[serde(default)]
    pub retry_policy: Option<WorkflowStageRetryPolicyRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibraryWorkflowInput {
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub version: i64,
    pub description: String,
    pub template: bool,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub stages: Vec<LibraryWorkflowStageInput>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLibraryWorkflowInput {
    pub slug: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkflowOverrideTarget {
    pub project_id: i64,
    pub workflow_slug: String,
}

pub fn library_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(LIBRARY_DIR_NAME)
}

pub fn workflow_dir(app_data_dir: &Path) -> PathBuf {
    library_root(app_data_dir).join(WORKFLOW_DIR_NAME)
}

pub fn pod_dir(app_data_dir: &Path) -> PathBuf {
    library_root(app_data_dir).join(POD_DIR_NAME)
}

pub fn project_workflow_override_dir(project_root: &Path) -> PathBuf {
    project_root
        .join(PROJECT_COMMANDER_DIR_NAME)
        .join(PROJECT_OVERRIDE_DIR_NAME)
}

fn project_workflow_override_path(project_root: &Path, workflow_slug: &str) -> Result<PathBuf, String> {
    let normalized_slug = normalize_required(workflow_slug, "workflow override slug")?;
    if normalized_slug.contains('/')
        || normalized_slug.contains('\\')
        || normalized_slug.contains("..")
    {
        return Err(format!(
            "workflow override slug '{normalized_slug}' contains an unsafe path segment"
        ));
    }

    Ok(project_workflow_override_dir(project_root).join(format!("{normalized_slug}.yaml")))
}

fn render_override_yaml(overrides: &WorkflowOverrideSetRecord) -> Result<String, String> {
    let mut yaml = serde_yaml::to_string(overrides)
        .map_err(|error| format!("failed to encode workflow override YAML: {error}"))?;
    if let Some(rest) = yaml.strip_prefix("---\n") {
        yaml = rest.to_string();
    }
    Ok(yaml)
}

fn render_workflow_yaml(input: &SaveLibraryWorkflowInput) -> Result<String, String> {
    let document = WorkflowDefinitionYaml {
        slug: Some(normalize_slug(&input.slug, "workflow slug")?),
        name: normalize_required(&input.name, "workflow name")?.to_string(),
        kind: normalize_required(&input.kind, "workflow kind")?.to_string(),
        version: input.version.max(1),
        description: input.description.trim().to_string(),
        template: input.template,
        categories: normalize_category_list(&input.categories)?,
        tags: normalize_string_list(&input.tags, "workflow tag")?,
        stages: input
            .stages
            .iter()
            .map(|stage| {
                let retry_policy = match stage.retry_policy.as_ref() {
                    Some(policy) => Some(RetryPolicyYaml {
                        max_attempts: Some(policy.max_attempts.max(1)),
                        on_fail_feedback_to: policy
                            .on_fail_feedback_to
                            .as_deref()
                            .map(|value| normalize_required(value, "retry feedback target"))
                            .transpose()?
                            .map(str::to_string),
                    }),
                    None => None,
                };
                Ok(WorkflowStageYaml {
                    name: normalize_required(&stage.name, "workflow stage name")?.to_string(),
                    role: normalize_required(&stage.role, "workflow stage role")?.to_string(),
                    pod_ref: stage
                        .pod_ref
                        .as_deref()
                        .map(|value| normalize_required(value, "workflow pod ref"))
                        .transpose()?
                        .map(str::to_string),
                    provider: normalize_optional(&stage.provider),
                    model: normalize_optional(&stage.model),
                    prompt_template_ref: normalize_optional(&stage.prompt_template_ref),
                    inputs: normalize_string_list(&stage.inputs, "workflow inputs")?,
                    outputs: normalize_string_list(&stage.outputs, "workflow outputs")?,
                    needs_secrets: normalize_string_list(
                        &stage.needs_secrets,
                        "workflow secrets",
                    )?,
                    vault_env_bindings: normalize_vault_binding_requests(
                        &stage.vault_env_bindings,
                        "workflow stage vault binding",
                    )?,
                    retry_policy,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    };

    let mut yaml = serde_yaml::to_string(&document)
        .map_err(|error| format!("failed to encode workflow YAML: {error}"))?;
    if let Some(rest) = yaml.strip_prefix("---\n") {
        yaml = rest.to_string();
    }
    Ok(yaml)
}

fn resolve_adopted_workflow_for_editor(
    connection: &Connection,
    adoption: &CatalogAdoptionRow,
) -> Result<WorkflowRecord, String> {
    match adoption.mode.as_str() {
        "linked" => load_library_workflow_by_slug(connection, &adoption.entity_slug),
        "forked" => {
            let detached_yaml = adoption.detached_yaml.as_deref().ok_or_else(|| {
                format!(
                    "forked workflow '{}' is missing its detached YAML snapshot",
                    adoption.entity_slug
                )
            })?;
            parse_detached_workflow(&adoption.entity_slug, detached_yaml)
        }
        other => Err(format!(
            "workflow adoption '{}' uses unsupported mode '{other}'",
            adoption.entity_slug
        )),
    }
}

fn canonicalize_override_set(
    workflow: &WorkflowRecord,
    overrides: WorkflowOverrideSetRecord,
) -> Result<WorkflowOverrideSetRecord, String> {
    let lookup = normalize_override_lookup(workflow, Some(overrides))?;
    let stage_overrides = workflow
        .stages
        .iter()
        .filter_map(|stage| lookup.get(&stage.name).cloned())
        .collect::<Vec<_>>();

    Ok(WorkflowOverrideSetRecord { stage_overrides })
}

fn parse_override_yaml_for_workflow(
    workflow: &WorkflowRecord,
    yaml: &str,
) -> Result<WorkflowOverrideSetRecord, String> {
    let overrides = serde_yaml::from_str::<WorkflowOverrideSetRecord>(yaml)
        .map_err(|error| format!("failed to parse workflow override YAML: {error}"))?;
    canonicalize_override_set(workflow, overrides)
}

pub fn seed_library_files(app_data_dir: &Path) -> Result<(), String> {
    let workflow_dir = workflow_dir(app_data_dir);
    let pod_dir = pod_dir(app_data_dir);
    fs::create_dir_all(&workflow_dir)
        .map_err(|error| format!("failed to create workflow library directory: {error}"))?;
    fs::create_dir_all(&pod_dir)
        .map_err(|error| format!("failed to create workflow pod directory: {error}"))?;

    for (file_name, contents) in SHIPPED_WORKFLOW_FILES {
        seed_library_file(&workflow_dir, file_name, contents)?;
    }
    for (file_name, contents) in SHIPPED_POD_FILES {
        seed_library_file(&pod_dir, file_name, contents)?;
    }

    Ok(())
}

pub fn sync_library_catalog(connection: &Connection, app_data_dir: &Path) -> Result<(), String> {
    seed_library_files(app_data_dir)?;
    ensure_seed_categories(connection)?;

    let pod_dir = pod_dir(app_data_dir);
    let workflow_dir = workflow_dir(app_data_dir);
    let pod_files = list_yaml_files(&pod_dir)?;
    let workflow_files = list_yaml_files(&workflow_dir)?;
    let shipped_pod_names = shipped_file_name_set(SHIPPED_POD_FILES);
    let shipped_workflow_names = shipped_file_name_set(SHIPPED_WORKFLOW_FILES);

    let parsed_pods = pod_files
        .iter()
        .map(|path| parse_pod_definition(path, shipped_pod_names.contains(file_name(path))))
        .collect::<Result<Vec<_>, _>>()?;
    let pod_slugs = parsed_pods
        .iter()
        .map(|pod| pod.slug.clone())
        .collect::<BTreeSet<_>>();

    let parsed_workflows = workflow_files
        .iter()
        .map(|path| {
            parse_workflow_definition(
                path,
                shipped_workflow_names.contains(file_name(path)),
                &pod_slugs,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin workflow catalog sync: {error}"))?;

    let sync_result = (|| {
        connection
            .execute("DELETE FROM library_workflow_category_assignments", [])
            .map_err(|error| format!("failed to clear workflow category assignments: {error}"))?;
        connection
            .execute("DELETE FROM library_pod_category_assignments", [])
            .map_err(|error| format!("failed to clear pod category assignments: {error}"))?;
        connection
            .execute("DELETE FROM library_workflows", [])
            .map_err(|error| format!("failed to clear library workflows: {error}"))?;
        connection
            .execute("DELETE FROM library_pods", [])
            .map_err(|error| format!("failed to clear library pods: {error}"))?;

        for pod in parsed_pods {
            let pod_id = insert_pod_definition(connection, &pod)?;
            assign_categories(
                connection,
                "library_pod_category_assignments",
                "pod_id",
                pod_id,
                &pod.categories,
            )?;
        }

        for workflow in parsed_workflows {
            let workflow_id = insert_workflow_definition(connection, &workflow)?;
            assign_categories(
                connection,
                "library_workflow_category_assignments",
                "workflow_id",
                workflow_id,
                &workflow.categories,
            )?;
        }

        Ok(())
    })();

    match sync_result {
        Ok(()) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit workflow catalog sync: {error}"))?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn load_library_snapshot(
    connection: &Connection,
    app_data_dir: &Path,
) -> Result<WorkflowLibrarySnapshot, String> {
    Ok(WorkflowLibrarySnapshot {
        library_root: library_root(app_data_dir).display().to_string(),
        workflow_dir: workflow_dir(app_data_dir).display().to_string(),
        pod_dir: pod_dir(app_data_dir).display().to_string(),
        categories: load_categories(connection)?,
        workflows: load_library_workflows(connection)?,
        pods: load_library_pods(connection)?,
    })
}

pub fn load_project_catalog(
    connection: &Connection,
    project_id: i64,
) -> Result<ProjectWorkflowCatalog, String> {
    let workflow_lookup = load_library_workflows(connection)?
        .into_iter()
        .map(|workflow| (workflow.slug.clone(), workflow))
        .collect::<HashMap<_, _>>();
    let pod_lookup = load_library_pods(connection)?
        .into_iter()
        .map(|pod| (pod.slug.clone(), pod))
        .collect::<HashMap<_, _>>();

    let mut statement = connection
        .prepare(
            "
            SELECT entity_type, entity_slug, pinned_version, mode, detached_yaml, updated_at
            FROM project_catalog_adoptions
            WHERE project_id = ?1
            ORDER BY entity_type ASC, entity_slug ASC
            ",
        )
        .map_err(|error| format!("failed to prepare project catalog query: {error}"))?;

    let adoptions = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| format!("failed to query project catalog adoptions: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect project catalog adoptions: {error}"))?;

    let mut workflows = Vec::new();
    let mut pods = Vec::new();

    for (entity_type, slug, pinned_version, mode, detached_yaml, updated_at) in adoptions {
        match entity_type.as_str() {
            "workflow" => {
                let (workflow, latest_version) =
                    resolve_project_workflow(&slug, detached_yaml.as_deref(), &workflow_lookup)?;
                workflows.push(ProjectWorkflowRecord {
                    adoption: AdoptionRecord {
                        slug: slug.clone(),
                        pinned_version,
                        latest_version,
                        mode: mode.clone(),
                        is_outdated: latest_version
                            .map(|value| value > pinned_version)
                            .unwrap_or(false),
                        updated_at: updated_at.clone(),
                    },
                    workflow,
                });
            }
            "pod" => {
                let (pod, latest_version) =
                    resolve_project_pod(&slug, detached_yaml.as_deref(), &pod_lookup)?;
                pods.push(ProjectPodRecord {
                    adoption: AdoptionRecord {
                        slug: slug.clone(),
                        pinned_version,
                        latest_version,
                        mode: mode.clone(),
                        is_outdated: latest_version
                            .map(|value| value > pinned_version)
                            .unwrap_or(false),
                        updated_at: updated_at.clone(),
                    },
                    pod,
                });
            }
            _ => {}
        }
    }

    Ok(ProjectWorkflowCatalog {
        project_id,
        workflows,
        pods,
    })
}

pub fn load_project_run_snapshot(
    connection: &Connection,
    project_id: i64,
) -> Result<ProjectWorkflowRunSnapshot, String> {
    ensure_project_exists(connection, project_id)?;
    Ok(ProjectWorkflowRunSnapshot {
        project_id,
        runs: load_workflow_runs(connection, project_id)?,
    })
}

pub fn save_library_workflow(
    connection: &Connection,
    app_data_dir: &Path,
    input: &SaveLibraryWorkflowInput,
) -> Result<WorkflowLibrarySnapshot, String> {
    ensure_seed_categories(connection)?;
    seed_library_files(app_data_dir)?;
    sync_library_catalog(connection, app_data_dir)?;

    let workflow_dir = workflow_dir(app_data_dir);
    fs::create_dir_all(&workflow_dir).map_err(|error| {
        format!(
            "failed to create workflow library directory {}: {error}",
            workflow_dir.display()
        )
    })?;

    let normalized_slug = normalize_slug(&input.slug, "workflow slug")?;
    let existing = load_library_workflows(connection)?
        .into_iter()
        .find(|workflow| workflow.slug == normalized_slug);

    if existing.as_ref().map(|workflow| workflow.source.as_str()) == Some("shipped") {
        return Err(format!(
            "workflow '{}' is app-shipped; clone it into a new slug instead of editing it in place",
            normalized_slug
        ));
    }

    let mut normalized_input = input.clone();
    normalized_input.slug = normalized_slug.clone();
    if let Some(existing_workflow) = existing.as_ref() {
        if normalized_input.version <= existing_workflow.version {
            normalized_input.version = existing_workflow.version + 1;
        }
    } else if normalized_input.version < 1 {
        normalized_input.version = 1;
    }

    let yaml = render_workflow_yaml(&normalized_input)?;
    let parsed = parse_workflow_yaml_for_detached(&yaml)?;
    let known_pods = load_library_pods(connection)?
        .into_iter()
        .map(|pod| pod.slug)
        .collect::<BTreeSet<_>>();
    for pod_ref in &parsed.pod_refs {
        if !known_pods.contains(pod_ref) {
            return Err(format!(
                "workflow '{}' references unknown pod '{}'",
                normalized_input.slug, pod_ref
            ));
        }
    }

    let target_path = workflow_dir.join(format!("{}.yaml", normalized_input.slug));
    fs::write(&target_path, yaml.as_bytes()).map_err(|error| {
        format!(
            "failed to write workflow library file {}: {error}",
            target_path.display()
        )
    })?;

    sync_library_catalog(connection, app_data_dir)?;
    load_library_snapshot(connection, app_data_dir)
}

pub fn delete_library_workflow(
    connection: &Connection,
    app_data_dir: &Path,
    input: &DeleteLibraryWorkflowInput,
) -> Result<WorkflowLibrarySnapshot, String> {
    sync_library_catalog(connection, app_data_dir)?;
    let slug = normalize_slug(&input.slug, "workflow slug")?;
    let workflow = load_library_workflow_by_slug(connection, &slug)?;
    if workflow.source == "shipped" {
        return Err(format!(
            "workflow '{}' is app-shipped and cannot be deleted",
            workflow.slug
        ));
    }

    let adoption_count = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM project_catalog_adoptions
            WHERE entity_type = 'workflow' AND entity_slug = ?1
            ",
            params![workflow.slug],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            format!(
                "failed to inspect workflow adoptions for '{}': {error}",
                workflow.slug
            )
        })?;
    if adoption_count > 0 {
        return Err(format!(
            "workflow '{}' is still adopted by {adoption_count} project(s); detach or switch those projects before deleting it",
            workflow.slug
        ));
    }

    let assigned_count = connection
        .query_row(
            "
            SELECT COUNT(*)
            FROM projects
            WHERE default_workflow_slug = ?1
            ",
            params![workflow.slug],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            format!(
                "failed to inspect default workflow assignments for '{}': {error}",
                workflow.slug
            )
        })?;
    if assigned_count > 0 {
        return Err(format!(
            "workflow '{}' is still configured as the default workflow for {assigned_count} project(s)",
            workflow.slug
        ));
    }

    let file_path = PathBuf::from(&workflow.file_path);
    if file_path.is_file() {
        fs::remove_file(&file_path).map_err(|error| {
            format!(
                "failed to delete workflow library file {}: {error}",
                file_path.display()
            )
        })?;
    }

    sync_library_catalog(connection, app_data_dir)?;
    load_library_snapshot(connection, app_data_dir)
}

pub fn ensure_project_workflow_available(
    connection: &Connection,
    project_id: i64,
    workflow_slug: &str,
) -> Result<WorkflowRecord, String> {
    let adoption = load_project_adoption(connection, project_id, "workflow", workflow_slug)?;
    resolve_adopted_workflow_for_editor(connection, &adoption)
}

pub fn start_workflow_run(
    connection: &Connection,
    input: &StartWorkflowRunInput,
) -> Result<WorkflowRunRecord, String> {
    start_workflow_run_with_project_root(connection, input, None)
}

pub fn start_workflow_run_with_project_root(
    connection: &Connection,
    input: &StartWorkflowRunInput,
    project_root: Option<&Path>,
) -> Result<WorkflowRunRecord, String> {
    ensure_project_exists(connection, input.project_id)?;
    let (work_item_id, work_item_call_sign) =
        load_work_item_for_run(connection, input.project_id, input.root_work_item_id)?;
    let resolved_workflow = resolve_effective_workflow_with_project_root(
        connection,
        input.project_id,
        &input.workflow_slug,
        project_root,
    )?;

    if load_active_workflow_run_for_work_item(
        connection,
        input.project_id,
        input.root_work_item_id,
    )?
    .is_some()
    {
        return Err(format!(
            "work item {} already has an active workflow run; finish or fail that run before starting another",
            work_item_call_sign
        ));
    }

    let resolved_workflow_json =
        serde_json::to_string_pretty(&resolved_workflow).map_err(|error| {
            format!(
                "failed to encode resolved workflow '{}' for run start: {error}",
                resolved_workflow.slug
            )
        })?;

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin workflow run transaction: {error}"))?;

    let run_result = (|| {
        connection
            .execute(
                "
                INSERT INTO workflow_runs (
                  project_id,
                  workflow_slug,
                  workflow_name,
                  workflow_kind,
                  workflow_version,
                  root_work_item_id,
                  root_work_item_call_sign,
                  root_worktree_id,
                  source_adoption_mode,
                  status,
                  has_overrides,
                  resolved_workflow_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'queued', ?10, ?11)
                ",
                params![
                    input.project_id,
                    resolved_workflow.slug,
                    resolved_workflow.name,
                    resolved_workflow.kind,
                    resolved_workflow.version,
                    work_item_id,
                    work_item_call_sign,
                    input.root_worktree_id,
                    resolved_workflow.adoption_mode,
                    resolved_workflow.has_overrides as i64,
                    resolved_workflow_json,
                ],
            )
            .map_err(|error| format!("failed to create workflow run: {error}"))?;
        let run_id = connection.last_insert_rowid();

        for stage in &resolved_workflow.stages {
            let resolved_stage_json = serde_json::to_string(stage).map_err(|error| {
                format!(
                    "failed to encode resolved workflow stage '{}': {error}",
                    stage.name
                )
            })?;
            connection
                .execute(
                    "
                    INSERT INTO workflow_run_stages (
                      run_id,
                      stage_ordinal,
                      stage_name,
                      stage_role,
                      pod_slug,
                      pod_version,
                      provider,
                      model,
                      worktree_id,
                      status,
                      attempt,
                      completion_context_json,
                      resolved_stage_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', 1, '{}', ?10)
                    ",
                    params![
                        run_id,
                        stage.ordinal,
                        stage.name,
                        stage.role,
                        stage.pod_slug,
                        stage.pod_version,
                        stage.provider,
                        stage.model,
                        input.root_worktree_id,
                        resolved_stage_json,
                    ],
                )
                .map_err(|error| {
                    format!(
                        "failed to create workflow run stage '{}' for run #{run_id}: {error}",
                        stage.name
                    )
                })?;
        }

        load_workflow_run_by_id(connection, run_id)
    })();

    match run_result {
        Ok(run) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit workflow run start: {error}"))?;
            Ok(run)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn mark_workflow_stage_dispatched(
    connection: &Connection,
    input: &MarkWorkflowStageDispatchedInput,
) -> Result<WorkflowRunRecord, String> {
    let run = load_workflow_run_for_project(connection, input.project_id, input.run_id)?;
    ensure_stage_exists(&run, &input.stage_name)?;

    connection
        .execute(
            "
            UPDATE workflow_run_stages
            SET worktree_id = COALESCE(?1, worktree_id),
                session_id = ?2,
                agent_name = ?3,
                thread_id = ?4,
                directive_message_id = ?5,
                response_message_id = NULL,
                status = 'running',
                completion_message_type = NULL,
                completion_summary = NULL,
                completion_context_json = '{}',
                artifact_validation_status = NULL,
                artifact_validation_error = NULL,
                failure_reason = NULL,
                started_at = CURRENT_TIMESTAMP,
                completed_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE run_id = ?6 AND stage_name = ?7
            ",
            params![
                input.worktree_id,
                input.session_id,
                input.agent_name,
                input.thread_id,
                input.directive_message_id,
                input.run_id,
                input.stage_name,
            ],
        )
        .map_err(|error| {
            format!(
                "failed to mark workflow stage '{}' dispatched for run #{}: {error}",
                input.stage_name, input.run_id
            )
        })?;

    connection
        .execute(
            "
            UPDATE workflow_runs
            SET status = 'running',
                root_worktree_id = COALESCE(?1, root_worktree_id),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?2
            ",
            params![input.worktree_id, input.run_id],
        )
        .map_err(|error| {
            format!(
                "failed to update workflow run #{} after stage dispatch: {error}",
                input.run_id
            )
        })?;

    load_workflow_run_by_id(connection, input.run_id)
}

pub fn record_workflow_stage_result(
    connection: &Connection,
    input: &RecordWorkflowStageResultInput,
) -> Result<RecordWorkflowStageResultOutput, String> {
    let run = load_workflow_run_for_project(connection, input.project_id, input.run_id)?;
    let stage = run
        .stages
        .iter()
        .find(|candidate| candidate.stage_name == input.stage_name)
        .cloned()
        .ok_or_else(|| {
            format!(
                "workflow run #{} does not contain stage '{}'",
                input.run_id, input.stage_name
            )
        })?;
    let completion_message_type = normalize_required(
        &input.completion_message_type,
        "workflow stage completion message type",
    )?
    .to_string();
    let completion_context_json = input
        .completion_context_json
        .as_deref()
        .map(normalize_completion_context_json)
        .transpose()?
        .unwrap_or_else(|| "{}".to_string());
    let produced_artifacts = parse_produced_artifacts_from_context(&completion_context_json)?;
    let completion_summary = input
        .completion_summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let (artifact_validation_status, artifact_validation_error) =
        validate_stage_artifact_outputs(&stage.resolved_stage, &produced_artifacts);
    let completion_message_type = if completion_message_type == "complete"
        && matches!(
            artifact_validation_status.as_deref(),
            Some("invalid") | Some("unreported")
        )
    {
        "produced_invalid_artifact".to_string()
    } else {
        completion_message_type
    };
    let (stage_status, run_status, failure_reason) = if completion_message_type == "complete" {
        if stage.stage_ordinal == run.resolved_workflow.stages.len() as i64 {
            ("completed".to_string(), "completed".to_string(), None)
        } else {
            ("completed".to_string(), "running".to_string(), None)
        }
    } else {
        (
            "blocked".to_string(),
            "blocked".to_string(),
            artifact_validation_error
                .clone()
                .or_else(|| completion_summary.clone()),
        )
    };

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin workflow stage result transaction: {error}"))?;

    let stage_result = (|| {
        connection
            .execute(
                "
                UPDATE workflow_run_stages
                SET response_message_id = ?1,
                    status = ?2,
                    completion_message_type = ?3,
                    completion_summary = ?4,
                    completion_context_json = ?5,
                    artifact_validation_status = ?6,
                    artifact_validation_error = ?7,
                    retry_source_stage_name = NULL,
                    retry_feedback_summary = NULL,
                    retry_feedback_context_json = '{}',
                    retry_requested_at = NULL,
                    failure_reason = ?8,
                    completed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE run_id = ?9 AND stage_name = ?10
                ",
                params![
                    input.response_message_id,
                    stage_status,
                    completion_message_type,
                    completion_summary,
                    completion_context_json,
                    artifact_validation_status,
                    artifact_validation_error,
                    failure_reason,
                    input.run_id,
                    input.stage_name,
                ],
            )
            .map_err(|error| {
                format!(
                    "failed to record workflow stage result for '{}' on run #{}: {error}",
                    input.stage_name, input.run_id
                )
            })?;

        let retry = if matches!(
            completion_message_type.as_str(),
            "blocked" | "produced_invalid_artifact"
        ) {
            maybe_prepare_workflow_stage_retry(
                connection,
                &run,
                &stage,
                completion_summary
                    .clone()
                    .or_else(|| failure_reason.clone()),
                &completion_context_json,
            )?
        } else {
            None
        };

        if retry.is_none() {
            let run_completed = run_status == "completed";
            connection
                .execute(
                    "
                    UPDATE workflow_runs
                    SET status = ?1,
                        failure_reason = ?2,
                        completed_at = CASE WHEN ?3 THEN CURRENT_TIMESTAMP ELSE completed_at END,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = ?4
                    ",
                    params![
                        run_status,
                        failure_reason,
                        run_completed as i64,
                        input.run_id
                    ],
                )
                .map_err(|error| {
                    format!(
                        "failed to update workflow run #{} after stage result: {error}",
                        input.run_id
                    )
                })?;
        }

        let updated_run = load_workflow_run_by_id(connection, input.run_id)?;
        Ok(RecordWorkflowStageResultOutput {
            run: updated_run,
            retry,
        })
    })();

    match stage_result {
        Ok(output) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit workflow stage result: {error}"))?;
            Ok(output)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn maybe_prepare_workflow_stage_retry(
    connection: &Connection,
    run: &WorkflowRunRecord,
    stage: &WorkflowRunStageRecord,
    feedback_summary: Option<String>,
    feedback_context_json: &str,
) -> Result<Option<WorkflowStageRetryDispatchRecord>, String> {
    let Some(retry_policy) = stage.resolved_stage.retry_policy.as_ref() else {
        return Ok(None);
    };
    if stage.attempt >= retry_policy.max_attempts {
        return Ok(None);
    }

    let target_stage_name = retry_policy
        .on_fail_feedback_to
        .as_deref()
        .unwrap_or(stage.stage_name.as_str())
        .to_string();
    let Some(target_stage) = run
        .stages
        .iter()
        .find(|candidate| candidate.stage_name == target_stage_name)
    else {
        return Err(format!(
            "workflow run #{} stage '{}' cannot retry because target stage '{}' does not exist",
            run.id, stage.stage_name, target_stage_name
        ));
    };
    if target_stage.stage_ordinal > stage.stage_ordinal {
        return Err(format!(
            "workflow run #{} stage '{}' cannot retry to later stage '{}'",
            run.id, stage.stage_name, target_stage_name
        ));
    }

    connection
        .execute(
            "
            UPDATE workflow_run_stages
            SET status = 'pending',
                session_id = NULL,
                agent_name = NULL,
                thread_id = NULL,
                directive_message_id = NULL,
                response_message_id = NULL,
                completion_message_type = NULL,
                completion_summary = NULL,
                completion_context_json = '{}',
                artifact_validation_status = NULL,
                artifact_validation_error = NULL,
                retry_source_stage_name = CASE
                  WHEN stage_name = ?1 THEN ?2
                  ELSE NULL
                END,
                retry_feedback_summary = CASE
                  WHEN stage_name = ?1 THEN ?3
                  ELSE NULL
                END,
                retry_feedback_context_json = CASE
                  WHEN stage_name = ?1 THEN ?4
                  ELSE '{}'
                END,
                retry_requested_at = CASE
                  WHEN stage_name = ?1 THEN CURRENT_TIMESTAMP
                  ELSE NULL
                END,
                failure_reason = NULL,
                attempt = CASE
                  WHEN stage_ordinal >= ?5 AND stage_ordinal <= ?6 THEN attempt + 1
                  ELSE attempt
                END,
                started_at = NULL,
                completed_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE run_id = ?7 AND stage_ordinal >= ?5
            ",
            params![
                target_stage_name,
                stage.stage_name,
                feedback_summary,
                feedback_context_json,
                target_stage.stage_ordinal,
                stage.stage_ordinal,
                run.id,
            ],
        )
        .map_err(|error| {
            format!(
                "failed to schedule workflow retry from stage '{}' to '{}' on run #{}: {error}",
                stage.stage_name, target_stage_name, run.id
            )
        })?;

    connection
        .execute(
            "
            UPDATE workflow_runs
            SET status = 'running',
                failure_reason = NULL,
                completed_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            ",
            [run.id],
        )
        .map_err(|error| {
            format!(
                "failed to update workflow run #{} after scheduling retry: {error}",
                run.id
            )
        })?;

    Ok(Some(WorkflowStageRetryDispatchRecord {
        source_stage_name: stage.stage_name.clone(),
        target_stage_name,
        next_attempt: target_stage.attempt + 1,
        max_attempts: retry_policy.max_attempts,
        feedback_summary,
    }))
}

pub fn fail_workflow_run(
    connection: &Connection,
    input: &FailWorkflowRunInput,
) -> Result<WorkflowRunRecord, String> {
    load_workflow_run_for_project(connection, input.project_id, input.run_id)?;
    let failure_reason = normalize_required(&input.failure_reason, "workflow failure reason")?;

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin workflow failure transaction: {error}"))?;

    let failure_result = (|| {
        if let Some(stage_name) = input
            .stage_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            connection
                .execute(
                    "
                    UPDATE workflow_run_stages
                    SET status = 'failed',
                        failure_reason = ?1,
                        completed_at = CURRENT_TIMESTAMP,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE run_id = ?2 AND stage_name = ?3
                    ",
                    params![failure_reason, input.run_id, stage_name],
                )
                .map_err(|error| {
                    format!(
                        "failed to mark workflow stage '{}' failed for run #{}: {error}",
                        stage_name, input.run_id
                    )
                })?;
        }

        connection
            .execute(
                "
                UPDATE workflow_runs
                SET status = 'failed',
                    failure_reason = ?1,
                    completed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?2
                ",
                params![failure_reason, input.run_id],
            )
            .map_err(|error| {
                format!(
                    "failed to mark workflow run #{} failed: {error}",
                    input.run_id
                )
            })?;

        load_workflow_run_by_id(connection, input.run_id)
    })();

    match failure_result {
        Ok(run) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit workflow failure: {error}"))?;
            Ok(run)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn adopt_catalog_entry(
    connection: &Connection,
    input: &AdoptCatalogEntryInput,
) -> Result<(), String> {
    let entity_type = normalize_entity_type(&input.entity_type)?;
    let mode = normalize_adoption_mode(input.mode.as_deref().unwrap_or("linked"))?;
    let slug = normalize_required(&input.slug, "catalog slug")?.to_string();
    ensure_project_exists(connection, input.project_id)?;

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin catalog adoption transaction: {error}"))?;

    let adoption_result = (|| {
        match entity_type {
            "workflow" => {
                let workflow = load_library_workflow_by_slug(connection, &slug)?;
                let detached_yaml = (mode == "forked").then(|| workflow.yaml.clone());
                upsert_adoption(
                    connection,
                    input.project_id,
                    "workflow",
                    &workflow.slug,
                    workflow.version,
                    mode,
                    detached_yaml.as_deref(),
                )?;

                for pod_ref in &workflow.pod_refs {
                    let pod = load_library_pod_by_slug(connection, pod_ref)?;
                    let existing_mode =
                        load_adoption_mode(connection, input.project_id, "pod", &pod.slug)?;
                    if existing_mode.is_none() {
                        upsert_adoption(
                            connection,
                            input.project_id,
                            "pod",
                            &pod.slug,
                            pod.version,
                            "linked",
                            None,
                        )?;
                    }
                }
            }
            "pod" => {
                let pod = load_library_pod_by_slug(connection, &slug)?;
                let detached_yaml = (mode == "forked").then(|| pod.yaml.clone());
                upsert_adoption(
                    connection,
                    input.project_id,
                    "pod",
                    &pod.slug,
                    pod.version,
                    mode,
                    detached_yaml.as_deref(),
                )?;
            }
            _ => unreachable!(),
        }
        Ok(())
    })();

    match adoption_result {
        Ok(()) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit catalog adoption: {error}"))?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn upgrade_catalog_adoption(
    connection: &Connection,
    input: &CatalogAdoptionTarget,
) -> Result<(), String> {
    let entity_type = normalize_entity_type(&input.entity_type)?;
    let slug = normalize_required(&input.slug, "catalog slug")?;
    ensure_project_exists(connection, input.project_id)?;
    let current_mode = load_adoption_mode(connection, input.project_id, entity_type, slug)?
        .ok_or_else(|| {
            format!("{entity_type} adoption '{slug}' does not exist for this project")
        })?;

    if current_mode == "forked" {
        return Err(format!(
            "forked {entity_type} adoption '{slug}' cannot be upgraded in place; detach resolution is already local"
        ));
    }

    match entity_type {
        "workflow" => {
            let workflow = load_library_workflow_by_slug(connection, slug)?;
            upsert_adoption(
                connection,
                input.project_id,
                "workflow",
                &workflow.slug,
                workflow.version,
                "linked",
                None,
            )?;
        }
        "pod" => {
            let pod = load_library_pod_by_slug(connection, slug)?;
            upsert_adoption(
                connection,
                input.project_id,
                "pod",
                &pod.slug,
                pod.version,
                "linked",
                None,
            )?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

pub fn detach_catalog_adoption(
    connection: &Connection,
    input: &CatalogAdoptionTarget,
) -> Result<(), String> {
    let entity_type = normalize_entity_type(&input.entity_type)?;
    let slug = normalize_required(&input.slug, "catalog slug")?;
    ensure_project_exists(connection, input.project_id)?;

    match entity_type {
        "workflow" => {
            let workflow = load_library_workflow_by_slug(connection, slug)?;
            upsert_adoption(
                connection,
                input.project_id,
                "workflow",
                &workflow.slug,
                workflow.version,
                "forked",
                Some(&workflow.yaml),
            )?;
        }
        "pod" => {
            let pod = load_library_pod_by_slug(connection, slug)?;
            upsert_adoption(
                connection,
                input.project_id,
                "pod",
                &pod.slug,
                pod.version,
                "forked",
                Some(&pod.yaml),
            )?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn seed_library_file(directory: &Path, file_name: &str, contents: &str) -> Result<(), String> {
    let path = directory.join(file_name);
    if path.exists() {
        return Ok(());
    }

    fs::write(&path, contents).map_err(|error| {
        format!(
            "failed to seed workflow library file {}: {error}",
            path.display()
        )
    })
}

fn shipped_file_name_set<'a>(files: &'a [(&'a str, &'a str)]) -> BTreeSet<&'a str> {
    files.iter().map(|(name, _)| *name).collect()
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
}

fn list_yaml_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|value| matches!(value.to_ascii_lowercase().as_str(), "yaml" | "yml"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn artifact_contract_for(name: &str) -> Option<WorkflowArtifactContractRecord> {
    BUILTIN_ARTIFACT_CONTRACTS
        .iter()
        .find(|(artifact_type, _, _, _, _)| *artifact_type == name)
        .map(
            |(
                artifact_type,
                label,
                description,
                required_frontmatter_fields,
                required_markdown_sections,
            )| WorkflowArtifactContractRecord {
                artifact_type: (*artifact_type).to_string(),
                label: (*label).to_string(),
                description: (*description).to_string(),
                required_frontmatter_fields: required_frontmatter_fields
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                required_markdown_sections: required_markdown_sections
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
            },
        )
}

fn output_artifact_contracts(
    output_names: &[String],
) -> Result<Vec<WorkflowArtifactContractRecord>, String> {
    output_names
        .iter()
        .map(|name| {
            artifact_contract_for(name)
                .ok_or_else(|| format!("workflow artifact contract '{name}' is not registered"))
        })
        .collect()
}

fn input_artifact_contracts(
    input_names: &[String],
) -> Result<Vec<WorkflowArtifactContractRecord>, String> {
    input_names
        .iter()
        .filter(|name| !WORKFLOW_RUNTIME_INPUTS.contains(&name.as_str()))
        .map(|name| {
            artifact_contract_for(name)
                .ok_or_else(|| format!("workflow artifact contract '{name}' is not registered"))
        })
        .collect()
}

fn validate_stage_artifact_contracts(
    workflow_slug: &str,
    workflow_source: &str,
    stages: &[WorkflowStageRecord],
) -> Result<(), String> {
    let stage_indexes = stages
        .iter()
        .enumerate()
        .map(|(index, stage)| (stage.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut available_outputs = BTreeSet::new();

    for (stage_index, stage) in stages.iter().enumerate() {
        for input in &stage.inputs {
            if WORKFLOW_RUNTIME_INPUTS.contains(&input.as_str()) {
                continue;
            }

            if artifact_contract_for(input).is_none() {
                return Err(format!(
                    "workflow '{workflow_slug}' in {workflow_source} uses unknown input artifact '{input}' on stage '{}'",
                    stage.name
                ));
            }

            if !available_outputs.contains(input) {
                return Err(format!(
                    "workflow '{workflow_slug}' in {workflow_source} expects input artifact '{input}' on stage '{}' but no earlier stage produces it",
                    stage.name
                ));
            }
        }

        for output in &stage.outputs {
            if artifact_contract_for(output).is_none() {
                return Err(format!(
                    "workflow '{workflow_slug}' in {workflow_source} uses unknown output artifact '{output}' on stage '{}'",
                    stage.name
                ));
            }
            available_outputs.insert(output.clone());
        }

        if let Some(retry_policy) = stage.retry_policy.as_ref() {
            if let Some(target_stage_name) = retry_policy.on_fail_feedback_to.as_deref() {
                let Some(target_index) = stage_indexes.get(target_stage_name).copied() else {
                    return Err(format!(
                        "workflow '{workflow_slug}' in {workflow_source} sends retry feedback from stage '{}' to unknown stage '{}'",
                        stage.name, target_stage_name
                    ));
                };
                if target_index > stage_index {
                    return Err(format!(
                        "workflow '{workflow_slug}' in {workflow_source} sends retry feedback from stage '{}' to later stage '{}'; retry targets must be the current or an earlier stage",
                        stage.name, target_stage_name
                    ));
                }
            }
        }
    }

    Ok(())
}

fn parse_workflow_definition(
    path: &Path,
    is_shipped: bool,
    known_pods: &BTreeSet<String>,
) -> Result<ParsedWorkflowDefinition, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read workflow definition {}: {error}",
            path.display()
        )
    })?;
    let parsed = serde_yaml::from_str::<WorkflowDefinitionYaml>(&raw).map_err(|error| {
        format!(
            "failed to parse workflow definition {}: {error}",
            path.display()
        )
    })?;

    let display_name = normalize_required(&parsed.name, "workflow name")?.to_string();
    let slug = match parsed.slug.as_deref() {
        Some(value) => normalize_slug(value, "workflow slug")?,
        None => normalize_slug(&display_name, "workflow name")?,
    };
    let kind = normalize_required(&parsed.kind, "workflow kind")?.to_string();

    if parsed.version < 1 {
        return Err(format!(
            "workflow '{slug}' in {} must have version >= 1",
            path.display()
        ));
    }
    if parsed.stages.is_empty() {
        return Err(format!(
            "workflow '{slug}' in {} must define at least one stage",
            path.display()
        ));
    }

    let categories = normalize_category_list(&parsed.categories)?;
    let tags = normalize_string_list(&parsed.tags, "workflow tags")?;
    let mut stage_names = BTreeSet::new();
    let mut pod_refs = BTreeSet::new();
    let mut stages = Vec::with_capacity(parsed.stages.len());

    for stage in parsed.stages {
        let stage_name = normalize_required(&stage.name, "workflow stage name")?.to_string();
        if !stage_names.insert(stage_name.clone()) {
            return Err(format!(
                "workflow '{slug}' in {} has duplicate stage name '{stage_name}'",
                path.display()
            ));
        }

        let role = normalize_required(&stage.role, "workflow stage role")?.to_string();
        let pod_ref = stage
            .pod_ref
            .as_deref()
            .map(|value| normalize_required(value, "workflow pod_ref"))
            .transpose()?
            .map(str::to_string);

        if let Some(ref pod_ref) = pod_ref {
            if !known_pods.contains(pod_ref) {
                return Err(format!(
                    "workflow '{slug}' in {} references unknown pod '{pod_ref}'",
                    path.display()
                ));
            }
            pod_refs.insert(pod_ref.clone());
        }

        let retry_policy = match stage.retry_policy.as_ref() {
            Some(policy) => Some(WorkflowStageRetryPolicyRecord {
                max_attempts: policy.max_attempts.unwrap_or(1).max(1),
                on_fail_feedback_to: policy
                    .on_fail_feedback_to
                    .as_deref()
                    .map(|value| normalize_required(value, "retry feedback target"))
                    .transpose()?
                    .map(str::to_string),
            }),
            None => None,
        };

        let retry_summary = retry_policy.as_ref().and_then(|policy| {
            let max_attempts = Some(format!("max {} attempts", policy.max_attempts));
            let feedback = policy
                .on_fail_feedback_to
                .as_deref()
                .map(|value| format!("feedback -> {value}"));
            match (max_attempts, feedback) {
                (Some(max_attempts), Some(feedback)) => Some(format!("{max_attempts}, {feedback}")),
                (Some(max_attempts), None) => Some(max_attempts),
                (None, Some(feedback)) => Some(feedback),
                (None, None) => None,
            }
        });

        stages.push(WorkflowStageRecord {
            name: stage_name,
            role,
            pod_ref,
            provider: normalize_optional(&stage.provider),
            model: normalize_optional(&stage.model),
            prompt_template_ref: normalize_optional(&stage.prompt_template_ref),
            inputs: normalize_string_list(&stage.inputs, "workflow inputs")?,
            outputs: normalize_string_list(&stage.outputs, "workflow outputs")?,
            input_contracts: Vec::new(),
            output_contracts: Vec::new(),
            needs_secrets: normalize_string_list(&stage.needs_secrets, "workflow secrets")?,
            vault_env_bindings: normalize_vault_binding_requests(
                &stage.vault_env_bindings,
                "workflow stage vault binding",
            )?,
            retry_policy,
            retry_summary,
        });
    }

    validate_stage_artifact_contracts(&slug, &path.display().to_string(), &stages)?;
    for stage in &mut stages {
        stage.input_contracts = input_artifact_contracts(&stage.inputs)?;
        stage.output_contracts = output_artifact_contracts(&stage.outputs)?;
    }

    Ok(ParsedWorkflowDefinition {
        slug: slug.clone(),
        name: display_name,
        kind,
        version: parsed.version,
        description: parsed.description.trim().to_string(),
        template: parsed.template,
        categories,
        tags,
        stages,
        pod_refs: pod_refs.into_iter().collect(),
        yaml: raw,
        source: if is_shipped {
            "shipped".to_string()
        } else {
            "user".to_string()
        },
        file_path: path.display().to_string(),
    })
}

fn parse_pod_definition(path: &Path, is_shipped: bool) -> Result<ParsedPodDefinition, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read pod definition {}: {error}", path.display()))?;
    let parsed = serde_yaml::from_str::<PodDefinitionYaml>(&raw)
        .map_err(|error| format!("failed to parse pod definition {}: {error}", path.display()))?;

    let slug = normalize_required(&parsed.name, "pod name")?.to_string();
    let role = normalize_required(&parsed.role, "pod role")?.to_string();
    let provider = normalize_required(&parsed.provider, "pod provider")?.to_string();
    let categories = normalize_category_list(&parsed.categories)?;
    if categories.is_empty() {
        return Err(format!(
            "pod '{slug}' in {} must declare at least one category",
            path.display()
        ));
    }
    if parsed.version < 1 {
        return Err(format!(
            "pod '{slug}' in {} must have version >= 1",
            path.display()
        ));
    }

    let default_policy_json =
        serde_json::to_string_pretty(&serde_json::to_value(&parsed.default_policy).map_err(
            |error| format!("failed to encode default policy for pod '{slug}': {error}"),
        )?)
        .map_err(|error| format!("failed to encode default policy for pod '{slug}': {error}"))?;

    Ok(ParsedPodDefinition {
        slug: slug.clone(),
        name: slug,
        role,
        version: parsed.version,
        description: parsed.description.trim().to_string(),
        provider,
        model: normalize_optional(&parsed.model),
        prompt_template_ref: normalize_optional(&parsed.prompt_template_ref),
        categories,
        tags: normalize_string_list(&parsed.tags, "pod tags")?,
        tool_allowlist: normalize_string_list(&parsed.tool_allowlist, "pod tool allowlist")?,
        secret_scopes: normalize_string_list(&parsed.secret_scopes, "pod secret scopes")?,
        default_policy_json,
        yaml: raw,
        source: if is_shipped {
            "shipped".to_string()
        } else {
            "user".to_string()
        },
        file_path: path.display().to_string(),
    })
}

fn normalize_entity_type(value: &str) -> Result<&str, String> {
    match value.trim() {
        "workflow" => Ok("workflow"),
        "pod" => Ok("pod"),
        _ => Err("entity type must be workflow or pod".to_string()),
    }
}

fn normalize_adoption_mode(value: &str) -> Result<&str, String> {
    match value.trim() {
        "linked" => Ok("linked"),
        "forked" => Ok("forked"),
        _ => Err("catalog adoption mode must be linked or forked".to_string()),
    }
}

fn normalize_required<'a>(value: &'a str, label: &str) -> Result<&'a str, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }
    Ok(trimmed)
}

fn normalize_slug(value: &str, label: &str) -> Result<String, String> {
    let trimmed = normalize_required(value, label)?;
    let mut normalized = String::with_capacity(trimmed.len());
    let mut pending_hyphen = false;

    for character in trimmed.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_hyphen && !normalized.is_empty() && !normalized.ends_with('-') {
                normalized.push('-');
            }
            normalized.push(character.to_ascii_lowercase());
            pending_hyphen = false;
            continue;
        }

        match character {
            '-' | '.' => {
                if !normalized.is_empty()
                    && !normalized.ends_with('-')
                    && !normalized.ends_with('.')
                {
                    normalized.push(character);
                }
                pending_hyphen = false;
            }
            ' ' | '_' => {
                pending_hyphen = true;
            }
            _ => {
                return Err(format!(
                    "{label} contains unsupported character '{character}'; use letters, numbers, spaces, '-', '_' or '.'"
                ));
            }
        }
    }

    let normalized = normalized
        .trim_matches(|character| character == '-' || character == '.')
        .to_string();

    if normalized.is_empty() {
        return Err(format!("{label} must contain at least one letter or number"));
    }
    if normalized.contains("..") || normalized.contains('/') || normalized.contains('\\') {
        return Err(format!("{label} contains an unsafe path segment"));
    }

    Ok(normalized)
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_string_list(values: &[String], label: &str) -> Result<Vec<String>, String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();

    for value in values {
        let trimmed = normalize_required(value, label)?.to_string();
        if seen.insert(trimmed.clone()) {
            output.push(trimmed);
        }
    }

    Ok(output)
}

fn normalize_vault_binding_requests(
    values: &[VaultAccessBindingRequest],
    label: &str,
) -> Result<Vec<VaultAccessBindingRequest>, String> {
    let mut seen_env_vars = BTreeSet::new();
    let mut output = Vec::new();

    for binding in values {
        let env_label = format!("{label} env var");
        let entry_label = format!("{label} entry name");
        let scope_label = format!("{label} scope tag");
        let env_var = normalize_required(&binding.env_var, &env_label)?.to_string();
        let entry_name = normalize_required(&binding.entry_name, &entry_label)?.to_string();
        let env_key = env_var.to_ascii_uppercase();

        if !seen_env_vars.insert(env_key) {
            return Err(format!(
                "{label} contains duplicate env var binding '{env_var}'"
            ));
        }

        output.push(VaultAccessBindingRequest {
            env_var,
            entry_name,
            required_scope_tags: normalize_string_list(&binding.required_scope_tags, &scope_label)?,
            delivery: binding.delivery.clone(),
        });
    }

    Ok(output)
}

fn validate_resolved_stage_vault_bindings(
    workflow_slug: &str,
    stage_name: &str,
    needs_secrets: &[String],
    resolved_pod: Option<&PodRecord>,
    bindings: &[VaultAccessBindingRequest],
) -> Result<(), String> {
    if bindings.is_empty() {
        return Ok(());
    }

    let declared_secret_tags = needs_secrets.iter().cloned().collect::<BTreeSet<_>>();
    let pod_secret_scopes =
        resolved_pod.map(|pod| pod.secret_scopes.iter().cloned().collect::<BTreeSet<_>>());

    for binding in bindings {
        if binding.required_scope_tags.is_empty() {
            return Err(format!(
                "workflow '{}' stage '{}' vault env binding '{}' must declare at least one scope tag",
                workflow_slug, stage_name, binding.env_var
            ));
        }

        let undeclared_scope_tags = binding
            .required_scope_tags
            .iter()
            .filter(|tag| !declared_secret_tags.contains(*tag))
            .cloned()
            .collect::<Vec<_>>();
        if !undeclared_scope_tags.is_empty() {
            return Err(format!(
                "workflow '{}' stage '{}' vault env binding '{}' requests scope tags not declared in needs_secrets: {}",
                workflow_slug,
                stage_name,
                binding.env_var,
                undeclared_scope_tags.join(", ")
            ));
        }

        if let (Some(pod), Some(allowed_scopes)) = (resolved_pod, pod_secret_scopes.as_ref()) {
            let disallowed_scope_tags = binding
                .required_scope_tags
                .iter()
                .filter(|tag| !allowed_scopes.contains(*tag))
                .cloned()
                .collect::<Vec<_>>();
            if !disallowed_scope_tags.is_empty() {
                return Err(format!(
                    "workflow '{}' stage '{}' vault env binding '{}' requests scope tags not granted by pod '{}' secret_scopes: {}",
                    workflow_slug,
                    stage_name,
                    binding.env_var,
                    pod.slug,
                    disallowed_scope_tags.join(", ")
                ));
            }
        }
    }

    Ok(())
}

fn normalize_category_list(values: &[String]) -> Result<Vec<String>, String> {
    let categories = normalize_string_list(values, "category")?;
    categories
        .iter()
        .try_for_each(|value| {
            if value.chars().all(|character| {
                character.is_ascii_uppercase()
                    || character == '_'
                    || character.is_ascii_digit()
            }) {
                Ok(())
            } else {
                Err(format!(
                    "category '{value}' must use uppercase registry-style names such as CODING or DOCUMENTATION"
                ))
            }
        })
        .map(|_| categories)
}

fn ensure_seed_categories(connection: &Connection) -> Result<(), String> {
    for (name, description) in SHIPPED_CATEGORIES {
        connection
            .execute(
                "
                INSERT INTO workflow_categories (name, description, is_shipped)
                VALUES (?1, ?2, 1)
                ON CONFLICT(name) DO UPDATE SET
                  description = excluded.description,
                  is_shipped = 1
                ",
                params![name, description],
            )
            .map_err(|error| format!("failed to seed workflow category {name}: {error}"))?;
    }
    Ok(())
}

fn ensure_category_id(connection: &Connection, category: &str) -> Result<i64, String> {
    let existing = connection
        .query_row(
            "SELECT id FROM workflow_categories WHERE name = ?1",
            [category],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to inspect workflow category {category}: {error}"))?;

    if let Some(id) = existing {
        return Ok(id);
    }

    connection
        .execute(
            "INSERT INTO workflow_categories (name, description, is_shipped) VALUES (?1, '', 0)",
            [category],
        )
        .map_err(|error| format!("failed to create workflow category {category}: {error}"))?;

    Ok(connection.last_insert_rowid())
}

fn insert_workflow_definition(
    connection: &Connection,
    workflow: &ParsedWorkflowDefinition,
) -> Result<i64, String> {
    let stages_json = serde_json::to_string(&workflow.stages).map_err(|error| {
        format!(
            "failed to encode workflow stages for {}: {error}",
            workflow.slug
        )
    })?;
    let pod_refs_json = serde_json::to_string(&workflow.pod_refs).map_err(|error| {
        format!(
            "failed to encode workflow pod refs for {}: {error}",
            workflow.slug
        )
    })?;
    let tags_json = serde_json::to_string(&workflow.tags).map_err(|error| {
        format!(
            "failed to encode workflow tags for {}: {error}",
            workflow.slug
        )
    })?;

    connection
        .execute(
            "
            INSERT INTO library_workflows (
              slug, name, kind, version, description, source, template,
              tags_json, stages_json, pod_refs_json, yaml, file_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                workflow.slug,
                workflow.name,
                workflow.kind,
                workflow.version,
                workflow.description,
                workflow.source,
                workflow.template as i32,
                tags_json,
                stages_json,
                pod_refs_json,
                workflow.yaml,
                workflow.file_path,
            ],
        )
        .map_err(|error| format!("failed to store workflow {}: {error}", workflow.slug))?;

    Ok(connection.last_insert_rowid())
}

fn insert_pod_definition(
    connection: &Connection,
    pod: &ParsedPodDefinition,
) -> Result<i64, String> {
    let tags_json = serde_json::to_string(&pod.tags)
        .map_err(|error| format!("failed to encode pod tags for {}: {error}", pod.slug))?;
    let tool_allowlist_json = serde_json::to_string(&pod.tool_allowlist)
        .map_err(|error| format!("failed to encode pod allowlist for {}: {error}", pod.slug))?;
    let secret_scopes_json = serde_json::to_string(&pod.secret_scopes).map_err(|error| {
        format!(
            "failed to encode pod secret scopes for {}: {error}",
            pod.slug
        )
    })?;

    connection
        .execute(
            "
            INSERT INTO library_pods (
              slug, name, role, version, description, provider, model,
              prompt_template_ref, tags_json, tool_allowlist_json,
              secret_scopes_json, default_policy_json, yaml, source, file_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ",
            params![
                pod.slug,
                pod.name,
                pod.role,
                pod.version,
                pod.description,
                pod.provider,
                pod.model,
                pod.prompt_template_ref,
                tags_json,
                tool_allowlist_json,
                secret_scopes_json,
                pod.default_policy_json,
                pod.yaml,
                pod.source,
                pod.file_path,
            ],
        )
        .map_err(|error| format!("failed to store pod {}: {error}", pod.slug))?;

    Ok(connection.last_insert_rowid())
}

fn assign_categories(
    connection: &Connection,
    table_name: &str,
    id_column: &str,
    record_id: i64,
    categories: &[String],
) -> Result<(), String> {
    for category in categories {
        let category_id = ensure_category_id(connection, category)?;
        connection
            .execute(
                &format!("INSERT INTO {table_name} ({id_column}, category_id) VALUES (?1, ?2)"),
                params![record_id, category_id],
            )
            .map_err(|error| format!("failed to assign category {category}: {error}"))?;
    }
    Ok(())
}

fn load_categories(connection: &Connection) -> Result<Vec<WorkflowCategoryRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, description, is_shipped, created_at
            FROM workflow_categories
            ORDER BY name ASC
            ",
        )
        .map_err(|error| format!("failed to prepare workflow category query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(WorkflowCategoryRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_shipped: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("failed to query workflow categories: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect workflow categories: {error}"))
}

fn load_library_workflows(connection: &Connection) -> Result<Vec<WorkflowRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, slug, name, kind, version, description, source, template,
                   tags_json, stages_json, pod_refs_json, yaml, file_path, updated_at
            FROM library_workflows
            ORDER BY kind ASC, name ASC
            ",
        )
        .map_err(|error| format!("failed to prepare workflow query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(WorkflowRecord {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                version: row.get(4)?,
                description: row.get(5)?,
                source: row.get(6)?,
                template: row.get::<_, i64>(7)? != 0,
                categories: Vec::new(),
                tags: parse_json_list(row.get::<_, String>(8)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                stages: parse_stage_list(row.get::<_, String>(9)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                pod_refs: parse_json_list(row.get::<_, String>(10)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                yaml: row.get(11)?,
                file_path: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .map_err(|error| format!("failed to query workflows: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect workflows: {error}"))?;

    rows.into_iter()
        .map(|workflow| {
            let categories = load_assignment_categories(
                connection,
                "library_workflow_category_assignments",
                "workflow_id",
                workflow.id,
            )?;
            Ok(WorkflowRecord {
                categories,
                ..workflow
            })
        })
        .collect()
}

fn load_library_pods(connection: &Connection) -> Result<Vec<PodRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, slug, name, role, version, description, provider, model,
                   prompt_template_ref, tags_json, tool_allowlist_json,
                   secret_scopes_json, default_policy_json, yaml, source, file_path, updated_at
            FROM library_pods
            ORDER BY role ASC, name ASC
            ",
        )
        .map_err(|error| format!("failed to prepare pod query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(PodRecord {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                role: row.get(3)?,
                version: row.get(4)?,
                description: row.get(5)?,
                provider: row.get(6)?,
                model: row.get(7)?,
                prompt_template_ref: row.get(8)?,
                categories: Vec::new(),
                tags: parse_json_list(row.get::<_, String>(9)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                tool_allowlist: parse_json_list(row.get::<_, String>(10)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                secret_scopes: parse_json_list(row.get::<_, String>(11)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                default_policy_json: row.get(12)?,
                yaml: row.get(13)?,
                source: row.get(14)?,
                file_path: row.get(15)?,
                updated_at: row.get(16)?,
            })
        })
        .map_err(|error| format!("failed to query pods: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect pods: {error}"))?;

    rows.into_iter()
        .map(|pod| {
            let categories = load_assignment_categories(
                connection,
                "library_pod_category_assignments",
                "pod_id",
                pod.id,
            )?;
            Ok(PodRecord { categories, ..pod })
        })
        .collect()
}

fn load_project_adoption_rows(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<CatalogAdoptionRow>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT project_id, entity_type, entity_slug, pinned_version, mode, detached_yaml, updated_at
            FROM project_catalog_adoptions
            WHERE project_id = ?1
            ORDER BY entity_type ASC, entity_slug ASC
            ",
        )
        .map_err(|error| format!("failed to prepare catalog adoption query: {error}"))?;

    let rows = statement
        .query_map([project_id], |row| {
            Ok(CatalogAdoptionRow {
                entity_type: row.get(1)?,
                entity_slug: row.get(2)?,
                pinned_version: row.get(3)?,
                mode: row.get(4)?,
                detached_yaml: row.get(5)?,
            })
        })
        .map_err(|error| format!("failed to query catalog adoptions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect catalog adoptions: {error}"))
}

fn load_project_adoption(
    connection: &Connection,
    project_id: i64,
    entity_type: &str,
    slug: &str,
) -> Result<CatalogAdoptionRow, String> {
    load_project_adoption_rows(connection, project_id)?
        .into_iter()
        .find(|row| row.entity_type == entity_type && row.entity_slug == slug)
        .ok_or_else(|| format!("project #{project_id} has not adopted {entity_type} '{slug}'"))
}

fn load_workflow_overrides(
    connection: &Connection,
    project_id: i64,
    workflow_slug: &str,
) -> Result<Option<WorkflowOverrideSetRecord>, String> {
    let raw = connection
        .query_row(
            "
            SELECT overrides_json
            FROM project_workflow_overrides
            WHERE project_id = ?1 AND workflow_slug = ?2
            ",
            params![project_id, workflow_slug],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| {
            format!(
                "failed to inspect workflow overrides for project #{project_id} workflow '{workflow_slug}': {error}"
            )
        })?;

    raw.map(|json| {
        serde_json::from_str::<WorkflowOverrideSetRecord>(&json).map_err(|error| {
            format!(
                "failed to parse workflow overrides for project #{project_id} workflow '{workflow_slug}': {error}"
            )
        })
    })
    .transpose()
}

fn persist_workflow_overrides(
    connection: &Connection,
    project_id: i64,
    workflow_slug: &str,
    overrides: &WorkflowOverrideSetRecord,
) -> Result<(), String> {
    if overrides.stage_overrides.is_empty() {
        connection
            .execute(
                "
                DELETE FROM project_workflow_overrides
                WHERE project_id = ?1 AND workflow_slug = ?2
                ",
                params![project_id, workflow_slug],
            )
            .map_err(|error| {
                format!(
                    "failed to clear workflow overrides for project #{project_id} workflow '{workflow_slug}': {error}"
                )
            })?;
        return Ok(());
    }

    let overrides_json = serde_json::to_string(overrides).map_err(|error| {
        format!(
            "failed to encode workflow overrides for project #{project_id} workflow '{workflow_slug}': {error}"
        )
    })?;

    connection
        .execute(
            "
            INSERT INTO project_workflow_overrides (project_id, workflow_slug, overrides_json)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(project_id, workflow_slug) DO UPDATE SET
              overrides_json = excluded.overrides_json,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![project_id, workflow_slug, overrides_json],
        )
        .map_err(|error| {
            format!(
                "failed to persist workflow overrides for project #{project_id} workflow '{workflow_slug}': {error}"
            )
        })?;

    Ok(())
}

fn preferred_workflow_overrides(
    connection: &Connection,
    project_id: i64,
    workflow: &WorkflowRecord,
    project_root: Option<&Path>,
) -> Result<Option<WorkflowOverrideSetRecord>, String> {
    if let Some(project_root) = project_root {
        let override_path = project_workflow_override_path(project_root, &workflow.slug)?;
        if override_path.is_file() {
            let yaml = fs::read_to_string(&override_path).map_err(|error| {
                format!(
                    "failed to read workflow override file {}: {error}",
                    override_path.display()
                )
            })?;
            let overrides = parse_override_yaml_for_workflow(workflow, &yaml)?;
            persist_workflow_overrides(connection, project_id, &workflow.slug, &overrides)?;
            return Ok(Some(overrides));
        }
    }

    load_workflow_overrides(connection, project_id, &workflow.slug)?
        .map(|overrides| canonicalize_override_set(workflow, overrides))
        .transpose()
}

pub fn load_project_workflow_override_document(
    connection: &Connection,
    project_id: i64,
    project_root: &Path,
    workflow_slug: &str,
) -> Result<ProjectWorkflowOverrideDocument, String> {
    let workflow_adoption =
        load_project_adoption(connection, project_id, "workflow", workflow_slug)?;
    let workflow = resolve_adopted_workflow_for_editor(connection, &workflow_adoption)?;
    let override_path = project_workflow_override_path(project_root, workflow_slug)?;

    if override_path.is_file() {
        let yaml = fs::read_to_string(&override_path).map_err(|error| {
            format!(
                "failed to read workflow override file {}: {error}",
                override_path.display()
            )
        })?;

        match parse_override_yaml_for_workflow(&workflow, &yaml) {
            Ok(overrides) => {
                persist_workflow_overrides(connection, project_id, workflow_slug, &overrides)?;
                Ok(ProjectWorkflowOverrideDocument {
                    project_id,
                    workflow_slug: workflow.slug,
                    file_path: override_path.display().to_string(),
                    exists: true,
                    source: "repo".to_string(),
                    yaml,
                    has_overrides: !overrides.stage_overrides.is_empty(),
                    stage_override_count: overrides.stage_overrides.len() as i64,
                    validation_error: None,
                })
            }
            Err(error) => {
                let has_overrides = !yaml.trim().is_empty();
                Ok(ProjectWorkflowOverrideDocument {
                    project_id,
                    workflow_slug: workflow.slug,
                    file_path: override_path.display().to_string(),
                    exists: true,
                    source: "repo".to_string(),
                    yaml,
                    has_overrides,
                    stage_override_count: 0,
                    validation_error: Some(error),
                })
            }
        }
    } else if let Some(overrides) = load_workflow_overrides(connection, project_id, workflow_slug)?
    {
        let overrides = canonicalize_override_set(&workflow, overrides)?;
        Ok(ProjectWorkflowOverrideDocument {
            project_id,
            workflow_slug: workflow.slug,
            file_path: override_path.display().to_string(),
            exists: false,
            source: "database".to_string(),
            yaml: render_override_yaml(&overrides)?,
            has_overrides: !overrides.stage_overrides.is_empty(),
            stage_override_count: overrides.stage_overrides.len() as i64,
            validation_error: None,
        })
    } else {
        let empty = WorkflowOverrideSetRecord::default();
        Ok(ProjectWorkflowOverrideDocument {
            project_id,
            workflow_slug: workflow.slug,
            file_path: override_path.display().to_string(),
            exists: false,
            source: "empty".to_string(),
            yaml: render_override_yaml(&empty)?,
            has_overrides: false,
            stage_override_count: 0,
            validation_error: None,
        })
    }
}

pub fn save_project_workflow_override_document(
    connection: &Connection,
    project_root: &Path,
    input: &SaveProjectWorkflowOverrideInput,
) -> Result<ProjectWorkflowOverrideDocument, String> {
    let workflow_adoption =
        load_project_adoption(connection, input.project_id, "workflow", &input.workflow_slug)?;
    let workflow = resolve_adopted_workflow_for_editor(connection, &workflow_adoption)?;
    let override_path = project_workflow_override_path(project_root, &input.workflow_slug)?;

    if !project_root.is_dir() {
        return Err(format!(
            "project root {} is not available for workflow override editing",
            project_root.display()
        ));
    }

    let overrides = parse_override_yaml_for_workflow(&workflow, &input.yaml)?;
    let canonical_yaml = render_override_yaml(&overrides)?;

    fs::create_dir_all(project_workflow_override_dir(project_root)).map_err(|error| {
        format!(
            "failed to create workflow override directory {}: {error}",
            project_workflow_override_dir(project_root).display()
        )
    })?;
    fs::write(&override_path, canonical_yaml.as_bytes()).map_err(|error| {
        format!(
            "failed to write workflow override file {}: {error}",
            override_path.display()
        )
    })?;

    persist_workflow_overrides(connection, input.project_id, &workflow.slug, &overrides)?;
    load_project_workflow_override_document(
        connection,
        input.project_id,
        project_root,
        &workflow.slug,
    )
}

pub fn clear_project_workflow_override_document(
    connection: &Connection,
    project_root: &Path,
    input: &ProjectWorkflowOverrideTarget,
) -> Result<ProjectWorkflowOverrideDocument, String> {
    let override_path = project_workflow_override_path(project_root, &input.workflow_slug)?;
    if override_path.exists() {
        fs::remove_file(&override_path).map_err(|error| {
            format!(
                "failed to remove workflow override file {}: {error}",
                override_path.display()
            )
        })?;
    }

    connection
        .execute(
            "
            DELETE FROM project_workflow_overrides
            WHERE project_id = ?1 AND workflow_slug = ?2
            ",
            params![input.project_id, input.workflow_slug],
        )
        .map_err(|error| {
            format!(
                "failed to clear workflow overrides for project #{} workflow '{}': {error}",
                input.project_id, input.workflow_slug
            )
        })?;

    load_project_workflow_override_document(
        connection,
        input.project_id,
        project_root,
        &input.workflow_slug,
    )
}

fn normalize_override_lookup(
    workflow: &WorkflowRecord,
    overrides: Option<WorkflowOverrideSetRecord>,
) -> Result<HashMap<String, WorkflowStageOverrideRecord>, String> {
    let Some(overrides) = overrides else {
        return Ok(HashMap::new());
    };
    let workflow_stage_names = workflow
        .stages
        .iter()
        .map(|stage| stage.name.clone())
        .collect::<BTreeSet<_>>();
    let mut lookup = HashMap::new();

    for override_record in overrides.stage_overrides {
        let stage_name =
            normalize_required(&override_record.stage_name, "workflow override stage name")?
                .to_string();
        if !workflow_stage_names.contains(&stage_name) {
            return Err(format!(
                "workflow override references unknown stage '{stage_name}' in workflow '{}'",
                workflow.slug
            ));
        }
        if lookup.contains_key(&stage_name) {
            return Err(format!(
                "workflow override includes duplicate stage entry for '{stage_name}'"
            ));
        }
        lookup.insert(
            stage_name,
            WorkflowStageOverrideRecord {
                stage_name: override_record.stage_name,
                pod_ref: override_record
                    .pod_ref
                    .as_deref()
                    .map(|value| normalize_required(value, "workflow override pod_ref"))
                    .transpose()?
                    .map(str::to_string),
                provider: normalize_optional(&override_record.provider),
                model: normalize_optional(&override_record.model),
                prompt_template_ref: normalize_optional(&override_record.prompt_template_ref),
                needs_secrets: override_record
                    .needs_secrets
                    .map(|values| normalize_string_list(&values, "workflow override secrets"))
                    .transpose()?,
                vault_env_bindings: override_record
                    .vault_env_bindings
                    .map(|values| {
                        normalize_vault_binding_requests(&values, "workflow override vault binding")
                    })
                    .transpose()?,
                retry_policy: override_record.retry_policy.map(|policy| {
                    WorkflowStageRetryPolicyRecord {
                        max_attempts: policy.max_attempts.max(1),
                        on_fail_feedback_to: policy.on_fail_feedback_to,
                    }
                }),
            },
        );
    }

    Ok(lookup)
}

fn resolve_adopted_workflow_for_run(
    connection: &Connection,
    adoption: &CatalogAdoptionRow,
) -> Result<WorkflowRecord, String> {
    match adoption.mode.as_str() {
        "linked" => {
            let workflow = load_library_workflow_by_slug(connection, &adoption.entity_slug)?;
            if workflow.version != adoption.pinned_version {
                return Err(format!(
                    "workflow '{}' is pinned to v{} but the library now exposes v{}; upgrade or detach the project adoption before running it",
                    adoption.entity_slug, adoption.pinned_version, workflow.version
                ));
            }
            Ok(workflow)
        }
        "forked" => {
            let detached_yaml = adoption.detached_yaml.as_deref().ok_or_else(|| {
                format!(
                    "forked workflow '{}' is missing its detached YAML snapshot",
                    adoption.entity_slug
                )
            })?;
            parse_detached_workflow(&adoption.entity_slug, detached_yaml)
        }
        other => Err(format!(
            "workflow adoption '{}' uses unsupported mode '{other}'",
            adoption.entity_slug
        )),
    }
}

fn resolve_adopted_pod_for_run(
    connection: &Connection,
    adoption: &CatalogAdoptionRow,
) -> Result<PodRecord, String> {
    match adoption.mode.as_str() {
        "linked" => {
            let pod = load_library_pod_by_slug(connection, &adoption.entity_slug)?;
            if pod.version != adoption.pinned_version {
                return Err(format!(
                    "pod '{}' is pinned to v{} but the library now exposes v{}; upgrade or detach the project adoption before running it",
                    adoption.entity_slug, adoption.pinned_version, pod.version
                ));
            }
            Ok(pod)
        }
        "forked" => {
            let detached_yaml = adoption.detached_yaml.as_deref().ok_or_else(|| {
                format!(
                    "forked pod '{}' is missing its detached YAML snapshot",
                    adoption.entity_slug
                )
            })?;
            parse_detached_pod(&adoption.entity_slug, detached_yaml)
        }
        other => Err(format!(
            "pod adoption '{}' uses unsupported mode '{other}'",
            adoption.entity_slug
        )),
    }
}

fn resolve_effective_workflow_with_project_root(
    connection: &Connection,
    project_id: i64,
    workflow_slug: &str,
    project_root: Option<&Path>,
) -> Result<ResolvedWorkflowRecord, String> {
    let workflow_adoption =
        load_project_adoption(connection, project_id, "workflow", workflow_slug)?;
    let workflow = resolve_adopted_workflow_for_run(connection, &workflow_adoption)?;
    let stage_overrides = normalize_override_lookup(
        &workflow,
        preferred_workflow_overrides(connection, project_id, &workflow, project_root)?,
    )?;
    let pod_adoptions = load_project_adoption_rows(connection, project_id)?
        .into_iter()
        .filter(|row| row.entity_type == "pod")
        .map(|row| (row.entity_slug.clone(), row))
        .collect::<HashMap<_, _>>();

    let mut stages = Vec::with_capacity(workflow.stages.len());
    let mut last_generator_provider: Option<String> = None;

    for (index, stage) in workflow.stages.iter().enumerate() {
        let override_record = stage_overrides.get(&stage.name);
        let pod_slug = override_record
            .and_then(|record| record.pod_ref.clone())
            .or_else(|| stage.pod_ref.clone());
        let resolved_pod = if let Some(ref pod_slug) = pod_slug {
            let adoption = pod_adoptions.get(pod_slug).ok_or_else(|| {
                format!(
                    "workflow '{}' references pod '{}' but the project has not adopted it",
                    workflow.slug, pod_slug
                )
            })?;
            let pod = resolve_adopted_pod_for_run(connection, adoption)?;
            if pod.role != stage.role {
                return Err(format!(
                    "workflow stage '{}' expects role '{}' but pod '{}' resolves to role '{}'",
                    stage.name, stage.role, pod.slug, pod.role
                ));
            }
            Some(pod)
        } else {
            None
        };

        let provider = override_record
            .and_then(|record| record.provider.clone())
            .or_else(|| stage.provider.clone())
            .or_else(|| resolved_pod.as_ref().map(|pod| pod.provider.clone()))
            .ok_or_else(|| {
                format!(
                    "workflow stage '{}' does not resolve to a provider",
                    stage.name
                )
            })?;
        let model = override_record
            .and_then(|record| record.model.clone())
            .or_else(|| stage.model.clone())
            .or_else(|| resolved_pod.as_ref().and_then(|pod| pod.model.clone()));
        let prompt_template_ref = override_record
            .and_then(|record| record.prompt_template_ref.clone())
            .or_else(|| stage.prompt_template_ref.clone())
            .or_else(|| {
                resolved_pod
                    .as_ref()
                    .and_then(|pod| pod.prompt_template_ref.clone())
            });
        let needs_secrets = override_record
            .and_then(|record| record.needs_secrets.clone())
            .unwrap_or_else(|| stage.needs_secrets.clone());
        let vault_env_bindings = override_record
            .and_then(|record| record.vault_env_bindings.clone())
            .unwrap_or_else(|| stage.vault_env_bindings.clone());
        let retry_policy = override_record
            .and_then(|record| record.retry_policy.clone())
            .or_else(|| stage.retry_policy.clone());

        validate_resolved_stage_vault_bindings(
            &workflow.slug,
            &stage.name,
            &needs_secrets,
            resolved_pod.as_ref(),
            &vault_env_bindings,
        )?;

        if stage.role == "generator" {
            last_generator_provider = Some(provider.clone());
        } else if stage.role == "evaluator" {
            if let Some(generator_provider) = last_generator_provider.as_deref() {
                if generator_provider == provider {
                    return Err(format!(
                        "workflow '{}' resolves evaluator stage '{}' to provider '{}' which matches the most recent generator provider; evaluator independence requires a different provider",
                        workflow.slug, stage.name, provider
                    ));
                }
            }
        }

        stages.push(ResolvedWorkflowStageRecord {
            ordinal: index as i64 + 1,
            name: stage.name.clone(),
            role: stage.role.clone(),
            pod_slug: resolved_pod.as_ref().map(|pod| pod.slug.clone()),
            pod_version: resolved_pod.as_ref().map(|pod| pod.version),
            provider,
            model,
            prompt_template_ref,
            tool_allowlist: resolved_pod
                .as_ref()
                .map(|pod| pod.tool_allowlist.clone())
                .unwrap_or_default(),
            secret_scopes: resolved_pod
                .as_ref()
                .map(|pod| pod.secret_scopes.clone())
                .unwrap_or_default(),
            default_policy_json: resolved_pod
                .as_ref()
                .map(|pod| pod.default_policy_json.clone())
                .unwrap_or_else(|| "{}".to_string()),
            inputs: stage.inputs.clone(),
            outputs: stage.outputs.clone(),
            input_contracts: stage.input_contracts.clone(),
            output_contracts: stage.output_contracts.clone(),
            needs_secrets,
            vault_env_bindings,
            retry_policy,
        });
    }

    Ok(ResolvedWorkflowRecord {
        slug: workflow.slug,
        name: workflow.name,
        kind: workflow.kind,
        version: workflow.version,
        description: workflow.description,
        source: workflow.source,
        template: workflow.template,
        categories: workflow.categories,
        tags: workflow.tags,
        adoption_mode: workflow_adoption.mode,
        has_overrides: !stage_overrides.is_empty(),
        stages,
    })
}

fn load_workflow_runs(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<WorkflowRunRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, project_id, workflow_slug, workflow_name, workflow_kind,
                   workflow_version, root_work_item_id, root_work_item_call_sign,
                   root_worktree_id, source_adoption_mode, status, has_overrides,
                   failure_reason, created_at, started_at, completed_at, updated_at,
                   resolved_workflow_json
            FROM workflow_runs
            WHERE project_id = ?1
            ORDER BY started_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare workflow run query: {error}"))?;

    let runs = statement
        .query_map([project_id], |row| {
            Ok(WorkflowRunRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                workflow_slug: row.get(2)?,
                workflow_name: row.get(3)?,
                workflow_kind: row.get(4)?,
                workflow_version: row.get(5)?,
                root_work_item_id: row.get(6)?,
                root_work_item_call_sign: row.get(7)?,
                root_worktree_id: row.get(8)?,
                source_adoption_mode: row.get(9)?,
                status: row.get(10)?,
                has_overrides: row.get::<_, i64>(11)? != 0,
                failure_reason: row.get(12)?,
                created_at: row.get(13)?,
                started_at: row.get(14)?,
                completed_at: row.get(15)?,
                updated_at: row.get(16)?,
                resolved_workflow: parse_resolved_workflow(row.get::<_, String>(17)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                stages: Vec::new(),
            })
        })
        .map_err(|error| format!("failed to query workflow runs: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect workflow runs: {error}"))?;

    runs.into_iter()
        .map(|run| {
            let stages = load_workflow_run_stages(connection, run.id)?;
            Ok(WorkflowRunRecord { stages, ..run })
        })
        .collect()
}

fn load_workflow_run_by_id(
    connection: &Connection,
    run_id: i64,
) -> Result<WorkflowRunRecord, String> {
    let mut runs = load_workflow_runs_for_ids(connection, &[run_id])?;
    runs.pop()
        .ok_or_else(|| format!("workflow run #{run_id} does not exist"))
}

fn load_workflow_run_for_project(
    connection: &Connection,
    project_id: i64,
    run_id: i64,
) -> Result<WorkflowRunRecord, String> {
    let run = load_workflow_run_by_id(connection, run_id)?;
    if run.project_id != project_id {
        return Err(format!(
            "workflow run #{run_id} does not belong to project #{project_id}"
        ));
    }
    Ok(run)
}

fn load_workflow_runs_for_ids(
    connection: &Connection,
    run_ids: &[i64],
) -> Result<Vec<WorkflowRunRecord>, String> {
    if run_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = run_ids
        .iter()
        .enumerate()
        .map(|(index, _)| format!("?{}", index + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        SELECT id, project_id, workflow_slug, workflow_name, workflow_kind,
               workflow_version, root_work_item_id, root_work_item_call_sign,
               root_worktree_id, source_adoption_mode, status, has_overrides,
               failure_reason, created_at, started_at, completed_at, updated_at,
               resolved_workflow_json
        FROM workflow_runs
        WHERE id IN ({placeholders})
        "
    );
    let params = run_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare workflow run id query: {error}"))?;
    let runs = statement
        .query_map(params.as_slice(), |row| {
            Ok(WorkflowRunRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                workflow_slug: row.get(2)?,
                workflow_name: row.get(3)?,
                workflow_kind: row.get(4)?,
                workflow_version: row.get(5)?,
                root_work_item_id: row.get(6)?,
                root_work_item_call_sign: row.get(7)?,
                root_worktree_id: row.get(8)?,
                source_adoption_mode: row.get(9)?,
                status: row.get(10)?,
                has_overrides: row.get::<_, i64>(11)? != 0,
                failure_reason: row.get(12)?,
                created_at: row.get(13)?,
                started_at: row.get(14)?,
                completed_at: row.get(15)?,
                updated_at: row.get(16)?,
                resolved_workflow: parse_resolved_workflow(row.get::<_, String>(17)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
                stages: Vec::new(),
            })
        })
        .map_err(|error| format!("failed to query workflow runs by id: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect workflow runs by id: {error}"))?;

    runs.into_iter()
        .map(|run| {
            let stages = load_workflow_run_stages(connection, run.id)?;
            Ok(WorkflowRunRecord { stages, ..run })
        })
        .collect()
}

fn load_workflow_run_stages(
    connection: &Connection,
    run_id: i64,
) -> Result<Vec<WorkflowRunStageRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, run_id, stage_ordinal, stage_name, stage_role,
                   pod_slug, pod_version, provider, model, worktree_id, session_id,
                   agent_name, thread_id, directive_message_id, response_message_id,
                   status, attempt, completion_message_type, completion_summary,
                   completion_context_json, artifact_validation_status,
                   artifact_validation_error, retry_source_stage_name,
                   retry_feedback_summary, retry_feedback_context_json,
                   retry_requested_at, failure_reason, created_at, started_at,
                   completed_at, updated_at, resolved_stage_json
            FROM workflow_run_stages
            WHERE run_id = ?1
            ORDER BY stage_ordinal ASC, attempt ASC, id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare workflow run stage query: {error}"))?;

    let rows = statement
        .query_map([run_id], |row| {
            Ok(WorkflowRunStageRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                stage_ordinal: row.get(2)?,
                stage_name: row.get(3)?,
                stage_role: row.get(4)?,
                pod_slug: row.get(5)?,
                pod_version: row.get(6)?,
                provider: row.get(7)?,
                model: row.get(8)?,
                worktree_id: row.get(9)?,
                session_id: row.get(10)?,
                agent_name: row.get(11)?,
                thread_id: row.get(12)?,
                directive_message_id: row.get(13)?,
                response_message_id: row.get(14)?,
                status: row.get(15)?,
                attempt: row.get(16)?,
                completion_message_type: row.get(17)?,
                completion_summary: row.get(18)?,
                completion_context_json: row.get(19)?,
                produced_artifacts: parse_produced_artifacts_from_context(
                    &row.get::<_, String>(19)?,
                )
                .map_err(|error| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        error,
                    )))
                })?,
                artifact_validation_status: row.get(20)?,
                artifact_validation_error: row.get(21)?,
                retry_source_stage_name: row.get(22)?,
                retry_feedback_summary: row.get(23)?,
                retry_feedback_context_json: row.get(24)?,
                retry_requested_at: row.get(25)?,
                failure_reason: row.get(26)?,
                created_at: row.get(27)?,
                started_at: row.get(28)?,
                completed_at: row.get(29)?,
                updated_at: row.get(30)?,
                resolved_stage: parse_resolved_stage(row.get::<_, String>(31)?)
                    .map_err(rusqlite::Error::ToSqlConversionFailure)?,
            })
        })
        .map_err(|error| format!("failed to query workflow run stages: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect workflow run stages: {error}"))
}

fn load_active_workflow_run_for_work_item(
    connection: &Connection,
    project_id: i64,
    root_work_item_id: i64,
) -> Result<Option<i64>, String> {
    connection
        .query_row(
            "
            SELECT id
            FROM workflow_runs
            WHERE project_id = ?1
              AND root_work_item_id = ?2
              AND status IN ('queued', 'running', 'blocked')
            ORDER BY started_at DESC, id DESC
            LIMIT 1
            ",
            params![project_id, root_work_item_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| {
            format!(
                "failed to inspect active workflow runs for work item #{root_work_item_id}: {error}"
            )
        })
}

fn load_work_item_for_run(
    connection: &Connection,
    project_id: i64,
    work_item_id: i64,
) -> Result<(i64, String), String> {
    connection
        .query_row(
            "
            SELECT id, call_sign
            FROM work_items
            WHERE id = ?1 AND project_id = ?2
            ",
            params![work_item_id, project_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("failed to load workflow run work item #{work_item_id}: {error}"))?
        .ok_or_else(|| {
            format!("work item #{work_item_id} does not belong to project #{project_id}")
        })
}

fn ensure_stage_exists(run: &WorkflowRunRecord, stage_name: &str) -> Result<(), String> {
    if run
        .stages
        .iter()
        .any(|stage| stage.stage_name == stage_name)
    {
        return Ok(());
    }

    Err(format!(
        "workflow run #{} does not contain stage '{}'",
        run.id, stage_name
    ))
}

fn load_assignment_categories(
    connection: &Connection,
    table_name: &str,
    id_column: &str,
    record_id: i64,
) -> Result<Vec<String>, String> {
    let sql = format!(
        "
        SELECT c.name
        FROM {table_name} a
        JOIN workflow_categories c ON c.id = a.category_id
        WHERE a.{id_column} = ?1
        ORDER BY c.name ASC
        "
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare category assignment query: {error}"))?;
    let rows = statement
        .query_map([record_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("failed to query category assignments: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect category assignments: {error}"))
}

fn parse_json_list(raw: String) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::from_str::<Vec<String>>(&raw)?)
}

fn parse_resolved_workflow(
    raw: String,
) -> Result<ResolvedWorkflowRecord, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::from_str::<ResolvedWorkflowRecord>(&raw)?)
}

fn parse_resolved_stage(
    raw: String,
) -> Result<ResolvedWorkflowStageRecord, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::from_str::<ResolvedWorkflowStageRecord>(&raw)?)
}

fn parse_stage_list(
    raw: String,
) -> Result<Vec<WorkflowStageRecord>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(serde_json::from_str::<Vec<WorkflowStageRecord>>(&raw)?)
}

fn normalize_completion_context_json(raw: &str) -> Result<String, String> {
    let value = serde_json::from_str::<serde_json::Value>(raw).map_err(|error| {
        format!("workflow stage completion context must be valid JSON: {error}")
    })?;
    serde_json::to_string_pretty(&value).map_err(|error| {
        format!("failed to encode workflow stage completion context JSON: {error}")
    })
}

fn parse_produced_artifacts_from_context(
    raw: &str,
) -> Result<Vec<WorkflowProducedArtifactRecord>, String> {
    let value = serde_json::from_str::<serde_json::Value>(raw).map_err(|error| {
        format!("workflow stage completion context must be valid JSON: {error}")
    })?;
    let Some(produced_artifacts) = value.as_object().and_then(|object| {
        object
            .get("producedArtifacts")
            .or_else(|| object.get("produced_artifacts"))
    }) else {
        return Ok(Vec::new());
    };

    serde_json::from_value::<Vec<WorkflowProducedArtifactRecord>>(produced_artifacts.clone())
        .map_err(|error| format!("workflow stage producedArtifacts must be a valid array: {error}"))
}

fn validate_stage_artifact_outputs(
    stage: &ResolvedWorkflowStageRecord,
    produced_artifacts: &[WorkflowProducedArtifactRecord],
) -> (Option<String>, Option<String>) {
    if stage.outputs.is_empty() {
        return (Some("not_required".to_string()), None);
    }

    if produced_artifacts.is_empty() {
        return (
            Some("unreported".to_string()),
            Some(format!(
                "stage '{}' declared output artifacts ({}) but reported none in producedArtifacts",
                stage.name,
                stage.outputs.join(", ")
            )),
        );
    }

    let declared_outputs = stage.outputs.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen_types = BTreeSet::new();

    for artifact in produced_artifacts {
        let artifact_type = artifact.artifact_type.trim();
        if artifact_type.is_empty() {
            return (
                Some("invalid".to_string()),
                Some(format!(
                    "stage '{}' reported an artifact with an empty type",
                    stage.name
                )),
            );
        }

        if !declared_outputs.contains(artifact_type) {
            return (
                Some("invalid".to_string()),
                Some(format!(
                    "stage '{}' reported undeclared artifact '{}'; declared outputs are {}",
                    stage.name,
                    artifact_type,
                    stage.outputs.join(", ")
                )),
            );
        }

        let Some(contract) = artifact_contract_for(artifact_type) else {
            return (
                Some("invalid".to_string()),
                Some(format!(
                    "stage '{}' reported artifact '{}' but no contract is registered for it",
                    stage.name, artifact_type
                )),
            );
        };

        for required_field in &contract.required_frontmatter_fields {
            let is_present = artifact
                .frontmatter
                .get(required_field)
                .map(|value| match value {
                    serde_json::Value::Null => false,
                    serde_json::Value::String(text) => !text.trim().is_empty(),
                    serde_json::Value::Array(values) => !values.is_empty(),
                    serde_json::Value::Object(values) => !values.is_empty(),
                    _ => true,
                })
                .unwrap_or(false);
            if !is_present {
                return (
                    Some("invalid".to_string()),
                    Some(format!(
                        "artifact '{}' from stage '{}' is missing required frontmatter field '{}'",
                        artifact_type, stage.name, required_field
                    )),
                );
            }
        }

        let body_markdown = artifact.body_markdown.as_deref().unwrap_or_default();
        for required_section in &contract.required_markdown_sections {
            if !body_markdown.contains(required_section) {
                return (
                    Some("invalid".to_string()),
                    Some(format!(
                        "artifact '{}' from stage '{}' is missing required markdown section '{}'",
                        artifact_type, stage.name, required_section
                    )),
                );
            }
        }

        seen_types.insert(artifact_type.to_string());
    }

    let missing_outputs = declared_outputs
        .difference(&seen_types)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_outputs.is_empty() {
        return (
            Some("invalid".to_string()),
            Some(format!(
                "stage '{}' did not report declared output artifacts: {}",
                stage.name,
                missing_outputs.join(", ")
            )),
        );
    }

    (Some("valid".to_string()), None)
}

fn resolve_project_workflow(
    slug: &str,
    detached_yaml: Option<&str>,
    workflow_lookup: &HashMap<String, WorkflowRecord>,
) -> Result<(WorkflowRecord, Option<i64>), String> {
    if let Some(workflow) = workflow_lookup.get(slug) {
        return Ok((workflow.clone(), Some(workflow.version)));
    }

    let detached_yaml = detached_yaml.ok_or_else(|| {
        format!("workflow adoption '{slug}' references a missing library entry and has no detached snapshot")
    })?;
    let detached = parse_detached_workflow(slug, detached_yaml)?;
    Ok((detached, None))
}

fn resolve_project_pod(
    slug: &str,
    detached_yaml: Option<&str>,
    pod_lookup: &HashMap<String, PodRecord>,
) -> Result<(PodRecord, Option<i64>), String> {
    if let Some(pod) = pod_lookup.get(slug) {
        return Ok((pod.clone(), Some(pod.version)));
    }

    let detached_yaml = detached_yaml.ok_or_else(|| {
        format!(
            "pod adoption '{slug}' references a missing library entry and has no detached snapshot"
        )
    })?;
    let detached = parse_detached_pod(slug, detached_yaml)?;
    Ok((detached, None))
}

fn parse_detached_workflow(slug: &str, yaml: &str) -> Result<WorkflowRecord, String> {
    let parsed = parse_workflow_yaml_for_detached(yaml)?;
    Ok(WorkflowRecord {
        id: 0,
        slug: slug.to_string(),
        name: parsed.slug,
        kind: parsed.kind,
        version: parsed.version,
        description: parsed.description,
        source: "detached".to_string(),
        template: parsed.template,
        categories: parsed.categories,
        tags: parsed.tags,
        stages: parsed.stages,
        pod_refs: parsed.pod_refs,
        yaml: parsed.yaml,
        file_path: "(detached snapshot)".to_string(),
        updated_at: String::new(),
    })
}

fn parse_detached_pod(slug: &str, yaml: &str) -> Result<PodRecord, String> {
    let parsed = parse_pod_yaml_for_detached(yaml)?;
    Ok(PodRecord {
        id: 0,
        slug: slug.to_string(),
        name: parsed.slug,
        role: parsed.role,
        version: parsed.version,
        description: parsed.description,
        provider: parsed.provider,
        model: parsed.model,
        prompt_template_ref: parsed.prompt_template_ref,
        categories: parsed.categories,
        tags: parsed.tags,
        tool_allowlist: parsed.tool_allowlist,
        secret_scopes: parsed.secret_scopes,
        default_policy_json: parsed.default_policy_json,
        yaml: parsed.yaml,
        source: "detached".to_string(),
        file_path: "(detached snapshot)".to_string(),
        updated_at: String::new(),
    })
}

fn parse_workflow_yaml_for_detached(yaml: &str) -> Result<ParsedWorkflowDefinition, String> {
    let parsed = serde_yaml::from_str::<WorkflowDefinitionYaml>(yaml)
        .map_err(|error| format!("failed to parse detached workflow snapshot: {error}"))?;
    let display_name = normalize_required(&parsed.name, "workflow name")?.to_string();
    let slug = match parsed.slug.as_deref() {
        Some(value) => normalize_slug(value, "workflow slug")?,
        None => normalize_slug(&display_name, "workflow name")?,
    };
    let kind = normalize_required(&parsed.kind, "workflow kind")?.to_string();
    let categories = normalize_category_list(&parsed.categories)?;
    let tags = normalize_string_list(&parsed.tags, "workflow tags")?;
    let mut stages = parsed
        .stages
        .into_iter()
        .map(|stage| {
            let retry_policy = match stage.retry_policy.as_ref() {
                Some(policy) => Some(WorkflowStageRetryPolicyRecord {
                    max_attempts: policy.max_attempts.unwrap_or(1).max(1),
                    on_fail_feedback_to: policy
                        .on_fail_feedback_to
                        .as_deref()
                        .map(|value| normalize_required(value, "retry feedback target"))
                        .transpose()?
                        .map(str::to_string),
                }),
                None => None,
            };
            Ok(WorkflowStageRecord {
                name: normalize_required(&stage.name, "workflow stage name")?.to_string(),
                role: normalize_required(&stage.role, "workflow stage role")?.to_string(),
                pod_ref: stage
                    .pod_ref
                    .as_deref()
                    .map(|value| normalize_required(value, "workflow pod_ref"))
                    .transpose()?
                    .map(str::to_string),
                provider: normalize_optional(&stage.provider),
                model: normalize_optional(&stage.model),
                prompt_template_ref: normalize_optional(&stage.prompt_template_ref),
                inputs: normalize_string_list(&stage.inputs, "workflow inputs")?,
                outputs: normalize_string_list(&stage.outputs, "workflow outputs")?,
                input_contracts: Vec::new(),
                output_contracts: Vec::new(),
                needs_secrets: normalize_string_list(&stage.needs_secrets, "workflow secrets")?,
                vault_env_bindings: normalize_vault_binding_requests(
                    &stage.vault_env_bindings,
                    "workflow stage vault binding",
                )?,
                retry_policy: retry_policy.clone(),
                retry_summary: retry_policy.as_ref().map(|policy| {
                    match policy.on_fail_feedback_to.as_deref() {
                        Some(target) => {
                            format!("max {} attempts, feedback -> {target}", policy.max_attempts)
                        }
                        None => format!("max {} attempts", policy.max_attempts),
                    }
                }),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    validate_stage_artifact_contracts(&slug, "detached workflow snapshot", &stages)?;
    for stage in &mut stages {
        stage.input_contracts = input_artifact_contracts(&stage.inputs)?;
        stage.output_contracts = output_artifact_contracts(&stage.outputs)?;
    }
    let pod_refs = stages
        .iter()
        .filter_map(|stage| stage.pod_ref.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Ok(ParsedWorkflowDefinition {
        slug: slug.clone(),
        name: display_name,
        kind,
        version: parsed.version,
        description: parsed.description.trim().to_string(),
        template: parsed.template,
        categories,
        tags,
        stages,
        pod_refs,
        yaml: yaml.to_string(),
        source: "detached".to_string(),
        file_path: "(detached snapshot)".to_string(),
    })
}

fn parse_pod_yaml_for_detached(yaml: &str) -> Result<ParsedPodDefinition, String> {
    let parsed = serde_yaml::from_str::<PodDefinitionYaml>(yaml)
        .map_err(|error| format!("failed to parse detached pod snapshot: {error}"))?;
    let slug = normalize_required(&parsed.name, "pod name")?.to_string();
    Ok(ParsedPodDefinition {
        slug: slug.clone(),
        name: slug,
        role: normalize_required(&parsed.role, "pod role")?.to_string(),
        version: parsed.version,
        description: parsed.description.trim().to_string(),
        provider: normalize_required(&parsed.provider, "pod provider")?.to_string(),
        model: normalize_optional(&parsed.model),
        prompt_template_ref: normalize_optional(&parsed.prompt_template_ref),
        categories: normalize_category_list(&parsed.categories)?,
        tags: normalize_string_list(&parsed.tags, "pod tags")?,
        tool_allowlist: normalize_string_list(&parsed.tool_allowlist, "pod tool allowlist")?,
        secret_scopes: normalize_string_list(&parsed.secret_scopes, "pod secret scopes")?,
        default_policy_json: serde_json::to_string_pretty(
            &serde_json::to_value(&parsed.default_policy)
                .map_err(|error| format!("failed to encode detached pod policy: {error}"))?,
        )
        .map_err(|error| format!("failed to encode detached pod policy: {error}"))?,
        yaml: yaml.to_string(),
        source: "detached".to_string(),
        file_path: "(detached snapshot)".to_string(),
    })
}

fn load_library_workflow_by_slug(
    connection: &Connection,
    slug: &str,
) -> Result<WorkflowRecord, String> {
    load_library_workflows(connection)?
        .into_iter()
        .find(|workflow| workflow.slug == slug)
        .ok_or_else(|| format!("workflow '{slug}' does not exist in the library"))
}

fn load_library_pod_by_slug(connection: &Connection, slug: &str) -> Result<PodRecord, String> {
    load_library_pods(connection)?
        .into_iter()
        .find(|pod| pod.slug == slug)
        .ok_or_else(|| format!("pod '{slug}' does not exist in the library"))
}

fn upsert_adoption(
    connection: &Connection,
    project_id: i64,
    entity_type: &str,
    slug: &str,
    pinned_version: i64,
    mode: &str,
    detached_yaml: Option<&str>,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO project_catalog_adoptions (
              project_id, entity_type, entity_slug, pinned_version, mode, detached_yaml
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(project_id, entity_type, entity_slug) DO UPDATE SET
              pinned_version = excluded.pinned_version,
              mode = excluded.mode,
              detached_yaml = excluded.detached_yaml,
              updated_at = CURRENT_TIMESTAMP
            ",
            params![
                project_id,
                entity_type,
                slug,
                pinned_version,
                mode,
                detached_yaml
            ],
        )
        .map_err(|error| format!("failed to upsert {entity_type} adoption '{slug}': {error}"))?;
    Ok(())
}

fn load_adoption_mode(
    connection: &Connection,
    project_id: i64,
    entity_type: &str,
    slug: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "
            SELECT mode
            FROM project_catalog_adoptions
            WHERE project_id = ?1 AND entity_type = ?2 AND entity_slug = ?3
            ",
            params![project_id, entity_type, slug],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to inspect {entity_type} adoption '{slug}': {error}"))
}

fn ensure_project_exists(connection: &Connection, project_id: i64) -> Result<(), String> {
    let existing = connection
        .query_row(
            "SELECT id FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to inspect project #{project_id}: {error}"))?;

    if existing.is_none() {
        return Err(format!("project #{project_id} does not exist"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("project-commander-workflow-{name}-{nanos}"))
    }

    fn create_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory sqlite should open");
        connection
            .execute_batch(
                "
                CREATE TABLE projects (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  name TEXT NOT NULL,
                  root_path TEXT NOT NULL UNIQUE,
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO projects (name, root_path) VALUES ('Alpha', 'C:\\Alpha');

                CREATE TABLE work_items (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  project_id INTEGER NOT NULL,
                  call_sign TEXT NOT NULL,
                  title TEXT NOT NULL DEFAULT '',
                  status TEXT NOT NULL DEFAULT 'backlog'
                );
                INSERT INTO work_items (project_id, call_sign, title, status)
                VALUES (1, 'ALPHA-1', 'Ship workflow execution', 'backlog');

                CREATE TABLE worktrees (
                  id INTEGER PRIMARY KEY AUTOINCREMENT
                );

                CREATE TABLE sessions (
                  id INTEGER PRIMARY KEY AUTOINCREMENT
                );

                CREATE TABLE agent_messages (
                  id INTEGER PRIMARY KEY AUTOINCREMENT
                );

                CREATE TABLE workflow_categories (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  name TEXT NOT NULL UNIQUE,
                  description TEXT NOT NULL DEFAULT '',
                  is_shipped INTEGER NOT NULL DEFAULT 0,
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE library_workflows (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  slug TEXT NOT NULL UNIQUE,
                  name TEXT NOT NULL,
                  kind TEXT NOT NULL,
                  version INTEGER NOT NULL,
                  description TEXT NOT NULL DEFAULT '',
                  source TEXT NOT NULL DEFAULT 'user',
                  template INTEGER NOT NULL DEFAULT 0,
                  tags_json TEXT NOT NULL DEFAULT '[]',
                  stages_json TEXT NOT NULL DEFAULT '[]',
                  pod_refs_json TEXT NOT NULL DEFAULT '[]',
                  yaml TEXT NOT NULL,
                  file_path TEXT NOT NULL DEFAULT '',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE library_pods (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  slug TEXT NOT NULL UNIQUE,
                  name TEXT NOT NULL,
                  role TEXT NOT NULL,
                  version INTEGER NOT NULL,
                  description TEXT NOT NULL DEFAULT '',
                  provider TEXT NOT NULL,
                  model TEXT,
                  prompt_template_ref TEXT,
                  tags_json TEXT NOT NULL DEFAULT '[]',
                  tool_allowlist_json TEXT NOT NULL DEFAULT '[]',
                  secret_scopes_json TEXT NOT NULL DEFAULT '[]',
                  default_policy_json TEXT NOT NULL DEFAULT '{}',
                  yaml TEXT NOT NULL,
                  source TEXT NOT NULL DEFAULT 'user',
                  file_path TEXT NOT NULL DEFAULT '',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE library_workflow_category_assignments (
                  workflow_id INTEGER NOT NULL,
                  category_id INTEGER NOT NULL,
                  PRIMARY KEY (workflow_id, category_id)
                );

                CREATE TABLE library_pod_category_assignments (
                  pod_id INTEGER NOT NULL,
                  category_id INTEGER NOT NULL,
                  PRIMARY KEY (pod_id, category_id)
                );

                CREATE TABLE project_catalog_adoptions (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  project_id INTEGER NOT NULL,
                  entity_type TEXT NOT NULL,
                  entity_slug TEXT NOT NULL,
                  pinned_version INTEGER NOT NULL,
                  mode TEXT NOT NULL DEFAULT 'linked',
                  detached_yaml TEXT,
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  UNIQUE(project_id, entity_type, entity_slug)
                );

                CREATE TABLE project_workflow_overrides (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  project_id INTEGER NOT NULL,
                  workflow_slug TEXT NOT NULL,
                  overrides_json TEXT NOT NULL DEFAULT '{}',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  UNIQUE(project_id, workflow_slug)
                );

                CREATE TABLE workflow_runs (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  project_id INTEGER NOT NULL,
                  workflow_slug TEXT NOT NULL,
                  workflow_name TEXT NOT NULL,
                  workflow_kind TEXT NOT NULL,
                  workflow_version INTEGER NOT NULL,
                  root_work_item_id INTEGER NOT NULL,
                  root_work_item_call_sign TEXT NOT NULL,
                  root_worktree_id INTEGER,
                  source_adoption_mode TEXT NOT NULL DEFAULT 'linked',
                  status TEXT NOT NULL DEFAULT 'queued',
                  failure_reason TEXT,
                  has_overrides INTEGER NOT NULL DEFAULT 0,
                  resolved_workflow_json TEXT NOT NULL DEFAULT '{}',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  completed_at TEXT,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE workflow_run_stages (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  run_id INTEGER NOT NULL,
                  stage_ordinal INTEGER NOT NULL,
                  stage_name TEXT NOT NULL,
                  stage_role TEXT NOT NULL,
                  pod_slug TEXT,
                  pod_version INTEGER,
                  provider TEXT NOT NULL,
                  model TEXT,
                  worktree_id INTEGER,
                  session_id INTEGER,
                  agent_name TEXT,
                  thread_id TEXT,
                  directive_message_id INTEGER,
                  response_message_id INTEGER,
                  status TEXT NOT NULL DEFAULT 'pending',
                  attempt INTEGER NOT NULL DEFAULT 1,
                  completion_message_type TEXT,
                  completion_summary TEXT,
                  completion_context_json TEXT NOT NULL DEFAULT '{}',
                  artifact_validation_status TEXT,
                  artifact_validation_error TEXT,
                  retry_source_stage_name TEXT,
                  retry_feedback_summary TEXT,
                  retry_feedback_context_json TEXT NOT NULL DEFAULT '{}',
                  retry_requested_at TEXT,
                  failure_reason TEXT,
                  resolved_stage_json TEXT NOT NULL DEFAULT '{}',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  started_at TEXT,
                  completed_at TEXT,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("test schema should initialize");
        connection
    }

    fn adopt_feature_dev(connection: &Connection) {
        adopt_catalog_entry(
            connection,
            &AdoptCatalogEntryInput {
                project_id: 1,
                entity_type: "workflow".to_string(),
                slug: "feature-dev".to_string(),
                mode: Some("linked".to_string()),
            },
        )
        .expect("feature-dev adoption should succeed");
    }

    fn adopt_pod(connection: &Connection, slug: &str) {
        adopt_catalog_entry(
            connection,
            &AdoptCatalogEntryInput {
                project_id: 1,
                entity_type: "pod".to_string(),
                slug: slug.to_string(),
                mode: Some("linked".to_string()),
            },
        )
        .expect("pod adoption should succeed");
    }

    fn set_project_root(connection: &Connection, root: &Path) {
        connection
            .execute(
                "UPDATE projects SET root_path = ?1 WHERE id = 1",
                [root.display().to_string()],
            )
            .expect("project root should update");
    }

    #[test]
    fn sync_library_catalog_loads_seeded_workflows_and_pods() {
        let root = temp_dir("seeded-catalog");
        let connection = create_connection();

        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        let snapshot = load_library_snapshot(&connection, &root).expect("snapshot should load");

        assert!(snapshot
            .workflows
            .iter()
            .any(|workflow| workflow.slug == "feature-dev"));
        assert!(snapshot
            .pods
            .iter()
            .any(|pod| pod.slug == "evaluator.codex.strict"));
        assert!(snapshot
            .categories
            .iter()
            .any(|category| category.name == "CODING"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workflow_adoption_auto_links_referenced_pods() {
        let root = temp_dir("adoption");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");

        adopt_feature_dev(&connection);

        let catalog = load_project_catalog(&connection, 1).expect("project catalog should load");

        assert_eq!(catalog.workflows.len(), 1);
        assert!(catalog
            .pods
            .iter()
            .any(|pod| pod.pod.slug == "planner.opus.standard"));
        assert!(catalog
            .pods
            .iter()
            .any(|pod| pod.pod.slug == "integrator.sonnet.merge"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn start_workflow_run_persists_resolved_run_and_stages() {
        let root = temp_dir("run-start");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        let run = start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        )
        .expect("workflow run should start");

        assert_eq!(run.status, "queued");
        assert_eq!(run.root_work_item_call_sign, "ALPHA-1");
        assert_eq!(run.stages.len(), 5);
        assert!(run.stages.iter().all(|stage| stage.status == "pending"));
        assert_eq!(run.stages[0].stage_name, "plan");
        assert_eq!(
            run.stages[2].pod_slug.as_deref(),
            Some("generator.sonnet.standard")
        );
        assert_eq!(
            run.stages[3]
                .resolved_stage
                .retry_policy
                .as_ref()
                .map(|policy| policy.max_attempts),
            Some(3)
        );

        let snapshot =
            load_project_run_snapshot(&connection, 1).expect("workflow run snapshot should load");
        assert_eq!(snapshot.runs.len(), 1);
        assert_eq!(snapshot.runs[0].id, run.id);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn start_workflow_run_rejects_outdated_linked_adoption() {
        let root = temp_dir("run-outdated");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);
        connection
            .execute(
                "UPDATE library_workflows SET version = 2 WHERE slug = 'feature-dev'",
                [],
            )
            .expect("workflow version should update");

        let error = match start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        ) {
            Ok(_) => panic!("outdated linked adoption should fail"),
            Err(error) => error,
        };

        assert!(error.contains("pinned to v1"));
        assert!(error.contains("upgrade or detach"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workflow_run_resolution_applies_project_overrides() {
        let root = temp_dir("run-overrides");
        let connection = create_connection();
        set_project_root(&connection, &root);
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);
        adopt_pod(&connection, "generator.opus.crosscutting");
        connection
            .execute(
                "UPDATE library_pods SET secret_scopes_json = ?1 WHERE slug = 'generator.opus.crosscutting'",
                [serde_json::json!(["github:repo"]).to_string()],
            )
            .expect("pod secret scopes should update");

        connection
            .execute(
                "
                INSERT INTO project_workflow_overrides (project_id, workflow_slug, overrides_json)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    1,
                    "feature-dev",
                    serde_json::json!({
                        "stageOverrides": [
                            {
                                "stageName": "generate",
                                "podRef": "generator.opus.crosscutting",
                                "needsSecrets": ["github:repo"],
                                "vaultEnvBindings": [
                                    {
                                        "envVar": "GITHUB_TOKEN",
                                        "entryName": "GitHub Repo Token",
                                        "scopeTags": ["github:repo"],
                                        "delivery": "file"
                                    }
                                ]
                            },
                            {
                                "stageName": "evaluate",
                                "retryPolicy": {
                                    "maxAttempts": 4,
                                    "onFailFeedbackTo": "generate"
                                }
                            }
                        ]
                    })
                    .to_string()
                ],
            )
            .expect("workflow overrides should insert");

        let run = start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        )
        .expect("workflow run should start with overrides");

        assert!(run.resolved_workflow.has_overrides);

        let generate_stage = run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "generate")
            .expect("generate stage should exist");
        assert_eq!(
            generate_stage.pod_slug.as_deref(),
            Some("generator.opus.crosscutting")
        );
        assert_eq!(generate_stage.model.as_deref(), Some("opus-4.6"));
        assert_eq!(
            generate_stage.resolved_stage.needs_secrets,
            vec!["github:repo".to_string()]
        );
        assert_eq!(generate_stage.resolved_stage.vault_env_bindings.len(), 1);
        assert_eq!(
            generate_stage.resolved_stage.vault_env_bindings[0].env_var,
            "GITHUB_TOKEN"
        );
        assert_eq!(
            generate_stage.resolved_stage.vault_env_bindings[0].entry_name,
            "GitHub Repo Token"
        );
        assert_eq!(
            generate_stage.resolved_stage.vault_env_bindings[0].delivery,
            crate::vault::VaultBindingDelivery::File
        );

        let evaluate_stage = run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "evaluate")
            .expect("evaluate stage should exist");
        assert_eq!(
            evaluate_stage
                .resolved_stage
                .retry_policy
                .as_ref()
                .map(|policy| policy.max_attempts),
            Some(4)
        );
        assert_eq!(
            evaluate_stage
                .resolved_stage
                .retry_policy
                .as_ref()
                .and_then(|policy| policy.on_fail_feedback_to.as_deref()),
            Some("generate")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workflow_override_document_exports_db_overrides_and_round_trips_repo_file() {
        let root = temp_dir("override-doc");
        fs::create_dir_all(&root).expect("project root should exist");
        let connection = create_connection();
        set_project_root(&connection, &root);
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        connection
            .execute(
                "
                INSERT INTO project_workflow_overrides (project_id, workflow_slug, overrides_json)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    1,
                    "feature-dev",
                    serde_json::json!({
                        "stageOverrides": [
                            {
                                "stageName": "evaluate",
                                "retryPolicy": {
                                    "maxAttempts": 5,
                                    "onFailFeedbackTo": "generate"
                                }
                            }
                        ]
                    })
                    .to_string()
                ],
            )
            .expect("workflow overrides should insert");

        let document = load_project_workflow_override_document(&connection, 1, &root, "feature-dev")
            .expect("override document should load");
        assert!(!document.exists);
        assert_eq!(document.source, "database");
        assert!(document.has_overrides);
        assert!(document.yaml.contains("stageOverrides"));
        assert!(document.yaml.contains("evaluate"));

        let saved = save_project_workflow_override_document(
            &connection,
            &root,
            &SaveProjectWorkflowOverrideInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                yaml: document.yaml.clone(),
            },
        )
        .expect("override document should save");
        assert!(saved.exists);
        assert_eq!(saved.source, "repo");
        assert!(Path::new(&saved.file_path).is_file());

        let cleared = clear_project_workflow_override_document(
            &connection,
            &root,
            &ProjectWorkflowOverrideTarget {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
            },
        )
        .expect("override document should clear");
        assert!(!cleared.exists);
        assert_eq!(cleared.source, "empty");
        assert!(
            load_workflow_overrides(&connection, 1, "feature-dev")
                .expect("db overrides should inspect")
                .is_none()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repo_override_file_takes_precedence_at_run_start() {
        let root = temp_dir("repo-override-run");
        fs::create_dir_all(&root).expect("project root should exist");
        let connection = create_connection();
        set_project_root(&connection, &root);
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        connection
            .execute(
                "
                INSERT INTO project_workflow_overrides (project_id, workflow_slug, overrides_json)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    1,
                    "feature-dev",
                    serde_json::json!({
                        "stageOverrides": [
                            {
                                "stageName": "evaluate",
                                "retryPolicy": {
                                    "maxAttempts": 2,
                                    "onFailFeedbackTo": "generate"
                                }
                            }
                        ]
                    })
                    .to_string()
                ],
            )
            .expect("db override should insert");

        save_project_workflow_override_document(
            &connection,
            &root,
            &SaveProjectWorkflowOverrideInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                yaml: serde_yaml::to_string(&serde_json::json!({
                    "stageOverrides": [
                        {
                            "stageName": "evaluate",
                            "retryPolicy": {
                                "maxAttempts": 6,
                                "onFailFeedbackTo": "generate"
                            }
                        }
                    ]
                }))
                .expect("yaml should encode"),
            },
        )
        .expect("repo override should save");

        let run = start_workflow_run_with_project_root(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
            Some(&root),
        )
        .expect("workflow run should start from repo override");

        let evaluate_stage = run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "evaluate")
            .expect("evaluate stage should exist");
        assert_eq!(
            evaluate_stage
                .resolved_stage
                .retry_policy
                .as_ref()
                .map(|policy| policy.max_attempts),
            Some(6)
        );

        let persisted = load_workflow_overrides(&connection, 1, "feature-dev")
            .expect("db overrides should inspect")
            .expect("db overrides should persist");
        assert_eq!(
            persisted.stage_overrides[0]
                .retry_policy
                .as_ref()
                .map(|policy| policy.max_attempts),
            Some(6)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workflow_run_rejects_stage_vault_binding_outside_pod_secret_scopes() {
        let root = temp_dir("run-vault-binding-scope");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        connection
            .execute(
                "
                INSERT INTO project_workflow_overrides (project_id, workflow_slug, overrides_json)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    1,
                    "feature-dev",
                    serde_json::json!({
                        "stageOverrides": [
                            {
                                "stageName": "generate",
                                "needsSecrets": ["github:repo"],
                                "vaultEnvBindings": [
                                    {
                                        "envVar": "GITHUB_TOKEN",
                                        "entryName": "GitHub Repo Token",
                                        "scopeTags": ["github:repo"]
                                    }
                                ]
                            }
                        ]
                    })
                    .to_string()
                ],
            )
            .expect("workflow overrides should insert");

        let error = match start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        ) {
            Ok(_) => panic!("run should reject stage vault binding outside pod secret scopes"),
            Err(error) => error,
        };

        assert!(error.contains("generator.sonnet.standard"));
        assert!(error.contains("secret_scopes"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolved_workflow_stages_include_artifact_contracts() {
        let root = temp_dir("run-artifact-contracts");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        let run = start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        )
        .expect("workflow run should start");

        let plan_stage = run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "plan")
            .expect("plan stage should exist");
        assert_eq!(plan_stage.resolved_stage.output_contracts.len(), 2);
        assert_eq!(
            plan_stage.resolved_stage.output_contracts[0].artifact_type,
            "plan_doc"
        );
        assert!(plan_stage.resolved_stage.output_contracts[0]
            .required_frontmatter_fields
            .contains(&"deliverables".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_artifact_contract_blocks_stage_completion() {
        let root = temp_dir("run-invalid-artifact");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        let run = start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        )
        .expect("workflow run should start");

        let output = record_workflow_stage_result(
            &connection,
            &RecordWorkflowStageResultInput {
                project_id: 1,
                run_id: run.id,
                stage_name: "plan".to_string(),
                response_message_id: Some(42),
                completion_message_type: "complete".to_string(),
                completion_summary: Some("planned".to_string()),
                completion_context_json: Some(
                    serde_json::json!({
                        "workflowRunId": run.id,
                        "stageName": "plan",
                        "producedArtifacts": [
                            {
                                "type": "plan_doc",
                                "summary": "missing required sections and frontmatter",
                                "frontmatter": {
                                    "deliverables": ["scope"]
                                },
                                "bodyMarkdown": "## Scope\nLimited scope only"
                            },
                            {
                                "type": "sprint_list",
                                "summary": "missing sprint breakdown section",
                                "frontmatter": {
                                    "sprints": ["one"]
                                },
                                "bodyMarkdown": "No heading"
                            }
                        ]
                    })
                    .to_string(),
                ),
            },
        )
        .expect("stage result should persist");

        assert!(output.retry.is_none());
        assert_eq!(output.run.status, "blocked");
        let plan_stage = output
            .run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "plan")
            .expect("plan stage should exist");
        assert_eq!(
            plan_stage.completion_message_type.as_deref(),
            Some("produced_invalid_artifact")
        );
        assert_eq!(
            plan_stage.artifact_validation_status.as_deref(),
            Some("invalid")
        );
        assert!(plan_stage
            .artifact_validation_error
            .as_deref()
            .unwrap_or_default()
            .contains("required"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_reported_artifacts_block_stage_completion() {
        let root = temp_dir("run-missing-artifacts");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        let run = start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        )
        .expect("workflow run should start");

        let output = record_workflow_stage_result(
            &connection,
            &RecordWorkflowStageResultInput {
                project_id: 1,
                run_id: run.id,
                stage_name: "plan".to_string(),
                response_message_id: Some(43),
                completion_message_type: "complete".to_string(),
                completion_summary: Some("planned".to_string()),
                completion_context_json: Some(
                    serde_json::json!({
                        "workflowRunId": run.id,
                        "stageName": "plan"
                    })
                    .to_string(),
                ),
            },
        )
        .expect("stage result should persist");

        assert_eq!(output.run.status, "blocked");
        let plan_stage = output
            .run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "plan")
            .expect("plan stage should exist");
        assert_eq!(
            plan_stage.completion_message_type.as_deref(),
            Some("produced_invalid_artifact")
        );
        assert_eq!(
            plan_stage.artifact_validation_status.as_deref(),
            Some("unreported")
        );
        assert!(plan_stage
            .artifact_validation_error
            .as_deref()
            .unwrap_or_default()
            .contains("reported none"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn blocked_evaluator_result_schedules_retry_feedback_to_generate_stage() {
        let root = temp_dir("run-eval-retry");
        let connection = create_connection();
        sync_library_catalog(&connection, &root).expect("workflow sync should succeed");
        adopt_feature_dev(&connection);

        let run = start_workflow_run(
            &connection,
            &StartWorkflowRunInput {
                project_id: 1,
                workflow_slug: "feature-dev".to_string(),
                root_work_item_id: 1,
                root_worktree_id: None,
            },
        )
        .expect("workflow run should start");

        let output = record_workflow_stage_result(
            &connection,
            &RecordWorkflowStageResultInput {
                project_id: 1,
                run_id: run.id,
                stage_name: "evaluate".to_string(),
                response_message_id: Some(77),
                completion_message_type: "blocked".to_string(),
                completion_summary: Some("generator missed acceptance criteria".to_string()),
                completion_context_json: Some(
                    serde_json::json!({
                        "workflowRunId": run.id,
                        "stageName": "evaluate",
                        "reason": "generator missed acceptance criteria"
                    })
                    .to_string(),
                ),
            },
        )
        .expect("blocked result should persist and schedule retry");

        let retry = output.retry.expect("retry should be scheduled");
        assert_eq!(retry.source_stage_name, "evaluate");
        assert_eq!(retry.target_stage_name, "generate");
        assert_eq!(retry.next_attempt, 2);
        assert_eq!(retry.max_attempts, 3);
        assert_eq!(output.run.status, "running");

        let generate_stage = output
            .run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "generate")
            .expect("generate stage should exist");
        assert_eq!(generate_stage.status, "pending");
        assert_eq!(generate_stage.attempt, 2);
        assert_eq!(
            generate_stage.retry_source_stage_name.as_deref(),
            Some("evaluate")
        );
        assert!(generate_stage
            .retry_feedback_summary
            .as_deref()
            .unwrap_or_default()
            .contains("acceptance criteria"));

        let evaluate_stage = output
            .run
            .stages
            .iter()
            .find(|stage| stage.stage_name == "evaluate")
            .expect("evaluate stage should exist");
        assert_eq!(evaluate_stage.status, "pending");
        assert_eq!(evaluate_stage.attempt, 2);

        let _ = fs::remove_dir_all(root);
    }
}
