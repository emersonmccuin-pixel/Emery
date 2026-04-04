import { useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  attachSession,
  bootstrapShell,
  connectionLabel,
  createPlanningAssignment,
  createDocument,
  createProject,
  createSession,
  createWorkItem,
  deletePlanningAssignment,
  detachSession,
  exportDiagnosticsBundle,
  getProject,
  getDocument,
  getWorkItem,
  interruptSession,
  listDocuments,
  listPlanningAssignments,
  listWorkflowReconciliationProposals,
  listWorkItems,
  saveWorkspace,
  sendSessionInput,
  terminateSession,
  updateDocument,
  updateProject,
  updateWorkflowReconciliationProposal,
  updateWorkItem,
  watchLiveSessions,
} from "./lib";
import {
  configureDiagnostics,
  diagnosticsEnabled,
  makeClientEvent,
  newCorrelationId,
  recordClientEvent,
  snapshotClientDiagnostics,
} from "./diagnostics";
import type {
  AccountSummary,
  ConnectionStatusEvent,
  DiagnosticsBundleResult,
  DocumentDetail,
  DocumentSummary,
  PlanningAssignmentSummary,
  ProjectDetail,
  ProjectSummary,
  SessionAttachResponse,
  SessionDetail,
  SessionOutputEvent,
  SessionStateChangedEvent,
  SessionSummary,
  ShellBootstrap,
  WorkItemDetail,
  WorkItemSummary,
  WorkflowReconciliationProposalSummary,
  WorkspacePayload,
  WorkspaceResource,
} from "./types";
import { DocumentPane, WorkItemPane, documentPreview } from "./workbench";

const WORK_ITEM_TYPES = ["epic", "task", "bug", "feature", "research", "support"] as const;
const WORK_ITEM_STATUSES = [
  "backlog",
  "planned",
  "in_progress",
  "blocked",
  "done",
  "archived",
] as const;
const PRIORITIES = ["", "low", "medium", "high", "urgent"] as const;
const DOCUMENT_STATUSES = ["draft", "active", "archived"] as const;

type PlanningViewMode = "all" | "day" | "week";

function resourceLabel(
  resource: WorkspaceResource,
  sessions: SessionSummary[],
  workItems: Record<string, WorkItemDetail>,
  documents: Record<string, DocumentDetail>,
  projectName?: string,
) {
  if (resource.resource_type === "project_home") {
    return projectName ? `${projectName}` : "Project";
  }
  if (resource.resource_type === "session_terminal") {
    const session = sessions.find((entry) => entry.id === resource.session_id);
    return session?.title ?? session?.current_mode ?? "Session";
  }
  if (resource.resource_type === "work_item_detail") {
    return workItems[resource.work_item_id]?.callsign ?? "Work item";
  }
  if (resource.resource_type === "document_detail") {
    return documents[resource.document_id]?.title ?? "Document";
  }
  return "Resource";
}

function decodeBase64Utf8(base64: string): string {
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function sessionTone(session: Pick<SessionSummary, "runtime_state" | "activity_state" | "needs_input_reason">) {
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

function buildWorkspacePayload(
  selectedProjectId: string | null,
  leftPanel: WorkspacePayload["left_panel"],
  openResources: WorkspaceResource[],
  activeResourceId: string | null,
): WorkspacePayload {
  return {
    version: 1,
    selected_project_id: selectedProjectId,
    left_panel: leftPanel,
    open_resources: openResources,
    active_resource_id: activeResourceId,
  };
}

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

function currentDayCadenceKey(now = new Date()) {
  return now.toISOString().slice(0, 10);
}

function currentWeekCadenceKey(now = new Date()) {
  const date = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
  const day = date.getUTCDay() || 7;
  date.setUTCDate(date.getUTCDate() + 4 - day);
  const yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1));
  const weekNumber = Math.ceil((((date.getTime() - yearStart.getTime()) / 86400000) + 1) / 7);
  return `${date.getUTCFullYear()}-W${String(weekNumber).padStart(2, "0")}`;
}

function planningAssignmentForKey(
  assignments: PlanningAssignmentSummary[],
  workItemId: string,
  cadenceType: "day" | "week",
  cadenceKey: string,
) {
  return (
    assignments.find(
      (assignment) =>
        assignment.work_item_id === workItemId &&
        assignment.removed_at === null &&
        assignment.cadence_type === cadenceType &&
        assignment.cadence_key === cadenceKey,
    ) ?? null
  );
}

function applySessionToList(
  items: SessionSummary[],
  nextSession: SessionSummary,
) {
  return upsertById(items, nextSession, compareSessions);
}

export default function App() {
  const [bootstrap, setBootstrap] = useState<ShellBootstrap | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [leftPanel, setLeftPanel] = useState<WorkspacePayload["left_panel"]>("projects");
  const [planningViewMode, setPlanningViewMode] = useState<PlanningViewMode>("all");
  const [openResources, setOpenResources] = useState<WorkspaceResource[]>([]);
  const [activeResourceId, setActiveResourceId] = useState<string | null>(null);
  const [attachedSessions, setAttachedSessions] = useState<Record<string, SessionAttachResponse>>({});
  const [terminalOutput, setTerminalOutput] = useState<Record<string, string>>({});
  const [terminalInput, setTerminalInput] = useState<Record<string, string>>({});
  const [projectDetails, setProjectDetails] = useState<Record<string, ProjectDetail>>({});
  const [workItemsByProject, setWorkItemsByProject] = useState<Record<string, WorkItemSummary[]>>({});
  const [documentsByProject, setDocumentsByProject] = useState<Record<string, DocumentSummary[]>>({});
  const [planningAssignmentsByProject, setPlanningAssignmentsByProject] = useState<
    Record<string, PlanningAssignmentSummary[]>
  >({});
  const [workItemDetails, setWorkItemDetails] = useState<Record<string, WorkItemDetail>>({});
  const [documentDetails, setDocumentDetails] = useState<Record<string, DocumentDetail>>({});
  const [reconciliationByWorkItem, setReconciliationByWorkItem] = useState<
    Record<string, WorkflowReconciliationProposalSummary[]>
  >({});
  const [connectionEvent, setConnectionEvent] = useState<ConnectionStatusEvent | null>(null);
  const [diagnosticsBundle, setDiagnosticsBundle] = useState<DiagnosticsBundleResult | null>(null);
  const [loadingKeys, setLoadingKeys] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [showProjectCreate, setShowProjectCreate] = useState(false);
  const [projectCreateForm, setProjectCreateForm] = useState({
    name: "",
    slug: "",
    default_account_id: "",
  });
  const [workItemCreateForm, setWorkItemCreateForm] = useState({
    title: "",
    description: "",
    acceptance_criteria: "",
    work_item_type: "task",
    status: "backlog",
    priority: "",
    parent_id: "",
  });
  const [documentCreateForm, setDocumentCreateForm] = useState({
    title: "",
    slug: "",
    doc_type: "note",
    status: "draft",
    work_item_id: "",
    content_markdown: "",
  });
  const restoreApplied = useRef(false);
  const persistTimeout = useRef<number | null>(null);

  function setLoading(key: string, value: boolean) {
    setLoadingKeys((current) => ({ ...current, [key]: value }));
  }

  function clearError() {
    setError(null);
  }

  function applyProjectDetail(detail: ProjectDetail) {
    setProjectDetails((current) => ({ ...current, [detail.id]: detail }));
    setBootstrap((current) => {
      if (!current) {
        return current;
      }
      const currentSummary = current.projects.find((project) => project.id === detail.id) ?? null;
      const nextProjects = upsertById(current.projects, toProjectSummary(detail, currentSummary), compareProjects);
      return {
        ...current,
        projects: nextProjects,
        bootstrap: {
          ...current.bootstrap,
          project_count: nextProjects.length,
        },
      };
    });
  }

  function applyWorkItemDetail(detail: WorkItemDetail) {
    setWorkItemDetails((current) => ({ ...current, [detail.id]: detail }));
    setWorkItemsByProject((current) => ({
      ...current,
      [detail.project_id]: upsertById(current[detail.project_id] ?? [], detail, compareWorkItems),
    }));
  }

  function applyDocumentDetail(detail: DocumentDetail) {
    setDocumentDetails((current) => ({ ...current, [detail.id]: detail }));
    setDocumentsByProject((current) => ({
      ...current,
      [detail.project_id]: upsertById(current[detail.project_id] ?? [], detail, compareDocuments),
    }));
  }

  function applyProposal(workItemId: string, proposal: WorkflowReconciliationProposalSummary) {
    setReconciliationByWorkItem((current) => ({
      ...current,
      [workItemId]: upsertById(current[workItemId] ?? [], proposal, compareProposals),
    }));
  }

  function applySessionDetail(detail: SessionDetail) {
    setSessions((current) => applySessionToList(current, detail));
  }

  function applyPlanningAssignment(projectId: string, assignment: PlanningAssignmentSummary) {
    setPlanningAssignmentsByProject((current) => ({
      ...current,
      [projectId]: upsertById(
        current[projectId] ?? [],
        assignment,
        (left, right) => right.updated_at - left.updated_at || left.id.localeCompare(right.id),
      ),
    }));
  }

  function removePlanningAssignment(projectId: string, planningAssignmentId: string) {
    setPlanningAssignmentsByProject((current) => ({
      ...current,
      [projectId]: (current[projectId] ?? []).filter((assignment) => assignment.id !== planningAssignmentId),
    }));
  }

  async function loadProjectReads(projectId: string, force = false) {
    if (
      !force &&
      projectDetails[projectId] &&
      workItemsByProject[projectId] &&
      documentsByProject[projectId] &&
      planningAssignmentsByProject[projectId]
    ) {
      return;
    }

    const correlationId = newCorrelationId("project-load");
    setLoading(`project:${projectId}`, true);
    try {
      const [projectDetail, workItems, documents, planningAssignments] = await Promise.all([
        getProject(projectId, correlationId),
        listWorkItems(projectId, correlationId),
        listDocuments(projectId, undefined, correlationId),
        listPlanningAssignments(projectId, undefined, correlationId),
      ]);
      applyProjectDetail(projectDetail);
      setWorkItemsByProject((current) => ({ ...current, [projectId]: workItems }));
      setDocumentsByProject((current) => ({ ...current, [projectId]: documents }));
      setPlanningAssignmentsByProject((current) => ({ ...current, [projectId]: planningAssignments }));
      recordClientEvent(
        makeClientEvent("shell", "project.reads_loaded", {
          correlation_id: correlationId,
          project_id: projectId,
          payload: {
            work_item_count: workItems.length,
            document_count: documents.length,
            planning_assignment_count: planningAssignments.length,
            force_refresh: force,
          },
        }),
      );
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`project:${projectId}`, false);
    }
  }

  async function ensureWorkItemDetail(workItemId: string, force = false) {
    if (!force && workItemDetails[workItemId]) {
      return;
    }

    const correlationId = newCorrelationId("work-item");
    setLoading(`work-item:${workItemId}`, true);
    try {
      const detail = await getWorkItem(workItemId, correlationId);
      applyWorkItemDetail(detail);
      recordClientEvent(
        makeClientEvent("workbench", "work_item.loaded", {
          correlation_id: correlationId,
          project_id: detail.project_id,
          work_item_id: workItemId,
        }),
      );
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`work-item:${workItemId}`, false);
    }
  }

  async function ensureDocumentDetail(documentId: string, force = false) {
    if (!force && documentDetails[documentId]) {
      return;
    }

    const correlationId = newCorrelationId("document");
    setLoading(`document:${documentId}`, true);
    try {
      const detail = await getDocument(documentId, correlationId);
      applyDocumentDetail(detail);
      recordClientEvent(
        makeClientEvent("workbench", "document.loaded", {
          correlation_id: correlationId,
          project_id: detail.project_id,
          work_item_id: detail.work_item_id ?? undefined,
          payload: {
            document_id: documentId,
          },
        }),
      );
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`document:${documentId}`, false);
    }
  }

  async function ensureReconciliationProposals(workItemId: string, force = false) {
    if (!force && reconciliationByWorkItem[workItemId]) {
      return;
    }

    const correlationId = newCorrelationId("proposal");
    setLoading(`proposal:${workItemId}`, true);
    try {
      const proposals = await listWorkflowReconciliationProposals(workItemId, correlationId);
      setReconciliationByWorkItem((current) => ({ ...current, [workItemId]: proposals }));
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`proposal:${workItemId}`, false);
    }
  }

  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function start() {
      try {
        const correlationId = newCorrelationId("bootstrap");
        const payload = await bootstrapShell(correlationId);
        if (cancelled) {
          return;
        }

        configureDiagnostics(payload.hello.diagnostics_enabled);
        setBootstrap(payload);
        setSessions(payload.sessions);
        recordClientEvent(
          makeClientEvent("shell", "bootstrap.completed", {
            correlation_id: correlationId,
            payload: {
              diagnostics_enabled: payload.hello.diagnostics_enabled,
              project_count: payload.projects.length,
              session_count: payload.sessions.length,
            },
          }),
        );
        const restored = payload.workspace?.payload;
        if (restored && !restoreApplied.current) {
          restoreApplied.current = true;
          setSelectedProjectId(restored.selected_project_id ?? payload.projects[0]?.id ?? null);
          setLeftPanel(restored.left_panel ?? "projects");
          setOpenResources(restored.open_resources ?? []);
          setActiveResourceId(restored.active_resource_id ?? restored.open_resources[0]?.resource_id ?? null);
        } else {
          restoreApplied.current = true;
          const firstProjectId = payload.projects[0]?.id ?? null;
          setSelectedProjectId(firstProjectId);
          if (firstProjectId) {
            const homeResource = {
              resource_type: "project_home" as const,
              project_id: firstProjectId,
              resource_id: `project_home:${firstProjectId}`,
            };
            setOpenResources([homeResource]);
            setActiveResourceId(homeResource.resource_id);
          }
        }

        await watchLiveSessions(
          payload.sessions.filter((entry) => entry.live).map((entry) => entry.id),
          correlationId,
        );
      } catch (invokeError) {
        setError(String(invokeError));
      }
    }

    void start();

    const listeners = Promise.all([
      listen<ConnectionStatusEvent>("supervisor://connection", (event) => {
        setConnectionEvent(event.payload);
        recordClientEvent(
          makeClientEvent("shell", "connection.status_changed", {
            payload: {
              state: event.payload.state,
              detail: event.payload.detail,
            },
          }),
        );
      }),
      listen<{ event: string; payload: SessionOutputEvent | SessionStateChangedEvent }>(
        "supervisor://event",
        (event) => {
          if (event.payload.event === "session.output") {
            const payload = event.payload.payload as SessionOutputEvent;
            setTerminalOutput((current) => ({
              ...current,
              [payload.session_id]: `${current[payload.session_id] ?? ""}${decodeBase64Utf8(payload.data)}`,
            }));
            return;
          }

          if (event.payload.event === "session.state_changed") {
            const payload = event.payload.payload as SessionStateChangedEvent;
            setSessions((current) =>
              current.map((entry) =>
                entry.id === payload.session_id
                  ? {
                      ...entry,
                      runtime_state: payload.runtime_state,
                      status: payload.status,
                      activity_state: payload.activity_state,
                      needs_input_reason: payload.needs_input_reason,
                      last_output_at: payload.last_output_at,
                      last_attached_at: payload.last_attached_at,
                      updated_at: payload.updated_at,
                      live: payload.live,
                    }
                  : entry,
              ),
            );
            setAttachedSessions((current) => {
              const existing = current[payload.session_id];
              if (!existing) {
                return current;
              }
              return {
                ...current,
                [payload.session_id]: {
                  ...existing,
                  session: {
                    ...existing.session,
                    runtime_state: payload.runtime_state,
                    status: payload.status,
                    activity_state: payload.activity_state,
                    needs_input_reason: payload.needs_input_reason,
                    last_output_at: payload.last_output_at,
                    last_attached_at: payload.last_attached_at,
                    updated_at: payload.updated_at,
                    live: payload.live,
                    runtime: existing.session.runtime
                      ? {
                          ...existing.session.runtime,
                          attached_clients: payload.attached_clients,
                          updated_at: payload.updated_at,
                          runtime_state: payload.runtime_state,
                        }
                      : null,
                  },
                },
              };
            });
          }
        },
      ),
    ]);

    void listeners.then((items) => {
      unlisteners.push(...items);
    });

    return () => {
      cancelled = true;
      if (persistTimeout.current) {
        window.clearTimeout(persistTimeout.current);
      }
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    if (!selectedProjectId) {
      return;
    }
    void loadProjectReads(selectedProjectId);
  }, [selectedProjectId]);

  useEffect(() => {
    if (!bootstrap || !restoreApplied.current) {
      return;
    }
    if (persistTimeout.current) {
      window.clearTimeout(persistTimeout.current);
    }
    persistTimeout.current = window.setTimeout(() => {
      void saveWorkspace(
        buildWorkspacePayload(selectedProjectId, leftPanel, openResources, activeResourceId),
        newCorrelationId("workspace-save"),
      ).catch((invokeError) => setError(String(invokeError)));
    }, 250);
  }, [activeResourceId, bootstrap, leftPanel, openResources, selectedProjectId]);

  useEffect(() => {
    if (!bootstrap || openResources.length === 0) {
      return;
    }
    const sessionResources = openResources.filter(
      (resource): resource is Extract<WorkspaceResource, { resource_type: "session_terminal" }> =>
        resource.resource_type === "session_terminal",
    );
    for (const resource of sessionResources) {
      if (!attachedSessions[resource.session_id]) {
        void openSession(resource.session_id);
      }
    }
  }, [attachedSessions, bootstrap, openResources]);

  const activeResource = openResources.find((resource) => resource.resource_id === activeResourceId) ?? null;

  useEffect(() => {
    if (!activeResource) {
      return;
    }
    if (activeResource.resource_type === "work_item_detail") {
      void ensureWorkItemDetail(activeResource.work_item_id);
      void ensureReconciliationProposals(activeResource.work_item_id);
    }
    if (activeResource.resource_type === "document_detail") {
      void ensureDocumentDetail(activeResource.document_id);
    }
  }, [activeResource]);

  const selectedProject = useMemo(
    () => bootstrap?.projects.find((project) => project.id === selectedProjectId) ?? null,
    [bootstrap, selectedProjectId],
  );
  const filteredSessions = useMemo(
    () => sessions.filter((entry) => !selectedProjectId || entry.project_id === selectedProjectId),
    [selectedProjectId, sessions],
  );
  const dayCadenceKey = useMemo(() => currentDayCadenceKey(), []);
  const weekCadenceKey = useMemo(() => currentWeekCadenceKey(), []);
  const currentPlanningAssignments = selectedProjectId
    ? planningAssignmentsByProject[selectedProjectId] ?? []
    : [];
  const allCurrentWorkItems = selectedProjectId ? workItemsByProject[selectedProjectId] ?? [] : [];
  const currentWorkItems = useMemo(() => {
    if (planningViewMode === "all") {
      return allCurrentWorkItems;
    }

    const cadenceType = planningViewMode === "day" ? "day" : "week";
    const cadenceKey = planningViewMode === "day" ? dayCadenceKey : weekCadenceKey;
    const assignedWorkItemIds = new Set(
      currentPlanningAssignments
        .filter(
          (assignment) =>
            assignment.removed_at === null &&
            assignment.cadence_type === cadenceType &&
            assignment.cadence_key === cadenceKey,
        )
        .map((assignment) => assignment.work_item_id),
    );
    return allCurrentWorkItems.filter((workItem) => assignedWorkItemIds.has(workItem.id));
  }, [allCurrentWorkItems, currentPlanningAssignments, dayCadenceKey, planningViewMode, weekCadenceKey]);
  const currentDocuments = selectedProjectId ? documentsByProject[selectedProjectId] ?? [] : [];

  async function openProject(projectId: string) {
    setSelectedProjectId(projectId);
    void loadProjectReads(projectId);
    const resourceId = `project_home:${projectId}`;
    if (!openResources.some((resource) => resource.resource_id === resourceId)) {
      setOpenResources((current) => [
        ...current,
        { resource_type: "project_home", project_id: projectId, resource_id: resourceId },
      ]);
    }
    setActiveResourceId(resourceId);
  }

  async function openSession(sessionId: string) {
    const existingSession = sessions.find((entry) => entry.id === sessionId);
    if (!existingSession) {
      return;
    }

    const correlationId = newCorrelationId("session-open");
    const resourceId = `session_terminal:${sessionId}`;
    if (!openResources.some((resource) => resource.resource_id === resourceId)) {
      setOpenResources((current) => [
        ...current,
        { resource_type: "session_terminal", session_id: sessionId, resource_id: resourceId },
      ]);
    }
    setActiveResourceId(resourceId);

    if (!existingSession.live || attachedSessions[sessionId]) {
      return;
    }

    try {
      const response = await attachSession(sessionId, correlationId);
      setAttachedSessions((current) => ({ ...current, [sessionId]: response }));
      setTerminalOutput((current) => ({
        ...current,
        [sessionId]: response.replay.chunks.map((chunk) => decodeBase64Utf8(chunk.data)).join(""),
      }));
      await watchLiveSessions([sessionId], correlationId);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    }
  }

  async function openWorkItem(workItemId: string, projectId: string) {
    void loadProjectReads(projectId);
    void ensureWorkItemDetail(workItemId);
    void ensureReconciliationProposals(workItemId);
    const resourceId = `work_item_detail:${workItemId}`;
    if (!openResources.some((resource) => resource.resource_id === resourceId)) {
      setOpenResources((current) => [
        ...current,
        {
          resource_type: "work_item_detail",
          work_item_id: workItemId,
          project_id: projectId,
          resource_id: resourceId,
        },
      ]);
    }
    setActiveResourceId(resourceId);
  }

  async function openDocument(documentId: string, projectId: string) {
    void loadProjectReads(projectId);
    void ensureDocumentDetail(documentId);
    const resourceId = `document_detail:${documentId}`;
    if (!openResources.some((resource) => resource.resource_id === resourceId)) {
      setOpenResources((current) => [
        ...current,
        {
          resource_type: "document_detail",
          document_id: documentId,
          project_id: projectId,
          resource_id: resourceId,
        },
      ]);
    }
    setActiveResourceId(resourceId);
  }

  async function closeResource(resource: WorkspaceResource) {
    const remaining = openResources.filter((entry) => entry.resource_id !== resource.resource_id);
    setOpenResources(remaining);
    if (activeResourceId === resource.resource_id) {
      setActiveResourceId(remaining[remaining.length - 1]?.resource_id ?? null);
    }

    if (resource.resource_type === "session_terminal") {
      const attachment = attachedSessions[resource.session_id];
      if (attachment) {
        try {
          await detachSession(resource.session_id, attachment.attachment_id, newCorrelationId("session-close"));
        } catch (invokeError) {
          setError(String(invokeError));
        }
      }

      setAttachedSessions((current) => {
        const next = { ...current };
        delete next[resource.session_id];
        return next;
      });
      setTerminalInput((current) => {
        const next = { ...current };
        delete next[resource.session_id];
        return next;
      });
      setTerminalOutput((current) => {
        const next = { ...current };
        delete next[resource.session_id];
        return next;
      });
    }
  }

  async function submitTerminalInput(sessionId: string) {
    const value = terminalInput[sessionId];
    if (!value) {
      return;
    }

    try {
      await sendSessionInput(sessionId, value.endsWith("\n") ? value : `${value}\n`, newCorrelationId("session-input"));
      setTerminalInput((current) => ({ ...current, [sessionId]: "" }));
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    }
  }

  async function handleCreateProject() {
    const correlationId = newCorrelationId("project-create");
    setLoading("create-project", true);
    try {
      const detail = await createProject(
        {
          name: projectCreateForm.name,
          slug: projectCreateForm.slug || undefined,
          default_account_id: projectCreateForm.default_account_id || null,
        },
        correlationId,
      );
      applyProjectDetail(detail);
      setWorkItemsByProject((current) => ({ ...current, [detail.id]: [] }));
      setDocumentsByProject((current) => ({ ...current, [detail.id]: [] }));
      setProjectCreateForm({ name: "", slug: "", default_account_id: "" });
      setShowProjectCreate(false);
      await openProject(detail.id);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading("create-project", false);
    }
  }

  async function handleUpdateProject(projectId: string, input: { name: string; slug: string; default_account_id: string }) {
    const correlationId = newCorrelationId("project-update");
    setLoading(`save-project:${projectId}`, true);
    try {
      const detail = await updateProject(
        projectId,
        {
          name: input.name,
          slug: input.slug,
          default_account_id: input.default_account_id || null,
        },
        correlationId,
      );
      applyProjectDetail(detail);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`save-project:${projectId}`, false);
    }
  }

  async function handleCreateWorkItem() {
    if (!selectedProjectId) {
      setError("Select a project before creating a work item.");
      return;
    }
    const correlationId = newCorrelationId("work-item-create");
    setLoading("create-work-item", true);
    try {
      const detail = await createWorkItem(
        {
          project_id: selectedProjectId,
          parent_id: workItemCreateForm.parent_id || null,
          title: workItemCreateForm.title,
          description: workItemCreateForm.description,
          acceptance_criteria: workItemCreateForm.acceptance_criteria || null,
          work_item_type: workItemCreateForm.work_item_type,
          status: workItemCreateForm.status,
          priority: workItemCreateForm.priority || null,
        },
        correlationId,
      );
      applyWorkItemDetail(detail);
      setWorkItemCreateForm({
        title: "",
        description: "",
        acceptance_criteria: "",
        work_item_type: "task",
        status: "backlog",
        priority: "",
        parent_id: "",
      });
      await openWorkItem(detail.id, detail.project_id);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading("create-work-item", false);
    }
  }

  async function handleUpdateWorkItem(
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
    setLoading(`save-work-item:${workItemId}`, true);
    try {
      const detail = await updateWorkItem(workItemId, input, correlationId);
      applyWorkItemDetail(detail);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
      } finally {
        setLoading(`save-work-item:${workItemId}`, false);
      }
    }

  async function handleTogglePlanningAssignment(
    workItemId: string,
    cadenceType: "day" | "week",
    cadenceKey: string,
  ) {
    if (!selectedProjectId) {
      setError("Select a project before updating planning assignments.");
      return;
    }

    const existingAssignment = planningAssignmentForKey(
      planningAssignmentsByProject[selectedProjectId] ?? [],
      workItemId,
      cadenceType,
      cadenceKey,
    );
    const loadingKey = `${cadenceType}-assignment:${workItemId}:${cadenceKey}`;
    const correlationId = newCorrelationId(`${cadenceType}-assignment`);
    setLoading(loadingKey, true);
    try {
      if (existingAssignment) {
        await deletePlanningAssignment(existingAssignment.id, correlationId);
        removePlanningAssignment(selectedProjectId, existingAssignment.id);
      } else {
        const assignment = await createPlanningAssignment(
          {
            work_item_id: workItemId,
            cadence_type: cadenceType,
            cadence_key: cadenceKey,
            created_by: "tauri-client",
          },
          correlationId,
        );
        applyPlanningAssignment(selectedProjectId, assignment);
      }
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(loadingKey, false);
    }
  }

  async function handleLaunchSessionFromWorkItem(workItemId: string) {
    if (!selectedProjectId) {
      setError("Select a project before starting a session.");
      return;
    }

    const correlationId = newCorrelationId("work-item-session");
    setLoading(`launch-session:${workItemId}`, true);
    try {
      const project = projectDetails[selectedProjectId] ?? (await getProject(selectedProjectId, correlationId));
      applyProjectDetail(project);

      const workItem = workItemDetails[workItemId] ?? (await getWorkItem(workItemId, correlationId));
      applyWorkItemDetail(workItem);

      const account =
        bootstrap?.accounts.find((entry) => entry.id === project.default_account_id) ??
        bootstrap?.accounts[0] ??
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
          project_id: selectedProjectId,
          project_root_id: root.id,
          worktree_id: null,
          work_item_id: workItemId,
          account_id: account.id,
          agent_kind: account.agent_kind,
          cwd: root.path,
          command: account.binary_path ?? account.agent_kind,
          args: [],
          env_preset_ref: account.env_preset_ref,
          origin_mode: "planning",
          current_mode: "planning",
          title: `${workItem.callsign} · ${workItem.title}`,
          title_policy: "manual",
          restore_policy: "reattach",
          initial_terminal_cols: 120,
          initial_terminal_rows: 40,
        },
        correlationId,
      );

      applySessionDetail(detail);
      await watchLiveSessions([detail.id], correlationId);
      const resourceId = `session_terminal:${detail.id}`;
      if (!openResources.some((resource) => resource.resource_id === resourceId)) {
        setOpenResources((current) => [
          ...current,
          { resource_type: "session_terminal", session_id: detail.id, resource_id: resourceId },
        ]);
      }
      setActiveResourceId(resourceId);
      if (detail.live) {
        const response = await attachSession(detail.id, correlationId);
        setAttachedSessions((current) => ({ ...current, [detail.id]: response }));
        setTerminalOutput((current) => ({
          ...current,
          [detail.id]: response.replay.chunks.map((chunk) => decodeBase64Utf8(chunk.data)).join(""),
        }));
      }
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`launch-session:${workItemId}`, false);
    }
  }

  async function handleCreateDocument() {
    if (!selectedProjectId) {
      setError("Select a project before creating a document.");
      return;
    }
    const correlationId = newCorrelationId("document-create");
    setLoading("create-document", true);
    try {
      const detail = await createDocument(
        {
          project_id: selectedProjectId,
          work_item_id: documentCreateForm.work_item_id || null,
          doc_type: documentCreateForm.doc_type,
          title: documentCreateForm.title,
          slug: documentCreateForm.slug || undefined,
          status: documentCreateForm.status,
          content_markdown: documentCreateForm.content_markdown,
        },
        correlationId,
      );
      applyDocumentDetail(detail);
      setDocumentCreateForm({
        title: "",
        slug: "",
        doc_type: "note",
        status: "draft",
        work_item_id: "",
        content_markdown: "",
      });
      await openDocument(detail.id, detail.project_id);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading("create-document", false);
    }
  }

  async function handleUpdateDocument(
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
    setLoading(`save-document:${documentId}`, true);
    try {
      const detail = await updateDocument(documentId, input, correlationId);
      applyDocumentDetail(detail);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`save-document:${documentId}`, false);
    }
  }

  async function handleDismissProposal(workItemId: string, proposalId: string) {
    const correlationId = newCorrelationId("proposal-dismiss");
    setLoading(`proposal-action:${proposalId}`, true);
    try {
      const detail = await updateWorkflowReconciliationProposal(proposalId, { status: "dismissed" }, correlationId);
      applyProposal(workItemId, detail);
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`proposal-action:${proposalId}`, false);
    }
  }

  async function handleApplyProposal(proposal: WorkflowReconciliationProposalSummary) {
    const correlationId = newCorrelationId("proposal-apply");
    setLoading(`proposal-action:${proposal.id}`, true);
    try {
      const payload = proposal.proposed_change_payload;
      if (proposal.target_entity_type === "work_item") {
        const workItemId = proposal.target_entity_id ?? proposal.work_item_id;
        if (!workItemId) {
          throw new Error(`Proposal ${proposal.id} does not specify a target work item.`);
        }
        const detail = await updateWorkItem(workItemId, payload, correlationId);
        applyWorkItemDetail(detail);
        applyProposal(
          proposal.work_item_id ?? workItemId,
          await updateWorkflowReconciliationProposal(proposal.id, { status: "applied" }, correlationId),
        );
      } else if (proposal.target_entity_type === "document") {
        const documentId = proposal.target_entity_id;
        if (!documentId) {
          throw new Error(`Proposal ${proposal.id} does not specify a target document.`);
        }
        const detail = await updateDocument(documentId, payload, correlationId);
        applyDocumentDetail(detail);
        applyProposal(
          proposal.work_item_id ?? detail.work_item_id ?? documentId,
          await updateWorkflowReconciliationProposal(proposal.id, { status: "applied" }, correlationId),
        );
      } else if (proposal.target_entity_type === "project") {
        const projectId = proposal.target_entity_id;
        if (!projectId) {
          throw new Error(`Proposal ${proposal.id} does not specify a target project.`);
        }
        const detail = await updateProject(projectId, payload, correlationId);
        applyProjectDetail(detail);
        applyProposal(
          proposal.work_item_id ?? projectId,
          await updateWorkflowReconciliationProposal(proposal.id, { status: "applied" }, correlationId),
        );
      } else {
        throw new Error(
          `Proposal ${proposal.id} targets ${proposal.target_entity_type}, which is outside the first write-capable slice.`,
        );
      }
      clearError();
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`proposal-action:${proposal.id}`, false);
    }
  }

  if (!bootstrap) {
    return <div className="app-shell loading-state">{error ?? "Connecting to the supervisor…"}</div>;
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand-block">
          <div className="eyebrow">EURI thin shell</div>
          <h1>Supervisor workspace</h1>
        </div>
        <div className="status-strip">
          <span className={`status-chip ${connectionLabel(connectionEvent)}`}>{connectionLabel(connectionEvent)}</span>
          <span className="status-chip neutral">{bootstrap.health.live_session_count} live</span>
          <span className="status-chip neutral">usage {bootstrap.accounts.length > 0 ? "unavailable" : "hidden"}</span>
          {bootstrap.hello.diagnostics_enabled ? (
            <button
              className="status-chip neutral"
              onClick={() => {
                const correlationId = newCorrelationId("bundle");
                const activeSessionId =
                  activeResource?.resource_type === "session_terminal" ? activeResource.session_id : null;
                void exportDiagnosticsBundle(
                  activeSessionId,
                  activeSessionId ? `session-${activeSessionId}` : "shell",
                  snapshotClientDiagnostics(),
                  correlationId,
                )
                  .then((result) => setDiagnosticsBundle(result))
                  .catch((invokeError) => setError(String(invokeError)));
              }}
            >
              Export debug bundle
            </button>
          ) : null}
        </div>
      </header>

      <div className="shell-grid">
        <aside className="sidebar">
          <div className="sidebar-tabs sidebar-tabs-wide">
            <button className={leftPanel === "projects" ? "active" : ""} onClick={() => setLeftPanel("projects")}>
              Projects
            </button>
            <button className={leftPanel === "sessions" ? "active" : ""} onClick={() => setLeftPanel("sessions")}>
              Sessions
            </button>
            <button className={leftPanel === "work" ? "active" : ""} onClick={() => setLeftPanel("work")}>
              Work
            </button>
            <button className={leftPanel === "docs" ? "active" : ""} onClick={() => setLeftPanel("docs")}>
              Docs
            </button>
          </div>

          {leftPanel === "projects" ? (
            <div className="panel-list">
              <section className="card editor-card compact-editor">
                <div className="section-header">
                  <h3>New project</h3>
                  <button className="secondary-button" onClick={() => setShowProjectCreate((current) => !current)}>
                    {showProjectCreate ? "Hide" : "Create"}
                  </button>
                </div>
                {showProjectCreate ? (
                  <>
                    <label className="field">
                      <span>Name</span>
                      <input
                        value={projectCreateForm.name}
                        onChange={(event) =>
                          setProjectCreateForm((current) => ({ ...current, name: event.target.value }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>Slug</span>
                      <input
                        value={projectCreateForm.slug}
                        onChange={(event) =>
                          setProjectCreateForm((current) => ({ ...current, slug: event.target.value }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>Default account</span>
                      <select
                        value={projectCreateForm.default_account_id}
                        onChange={(event) =>
                          setProjectCreateForm((current) => ({
                            ...current,
                            default_account_id: event.target.value,
                          }))
                        }
                      >
                        <option value="">None</option>
                        {bootstrap.accounts.map((account) => (
                          <option key={account.id} value={account.id}>
                            {account.label}
                          </option>
                        ))}
                      </select>
                    </label>
                    <button onClick={() => void handleCreateProject()} disabled={Boolean(loadingKeys["create-project"])}>
                      {loadingKeys["create-project"] ? "Creating…" : "Create project"}
                    </button>
                  </>
                ) : null}
              </section>

              {bootstrap.projects.map((project) => (
                <button
                  key={project.id}
                  className={`list-card ${project.id === selectedProjectId ? "selected" : ""}`}
                  onClick={() => void openProject(project.id)}
                >
                  <span className="list-title">{project.name}</span>
                  <span className="list-meta">
                    {project.root_count} roots · {project.live_session_count} live sessions
                  </span>
                </button>
              ))}
            </div>
          ) : null}

          {leftPanel === "sessions" ? (
            <div className="panel-list">
              {filteredSessions.map((session) => (
                <button key={session.id} className="list-card" onClick={() => void openSession(session.id)}>
                  <span className="list-title">{session.title ?? session.current_mode}</span>
                  <span className="list-meta">
                    <span className={`indicator ${sessionTone(session)}`} />
                    {session.runtime_state}
                    {session.activity_state === "waiting_for_input" ? " · waiting" : ""}
                  </span>
                </button>
              ))}
            </div>
          ) : null}

          {leftPanel === "work" ? (
            <div className="panel-list">
              <section className="card compact-editor">
                <div className="section-header">
                  <h3>Planning view</h3>
                  <span className="status-chip neutral">
                    {planningViewMode === "all"
                      ? "all work"
                      : planningViewMode === "day"
                        ? dayCadenceKey
                        : weekCadenceKey}
                  </span>
                </div>
                <div className="segmented-control">
                  <button
                    className={planningViewMode === "all" ? "active" : ""}
                    onClick={() => setPlanningViewMode("all")}
                  >
                    All
                  </button>
                  <button
                    className={planningViewMode === "day" ? "active" : ""}
                    onClick={() => setPlanningViewMode("day")}
                  >
                    Today
                  </button>
                  <button
                    className={planningViewMode === "week" ? "active" : ""}
                    onClick={() => setPlanningViewMode("week")}
                  >
                    This week
                  </button>
                </div>
              </section>

              <section className="card editor-card compact-editor">
                <div className="section-header">
                  <h3>New work item</h3>
                  <span className="status-chip neutral">{selectedProject ? selectedProject.slug : "no project"}</span>
                </div>
                <label className="field">
                  <span>Title</span>
                  <input
                    value={workItemCreateForm.title}
                    onChange={(event) =>
                      setWorkItemCreateForm((current) => ({ ...current, title: event.target.value }))
                    }
                  />
                </label>
                <label className="field">
                  <span>Description</span>
                  <textarea
                    value={workItemCreateForm.description}
                    onChange={(event) =>
                      setWorkItemCreateForm((current) => ({ ...current, description: event.target.value }))
                    }
                  />
                </label>
                <div className="field-row">
                  <label className="field">
                    <span>Parent</span>
                    <select
                      value={workItemCreateForm.parent_id}
                      onChange={(event) =>
                        setWorkItemCreateForm((current) => ({ ...current, parent_id: event.target.value }))
                      }
                    >
                      <option value="">None</option>
                      {allCurrentWorkItems.map((item) => (
                        <option key={item.id} value={item.id}>
                          {item.callsign}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field">
                    <span>Type</span>
                    <select
                      value={workItemCreateForm.work_item_type}
                      onChange={(event) =>
                        setWorkItemCreateForm((current) => ({
                          ...current,
                          work_item_type: event.target.value,
                        }))
                      }
                    >
                      {WORK_ITEM_TYPES.map((value) => (
                        <option key={value} value={value}>
                          {value}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <div className="field-row">
                  <label className="field">
                    <span>Status</span>
                    <select
                      value={workItemCreateForm.status}
                      onChange={(event) =>
                        setWorkItemCreateForm((current) => ({ ...current, status: event.target.value }))
                      }
                    >
                      {WORK_ITEM_STATUSES.map((value) => (
                        <option key={value} value={value}>
                          {value}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field">
                    <span>Priority</span>
                    <select
                      value={workItemCreateForm.priority}
                      onChange={(event) =>
                        setWorkItemCreateForm((current) => ({ ...current, priority: event.target.value }))
                      }
                    >
                      {PRIORITIES.map((value) => (
                        <option key={value || "none"} value={value}>
                          {value || "none"}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label className="field">
                  <span>Acceptance criteria</span>
                  <textarea
                    value={workItemCreateForm.acceptance_criteria}
                    onChange={(event) =>
                      setWorkItemCreateForm((current) => ({
                        ...current,
                        acceptance_criteria: event.target.value,
                      }))
                    }
                  />
                </label>
                <button onClick={() => void handleCreateWorkItem()} disabled={Boolean(loadingKeys["create-work-item"])}>
                  {loadingKeys["create-work-item"] ? "Creating…" : "Create work item"}
                </button>
              </section>

              {selectedProjectId && loadingKeys[`project:${selectedProjectId}`] ? (
                <div className="empty-panel">Loading work items…</div>
              ) : currentWorkItems.length > 0 ? (
                currentWorkItems.map((workItem) => (
                  <button
                    key={workItem.id}
                    className="list-card"
                    onClick={() => void openWorkItem(workItem.id, workItem.project_id)}
                  >
                    <span className="list-title">
                      {workItem.callsign} · {workItem.title}
                    </span>
                    <span className="list-meta">
                      {workItem.status} · {workItem.work_item_type}
                      {workItem.priority ? ` · ${workItem.priority}` : ""}
                    </span>
                  </button>
                ))
              ) : (
                <div className="empty-panel">
                  {planningViewMode === "all"
                    ? "No work items for this project yet."
                    : `No work items assigned for ${planningViewMode === "day" ? dayCadenceKey : weekCadenceKey}.`}
                </div>
              )}
            </div>
          ) : null}

          {leftPanel === "docs" ? (
            <div className="panel-list">
              <section className="card editor-card compact-editor">
                <div className="section-header">
                  <h3>New document</h3>
                  <span className="status-chip neutral">{selectedProject ? selectedProject.slug : "no project"}</span>
                </div>
                <label className="field">
                  <span>Title</span>
                  <input
                    value={documentCreateForm.title}
                    onChange={(event) =>
                      setDocumentCreateForm((current) => ({ ...current, title: event.target.value }))
                    }
                  />
                </label>
                <div className="field-row">
                  <label className="field">
                    <span>Slug</span>
                    <input
                      value={documentCreateForm.slug}
                      onChange={(event) =>
                        setDocumentCreateForm((current) => ({ ...current, slug: event.target.value }))
                      }
                    />
                  </label>
                  <label className="field">
                    <span>Type</span>
                    <input
                      value={documentCreateForm.doc_type}
                      onChange={(event) =>
                        setDocumentCreateForm((current) => ({ ...current, doc_type: event.target.value }))
                      }
                    />
                  </label>
                </div>
                <div className="field-row">
                  <label className="field">
                    <span>Status</span>
                    <select
                      value={documentCreateForm.status}
                      onChange={(event) =>
                        setDocumentCreateForm((current) => ({ ...current, status: event.target.value }))
                      }
                    >
                      {DOCUMENT_STATUSES.map((value) => (
                        <option key={value} value={value}>
                          {value}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="field">
                    <span>Linked work item</span>
                    <select
                      value={documentCreateForm.work_item_id}
                      onChange={(event) =>
                        setDocumentCreateForm((current) => ({ ...current, work_item_id: event.target.value }))
                      }
                    >
                      <option value="">None</option>
                      {currentWorkItems.map((item) => (
                        <option key={item.id} value={item.id}>
                          {item.callsign}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label className="field">
                  <span>Markdown</span>
                  <textarea
                    className="markdown-editor compact-markdown-editor"
                    value={documentCreateForm.content_markdown}
                    onChange={(event) =>
                      setDocumentCreateForm((current) => ({
                        ...current,
                        content_markdown: event.target.value,
                      }))
                    }
                  />
                </label>
                <button onClick={() => void handleCreateDocument()} disabled={Boolean(loadingKeys["create-document"])}>
                  {loadingKeys["create-document"] ? "Creating…" : "Create document"}
                </button>
              </section>

              {selectedProjectId && loadingKeys[`project:${selectedProjectId}`] ? (
                <div className="empty-panel">Loading documents…</div>
              ) : currentDocuments.length > 0 ? (
                currentDocuments.map((document) => (
                  <button
                    key={document.id}
                    className="list-card"
                    onClick={() => void openDocument(document.id, document.project_id)}
                  >
                    <span className="list-title">{document.title}</span>
                    <span className="list-meta">{document.doc_type} · {document.status}</span>
                    <span className="list-meta list-preview">{documentPreview(document.content_markdown)}</span>
                  </button>
                ))
              ) : (
                <div className="empty-panel">No documents for this project yet.</div>
              )}
            </div>
          ) : null}
        </aside>

        <main className="workspace">
          <div className="workspace-summary">
            <div>
              <div className="eyebrow">Selected project</div>
              <strong>{selectedProject?.name ?? "No project selected"}</strong>
              {diagnosticsEnabled() && diagnosticsBundle?.bundle_path ? (
                <div className="eyebrow">{diagnosticsBundle.bundle_path}</div>
              ) : null}
            </div>
            <div className="summary-stats">
              <span>{bootstrap.bootstrap.project_count} projects</span>
              <span>{currentWorkItems.length} work items</span>
              <span>{currentDocuments.length} docs</span>
            </div>
          </div>

          {error ? (
            <div className="error-banner">
              <span>{error}</span>
              <button className="secondary-button" onClick={() => setError(null)}>
                Dismiss
              </button>
            </div>
          ) : null}

          <div className="tab-strip">
            {openResources.map((resource) => (
              <button
                key={resource.resource_id}
                className={`tab ${resource.resource_id === activeResourceId ? "active" : ""}`}
                onClick={() => setActiveResourceId(resource.resource_id)}
              >
                {resourceLabel(
                  resource,
                  sessions,
                  workItemDetails,
                  documentDetails,
                  bootstrap.projects.find(
                    (project) => resource.resource_type === "project_home" && project.id === resource.project_id,
                  )?.name,
                )}
                <span
                  className="tab-close"
                  onClick={(event) => {
                    event.stopPropagation();
                    void closeResource(resource);
                  }}
                >
                  ×
                </span>
              </button>
            ))}
          </div>

          <section className="workspace-pane">
            {!activeResource ? (
              <div className="empty-pane">Open a project, session, work item, or document.</div>
            ) : activeResource.resource_type === "project_home" ? (
              <ProjectHome
                project={bootstrap.projects.find((project) => project.id === activeResource.project_id) ?? null}
                accounts={bootstrap.accounts}
                sessions={sessions.filter((entry) => entry.project_id === activeResource.project_id)}
                workItems={workItemsByProject[activeResource.project_id] ?? []}
                documents={documentsByProject[activeResource.project_id] ?? []}
                loading={Boolean(loadingKeys[`project:${activeResource.project_id}`])}
                saving={Boolean(loadingKeys[`save-project:${activeResource.project_id}`])}
                onSaveProject={(input) => void handleUpdateProject(activeResource.project_id, input)}
                onOpenSession={openSession}
                onOpenWorkItem={openWorkItem}
                onOpenDocument={openDocument}
              />
            ) : activeResource.resource_type === "session_terminal" ? (
              <SessionPane
                session={sessions.find((entry) => entry.id === activeResource.session_id) ?? null}
                attachment={attachedSessions[activeResource.session_id] ?? null}
                output={terminalOutput[activeResource.session_id] ?? ""}
                input={terminalInput[activeResource.session_id] ?? ""}
                onInputChange={(value) =>
                  setTerminalInput((current) => ({ ...current, [activeResource.session_id]: value }))
                }
                onSubmitInput={() => void submitTerminalInput(activeResource.session_id)}
                onInterrupt={() => void interruptSession(activeResource.session_id, newCorrelationId("session-interrupt"))}
                onTerminate={() => void terminateSession(activeResource.session_id, newCorrelationId("session-terminate"))}
              />
            ) : activeResource.resource_type === "work_item_detail" ? (
              <WorkItemPane
                detail={workItemDetails[activeResource.work_item_id] ?? null}
                dailyAssignment={planningAssignmentForKey(
                  planningAssignmentsByProject[activeResource.project_id] ?? [],
                  activeResource.work_item_id,
                  "day",
                  dayCadenceKey,
                )}
                weeklyAssignment={planningAssignmentForKey(
                  planningAssignmentsByProject[activeResource.project_id] ?? [],
                  activeResource.work_item_id,
                  "week",
                  weekCadenceKey,
                )}
                dayKey={dayCadenceKey}
                weekKey={weekCadenceKey}
                relatedDocs={(documentsByProject[activeResource.project_id] ?? []).filter(
                  (document) => document.work_item_id === activeResource.work_item_id,
                )}
                relatedSessions={sessions.filter((session) => session.work_item_id === activeResource.work_item_id)}
                proposals={reconciliationByWorkItem[activeResource.work_item_id] ?? []}
                loading={
                  Boolean(loadingKeys[`work-item:${activeResource.work_item_id}`]) ||
                  Boolean(loadingKeys[`proposal:${activeResource.work_item_id}`])
                }
                saving={Boolean(loadingKeys[`save-work-item:${activeResource.work_item_id}`])}
                launchingSession={Boolean(loadingKeys[`launch-session:${activeResource.work_item_id}`])}
                onOpenDocument={openDocument}
                onOpenSession={openSession}
                onSave={(input) => void handleUpdateWorkItem(activeResource.work_item_id, input)}
                onToggleDailyAssignment={() =>
                  void handleTogglePlanningAssignment(activeResource.work_item_id, "day", dayCadenceKey)
                }
                onToggleWeeklyAssignment={() =>
                  void handleTogglePlanningAssignment(activeResource.work_item_id, "week", weekCadenceKey)
                }
                onLaunchSession={() => void handleLaunchSessionFromWorkItem(activeResource.work_item_id)}
                onApplyProposal={(proposalId) => {
                  const proposal = (reconciliationByWorkItem[activeResource.work_item_id] ?? []).find(
                    (item) => item.id === proposalId,
                  );
                  if (proposal) {
                    void handleApplyProposal(proposal);
                  }
                }}
                onDismissProposal={(proposalId) =>
                  void handleDismissProposal(activeResource.work_item_id, proposalId)
                }
              />
            ) : (
              <DocumentPane
                detail={documentDetails[activeResource.document_id] ?? null}
                loading={Boolean(loadingKeys[`document:${activeResource.document_id}`])}
                workItems={workItemsByProject[activeResource.project_id] ?? []}
                saving={Boolean(loadingKeys[`save-document:${activeResource.document_id}`])}
                onOpenWorkItem={openWorkItem}
                onSave={(input) => void handleUpdateDocument(activeResource.document_id, input)}
              />
            )}
          </section>
        </main>
      </div>
    </div>
  );
}

function ProjectHome({
  project,
  accounts,
  sessions,
  workItems,
  documents,
  loading,
  saving,
  onSaveProject,
  onOpenSession,
  onOpenWorkItem,
  onOpenDocument,
}: {
  project: ShellBootstrap["projects"][number] | null;
  accounts: AccountSummary[];
  sessions: SessionSummary[];
  workItems: WorkItemSummary[];
  documents: DocumentSummary[];
  loading: boolean;
  saving: boolean;
  onSaveProject: (input: { name: string; slug: string; default_account_id: string }) => void;
  onOpenSession: (sessionId: string) => void;
  onOpenWorkItem: (workItemId: string, projectId: string) => void;
  onOpenDocument: (documentId: string, projectId: string) => void;
}) {
  const [form, setForm] = useState({ name: "", slug: "", default_account_id: "" });

  useEffect(() => {
    if (!project) {
      return;
    }
    setForm({
      name: project.name,
      slug: project.slug,
      default_account_id: project.default_account_id ?? "",
    });
  }, [project]);

  if (!project) {
    return <div className="empty-pane">Pick a project to start.</div>;
  }

  return (
    <div className="project-home">
      <div className="hero-panel">
        <div className="eyebrow">Project home</div>
        <h2>{project.name}</h2>
        <p>{project.slug}</p>
      </div>

      <div className="project-grid">
        <section className="card editor-card">
          <div className="section-header">
            <h3>Project settings</h3>
            <button className="secondary-button" onClick={() => onSaveProject(form)} disabled={saving}>
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
          <label className="field">
            <span>Name</span>
            <input value={form.name} onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))} />
          </label>
          <label className="field">
            <span>Slug</span>
            <input value={form.slug} onChange={(event) => setForm((current) => ({ ...current, slug: event.target.value }))} />
          </label>
          <label className="field">
            <span>Default account</span>
            <select
              value={form.default_account_id}
              onChange={(event) =>
                setForm((current) => ({ ...current, default_account_id: event.target.value }))
              }
            >
              <option value="">None</option>
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.label}
                </option>
              ))}
            </select>
          </label>
        </section>

        <section className="card">
          <h3>Roots</h3>
          <p>{project.root_count} configured project roots.</p>
        </section>
        <section className="card">
          <h3>Workbench</h3>
          <p>{loading ? "Loading…" : `${workItems.length} work items · ${documents.length} docs`}</p>
        </section>
        <section className="card session-card-list">
          <h3>Recent sessions</h3>
          {sessions.slice(0, 6).map((session) => (
            <button key={session.id} className="session-link" onClick={() => onOpenSession(session.id)}>
              {session.title ?? session.current_mode}
            </button>
          ))}
        </section>
        <section className="card session-card-list">
          <h3>Open work</h3>
          {workItems.slice(0, 5).map((workItem) => (
            <button
              key={workItem.id}
              className="session-link"
              onClick={() => onOpenWorkItem(workItem.id, workItem.project_id)}
            >
              {workItem.callsign} · {workItem.title}
            </button>
          ))}
        </section>
        <section className="card session-card-list">
          <h3>Recent docs</h3>
          {documents.slice(0, 5).map((document) => (
            <button
              key={document.id}
              className="session-link"
              onClick={() => onOpenDocument(document.id, document.project_id)}
            >
              {document.title}
            </button>
          ))}
        </section>
      </div>
    </div>
  );
}

function SessionPane({
  session,
  attachment,
  output,
  input,
  onInputChange,
  onSubmitInput,
  onInterrupt,
  onTerminate,
}: {
  session: SessionSummary | null;
  attachment: SessionAttachResponse | null;
  output: string;
  input: string;
  onInputChange: (value: string) => void;
  onSubmitInput: () => void;
  onInterrupt: () => void;
  onTerminate: () => void;
}) {
  if (!session) {
    return <div className="empty-pane">Session not found.</div>;
  }

  const tone = sessionTone(session);

  return (
    <div className="session-pane">
      <div className="session-header">
        <div>
          <div className="eyebrow">Attached session</div>
          <h2>{session.title ?? session.current_mode}</h2>
          <p>
            {session.agent_kind} · {session.cwd}
          </p>
        </div>
        <div className="session-status-cluster">
          <span className={`status-chip ${tone}`}>{session.runtime_state}</span>
          <span className="status-chip neutral">{session.activity_state}</span>
          {session.needs_input_reason ? (
            <span className="status-chip neutral">{session.needs_input_reason}</span>
          ) : null}
        </div>
      </div>

      <div className="session-meta">
        <span>status {session.status}</span>
        <span>live {session.live ? "yes" : "no"}</span>
        <span>attached {attachment?.session.runtime?.attached_clients ?? 0}</span>
      </div>

      <pre className="terminal-surface">{output || "Attached. Waiting for output…"}</pre>

      {!session.live && !attachment ? (
        <div className="card compact-card">
          This session is not live. The shell keeps the tab as a read surface, but attach and input are disabled.
        </div>
      ) : null}

      <div className="terminal-controls">
        <textarea
          value={input}
          onChange={(event) => onInputChange(event.target.value)}
          placeholder="Send input to the attached session"
          disabled={!session.live}
        />
        <div className="action-row">
          <button onClick={onSubmitInput} disabled={!session.live}>
            Send input
          </button>
          <button onClick={onInterrupt} disabled={!session.live}>
            Interrupt
          </button>
          <button className="danger" onClick={onTerminate} disabled={!session.live}>
            Terminate
          </button>
        </div>
      </div>
    </div>
  );
}
