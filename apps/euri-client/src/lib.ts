import { invoke } from "@tauri-apps/api/core";
import type {
  ConnectionStatusEvent,
  SessionAttachResponse,
  ShellBootstrap,
  WorkspacePayload,
} from "./types";

export async function bootstrapShell(): Promise<ShellBootstrap> {
  return invoke("bootstrap_shell");
}

export async function attachSession(sessionId: string): Promise<SessionAttachResponse> {
  return invoke("attach_session", { sessionId });
}

export async function detachSession(sessionId: string, attachmentId: string): Promise<void> {
  await invoke("detach_session", { sessionId, attachmentId });
}

export async function sendSessionInput(sessionId: string, input: string): Promise<void> {
  await invoke("send_session_input", { sessionId, input });
}

export async function interruptSession(sessionId: string): Promise<void> {
  await invoke("interrupt_session", { sessionId });
}

export async function terminateSession(sessionId: string): Promise<void> {
  await invoke("terminate_session", { sessionId });
}

export async function saveWorkspace(payload: WorkspacePayload): Promise<void> {
  await invoke("save_workspace_state", { payload });
}

export async function watchLiveSessions(sessionIds: string[]): Promise<void> {
  await invoke("watch_live_sessions", { sessionIds });
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
