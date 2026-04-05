import { invoke } from "@tauri-apps/api/core";
import type {
  AgentTemplateDetail,
  AgentTemplateSummary,
  ConflictWarning,
  ConnectionStatusEvent,
  DiagnosticsBundleResult,
  DocumentDetail,
  DocumentSummary,
  GitHealthStatus,
  MergeQueueEntry,
  PlanningAssignmentDetail,
  PlanningAssignmentSummary,
  ProjectDetail,
  SessionAttachResponse,
  SessionDetail,
  ShellBootstrap,
  VaultAuditEntry,
  VaultEntry,
  VaultLockStatus,
  WorkflowReconciliationProposalDetail,
  WorkflowReconciliationProposalSummary,
  WorkItemDetail,
  WorkItemSummary,
  WorkspacePayload,
} from "./types";

export async function bootstrapShell(correlationId?: string): Promise<ShellBootstrap> {
  return invoke("bootstrap_shell", { correlationId });
}

export async function attachSession(
  sessionId: string,
  correlationId?: string,
): Promise<SessionAttachResponse> {
  return invoke("attach_session", { sessionId, correlationId });
}

export async function getSession(
  sessionId: string,
  correlationId?: string,
): Promise<SessionDetail> {
  return invoke("get_session", { sessionId, correlationId });
}

export async function detachSession(
  sessionId: string,
  attachmentId: string,
  correlationId?: string,
): Promise<void> {
  await invoke("detach_session", { sessionId, attachmentId, correlationId });
}

export async function sendSessionInput(
  sessionId: string,
  input: string,
  correlationId?: string,
): Promise<void> {
  await invoke("send_session_input", { sessionId, input, correlationId });
}

export async function resizeSession(
  sessionId: string,
  cols: number,
  rows: number,
  correlationId?: string,
): Promise<void> {
  await invoke("resize_session", { sessionId, cols, rows, correlationId });
}

export async function interruptSession(sessionId: string, correlationId?: string): Promise<void> {
  await invoke("interrupt_session", { sessionId, correlationId });
}

export async function terminateSession(sessionId: string, correlationId?: string): Promise<void> {
  await invoke("terminate_session", { sessionId, correlationId });
}

export async function saveWorkspace(
  payload: WorkspacePayload,
  correlationId?: string,
): Promise<void> {
  await invoke("save_workspace_state", { payload, correlationId });
}

export type PollableEvent = {
  event: string;
  payload: unknown;
};

export async function pollEvents(): Promise<PollableEvent[]> {
  return invoke("poll_events");
}

export async function watchLiveSessions(
  sessionIds: string[],
  correlationId?: string,
): Promise<void> {
  await invoke("watch_live_sessions", { sessionIds, correlationId });
}

export async function listWorkItems(
  projectId: string,
  correlationId?: string,
): Promise<WorkItemSummary[]> {
  return invoke("list_work_items", { projectId, correlationId });
}

export async function getProject(
  projectId: string,
  correlationId?: string,
): Promise<ProjectDetail> {
  return invoke("get_project", { projectId, correlationId });
}

export async function listNamespaceSuggestions(
  correlationId?: string,
): Promise<string[]> {
  return invoke("list_namespace_suggestions", { correlationId });
}

export async function createProject(
  input: {
    name: string;
    slug?: string;
    default_account_id?: string | null;
    project_type?: string | null;
    wcp_namespace?: string | null;
  },
  correlationId?: string,
): Promise<ProjectDetail> {
  return invoke("create_project", { input, correlationId });
}

export async function updateProject(
  projectId: string,
  input: {
    name?: string;
    slug?: string;
    default_account_id?: string | null;
    model_defaults_json?: string | null;
    agent_safety_overrides_json?: string | null;
    instructions_md?: string | null;
  },
  correlationId?: string,
): Promise<ProjectDetail> {
  return invoke("update_project", { projectId, input, correlationId });
}

export async function archiveProject(
  projectId: string,
  correlationId?: string,
): Promise<ProjectDetail> {
  return invoke("archive_project", { projectId, correlationId });
}

export async function deleteProject(
  projectId: string,
  correlationId?: string,
): Promise<void> {
  return invoke("delete_project", { projectId, correlationId });
}

export async function getWorkItem(
  workItemId: string,
  correlationId?: string,
): Promise<WorkItemDetail> {
  return invoke("get_work_item", { workItemId, correlationId });
}

export async function createWorkItem(
  input: {
    project_id: string;
    parent_id?: string | null;
    title: string;
    description: string;
    acceptance_criteria?: string | null;
    work_item_type: string;
    status?: string;
    priority?: string | null;
  },
  correlationId?: string,
): Promise<WorkItemDetail> {
  return invoke("create_work_item", { input, correlationId });
}

export async function updateWorkItem(
  workItemId: string,
  input: {
    title?: string;
    description?: string;
    acceptance_criteria?: string | null;
    work_item_type?: string;
    status?: string;
    priority?: string | null;
  },
  correlationId?: string,
): Promise<WorkItemDetail> {
  return invoke("update_work_item", { workItemId, input, correlationId });
}

export async function listPlanningAssignments(
  projectId: string,
  workItemId?: string | null,
  correlationId?: string,
): Promise<PlanningAssignmentSummary[]> {
  return invoke("list_planning_assignments", { projectId, workItemId, correlationId });
}

export async function createPlanningAssignment(
  input: {
    work_item_id: string;
    cadence_type: string;
    cadence_key: string;
    created_by: string;
  },
  correlationId?: string,
): Promise<PlanningAssignmentDetail> {
  return invoke("create_planning_assignment", { input, correlationId });
}

export async function deletePlanningAssignment(
  planningAssignmentId: string,
  correlationId?: string,
): Promise<PlanningAssignmentDetail> {
  return invoke("delete_planning_assignment", { planningAssignmentId, correlationId });
}

export async function listDocuments(
  projectId: string,
  workItemId?: string | null,
  correlationId?: string,
): Promise<DocumentSummary[]> {
  return invoke("list_documents", { projectId, workItemId, correlationId });
}

export async function getDocument(
  documentId: string,
  correlationId?: string,
): Promise<DocumentDetail> {
  return invoke("get_document", { documentId, correlationId });
}

export async function createDocument(
  input: {
    project_id: string;
    work_item_id?: string | null;
    session_id?: string | null;
    doc_type: string;
    title: string;
    slug?: string;
    status?: string;
    content_markdown: string;
  },
  correlationId?: string,
): Promise<DocumentDetail> {
  return invoke("create_document", { input, correlationId });
}

export async function updateDocument(
  documentId: string,
  input: {
    work_item_id?: string | null;
    session_id?: string | null;
    doc_type?: string;
    title?: string;
    slug?: string;
    status?: string;
    content_markdown?: string;
  },
  correlationId?: string,
): Promise<DocumentDetail> {
  return invoke("update_document", { documentId, input, correlationId });
}

export async function listWorkflowReconciliationProposals(
  workItemId: string,
  correlationId?: string,
): Promise<WorkflowReconciliationProposalSummary[]> {
  return invoke("list_workflow_reconciliation_proposals", {
    workItemId,
    correlationId,
  });
}

export async function updateWorkflowReconciliationProposal(
  proposalId: string,
  input: {
    status?: string;
  },
  correlationId?: string,
): Promise<WorkflowReconciliationProposalDetail> {
  return invoke("update_workflow_reconciliation_proposal", {
    proposalId,
    input,
    correlationId,
  });
}

export async function createSession(
  input: {
    project_id: string;
    project_root_id?: string | null;
    worktree_id?: string | null;
    work_item_id?: string | null;
    account_id: string;
    agent_kind: string;
    cwd: string;
    command: string;
    args?: string[];
    env_preset_ref?: string | null;
    origin_mode: string;
    current_mode?: string | null;
    title?: string | null;
    title_policy?: string;
    restore_policy?: string;
    initial_terminal_cols?: number;
    initial_terminal_rows?: number;
    auto_worktree?: boolean;
    safety_mode?: string;
    extra_args?: string[];
    model?: string | null;
    stop_rules?: string[];
  },
  correlationId?: string,
): Promise<SessionDetail> {
  return invoke("create_session", { input, correlationId });
}

export async function createSessionBatch(
  input: Array<Parameters<typeof createSession>[0]>,
  correlationId?: string,
): Promise<SessionDetail[]> {
  return invoke("create_session_batch", { input, correlationId });
}

export async function checkDispatchConflicts(
  workItemIds: string[],
  correlationId?: string,
): Promise<{ warnings: ConflictWarning[] }> {
  return invoke("check_dispatch_conflicts", { workItemIds, correlationId });
}

export async function exportDiagnosticsBundle(
  sessionId: string | null,
  incidentLabel: string | null,
  frontendEvents: unknown[],
  correlationId?: string,
): Promise<DiagnosticsBundleResult> {
  return invoke("export_diagnostics_bundle", {
    sessionId,
    incidentLabel,
    frontendEvents,
    correlationId,
  });
}

// --- Merge Queue ---

export async function listMergeQueue(
  projectId: string,
  correlationId?: string,
): Promise<MergeQueueEntry[]> {
  return invoke("list_merge_queue", { projectId, correlationId });
}

export async function getMergeQueueDiff(
  mergeQueueId: string,
  correlationId?: string,
): Promise<{ diff: string }> {
  return invoke("get_merge_queue_diff", { mergeQueueId, correlationId });
}

export async function mergeQueueMerge(
  mergeQueueId: string,
  correlationId?: string,
): Promise<void> {
  await invoke("merge_queue_merge", { mergeQueueId, correlationId });
}

export async function mergeQueuePark(
  mergeQueueId: string,
  correlationId?: string,
): Promise<void> {
  await invoke("merge_queue_park", { mergeQueueId, correlationId });
}

export async function mergeQueueReorder(
  projectId: string,
  orderedIds: string[],
  correlationId?: string,
): Promise<void> {
  await invoke("merge_queue_reorder", { projectId, orderedIds, correlationId });
}

export async function mergeQueueCheckConflicts(
  mergeQueueId: string,
  correlationId?: string,
): Promise<{ conflicts: string[] }> {
  return invoke("merge_queue_check_conflicts", { mergeQueueId, correlationId });
}

export async function pickFolder(): Promise<string | null> {
  return invoke("pick_folder");
}

// --- Inbox ---

export type InboxEntrySummary = {
  id: string;
  project_id: string;
  session_id: string | null;
  work_item_id: string | null;
  worktree_id: string | null;
  entry_type: string;
  title: string;
  summary: string;
  status: string;
  branch_name: string | null;
  diff_stat_json: string | null;
  metadata_json: string | null;
  read_at: number | null;
  resolved_at: number | null;
  created_at: number;
  updated_at: number;
  session_title: string | null;
  work_item_callsign: string | null;
};

export type InboxEntryDetail = InboxEntrySummary;

export async function listInboxEntries(
  projectId: string,
  status?: string,
  unreadOnly?: boolean,
  correlationId?: string,
): Promise<InboxEntrySummary[]> {
  return invoke("list_inbox_entries", { projectId, status, unreadOnly, correlationId });
}

export async function getInboxEntry(
  inboxEntryId: string,
  correlationId?: string,
): Promise<InboxEntryDetail> {
  return invoke("get_inbox_entry", { inboxEntryId, correlationId });
}

export async function updateInboxEntry(
  inboxEntryId: string,
  updates: { status?: string; read_at?: number | null; resolved_at?: number | null },
  correlationId?: string,
): Promise<InboxEntryDetail> {
  return invoke("update_inbox_entry", {
    inboxEntryId,
    status: updates.status,
    readAt: updates.read_at,
    resolvedAt: updates.resolved_at,
    correlationId,
  });
}

export async function countUnreadInboxEntries(
  projectId: string,
  correlationId?: string,
): Promise<{ count: number }> {
  return invoke("count_unread_inbox_entries", { projectId, correlationId });
}

export async function createProjectRoot(
  input: {
    project_id: string;
    label: string;
    path: string;
    root_kind: string;
    git_root_path?: string | null;
    remote_url?: string | null;
    sort_order?: number | null;
  },
  correlationId?: string,
): Promise<unknown> {
  return invoke("create_project_root", { input, correlationId });
}

export async function listProjectRoots(
  projectId: string,
  correlationId?: string,
): Promise<unknown[]> {
  return invoke("list_project_roots", { projectId, correlationId });
}

export async function updateProjectRoot(
  rootId: string,
  input: {
    label?: string;
    path?: string;
    remote_url?: string | null;
  },
  correlationId?: string,
): Promise<unknown> {
  return invoke("update_project_root", { rootId, input, correlationId });
}

export async function removeProjectRoot(
  rootId: string,
  correlationId?: string,
): Promise<unknown> {
  return invoke("remove_project_root", { rootId, correlationId });
}

export async function gitInitProjectRoot(
  rootId: string,
  correlationId?: string,
): Promise<unknown> {
  return invoke("git_init_project_root", { rootId, correlationId });
}

export async function setProjectRootRemote(
  rootId: string,
  remoteUrl: string,
  correlationId?: string,
): Promise<unknown> {
  return invoke("set_project_root_remote", { rootId, remoteUrl, correlationId });
}

// --- Accounts ---

export async function listAccounts(correlationId?: string): Promise<unknown[]> {
  return invoke("list_accounts", { correlationId });
}

export async function getAccount(accountId: string, correlationId?: string): Promise<unknown> {
  return invoke("get_account", { accountId, correlationId });
}

export async function createAccount(
  input: {
    label: string;
    agent_kind?: string;
    binary_path?: string | null;
    config_root?: string | null;
    is_default?: boolean;
  },
  correlationId?: string,
): Promise<unknown> {
  return invoke("create_account", { input, correlationId });
}

export async function updateAccount(
  accountId: string,
  input: {
    label?: string;
    binary_path?: string | null;
    config_root?: string | null;
    is_default?: boolean;
    status?: string;
    default_model?: string | null;
    default_safety_mode?: string | null;
    default_launch_args?: string[] | null;
  },
  correlationId?: string,
): Promise<unknown> {
  return invoke("update_account", { accountId, input, correlationId });
}

export async function getProjectRootGitStatus(
  rootId: string,
  correlationId?: string,
): Promise<GitHealthStatus | null> {
  return invoke("get_project_root_git_status", { rootId, correlationId });
}

export async function listAgentTemplates(
  projectId: string,
  correlationId?: string,
): Promise<AgentTemplateSummary[]> {
  return invoke("list_agent_templates", { projectId, correlationId });
}

export async function createAgentTemplate(
  input: {
    project_id: string;
    template_key: string;
    label: string;
    origin_mode?: string;
    default_model?: string | null;
    instructions_md?: string | null;
    stop_rules_json?: string | null;
    sort_order?: number;
  },
  correlationId?: string,
): Promise<AgentTemplateDetail> {
  return invoke("create_agent_template", { input, correlationId });
}

export async function updateAgentTemplate(
  agentTemplateId: string,
  input: {
    template_key?: string;
    label?: string;
    origin_mode?: string;
    default_model?: string | null;
    instructions_md?: string | null;
    stop_rules_json?: string | null;
    sort_order?: number;
  },
  correlationId?: string,
): Promise<AgentTemplateDetail> {
  return invoke("update_agent_template", { agentTemplateId, input, correlationId });
}

export async function archiveAgentTemplate(
  agentTemplateId: string,
  correlationId?: string,
): Promise<AgentTemplateDetail> {
  return invoke("archive_agent_template", { agentTemplateId, correlationId });
}

// --- Vault ---

export async function vaultList(scope?: string, correlationId?: string): Promise<VaultEntry[]> {
  return invoke("vault_list", { scope, correlationId });
}

export async function vaultSet(
  scope: string,
  key: string,
  value: string,
  description?: string | null,
  correlationId?: string,
): Promise<VaultEntry> {
  return invoke("vault_set", { scope, key, value, description, correlationId });
}

export async function vaultDelete(id: string, correlationId?: string): Promise<void> {
  await invoke("vault_delete", { id, correlationId });
}

export async function vaultUnlock(
  durationMinutes?: number,
  correlationId?: string,
): Promise<VaultLockStatus> {
  return invoke("vault_unlock", { durationMinutes, correlationId });
}

export async function vaultLock(correlationId?: string): Promise<VaultLockStatus> {
  return invoke("vault_lock", { correlationId });
}

export async function vaultStatus(correlationId?: string): Promise<VaultLockStatus> {
  return invoke("vault_status", { correlationId });
}

export async function vaultAuditLog(correlationId?: string): Promise<VaultAuditEntry[]> {
  return invoke("vault_audit_log", { correlationId });
}

export function connectionLabel(event: ConnectionStatusEvent | null): string {
  if (!event) {
    return "connecting";
  }
  if (event.state === "connected") {
    return "connected";
  }
  if (event.state === "reconnecting") {
    return "reconnecting";
  }
  return "disconnected";
}
