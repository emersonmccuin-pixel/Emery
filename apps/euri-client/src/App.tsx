import { useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  attachSession,
  bootstrapShell,
  connectionLabel,
  detachSession,
  exportDiagnosticsBundle,
  getDocument,
  getWorkItem,
  interruptSession,
  listDocuments,
  listWorkItems,
  saveWorkspace,
  sendSessionInput,
  terminateSession,
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
  ConnectionStatusEvent,
  DiagnosticsBundleResult,
  DocumentDetail,
  DocumentSummary,
  SessionAttachResponse,
  SessionOutputEvent,
  SessionStateChangedEvent,
  SessionSummary,
  ShellBootstrap,
  WorkItemDetail,
  WorkItemSummary,
  WorkspacePayload,
  WorkspaceResource,
} from "./types";
import { DocumentPane, WorkItemPane, documentPreview } from "./workbench";

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

export default function App() {
  const [bootstrap, setBootstrap] = useState<ShellBootstrap | null>(null);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [leftPanel, setLeftPanel] = useState<WorkspacePayload["left_panel"]>("projects");
  const [openResources, setOpenResources] = useState<WorkspaceResource[]>([]);
  const [activeResourceId, setActiveResourceId] = useState<string | null>(null);
  const [attachedSessions, setAttachedSessions] = useState<Record<string, SessionAttachResponse>>({});
  const [terminalOutput, setTerminalOutput] = useState<Record<string, string>>({});
  const [terminalInput, setTerminalInput] = useState<Record<string, string>>({});
  const [workItemsByProject, setWorkItemsByProject] = useState<Record<string, WorkItemSummary[]>>({});
  const [documentsByProject, setDocumentsByProject] = useState<Record<string, DocumentSummary[]>>({});
  const [workItemDetails, setWorkItemDetails] = useState<Record<string, WorkItemDetail>>({});
  const [documentDetails, setDocumentDetails] = useState<Record<string, DocumentDetail>>({});
  const [connectionEvent, setConnectionEvent] = useState<ConnectionStatusEvent | null>(null);
  const [diagnosticsBundle, setDiagnosticsBundle] = useState<DiagnosticsBundleResult | null>(null);
  const [loadingKeys, setLoadingKeys] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const restoreApplied = useRef(false);
  const persistTimeout = useRef<number | null>(null);

  function setLoading(key: string, value: boolean) {
    setLoadingKeys((current) => ({ ...current, [key]: value }));
  }

  async function loadProjectReads(projectId: string) {
    if (workItemsByProject[projectId] && documentsByProject[projectId]) {
      return;
    }

    const correlationId = newCorrelationId("project-load");
    setLoading(`project:${projectId}`, true);
    try {
      const [workItems, documents] = await Promise.all([
        workItemsByProject[projectId]
          ? Promise.resolve(workItemsByProject[projectId])
          : listWorkItems(projectId, correlationId),
        documentsByProject[projectId]
          ? Promise.resolve(documentsByProject[projectId])
          : listDocuments(projectId, undefined, correlationId),
      ]);
      setWorkItemsByProject((current) => ({ ...current, [projectId]: workItems }));
      setDocumentsByProject((current) => ({ ...current, [projectId]: documents }));
      recordClientEvent(
        makeClientEvent("shell", "project.reads_loaded", {
          correlation_id: correlationId,
          project_id: projectId,
          payload: {
            work_item_count: workItems.length,
            document_count: documents.length,
          },
        }),
      );
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`project:${projectId}`, false);
    }
  }

  async function ensureWorkItemDetail(workItemId: string) {
    if (workItemDetails[workItemId]) {
      return;
    }

    const correlationId = newCorrelationId("work-item");
    setLoading(`work-item:${workItemId}`, true);
    try {
      const detail = await getWorkItem(workItemId, correlationId);
      setWorkItemDetails((current) => ({ ...current, [workItemId]: detail }));
      recordClientEvent(
        makeClientEvent("workbench", "work_item.loaded", {
          correlation_id: correlationId,
          project_id: detail.project_id,
          work_item_id: workItemId,
        }),
      );
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`work-item:${workItemId}`, false);
    }
  }

  async function ensureDocumentDetail(documentId: string) {
    if (documentDetails[documentId]) {
      return;
    }

    const correlationId = newCorrelationId("document");
    setLoading(`document:${documentId}`, true);
    try {
      const detail = await getDocument(documentId, correlationId);
      setDocumentDetails((current) => ({ ...current, [documentId]: detail }));
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
    } catch (invokeError) {
      setError(String(invokeError));
    } finally {
      setLoading(`document:${documentId}`, false);
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
            recordClientEvent(
              makeClientEvent("session", "session.state_changed", {
                session_id: payload.session_id,
                payload: {
                  runtime_state: payload.runtime_state,
                  activity_state: payload.activity_state,
                  live: payload.live,
                  attached_clients: payload.attached_clients,
                },
              }),
            );
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

  useEffect(() => {
    if (!activeResource) {
      return;
    }
    if (activeResource.resource_type === "work_item_detail") {
      void ensureWorkItemDetail(activeResource.work_item_id);
    }
    if (activeResource.resource_type === "document_detail") {
      void ensureDocumentDetail(activeResource.document_id);
    }
  }, [activeResourceId, openResources]);

  const selectedProject = useMemo(
    () => bootstrap?.projects.find((project) => project.id === selectedProjectId) ?? null,
    [bootstrap, selectedProjectId],
  );
  const filteredSessions = useMemo(
    () => sessions.filter((entry) => !selectedProjectId || entry.project_id === selectedProjectId),
    [selectedProjectId, sessions],
  );
  const activeResource = openResources.find((resource) => resource.resource_id === activeResourceId) ?? null;
  const currentWorkItems = selectedProjectId ? workItemsByProject[selectedProjectId] ?? [] : [];
  const currentDocuments = selectedProjectId ? documentsByProject[selectedProjectId] ?? [] : [];

  async function openProject(projectId: string) {
    recordClientEvent(
      makeClientEvent("shell", "project.opened", {
        correlation_id: newCorrelationId("project-open"),
        project_id: projectId,
      }),
    );
    setSelectedProjectId(projectId);
    void loadProjectReads(projectId);
    const resourceId = `project_home:${projectId}`;
    const existing = openResources.find((resource) => resource.resource_id === resourceId);
    if (!existing) {
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
    recordClientEvent(
      makeClientEvent("session", "session.open_requested", {
        correlation_id: correlationId,
        session_id: sessionId,
        project_id: existingSession.project_id,
        work_item_id: existingSession.work_item_id ?? undefined,
      }),
    );
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
      recordClientEvent(
        makeClientEvent("session", "session.attach_succeeded", {
          correlation_id: correlationId,
          session_id: sessionId,
          project_id: response.session.project_id,
          work_item_id: response.session.work_item_id ?? undefined,
          payload: {
            output_cursor: response.output_cursor,
          },
        }),
      );
    } catch (invokeError) {
      recordClientEvent(
        makeClientEvent("session", "session.attach_failed", {
          correlation_id: correlationId,
          session_id: sessionId,
          payload: {
            error: String(invokeError),
          },
        }),
      );
      setError(String(invokeError));
    }
  }

  async function openWorkItem(workItemId: string, projectId: string) {
    recordClientEvent(
      makeClientEvent("workbench", "work_item.open_requested", {
        correlation_id: newCorrelationId("work-item-open"),
        project_id: projectId,
        work_item_id: workItemId,
      }),
    );
    void loadProjectReads(projectId);
    void ensureWorkItemDetail(workItemId);
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
    recordClientEvent(
      makeClientEvent("workbench", "document.open_requested", {
        correlation_id: newCorrelationId("document-open"),
        project_id: projectId,
        payload: {
          document_id: documentId,
        },
      }),
    );
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
          const correlationId = newCorrelationId("session-close");
          await detachSession(resource.session_id, attachment.attachment_id, correlationId);
          recordClientEvent(
            makeClientEvent("session", "session.detach_requested", {
              correlation_id: correlationId,
              session_id: resource.session_id,
            }),
          );
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

    const correlationId = newCorrelationId("session-input");
    try {
      await sendSessionInput(sessionId, value.endsWith("\n") ? value : `${value}\n`, correlationId);
      recordClientEvent(
        makeClientEvent("session", "session.input_submitted", {
          correlation_id: correlationId,
          session_id: sessionId,
          payload: {
            byte_count: value.length,
          },
        }),
      );
      setTerminalInput((current) => ({ ...current, [sessionId]: "" }));
    } catch (invokeError) {
      setError(String(invokeError));
    }
  }

  if (error) {
    return <div className="app-shell error-state">{error}</div>;
  }

  if (!bootstrap) {
    return <div className="app-shell loading-state">Connecting to the supervisor…</div>;
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand-block">
          <div className="eyebrow">EURI thin shell</div>
          <h1>Supervisor workspace</h1>
        </div>
        <div className="status-strip">
          <span className={`status-chip ${connectionLabel(connectionEvent)}`}>
            {connectionLabel(connectionEvent)}
          </span>
          <span className="status-chip neutral">{bootstrap.health.live_session_count} live</span>
          <span className="status-chip neutral">
            usage {bootstrap.accounts.length > 0 ? "unavailable" : "hidden"}
          </span>
          {bootstrap.hello.diagnostics_enabled ? (
            <button
              className="status-chip neutral"
              onClick={() => {
                const correlationId = newCorrelationId("bundle");
                const activeSessionId =
                  activeResource?.resource_type === "session_terminal"
                    ? activeResource.session_id
                    : null;
                void exportDiagnosticsBundle(
                  activeSessionId,
                  activeSessionId ? `session-${activeSessionId}` : "shell",
                  snapshotClientDiagnostics(),
                  correlationId,
                )
                  .then((result) => {
                    setDiagnosticsBundle(result);
                    recordClientEvent(
                      makeClientEvent("shell", "diagnostics.bundle_exported", {
                        correlation_id: correlationId,
                        session_id: activeSessionId ?? undefined,
                        payload: {
                          bundle_path: result.bundle_path,
                        },
                      }),
                    );
                  })
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
            <button
              className={leftPanel === "projects" ? "active" : ""}
              onClick={() => setLeftPanel("projects")}
            >
              Projects
            </button>
            <button
              className={leftPanel === "sessions" ? "active" : ""}
              onClick={() => setLeftPanel("sessions")}
            >
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
                <button
                  key={session.id}
                  className="list-card"
                  onClick={() => void openSession(session.id)}
                >
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
                <div className="empty-panel">No work items for this project yet.</div>
              )}
            </div>
          ) : null}

          {leftPanel === "docs" ? (
            <div className="panel-list">
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
                  bootstrap.projects.find((project) =>
                    resource.resource_type === "project_home" && project.id === resource.project_id,
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
                sessions={sessions.filter((entry) => entry.project_id === activeResource.project_id)}
                workItems={workItemsByProject[activeResource.project_id] ?? []}
                documents={documentsByProject[activeResource.project_id] ?? []}
                loading={Boolean(loadingKeys[`project:${activeResource.project_id}`])}
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
                onInterrupt={() => {
                  const correlationId = newCorrelationId("session-interrupt");
                  recordClientEvent(
                    makeClientEvent("session", "session.interrupt_requested", {
                      correlation_id: correlationId,
                      session_id: activeResource.session_id,
                    }),
                  );
                  void interruptSession(activeResource.session_id, correlationId);
                }}
                onTerminate={() => {
                  const correlationId = newCorrelationId("session-terminate");
                  recordClientEvent(
                    makeClientEvent("session", "session.terminate_requested", {
                      correlation_id: correlationId,
                      session_id: activeResource.session_id,
                    }),
                  );
                  void terminateSession(activeResource.session_id, correlationId);
                }}
              />
            ) : activeResource.resource_type === "work_item_detail" ? (
              <WorkItemPane
                detail={workItemDetails[activeResource.work_item_id] ?? null}
                relatedDocs={(documentsByProject[activeResource.project_id] ?? []).filter(
                  (document) => document.work_item_id === activeResource.work_item_id,
                )}
                relatedSessions={sessions.filter((session) => session.work_item_id === activeResource.work_item_id)}
                loading={Boolean(loadingKeys[`work-item:${activeResource.work_item_id}`])}
                onOpenDocument={openDocument}
                onOpenSession={openSession}
              />
            ) : (
              <DocumentPane
                detail={documentDetails[activeResource.document_id] ?? null}
                loading={Boolean(loadingKeys[`document:${activeResource.document_id}`])}
                linkedWorkItem={
                  documentDetails[activeResource.document_id]?.work_item_id
                    ? (workItemsByProject[activeResource.project_id] ?? []).find(
                        (item) => item.id === documentDetails[activeResource.document_id]?.work_item_id,
                      ) ?? null
                    : null
                }
                onOpenWorkItem={openWorkItem}
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
  sessions,
  workItems,
  documents,
  loading,
  onOpenSession,
  onOpenWorkItem,
  onOpenDocument,
}: {
  project: ShellBootstrap["projects"][number] | null;
  sessions: SessionSummary[];
  workItems: WorkItemSummary[];
  documents: DocumentSummary[];
  loading: boolean;
  onOpenSession: (sessionId: string) => void;
  onOpenWorkItem: (workItemId: string, projectId: string) => void;
  onOpenDocument: (documentId: string, projectId: string) => void;
}) {
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
        <section className="card">
          <h3>Roots</h3>
          <p>{project.root_count} configured project roots.</p>
        </section>
        <section className="card">
          <h3>Live sessions</h3>
          <p>{project.live_session_count} active runtime sessions.</p>
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
