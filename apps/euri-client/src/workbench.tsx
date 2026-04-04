import type {
  DocumentDetail,
  DocumentSummary,
  SessionSummary,
  WorkItemDetail,
  WorkItemSummary,
} from "./types";

export function WorkItemPane({
  detail,
  relatedDocs,
  relatedSessions,
  loading,
  onOpenDocument,
  onOpenSession,
}: {
  detail: WorkItemDetail | null;
  relatedDocs: DocumentSummary[];
  relatedSessions: SessionSummary[];
  loading: boolean;
  onOpenDocument: (documentId: string, projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
}) {
  if (loading || !detail) {
    return <div className="empty-pane">Loading work item…</div>;
  }

  return (
    <div className="detail-pane">
      <div className="detail-header">
        <div>
          <div className="eyebrow">Work item</div>
          <h2>
            {detail.callsign} · {detail.title}
          </h2>
        </div>
        <div className="status-strip">
          <span className="status-chip neutral">{detail.status}</span>
          <span className="status-chip neutral">{detail.work_item_type}</span>
          {detail.priority ? <span className="status-chip neutral">{detail.priority}</span> : null}
        </div>
      </div>

      <div className="detail-grid">
        <section className="card">
          <h3>Description</h3>
          <p className="detail-copy">{detail.description}</p>
        </section>
        <section className="card">
          <h3>Acceptance criteria</h3>
          <p className="detail-copy">{detail.acceptance_criteria ?? "None recorded."}</p>
        </section>
        <section className="card session-card-list">
          <h3>Linked docs</h3>
          {relatedDocs.length > 0 ? (
            relatedDocs.map((document) => (
              <button
                key={document.id}
                className="session-link"
                onClick={() => onOpenDocument(document.id, document.project_id)}
              >
                {document.title}
              </button>
            ))
          ) : (
            <p className="detail-copy">No linked documents yet.</p>
          )}
        </section>
        <section className="card session-card-list">
          <h3>Related sessions</h3>
          {relatedSessions.length > 0 ? (
            relatedSessions.map((session) => (
              <button key={session.id} className="session-link" onClick={() => onOpenSession(session.id)}>
                {session.title ?? session.current_mode}
              </button>
            ))
          ) : (
            <p className="detail-copy">No related sessions yet.</p>
          )}
        </section>
      </div>
    </div>
  );
}

export function DocumentPane({
  detail,
  loading,
  linkedWorkItem,
  onOpenWorkItem,
}: {
  detail: DocumentDetail | null;
  loading: boolean;
  linkedWorkItem: WorkItemSummary | null;
  onOpenWorkItem: (workItemId: string, projectId: string) => void;
}) {
  if (loading || !detail) {
    return <div className="empty-pane">Loading document…</div>;
  }

  return (
    <div className="detail-pane">
      <div className="detail-header">
        <div>
          <div className="eyebrow">Document</div>
          <h2>{detail.title}</h2>
        </div>
        <div className="status-strip">
          <span className="status-chip neutral">{detail.doc_type}</span>
          <span className="status-chip neutral">{detail.status}</span>
          <span className="status-chip neutral">{detail.slug}</span>
        </div>
      </div>

      <div className="detail-grid">
        <section className="card">
          <h3>Linked work item</h3>
          {linkedWorkItem ? (
            <button
              className="session-link inline-link"
              onClick={() => onOpenWorkItem(linkedWorkItem.id, linkedWorkItem.project_id)}
            >
              {linkedWorkItem.callsign} · {linkedWorkItem.title}
            </button>
          ) : (
            <p className="detail-copy">No linked work item.</p>
          )}
        </section>
        <section className="card">
          <h3>Session link</h3>
          <p className="detail-copy">{detail.session_id ?? "No linked session."}</p>
        </section>
      </div>

      <section className="card markdown-card">
        <h3>Markdown</h3>
        <pre className="markdown-surface">{detail.content_markdown}</pre>
      </section>
    </div>
  );
}

export function documentPreview(markdown: string): string {
  return markdown.replace(/\s+/g, " ").trim().slice(0, 120) || "No preview";
}
