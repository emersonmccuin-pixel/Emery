import { invoke } from "@tauri-apps/api/core";
import type {
  ConnectionStatusEvent,
  DiagnosticsBundleResult,
  DocumentDetail,
  DocumentSummary,
  PlanningAssignmentDetail,
  PlanningAssignmentSummary,
  ProjectDetail,
  SessionAttachResponse,
  SessionDetail,
  ShellBootstrap,
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

export async function createProject(
  input: {
    name: string;
    slug?: string;
    default_account_id?: string | null;
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
  },
  correlationId?: string,
): Promise<ProjectDetail> {
  return invoke("update_project", { projectId, input, correlationId });
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
  },
  correlationId?: string,
): Promise<SessionDetail> {
  return invoke("create_session", { input, correlationId });
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
