import { useCallback, useRef, useSyncExternalStore } from "react";
import {
  archiveProject,
  deleteProject,
  bootstrapShell,
  checkDispatchConflicts,
  createDocument,
  createProject,
  createProjectRoot,
  removeProjectRoot,
  gitInitProjectRoot,
  setProjectRootRemote,
  createSession,
  createSessionBatch,
  createWorkItem,
  getMergeQueueDiff,
  getProject,
  getSession,
  getDocument,
  getWorkItem,
  interruptSession,
  listDocuments,
  listMergeQueue,
  listWorkflowReconciliationProposals,
  listAccounts,
  createAccount,
  updateAccount,
  listWorkItems,
  mergeQueueCheckConflicts,
  mergeQueueMerge,
  mergeQueuePark,
  terminateSession,
  updateDocument,
  updateProject,
  updateWorkflowReconciliationProposal,
  updateWorkItem,
  watchLiveSessions,
} from "./lib";
import { sessionStore } from "./session-store";
import { navStore } from "./nav-store";
import { toastStore } from "./toast-store";
import {
  makeClientEvent,
  newCorrelationId,
  recordClientEvent,
} from "./diagnostics";
// ── Completion ding ──
// Baking timer style ding when an agent session finishes its work.
// Uses Web Audio API — no audio file needed.
function playCompletionDing() {
  try {
    const ctx = new AudioContext();
    const now = ctx.currentTime;

    // Three-note ding pattern (like a kitchen timer)
    const notes = [1047, 1319, 1568]; // C6, E6, G6 — major chord ascending
    for (let i = 0; i < notes.length; i++) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.type = "sine";
      osc.frequency.value = notes[i];
      const start = now + i * 0.12;
      gain.gain.setValueAtTime(0.2, start);
      gain.gain.exponentialRampToValueAtTime(0.001, start + 0.3);
      osc.start(start);
      osc.stop(start + 0.3);
    }
  } catch {
    // Audio not available — silently skip
  }
}

function playErrorChime() {
  try {
    const ctx = new AudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.type = "triangle";
    osc.frequency.value = 330;
    gain.gain.setValueAtTime(0.2, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.6);
    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + 0.6);
  } catch {
    // Audio not available — silently skip
  }
}

import type {
  ConflictWarning,
  ConnectionStatusEvent,
  DocumentDetail,
  DocumentSummary,
  GitHealthStatus,
  MergeQueueEntry,
  PendingDispatch,
  ProjectDetail,
  ProjectSummary,
  SessionDetail,
  SessionStateChangedEvent,
  SessionSummary,
  ShellBootstrap,
  WorkItemDetail,
  WorkItemSummary,
  WorkflowReconciliationProposalSummary,
} from "./types";
import { getProjectRootGitStatus } from "./lib";

// --- Constants ---

export const WORK_ITEM_TYPES = ["epic", "task", "bug", "feature", "research", "support"] as const;
export const WORK_ITEM_STATUSES = [
  "backlog",
  "planned",
  "in_progress",
  "blocked",
  "done",
  "archived",
] as const;
export const PRIORITIES = ["", "low", "medium", "high", "urgent"] as const;
export const DOCUMENT_STATUSES = ["draft", "active", "archived"] as const;

// --- Pure helpers ---

function toProjectSummary(detail: ProjectDetail, current?: ProjectSummary | null): ProjectSummary {
  return {
    id: detail.id,
    name: detail.name,
    slug: detail.slug,
    sort_order: detail.sort_order,
    default_account_id: detail.default_account_id,
    root_count: detail.roots.length,
    live_session_count: current?.live_session_count ?? 0,
    created_at: detail.created_at,
    updated_at: detail.updated_at,
    archived_at: detail.archived_at,
    project_type: detail.project_type,
    model_defaults_json: detail.model_defaults_json,
    wcp_namespace: detail.wcp_namespace,
  };
}

function upsertById<T extends { id: string }>(
  items: T[],
  nextItem: T,
  compare: (left: T, right: T) => number,
) {
  const nextItems = items.filter((item) => item.id !== nextItem.id);
  nextItems.push(nextItem);
  nextItems.sort(compare);
  return nextItems;
}

function compareProjects(left: ProjectSummary, right: ProjectSummary) {
  return left.sort_order - right.sort_order || left.name.localeCompare(right.name);
}

function compareWorkItems(left: WorkItemSummary, right: WorkItemSummary) {
  return right.updated_at - left.updated_at || left.callsign.localeCompare(right.callsign);
}

function compareDocuments(left: DocumentSummary, right: DocumentSummary) {
  return right.updated_at - left.updated_at || left.title.localeCompare(right.title);
}

function compareProposals(
  left: WorkflowReconciliationProposalSummary,
  right: WorkflowReconciliationProposalSummary,
) {
  return right.created_at - left.created_at || left.id.localeCompare(right.id);
}

function compareSessions(left: SessionSummary, right: SessionSummary) {
  return right.updated_at - left.updated_at || left.id.localeCompare(right.id);
}

function sessionSummaryFromDetail(detail: SessionDetail): SessionSummary {
  const { runtime: _runtime, ...summary } = detail;
  return summary;
}

export function sessionTone(session: Pick<SessionSummary, "runtime_state" | "activity_state" | "needs_input_reason">) {
  if (session.runtime_state === "failed" || session.runtime_state === "interrupted") {
    return "danger";
  }
  if (session.runtime_state === "stopping") {
    return "warning";
  }
  if (session.runtime_state === "running" && session.activity_state === "waiting_for_input") {
    return "muted";
  }
  if (session.runtime_state === "running") {
    return "live";
  }
  if (session.runtime_state === "starting") {
    return "pending";
  }
  return "muted";
}

/** Returns true if any session for the given project needs input. */
export function projectNeedsAttention(projectId: string, sessions: SessionSummary[]): boolean {
  return sessions.some(s => s.project_id === projectId && s.activity_state === "needs_input");
}

// --- State shape ---

export type AppState = {
  bootstrap: ShellBootstrap | null;
  bootstrapping: boolean;
  sessions: SessionSummary[];
  selectedProjectId: string | null;
  projectDetails: Record<string, ProjectDetail>;
  workItemsByProject: Record<string, WorkItemSummary[]>;
  documentsByProject: Record<string, DocumentSummary[]>;
  workItemDetails: Record<string, WorkItemDetail>;
  documentDetails: Record<string, DocumentDetail>;
  reconciliationByWorkItem: Record<string, WorkflowReconciliationProposalSummary[]>;
  mergeQueueByProject: Record<string, MergeQueueEntry[]>;
  mergeQueueDiffs: Record<string, string>;
  gitStatusByRootId: Record<string, GitHealthStatus>;
  connectionEvent: ConnectionStatusEvent | null;
  loadingKeys: Record<string, boolean>;
  error: string | null;
  showCreateForm: false | "work" | "doc";
  workItemCreateForm: {
    title: string;
    description: string;
    acceptance_criteria: string;
    work_item_type: string;
    status: string;
    priority: string;
    parent_id: string;
  };
  documentCreateForm: {
    title: string;
    slug: string;
    doc_type: string;
    status: string;
    work_item_id: string;
    content_markdown: string;
  };
  pendingDispatch: PendingDispatch | null;
  selectedWorkItemIds: string[];
  dispatchConflicts: ConflictWarning[];
  focusProjectIds: string[];
  maxFocusSlots: number;
  connectionState: "connecting" | "connected" | "reconnecting" | "disconnected";
  editingWorkItemId: string | null;
  githubToken: string;
  knowledgeStoreBackend: "embedded" | "wcp_cloud";
};

function initialState(): AppState {
  return {
    bootstrap: null,
    bootstrapping: true,
    sessions: [],
    selectedProjectId: null,
    projectDetails: {},
    workItemsByProject: {},
    documentsByProject: {},
    workItemDetails: {},
    documentDetails: {},
    reconciliationByWorkItem: {},
    mergeQueueByProject: {},
    mergeQueueDiffs: {},
    gitStatusByRootId: {},
    connectionEvent: null,
    loadingKeys: {},
    error: null,
    showCreateForm: false,
    workItemCreateForm: {
      title: "",
      description: "",
      acceptance_criteria: "",
      work_item_type: "task",
      status: "backlog",
      priority: "",
      parent_id: "",
    },
    documentCreateForm: {
      title: "",
      slug: "",
      doc_type: "note",
      status: "draft",
      work_item_id: "",
      content_markdown: "",
    },
    pendingDispatch: null,
    focusProjectIds: [],
    maxFocusSlots: 3,
    selectedWorkItemIds: [],
    dispatchConflicts: [],
    connectionState: "connecting",
    editingWorkItemId: null,
    githubToken: "",
    knowledgeStoreBackend: "embedded",
  };
}

// --- Store ---

type Listener = () => void;

class AppStore {
  private state: AppState = initialState();
  private listeners = new Set<Listener>();

  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getState = (): AppState => this.state;

  private update(partial: Partial<AppState>) {
    this.state = { ...this.state, ...partial };
    for (const listener of this.listeners) {
      listener();
    }
  }

  private setLoading(key: string, value: boolean) {
    this.update({ loadingKeys: { ...this.state.loadingKeys, [key]: value } });
  }

  clearError() {
    this.update({ error: null });
  }

  // --- Low-level setters (used by bootstrap in App.tsx) ---

  setBootstrap(bootstrap: ShellBootstrap) {
    this.update({ bootstrap, bootstrapping: false });
  }

  setSessions(sessions: SessionSummary[]) {
    this.update({ sessions });
  }

  setSelectedProjectId(id: string | null) {
    this.update({ selectedProjectId: id });
  }

  setConnectionEvent(event: ConnectionStatusEvent | null) {
    this.update({ connectionEvent: event });
  }

  setConnectionState(state: AppState["connectionState"]) {
    this.update({ connectionState: state });
  }

  async rebootstrap() {
    try {
      const correlationId = newCorrelationId("rebootstrap");
      const payload = await bootstrapShell(correlationId);

      // Update sessions — replace stale data with fresh
      this.setSessions(payload.sessions);

      // Re-seed session store with fresh snapshots
      for (const session of payload.sessions) {
        sessionStore.seedSession(session.id, {
          runtime_state: session.runtime_state,
          status: session.status,
          activity_state: session.activity_state,
          needs_input_reason: session.needs_input_reason,
          tab_status: null,
          live: session.live,
          title: session.title,
          current_mode: session.current_mode,
          agent_kind: session.agent_kind,
          cwd: session.cwd,
          attached_clients: 0,
        });
      }
      sessionStore.seedComplete();

      // Re-watch live sessions
      const liveIds = payload.sessions.filter((s) => s.live).map((s) => s.id);
      if (liveIds.length > 0) {
        await watchLiveSessions(liveIds, correlationId);
      }

      // Re-load project data if a project is selected
      const projectId = this.state.selectedProjectId;
      if (projectId) {
        void this.loadProjectReads(projectId, true);
      }

      this.setConnectionState("connected");
      this.clearError();
    } catch (err) {
      this.update({ error: `Reconnect failed: ${String(err)}` });
      this.setConnectionState("disconnected");
    }
  }

  setError(error: string) {
    this.update({ error });
  }

  setFocusProjectIds(ids: string[]) {
    this.update({ focusProjectIds: ids });
  }

  setMaxFocusSlots(max: number) {
    const clamped = Math.max(1, Math.min(5, max));
    let focus = this.state.focusProjectIds;
    if (focus.length > clamped) {
      focus = focus.slice(0, clamped);
    }
    this.update({ maxFocusSlots: clamped, focusProjectIds: focus });
  }

  pinProject(projectId: string) {
    if (this.state.focusProjectIds.includes(projectId)) return;
    if (this.state.focusProjectIds.length >= this.state.maxFocusSlots) {
      this.update({ error: `Focus full — unpin a project first (max ${this.state.maxFocusSlots})` });
      return;
    }
    this.update({ focusProjectIds: [...this.state.focusProjectIds, projectId] });
  }

  unpinProject(projectId: string) {
    this.update({
      focusProjectIds: this.state.focusProjectIds.filter((id) => id !== projectId),
    });
  }

  reorderFocus(orderedIds: string[]) {
    const valid = orderedIds.filter((id) => this.state.focusProjectIds.includes(id));
    this.update({ focusProjectIds: valid });
  }

  setShowCreateForm(mode: false | "work" | "doc") {
    this.update({ showCreateForm: mode });
  }

  setWorkItemCreateForm(form: AppState["workItemCreateForm"]) {
    this.update({ workItemCreateForm: form });
  }

  setEditingWorkItemId(id: string | null) {
    this.update({ editingWorkItemId: id });
  }

  setDocumentCreateForm(form: AppState["documentCreateForm"]) {
    this.update({ documentCreateForm: form });
  }

  // --- Apply helpers ---

  applyProjectDetail(detail: ProjectDetail) {
    const s = this.state;
    const projectDetails = { ...s.projectDetails, [detail.id]: detail };
    let bootstrap = s.bootstrap;
    if (bootstrap) {
      const currentSummary = bootstrap.projects.find((p) => p.id === detail.id) ?? null;
      const nextProjects = upsertById(bootstrap.projects, toProjectSummary(detail, currentSummary), compareProjects);
      bootstrap = {
        ...bootstrap,
        projects: nextProjects,
        bootstrap: { ...bootstrap.bootstrap, project_count: nextProjects.length },
      };
    }
    this.update({ projectDetails, bootstrap });
  }

  applyWorkItemDetail(detail: WorkItemDetail) {
    const s = this.state;
    this.update({
      workItemDetails: { ...s.workItemDetails, [detail.id]: detail },
      workItemsByProject: {
        ...s.workItemsByProject,
        [detail.project_id]: upsertById(s.workItemsByProject[detail.project_id] ?? [], detail, compareWorkItems),
      },
    });
  }

  applyDocumentDetail(detail: DocumentDetail) {
    const s = this.state;
    this.update({
      documentDetails: { ...s.documentDetails, [detail.id]: detail },
      documentsByProject: {
        ...s.documentsByProject,
        [detail.project_id]: upsertById(s.documentsByProject[detail.project_id] ?? [], detail, compareDocuments),
      },
    });
  }

  applyProposal(workItemId: string, proposal: WorkflowReconciliationProposalSummary) {
    const s = this.state;
    this.update({
      reconciliationByWorkItem: {
        ...s.reconciliationByWorkItem,
        [workItemId]: upsertById(s.reconciliationByWorkItem[workItemId] ?? [], proposal, compareProposals),
      },
    });
  }

  applySessionDetail(detail: SessionDetail) {
    this.update({
      sessions: upsertById(this.state.sessions, sessionSummaryFromDetail(detail), compareSessions),
    });
  }

  applySessionStateChange(payload: SessionStateChangedEvent) {
    const s = this.state;
    const index = s.sessions.findIndex((entry) => entry.id === payload.session_id);
    if (index === -1) return;
    const entry = s.sessions[index];
    if (
      entry.runtime_state === payload.runtime_state &&
      entry.status === payload.status &&
      entry.activity_state === payload.activity_state &&
      entry.needs_input_reason === payload.needs_input_reason &&
      entry.live === payload.live
    ) {
      // No visible change — still update the external session store but skip re-render
      sessionStore.updateSession(payload.session_id, {
        runtime_state: payload.runtime_state,
        status: payload.status,
        activity_state: payload.activity_state,
        needs_input_reason: payload.needs_input_reason,
        tab_status: payload.tab_status ?? null,
        live: payload.live,
        attached_clients: payload.attached_clients,
      });
      return;
    }
    const next = [...s.sessions];
    next[index] = {
      ...entry,
      runtime_state: payload.runtime_state,
      status: payload.status,
      activity_state: payload.activity_state,
      needs_input_reason: payload.needs_input_reason,
      last_output_at: payload.last_output_at,
      last_attached_at: payload.last_attached_at,
      updated_at: payload.updated_at,
      live: payload.live,
    };
    this.update({ sessions: next });
    sessionStore.updateSession(payload.session_id, {
      runtime_state: payload.runtime_state,
      status: payload.status,
      activity_state: payload.activity_state,
      needs_input_reason: payload.needs_input_reason,
      tab_status: payload.tab_status ?? null,
      live: payload.live,
      attached_clients: payload.attached_clients,
    });

    // Session ended: release per-session memory in session-store
    if (entry.live && !payload.live) {
      sessionStore.onSessionEnded(payload.session_id);

      // DING when an agent finishes its work
      if (payload.runtime_state === "exited") {
        playCompletionDing(); // baking timer — agent done
        toastStore.addToast({
          type: "success",
          message: `${entry.title || payload.session_id} completed`,
          action: entry.project_id
            ? {
                label: "View Terminal",
                onClick: () => navStore.goToAgent(entry.project_id!, payload.session_id),
              }
            : undefined,
        });
      } else {
        playErrorChime(); // low tone — something went wrong
        toastStore.addToast({
          type: "error",
          message: `${entry.title || payload.session_id} errored`,
          action: entry.project_id
            ? {
                label: "View Terminal",
                onClick: () => navStore.goToAgent(entry.project_id!, payload.session_id),
              }
            : undefined,
        });
      }

    }
  }

  // --- Data loading ---

  async loadProjectReads(projectId: string, force = false) {
    const s = this.state;
    if (
      !force &&
      s.projectDetails[projectId] &&
      s.workItemsByProject[projectId] &&
      s.documentsByProject[projectId]
    ) {
      return;
    }

    const correlationId = newCorrelationId("project-load");
    this.setLoading(`project:${projectId}`, true);
    try {
      const [projectDetail, workItems, documents] = await Promise.all([
        getProject(projectId, correlationId),
        listWorkItems(projectId, correlationId),
        listDocuments(projectId, undefined, correlationId),
      ]);
      this.applyProjectDetail(projectDetail);
      this.update({
        workItemsByProject: { ...this.state.workItemsByProject, [projectId]: workItems },
        documentsByProject: { ...this.state.documentsByProject, [projectId]: documents },
      });
      recordClientEvent(
        makeClientEvent("shell", "project.reads_loaded", {
          correlation_id: correlationId,
          project_id: projectId,
          payload: {
            work_item_count: workItems.length,
            document_count: documents.length,
            force_refresh: force,
          },
        }),
      );
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`project:${projectId}`, false);
    }
  }

  async ensureWorkItemDetail(workItemId: string, force = false) {
    if (!force && this.state.workItemDetails[workItemId]) {
      return;
    }

    const correlationId = newCorrelationId("work-item");
    this.setLoading(`work-item:${workItemId}`, true);
    try {
      const detail = await getWorkItem(workItemId, correlationId);
      this.applyWorkItemDetail(detail);
      recordClientEvent(
        makeClientEvent("workbench", "work_item.loaded", {
          correlation_id: correlationId,
          project_id: detail.project_id,
          work_item_id: workItemId,
        }),
      );
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`work-item:${workItemId}`, false);
    }
  }

  async ensureDocumentDetail(documentId: string, force = false) {
    if (!force && this.state.documentDetails[documentId]) {
      return;
    }

    const correlationId = newCorrelationId("document");
    this.setLoading(`document:${documentId}`, true);
    try {
      const detail = await getDocument(documentId, correlationId);
      this.applyDocumentDetail(detail);
      recordClientEvent(
        makeClientEvent("workbench", "document.loaded", {
          correlation_id: correlationId,
          project_id: detail.project_id,
          work_item_id: detail.work_item_id ?? undefined,
          payload: { document_id: documentId },
        }),
      );
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`document:${documentId}`, false);
    }
  }

  async ensureReconciliationProposals(workItemId: string, force = false) {
    if (!force && this.state.reconciliationByWorkItem[workItemId]) {
      return;
    }

    const correlationId = newCorrelationId("proposal");
    this.setLoading(`proposal:${workItemId}`, true);
    try {
      const proposals = await listWorkflowReconciliationProposals(workItemId, correlationId);
      this.update({
        reconciliationByWorkItem: { ...this.state.reconciliationByWorkItem, [workItemId]: proposals },
      });
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`proposal:${workItemId}`, false);
    }
  }

  async fetchWorkItemBundle(projectId: string, workItemId: string) {
    const correlationId = newCorrelationId("work-item-bundle");
    this.setLoading(`work-item-bundle:${workItemId}`, true);
    try {
      const [workItem, docs] = await Promise.all([
        getWorkItem(workItemId, correlationId),
        listDocuments(projectId, workItemId, correlationId),
      ]);

      this.applyWorkItemDetail(workItem);

      // Merge linked documents into existing project documents
      const allProjectDocs = this.state.documentsByProject[projectId] ?? [];
      const updatedDocs = [...allProjectDocs];
      for (const doc of docs) {
        if (!updatedDocs.some((d) => d.id === doc.id)) {
          updatedDocs.push(doc);
        }
      }
      this.update({
        documentsByProject: {
          ...this.state.documentsByProject,
          [projectId]: updatedDocs.sort(compareDocuments),
        },
      });
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`work-item-bundle:${workItemId}`, false);
    }
  }

  // --- Session helpers ---

  private async reconcileSessionState(sessionId: string) {
    try {
      const detail = await getSession(sessionId, newCorrelationId("session-reconcile"));
      this.applySessionDetail(detail);
      this.clearError();
    } catch {
      // ignore
    }
  }

  private async handleSessionAction<T>(sessionId: string, action: () => Promise<T>) {
    const session = this.state.sessions.find((entry) => entry.id === sessionId) ?? null;
    if (!session?.live) {
      void this.reconcileSessionState(sessionId);
      return null;
    }

    try {
      const result = await action();
      this.clearError();
      return result;
    } catch (invokeError) {
      const message = String(invokeError);
      if (message.includes("session_not_live")) {
        await this.reconcileSessionState(sessionId);
        return null;
      }
      this.update({ error: message });
      return null;
    }
  }

  // --- Action handlers ---

  async handleInterruptSession(sessionId: string) {
    await this.handleSessionAction(sessionId, () =>
      interruptSession(sessionId, newCorrelationId("session-interrupt")),
    );
  }

  async handleTerminateSession(sessionId: string) {
    const result = await this.handleSessionAction(sessionId, () =>
      terminateSession(sessionId, newCorrelationId("session-terminate")),
    );
    if (result !== null) {
      window.setTimeout(() => {
        void this.reconcileSessionState(sessionId);
      }, 250);
    }
  }

  async handleCreateProject(name: string, folderPath: string, initGit = false, projectType?: string | null, wcpNamespace?: string | null): Promise<string | null> {
    const correlationId = newCorrelationId("project-create");
    try {
      const project = await createProject({ name, project_type: projectType ?? null, wcp_namespace: wcpNamespace ?? null }, correlationId);
      const root = await createProjectRoot(
        {
          project_id: project.id,
          label: name,
          path: folderPath,
          root_kind: "repo",
        },
        correlationId,
      ) as { id: string };
      if (initGit) {
        try {
          await gitInitProjectRoot(root.id, correlationId);
        } catch {
          // git init failure is non-fatal — project is created
        }
      }
      await this.rebootstrap();
      return project.id;
    } catch (err) {
      this.update({ error: String(err) });
      return null;
    }
  }

  async handleUpdateProject(projectId: string, input: { name: string; slug: string; default_account_id: string }) {
    const correlationId = newCorrelationId("project-update");
    this.setLoading(`save-project:${projectId}`, true);
    try {
      const detail = await updateProject(
        projectId,
        { name: input.name, slug: input.slug, default_account_id: input.default_account_id || null },
        correlationId,
      );
      this.applyProjectDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`save-project:${projectId}`, false);
    }
  }

  async handleUpdateProjectName(projectId: string, name: string) {
    const correlationId = newCorrelationId("project-rename");
    this.setLoading(`save-project-name:${projectId}`, true);
    try {
      const detail = await updateProject(projectId, { name }, correlationId);
      this.applyProjectDetail(detail);
      await this.rebootstrap();
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`save-project-name:${projectId}`, false);
    }
  }

  async handleAddProjectRoot(projectId: string, label: string, path: string) {
    const correlationId = newCorrelationId("project-root-add");
    this.setLoading(`add-project-root:${projectId}`, true);
    try {
      await createProjectRoot(
        {
          project_id: projectId,
          label,
          path,
          root_kind: "repo",
        },
        correlationId,
      );
      const detail = await getProject(projectId, correlationId);
      this.applyProjectDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`add-project-root:${projectId}`, false);
    }
  }

  async handleRemoveProjectRoot(projectId: string, rootId: string) {
    const correlationId = newCorrelationId("project-root-remove");
    this.setLoading(`remove-project-root:${rootId}`, true);
    try {
      await removeProjectRoot(rootId, correlationId);
      const detail = await getProject(projectId, correlationId);
      this.applyProjectDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`remove-project-root:${rootId}`, false);
    }
  }

  async handleGitInitProjectRoot(projectId: string, rootId: string) {
    const correlationId = newCorrelationId("project-root-git-init");
    this.setLoading(`git-init-project-root:${rootId}`, true);
    try {
      await gitInitProjectRoot(rootId, correlationId);
      const detail = await getProject(projectId, correlationId);
      this.applyProjectDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`git-init-project-root:${rootId}`, false);
    }
  }

  async handleSetProjectRootRemote(projectId: string, rootId: string, remoteUrl: string) {
    const correlationId = newCorrelationId("project-root-set-remote");
    this.setLoading(`set-project-root-remote:${rootId}`, true);
    try {
      await setProjectRootRemote(rootId, remoteUrl, correlationId);
      const detail = await getProject(projectId, correlationId);
      this.applyProjectDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`set-project-root-remote:${rootId}`, false);
    }
  }

  async handleArchiveProject(projectId: string) {
    const correlationId = newCorrelationId("project-archive");
    this.setLoading(`archive-project:${projectId}`, true);
    try {
      await archiveProject(projectId, correlationId);
      await this.rebootstrap();
      navStore.goHome();
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`archive-project:${projectId}`, false);
    }
  }

  async handleDeleteProject(projectId: string) {
    const correlationId = newCorrelationId("project-delete");
    this.setLoading(`delete-project:${projectId}`, true);
    try {
      await deleteProject(projectId, correlationId);
      await this.rebootstrap();
      navStore.goHome();
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`delete-project:${projectId}`, false);
    }
  }

  async handleCreateWorkItem(): Promise<string | null> {
    const s = this.state;
    if (!s.selectedProjectId) {
      this.update({ error: "Select a project before creating a work item." });
      return null;
    }
    const correlationId = newCorrelationId("work-item-create");
    this.setLoading("create-work-item", true);
    try {
      const detail = await createWorkItem(
        {
          project_id: s.selectedProjectId,
          parent_id: s.workItemCreateForm.parent_id || null,
          title: s.workItemCreateForm.title,
          description: s.workItemCreateForm.description,
          acceptance_criteria: s.workItemCreateForm.acceptance_criteria || null,
          work_item_type: s.workItemCreateForm.work_item_type,
          status: s.workItemCreateForm.status,
          priority: s.workItemCreateForm.priority || null,
        },
        correlationId,
      );
      this.applyWorkItemDetail(detail);
      this.update({
        showCreateForm: false,
        workItemCreateForm: {
          title: "",
          description: "",
          acceptance_criteria: "",
          work_item_type: "task",
          status: "backlog",
          priority: "",
          parent_id: "",
        },
      });
      this.clearError();
      return detail.id;
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
      return null;
    } finally {
      this.setLoading("create-work-item", false);
    }
  }

  async handleCreateChildWorkItem(
    projectId: string,
    parentId: string,
    input: {
      title: string;
      description: string;
      acceptance_criteria?: string | null;
      work_item_type: string;
      status: string;
      priority?: string | null;
    },
  ): Promise<void> {
    const correlationId = newCorrelationId("work-item-create-child");
    this.setLoading(`create-child-work-item:${parentId}`, true);
    try {
      const detail = await createWorkItem(
        {
          project_id: projectId,
          parent_id: parentId,
          title: input.title,
          description: input.description,
          acceptance_criteria: input.acceptance_criteria || null,
          work_item_type: input.work_item_type,
          status: input.status,
          priority: input.priority || null,
        },
        correlationId,
      );
      this.applyWorkItemDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`create-child-work-item:${parentId}`, false);
    }
  }

  async handleUpdateWorkItem(
    workItemId: string,
    input: {
      title: string;
      description: string;
      acceptance_criteria?: string | null;
      work_item_type: string;
      status: string;
      priority?: string | null;
    },
  ) {
    const correlationId = newCorrelationId("work-item-update");
    this.setLoading(`save-work-item:${workItemId}`, true);
    try {
      const detail = await updateWorkItem(workItemId, input, correlationId);
      this.applyWorkItemDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`save-work-item:${workItemId}`, false);
    }
  }

  async handleLaunchSessionFromWorkItem(workItemId: string) {
    const s = this.state;
    if (!s.selectedProjectId) {
      this.update({ error: "Select a project before starting a session." });
      return;
    }

    // Pre-fetch project + work item so the dispatch sheet has data, then show it
    const correlationId = newCorrelationId("work-item-session");
    try {
      const project =
        s.projectDetails[s.selectedProjectId] ?? (await getProject(s.selectedProjectId, correlationId));
      this.applyProjectDetail(project);

      const workItem =
        s.workItemDetails[workItemId] ?? (await getWorkItem(workItemId, correlationId));
      this.applyWorkItemDetail(workItem);
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
      return;
    }

    this.update({ pendingDispatch: { mode: "single", workItemId, projectId: s.selectedProjectId, originMode: "execution" } });
    navStore.openModal({ modal: "dispatch_single", projectId: s.selectedProjectId, workItemId, originMode: "execution" });
  }

  cancelDispatch() {
    this.update({ pendingDispatch: null });
  }

  async handleLaunchDispatcher(projectId: string) {
    const loadKey = `dispatch:${projectId}`;
    this.setLoading(loadKey, true);
    const correlationId = newCorrelationId("launch-dispatcher");
    try {
      const project =
        this.state.projectDetails[projectId] ?? (await getProject(projectId, correlationId));
      this.applyProjectDetail(project);

      const account =
        this.state.bootstrap?.accounts.find((a) => a.id === project.default_account_id) ??
        this.state.bootstrap?.accounts[0] ??
        null;
      if (!account) {
        throw new Error("No account configured. Add one in Settings first.");
      }

      const root = project.roots[0] ?? null;
      if (!root) {
        throw new Error("Project needs at least one root before launching.");
      }

      const detail = await createSession(
        {
          project_id: projectId,
          project_root_id: root.id,
          worktree_id: null,
          work_item_id: null,
          account_id: account.id,
          agent_kind: account.agent_kind,
          cwd: root.path,
          command: account.binary_path ?? account.agent_kind,
          args: [],
          env_preset_ref: account.env_preset_ref,
          origin_mode: "dispatch",
          current_mode: "dispatch",
          title: `${project.name} — Dispatcher`,
          title_policy: "manual",
          restore_policy: "reattach",
          initial_terminal_cols: 120,
          initial_terminal_rows: 40,
          auto_worktree: false,
          safety_mode: "yolo",
          model: "opus",
        },
        correlationId,
      );

      this.applySessionDetail(detail);
      await watchLiveSessions([detail.id], correlationId);
      this.clearError();

      toastStore.addToast({
        type: "success",
        message: `Dispatcher launched for ${project.name}`,
      });
    } catch (err) {
      this.update({ error: String(err) });
    } finally {
      this.setLoading(loadKey, false);
    }
  }

  toggleWorkItemSelection(workItemId: string) {
    const current = this.state.selectedWorkItemIds;
    const next = current.includes(workItemId)
      ? current.filter((id) => id !== workItemId)
      : [...current, workItemId];
    this.update({ selectedWorkItemIds: next });
  }

  clearWorkItemSelection() {
    this.update({ selectedWorkItemIds: [] });
  }

  async handleMultiDispatch(projectId: string) {
    const ids = this.state.selectedWorkItemIds;
    if (ids.length === 0) return;

    const correlationId = newCorrelationId("conflict-check");
    try {
      const result = await checkDispatchConflicts(ids, correlationId);
      this.update({
        dispatchConflicts: result.warnings,
        pendingDispatch: { mode: "multi", workItemIds: ids, projectId },
      });
      navStore.openModal({ modal: "dispatch_multi", projectId, workItemIds: ids });
    } catch (err) {
      this.update({ error: String(err) });
    }
  }

  async confirmMultiDispatch(
    dispatches: Array<{ workItemId: string; accountId: string; agentKind: string; binaryPath?: string | null; safetyMode?: string; model?: string }>,
  ) {
    const pending = this.state.pendingDispatch;
    if (!pending || pending.mode !== "multi") return;

    const projectDetail = this.state.projectDetails[pending.projectId];
    if (!projectDetail) return;
    const root = projectDetail.roots[0];
    if (!root) return;

    const correlationId = newCorrelationId("batch-dispatch");
    try {
      const requests = dispatches.map((d) => ({
        project_id: pending.projectId,
        project_root_id: root.id,
        work_item_id: d.workItemId,
        account_id: d.accountId,
        agent_kind: d.agentKind,
        cwd: root.path,
        command: d.binaryPath ?? d.agentKind,
        origin_mode: "dispatch" as const,
        auto_worktree: true,
        safety_mode: d.safetyMode,
        model: d.model,
      }));

      const sessions = await createSessionBatch(requests, correlationId);

      for (const session of sessions) {
        this.applySessionDetail(session);
      }

      this.update({
        pendingDispatch: null,
        selectedWorkItemIds: [],
        dispatchConflicts: [],
      });

      toastStore.addToast({
        type: "success",
        message: `${sessions.length} agent${sessions.length === 1 ? "" : "s"} launched`,
      });

      await watchLiveSessions(
        sessions.filter((s) => s.live).map((s) => s.id),
        correlationId,
      );
    } catch (err) {
      toastStore.addToast({
        type: "error",
        message: `Launch failed: ${String(err)}`,
      });
      this.update({ error: String(err) });
    }
  }

  async confirmDispatch(opts: { autoWorktree: boolean; originMode: string; safetyMode?: string; model?: string }) {
    const dispatch = this.state.pendingDispatch;
    if (!dispatch || dispatch.mode !== "single") return;

    const { workItemId, projectId } = dispatch;
    this.update({ pendingDispatch: null });

    const correlationId = newCorrelationId("work-item-session");
    this.setLoading(`launch-session:${workItemId}`, true);
    try {
      const s = this.state;
      const project = s.projectDetails[projectId];
      if (!project) throw new Error("Project detail not loaded.");

      const workItem = s.workItemDetails[workItemId];
      if (!workItem) throw new Error("Work item detail not loaded.");

      const account =
        s.bootstrap?.accounts.find((entry) => entry.id === project.default_account_id) ??
        s.bootstrap?.accounts[0] ??
        null;
      if (!account) {
        throw new Error("No account is configured for this project yet.");
      }

      const root = project.roots[0] ?? null;
      if (!root) {
        throw new Error("The selected project needs at least one root before launching a session.");
      }

      const detail = await createSession(
        {
          project_id: projectId,
          project_root_id: root.id,
          worktree_id: null,
          work_item_id: workItemId,
          account_id: account.id,
          agent_kind: account.agent_kind,
          cwd: root.path,
          command: account.binary_path ?? account.agent_kind,
          args: [],
          env_preset_ref: account.env_preset_ref,
          origin_mode: opts.originMode,
          current_mode: opts.originMode,
          title: `${workItem.callsign} · ${workItem.title}`,
          title_policy: "manual",
          restore_policy: "reattach",
          initial_terminal_cols: 120,
          initial_terminal_rows: 40,
          auto_worktree: opts.autoWorktree,
          safety_mode: opts.safetyMode,
          model: opts.model,
        },
        correlationId,
      );

      this.applySessionDetail(detail);
      await watchLiveSessions([detail.id], correlationId);
      this.clearError();

      toastStore.addToast({
        type: "success",
        message: `Agent launched for ${workItem.callsign}: ${workItem.title}`,
      });
      navStore.goToAgent(projectId, detail.id);
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`launch-session:${workItemId}`, false);
    }
  }

  async handleCreateDocument() {
    const s = this.state;
    if (!s.selectedProjectId) {
      this.update({ error: "Select a project before creating a document." });
      return;
    }
    const correlationId = newCorrelationId("document-create");
    this.setLoading("create-document", true);
    try {
      const detail = await createDocument(
        {
          project_id: s.selectedProjectId,
          work_item_id: s.documentCreateForm.work_item_id || null,
          doc_type: s.documentCreateForm.doc_type,
          title: s.documentCreateForm.title,
          slug: s.documentCreateForm.slug || undefined,
          status: s.documentCreateForm.status,
          content_markdown: s.documentCreateForm.content_markdown,
        },
        correlationId,
      );
      this.applyDocumentDetail(detail);
      this.update({
        documentCreateForm: {
          title: "",
          slug: "",
          doc_type: "note",
          status: "draft",
          work_item_id: "",
          content_markdown: "",
        },
      });
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading("create-document", false);
    }
  }

  async handleCreateDocumentWithParams(params: {
    project_id: string;
    title: string;
    doc_type: string;
    status: string;
    content_markdown: string;
    work_item_id?: string | null;
  }): Promise<DocumentDetail | null> {
    const correlationId = newCorrelationId("document-create");
    this.setLoading("create-document", true);
    try {
      const detail = await createDocument(params, correlationId);
      this.applyDocumentDetail(detail);
      this.clearError();
      return detail;
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
      return null;
    } finally {
      this.setLoading("create-document", false);
    }
  }

  async handleUpdateDocument(
    documentId: string,
    input: {
      work_item_id?: string | null;
      doc_type: string;
      title: string;
      slug?: string;
      status: string;
      content_markdown: string;
    },
  ) {
    const correlationId = newCorrelationId("document-update");
    this.setLoading(`save-document:${documentId}`, true);
    try {
      const detail = await updateDocument(documentId, input, correlationId);
      this.applyDocumentDetail(detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`save-document:${documentId}`, false);
    }
  }

  async handleDismissProposal(workItemId: string, proposalId: string) {
    const correlationId = newCorrelationId("proposal-dismiss");
    this.setLoading(`proposal-action:${proposalId}`, true);
    try {
      const detail = await updateWorkflowReconciliationProposal(proposalId, { status: "dismissed" }, correlationId);
      this.applyProposal(workItemId, detail);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`proposal-action:${proposalId}`, false);
    }
  }

  async handleApplyProposal(proposal: WorkflowReconciliationProposalSummary) {
    const correlationId = newCorrelationId("proposal-apply");
    this.setLoading(`proposal-action:${proposal.id}`, true);
    try {
      const payload = proposal.proposed_change_payload;
      if (proposal.target_entity_type === "work_item") {
        const workItemId = proposal.target_entity_id ?? proposal.work_item_id;
        if (!workItemId) {
          throw new Error(`Proposal ${proposal.id} does not specify a target work item.`);
        }
        const detail = await updateWorkItem(workItemId, payload, correlationId);
        this.applyWorkItemDetail(detail);
        this.applyProposal(
          proposal.work_item_id ?? workItemId,
          await updateWorkflowReconciliationProposal(proposal.id, { status: "applied" }, correlationId),
        );
      } else if (proposal.target_entity_type === "document") {
        const documentId = proposal.target_entity_id;
        if (!documentId) {
          throw new Error(`Proposal ${proposal.id} does not specify a target document.`);
        }
        const detail = await updateDocument(documentId, payload, correlationId);
        this.applyDocumentDetail(detail);
        this.applyProposal(
          proposal.work_item_id ?? detail.work_item_id ?? documentId,
          await updateWorkflowReconciliationProposal(proposal.id, { status: "applied" }, correlationId),
        );
      } else if (proposal.target_entity_type === "project") {
        const projectId = proposal.target_entity_id;
        if (!projectId) {
          throw new Error(`Proposal ${proposal.id} does not specify a target project.`);
        }
        const detail = await updateProject(projectId, payload, correlationId);
        this.applyProjectDetail(detail);
        this.applyProposal(
          proposal.work_item_id ?? projectId,
          await updateWorkflowReconciliationProposal(proposal.id, { status: "applied" }, correlationId),
        );
      } else {
        throw new Error(
          `Proposal ${proposal.id} targets ${proposal.target_entity_type}, which is outside the first write-capable slice.`,
        );
      }
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`proposal-action:${proposal.id}`, false);
    }
  }

  // --- Merge Queue ---

  async loadGitStatus(projectId: string) {
    const detail = this.state.projectDetails[projectId];
    if (!detail) return;

    const activeRoot = detail.roots.find((r) => r.archived_at === null && r.git_root_path);
    if (!activeRoot) return;

    try {
      const status = await getProjectRootGitStatus(activeRoot.id);
      if (status) {
        this.update({
          gitStatusByRootId: { ...this.state.gitStatusByRootId, [activeRoot.id]: status },
        });
      }
    } catch {
      // Gracefully degrade — git status is informational only
    }
  }

  async handleLoadMergeQueue(projectId: string) {
    this.setLoading("merge-queue", true);
    try {
      const entries = await listMergeQueue(projectId);
      this.update({
        mergeQueueByProject: { ...this.state.mergeQueueByProject, [projectId]: entries },
      });
    } catch (invokeError) {
      // Gracefully degrade if merge queue isn't supported yet (stale supervisor)
      const msg = String(invokeError);
      if (msg.includes("unsupported_operation") || msg.includes("Unsupported method")) {
        this.update({
          mergeQueueByProject: { ...this.state.mergeQueueByProject, [projectId]: [] },
        });
      } else {
        this.update({ error: msg });
      }
    } finally {
      this.setLoading("merge-queue", false);
    }
  }

  async handleMergeQueueMerge(entryId: string, projectId: string) {
    this.setLoading(`merge:${entryId}`, true);
    try {
      await mergeQueueMerge(entryId);
      await this.handleLoadMergeQueue(projectId);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`merge:${entryId}`, false);
    }
  }

  async handleMergeQueuePark(entryId: string, projectId: string) {
    try {
      await mergeQueuePark(entryId);
      await this.handleLoadMergeQueue(projectId);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    }
  }

  async handleLoadMergeQueueDiff(entryId: string) {
    this.setLoading(`merge-diff:${entryId}`, true);
    try {
      const result = await getMergeQueueDiff(entryId);
      this.update({
        mergeQueueDiffs: { ...this.state.mergeQueueDiffs, [entryId]: result.diff },
      });
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`merge-diff:${entryId}`, false);
    }
  }

  async handleMergeQueueCheckConflicts(entryId: string, projectId: string) {
    this.setLoading(`merge-conflicts:${entryId}`, true);
    try {
      await mergeQueueCheckConflicts(entryId);
      await this.handleLoadMergeQueue(projectId);
      this.clearError();
    } catch (invokeError) {
      this.update({ error: String(invokeError) });
    } finally {
      this.setLoading(`merge-conflicts:${entryId}`, false);
    }
  }

  // --- Derived state helpers ---

  filteredSessions(): SessionSummary[] {
    const s = this.state;
    return s.sessions.filter((entry) => !s.selectedProjectId || entry.project_id === s.selectedProjectId);
  }

  liveSessionCount(): number {
    return this.state.sessions.filter((session) => session.live).length;
  }

  liveSessionsByProject(): Record<string, number> {
    const counts: Record<string, number> = {};
    for (const session of this.state.sessions) {
      if (!session.live) continue;
      counts[session.project_id] = (counts[session.project_id] ?? 0) + 1;
    }
    return counts;
  }

  allCurrentWorkItems(): WorkItemSummary[] {
    const s = this.state;
    return s.selectedProjectId ? s.workItemsByProject[s.selectedProjectId] ?? [] : [];
  }

  currentDocuments(): DocumentSummary[] {
    const s = this.state;
    return s.selectedProjectId ? s.documentsByProject[s.selectedProjectId] ?? [] : [];
  }

  selectedProject(): ProjectSummary | null {
    const s = this.state;
    return s.bootstrap?.projects.find((p) => p.id === s.selectedProjectId) ?? null;
  }

  // --- Account actions ---

  async refreshAccounts() {
    try {
      const accounts = await listAccounts(newCorrelationId("refresh-accounts"));
      if (this.state.bootstrap) {
        this.update({
          bootstrap: {
            ...this.state.bootstrap,
            accounts: accounts as import("./types").AccountSummary[],
          },
        });
      }
    } catch (err) {
      this.update({ error: String(err) });
    }
  }

  async handleCreateAccount(input: { label: string; agent_kind?: string; binary_path?: string | null; config_root?: string | null; is_default?: boolean; default_safety_mode?: string | null }) {
    const key = "create-account";
    this.setLoading(key, true);
    try {
      await createAccount(input, newCorrelationId("create-account"));
      await this.refreshAccounts();
    } catch (err) {
      this.update({ error: String(err) });
    } finally {
      this.setLoading(key, false);
    }
  }

  async handleUpdateAccount(accountId: string, input: { label?: string; binary_path?: string | null; config_root?: string | null; is_default?: boolean; status?: string; default_model?: string | null; default_safety_mode?: string | null; default_launch_args?: string[] | null }) {
    const key = `update-account:${accountId}`;
    this.setLoading(key, true);
    try {
      await updateAccount(accountId, input, newCorrelationId("update-account"));
      await this.refreshAccounts();
    } catch (err) {
      this.update({ error: String(err) });
    } finally {
      this.setLoading(key, false);
    }
  }

  async handleDeleteAccount(accountId: string) {
    const key = `delete-account:${accountId}`;
    this.setLoading(key, true);
    try {
      await updateAccount(accountId, { status: "disabled" }, newCorrelationId("delete-account"));
      await this.refreshAccounts();
    } catch (err) {
      this.update({ error: String(err) });
    } finally {
      this.setLoading(key, false);
    }
  }

  // --- Quick Chat ---

  /**
   * Get or create the scratch "Quick Chats" project used for unscoped sessions.
   * Returns the project ID or null on failure.
   */
  async getOrCreateScratchProject(): Promise<string | null> {
    // Fast path: cached in localStorage
    const cached =
      localStorage.getItem("emery:scratch-project-id") ??
      localStorage.getItem("euri:scratch-project-id");
    if (cached) {
      // Verify it still exists in bootstrap
      const exists = this.state.bootstrap?.projects.find((p) => p.id === cached && p.archived_at === null);
      if (exists) return cached;
      // Stale — clear and re-check
      localStorage.removeItem("emery:scratch-project-id");
      localStorage.removeItem("euri:scratch-project-id");
    }

    // Check if project already exists by name
    const existing = this.state.bootstrap?.projects.find(
      (p) => p.name === "Quick Chats" && p.archived_at === null,
    );
    if (existing) {
      localStorage.setItem("emery:scratch-project-id", existing.id);
      return existing.id;
    }

    // Create it
    try {
      const correlationId = newCorrelationId("quick-chat-project");
      const project = await createProject(
        { name: "Quick Chats", project_type: "scratch" },
        correlationId,
      );
      // Create a placeholder root so sessions have a cwd
      const appDataRoot = this.state.bootstrap?.health.app_data_root ?? ".";
      await createProjectRoot(
        {
          project_id: project.id,
          label: "scratch",
          path: appDataRoot,
          root_kind: "virtual",
        },
        correlationId,
      );
      await this.rebootstrap();
      localStorage.setItem("emery:scratch-project-id", project.id);
      return project.id;
    } catch (err) {
      toastStore.addToast({ type: "error", message: `Failed to create scratch project: ${err}` });
      return null;
    }
  }

  /**
   * Launch a Quick Chat session with the given account.
   * Returns the session ID or null on failure.
   */
  async launchQuickChat(accountId: string): Promise<string | null> {
    this.setLoading("quick-chat", true);
    try {
      const projectId = await this.getOrCreateScratchProject();
      if (!projectId) return null;

      const account = this.state.bootstrap?.accounts.find((a) => a.id === accountId);
      if (!account) {
        toastStore.addToast({ type: "error", message: "Account not found." });
        return null;
      }

      const appDataRoot = this.state.bootstrap?.health.app_data_root ?? ".";
      const correlationId = newCorrelationId("quick-chat-session");

      const detail = await createSession(
        {
          project_id: projectId,
          project_root_id: null,
          worktree_id: null,
          work_item_id: null,
          account_id: account.id,
          agent_kind: account.agent_kind,
          cwd: appDataRoot,
          command: account.binary_path ?? account.agent_kind,
          args: [],
          env_preset_ref: account.env_preset_ref,
          origin_mode: "chat",
          current_mode: "chat",
          title: `Quick Chat ${new Date().toLocaleTimeString()}`,
          title_policy: "manual",
          restore_policy: "reattach",
          initial_terminal_cols: 120,
          initial_terminal_rows: 40,
        },
        correlationId,
      );

      this.applySessionDetail(detail);
      await watchLiveSessions([detail.id], correlationId);
      this.clearError();

      // Navigate to the new session
      navStore.goToAgent(projectId, detail.id);

      toastStore.addToast({ type: "success", message: "Quick Chat started" });
      return detail.id;
    } catch (err) {
      toastStore.addToast({ type: "error", message: `Quick Chat failed: ${err}` });
      return null;
    } finally {
      this.setLoading("quick-chat", false);
    }
  }

  // --- GitHub token ---

  loadGithubToken() {
    const token =
      localStorage.getItem("emery.github_token") ??
      localStorage.getItem("euri.github_token") ??
      "";
    this.update({ githubToken: token });
  }

  saveGithubToken(token: string) {
    localStorage.setItem("emery.github_token", token);
    localStorage.removeItem("euri.github_token");
    this.update({ githubToken: token });
  }

  // --- Knowledge store backend ---

  loadKnowledgeStoreBackend() {
    const raw = localStorage.getItem("emery.knowledge_store_backend") ?? "embedded";
    const backend = raw === "wcp_cloud" ? "wcp_cloud" : "embedded";
    this.update({ knowledgeStoreBackend: backend });
  }

  saveKnowledgeStoreBackend(backend: "embedded" | "wcp_cloud") {
    localStorage.setItem("emery.knowledge_store_backend", backend);
    this.update({ knowledgeStoreBackend: backend });
  }

}

export const appStore = new AppStore();

export function useAppStore<T>(selector: (state: AppState) => T): T {
  const selectorRef = useRef(selector);
  const resultRef = useRef<T>(selector(appStore.getState()));
  selectorRef.current = selector;

  const getSnapshot = useCallback(() => {
    const next = selectorRef.current(appStore.getState());
    // Shallow equality check for arrays and objects to prevent infinite re-renders
    if (resultRef.current === next) return resultRef.current;
    if (
      Array.isArray(next) &&
      Array.isArray(resultRef.current) &&
      next.length === (resultRef.current as unknown[]).length &&
      next.every((v, i) => v === (resultRef.current as unknown[])[i])
    ) {
      return resultRef.current;
    }
    resultRef.current = next;
    return next;
  }, []);

  return useSyncExternalStore(appStore.subscribe, getSnapshot);
}
