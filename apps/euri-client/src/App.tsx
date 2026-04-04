import { useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  attachSession,
  bootstrapShell,
  connectionLabel,
  detachSession,
  interruptSession,
  saveWorkspace,
  sendSessionInput,
  terminateSession,
  watchLiveSessions,
} from "./lib";
import type {
  ConnectionStatusEvent,
  SessionAttachResponse,
  SessionOutputEvent,
  SessionStateChangedEvent,
  SessionSummary,
  ShellBootstrap,
  WorkspacePayload,
  WorkspaceResource,
} from "./types";

function resourceLabel(resource: WorkspaceResource, sessions: SessionSummary[], projectName?: string) {
  if (resource.resource_type === "project_home") {
    return projectName ? `${projectName}` : "Project";
  }
  const session = sessions.find((entry) => entry.id === resource.session_id);
  return session?.title ?? session?.current_mode ?? "Session";
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
  leftPanel: "projects" | "sessions",
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
  const [leftPanel, setLeftPanel] = useState<"projects" | "sessions">("projects");
  const [openResources, setOpenResources] = useState<WorkspaceResource[]>([]);
  const [activeResourceId, setActiveResourceId] = useState<string | null>(null);
  const [attachedSessions, setAttachedSessions] = useState<Record<string, SessionAttachResponse>>({});
  const [terminalOutput, setTerminalOutput] = useState<Record<string, string>>({});
  const [terminalInput, setTerminalInput] = useState<Record<string, string>>({});
  const [connectionEvent, setConnectionEvent] = useState<ConnectionStatusEvent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const restoreApplied = useRef(false);
  const persistTimeout = useRef<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function start() {
      try {
        const payload = await bootstrapShell();
        if (cancelled) {
          return;
        }

        setBootstrap(payload);
        setSessions(payload.sessions);
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

        await watchLiveSessions(payload.sessions.filter((entry) => entry.live).map((entry) => entry.id));
      } catch (invokeError) {
        setError(String(invokeError));
      }
    }

    void start();

    const listeners = Promise.all([
      listen<ConnectionStatusEvent>("supervisor://connection", (event) => {
        setConnectionEvent(event.payload);
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
    if (!bootstrap || !restoreApplied.current) {
      return;
    }
    if (persistTimeout.current) {
      window.clearTimeout(persistTimeout.current);
    }
    persistTimeout.current = window.setTimeout(() => {
      void saveWorkspace(
        buildWorkspacePayload(selectedProjectId, leftPanel, openResources, activeResourceId),
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

  const selectedProject = useMemo(
    () => bootstrap?.projects.find((project) => project.id === selectedProjectId) ?? null,
    [bootstrap, selectedProjectId],
  );
  const filteredSessions = useMemo(
    () => sessions.filter((entry) => !selectedProjectId || entry.project_id === selectedProjectId),
    [selectedProjectId, sessions],
  );
  const activeResource = openResources.find((resource) => resource.resource_id === activeResourceId) ?? null;

  async function openProject(projectId: string) {
    setSelectedProjectId(projectId);
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
      const response = await attachSession(sessionId);
      setAttachedSessions((current) => ({ ...current, [sessionId]: response }));
      setTerminalOutput((current) => ({
        ...current,
        [sessionId]: response.replay.chunks.map((chunk) => decodeBase64Utf8(chunk.data)).join(""),
      }));
      await watchLiveSessions([sessionId]);
    } catch (invokeError) {
      setError(String(invokeError));
    }
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
          await detachSession(resource.session_id, attachment.attachment_id);
        } catch (invokeError) {
          setError(String(invokeError));
        }
      }
    }
  }

  async function submitTerminalInput(sessionId: string) {
    const value = terminalInput[sessionId];
    if (!value) {
      return;
    }

    try {
      await sendSessionInput(sessionId, value.endsWith("\n") ? value : `${value}\n`);
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
        </div>
      </header>

      <div className="shell-grid">
        <aside className="sidebar">
          <div className="sidebar-tabs">
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
          ) : (
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
          )}
        </aside>

        <main className="workspace">
          <div className="workspace-summary">
            <div>
              <div className="eyebrow">Selected project</div>
              <strong>{selectedProject?.name ?? "No project selected"}</strong>
            </div>
            <div className="summary-stats">
              <span>{bootstrap.bootstrap.project_count} projects</span>
              <span>{bootstrap.bootstrap.account_count} accounts</span>
              <span>{bootstrap.bootstrap.restorable_workspace_count} saved workspaces</span>
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
              <div className="empty-pane">Open a project or session.</div>
            ) : activeResource.resource_type === "project_home" ? (
              <ProjectHome
                project={bootstrap.projects.find((project) => project.id === activeResource.project_id) ?? null}
                sessions={sessions.filter((entry) => entry.project_id === activeResource.project_id)}
                onOpenSession={openSession}
              />
            ) : (
              <SessionPane
                session={sessions.find((entry) => entry.id === activeResource.session_id) ?? null}
                attachment={attachedSessions[activeResource.session_id] ?? null}
                output={terminalOutput[activeResource.session_id] ?? ""}
                input={terminalInput[activeResource.session_id] ?? ""}
                onInputChange={(value) =>
                  setTerminalInput((current) => ({ ...current, [activeResource.session_id]: value }))
                }
                onSubmitInput={() => void submitTerminalInput(activeResource.session_id)}
                onInterrupt={() => void interruptSession(activeResource.session_id)}
                onTerminate={() => void terminateSession(activeResource.session_id)}
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
  onOpenSession,
}: {
  project: ShellBootstrap["projects"][number] | null;
  sessions: SessionSummary[];
  onOpenSession: (sessionId: string) => void;
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
        <section className="card session-card-list">
          <h3>Recent sessions</h3>
          {sessions.slice(0, 6).map((session) => (
            <button key={session.id} className="session-link" onClick={() => onOpenSession(session.id)}>
              {session.title ?? session.current_mode}
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

      <div className="terminal-controls">
        <textarea
          value={input}
          onChange={(event) => onInputChange(event.target.value)}
          placeholder="Send input to the attached session"
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
