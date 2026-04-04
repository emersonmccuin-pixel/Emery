import { invoke } from "@tauri-apps/api/core";
import type {
  ConnectionStatusEvent,
  DiagnosticsBundleResult,
  DocumentDetail,
  DocumentSummary,
  SessionAttachResponse,
  ShellBootstrap,
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

export async function getWorkItem(
  workItemId: string,
  correlationId?: string,
): Promise<WorkItemDetail> {
  return invoke("get_work_item", { workItemId, correlationId });
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
