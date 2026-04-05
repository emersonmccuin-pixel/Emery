import { useEffect, useState } from "react";
import type {
  AccountSummary,
  DocumentSummary,
  SessionSummary,
  ShellBootstrap,
  WorkItemSummary,
} from "./types";
import { FleetStrip } from "./fleet-strip";

export function ProjectHome({
  project,
  accounts,
  sessions,
  workItems,
  documents,
  mergeQueueCount,
  saving,
  onSaveProject,
  onOpenSession,
  onOpenWorkItem,
  onOpenDocument,
  onOpenMergeQueue,
}: {
  project: ShellBootstrap["projects"][number] | null;
  accounts: AccountSummary[];
  sessions: SessionSummary[];
  workItems: WorkItemSummary[];
  documents: DocumentSummary[];
  mergeQueueCount: number;
  saving: boolean;
  onSaveProject: (input: { name: string; slug: string; default_account_id: string }) => void;
  onOpenSession: (sessionId: string) => void;
  onOpenWorkItem: (workItemId: string, projectId: string) => void;
  onOpenDocument: (documentId: string, projectId: string) => void;
  onOpenMergeQueue: () => void;
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

  const liveSessions = sessions.filter((s) => s.live);
  const endedSessions = sessions.filter((s) => !s.live);
  const activeWorkItems = workItems.filter((w) => w.status === "in_progress" || w.status === "blocked");
  const plannedWorkItems = workItems.filter((w) => w.status === "planned" || w.status === "backlog");

  return (
    <div className="project-home">
      <div className="project-grid">
        {liveSessions.length > 0 ? (
          <FleetStrip
            sessions={sessions}
            workItems={workItems}
            onOpenSession={onOpenSession}
          />
        ) : null}

        {mergeQueueCount > 0 ? (
          <section className="card session-card-list">
            <h3>Merge queue</h3>
            <button className="session-link" onClick={onOpenMergeQueue}>
              {mergeQueueCount} branch{mergeQueueCount !== 1 ? "es" : ""} ready to merge
            </button>
          </section>
        ) : null}

        {activeWorkItems.length > 0 ? (
          <section className="card session-card-list">
            <h3>Active work</h3>
            {activeWorkItems.slice(0, 8).map((workItem) => (
              <button
                key={workItem.id}
                className="session-link"
                onClick={() => onOpenWorkItem(workItem.id, workItem.project_id)}
              >
                {workItem.callsign} · {workItem.title}
              </button>
            ))}
          </section>
        ) : null}

        {endedSessions.length > 0 ? (
          <section className="card session-card-list">
            <h3>Recent sessions</h3>
            {endedSessions.slice(0, 5).map((session) => (
              <button key={session.id} className="session-link" onClick={() => onOpenSession(session.id)}>
                {session.title ?? session.current_mode}
              </button>
            ))}
          </section>
        ) : null}

        {plannedWorkItems.length > 0 ? (
          <section className="card session-card-list">
            <h3>Planned</h3>
            {plannedWorkItems.slice(0, 6).map((workItem) => (
              <button
                key={workItem.id}
                className="session-link"
                onClick={() => onOpenWorkItem(workItem.id, workItem.project_id)}
              >
                {workItem.callsign} · {workItem.title}
              </button>
            ))}
          </section>
        ) : null}

        <section className="card session-card-list">
          <h3>Docs</h3>
          {documents.length > 0 ? (
            documents.slice(0, 5).map((document) => (
              <button
                key={document.id}
                className="session-link"
                onClick={() => onOpenDocument(document.id, document.project_id)}
              >
                {document.title}
              </button>
            ))
          ) : (
            <span className="detail-copy">No docs yet.</span>
          )}
        </section>

        <section className="card editor-card">
          <div className="section-header">
            <h3>Settings</h3>
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
            <span>Account</span>
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
      </div>
    </div>
  );
}
