import { useEffect, useState } from "react";
import type {
  DocumentDetail,
  DocumentSummary,
  PlanningAssignmentSummary,
  SessionSummary,
  WorkItemDetail,
  WorkItemSummary,
  WorkflowReconciliationProposalSummary,
} from "./types";
import { Button } from "./components/ui";

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

export function WorkItemPane({
  detail,
  dailyAssignment,
  weeklyAssignment,
  dayKey,
  weekKey,
  relatedDocs,
  relatedSessions,
  proposals,
  loading,
  saving,
  launchingSession,
  onOpenDocument,
  onOpenSession,
  onSave,
  onToggleDailyAssignment,
  onToggleWeeklyAssignment,
  onLaunchSession,
  onApplyProposal,
  onDismissProposal,
}: {
  detail: WorkItemDetail | null;
  dailyAssignment: PlanningAssignmentSummary | null;
  weeklyAssignment: PlanningAssignmentSummary | null;
  dayKey: string;
  weekKey: string;
  relatedDocs: DocumentSummary[];
  relatedSessions: SessionSummary[];
  proposals: WorkflowReconciliationProposalSummary[];
  loading: boolean;
  saving: boolean;
  launchingSession: boolean;
  onOpenDocument: (documentId: string, projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSave: (input: {
    title: string;
    description: string;
    acceptance_criteria?: string | null;
    work_item_type: string;
    status: string;
    priority?: string | null;
  }) => void;
  onToggleDailyAssignment: () => void;
  onToggleWeeklyAssignment: () => void;
  onLaunchSession: () => void;
  onApplyProposal: (proposalId: string) => void;
  onDismissProposal: (proposalId: string) => void;
}) {
  const [form, setForm] = useState({
    title: "",
    description: "",
    acceptance_criteria: "",
    work_item_type: "task",
    status: "backlog",
    priority: "",
  });

  useEffect(() => {
    if (!detail) {
      return;
    }
    setForm({
      title: detail.title,
      description: detail.description,
      acceptance_criteria: detail.acceptance_criteria ?? "",
      work_item_type: detail.work_item_type,
      status: detail.status,
      priority: detail.priority ?? "",
    });
  }, [detail]);

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
        <div className="detail-header-actions">
          <div className="status-strip">
            <span className="status-chip neutral">{detail.status}</span>
            <span className="status-chip neutral">{detail.work_item_type}</span>
            {detail.priority ? <span className="status-chip neutral">{detail.priority}</span> : null}
          </div>
          <Button variant="secondary" onClick={onLaunchSession} disabled={launchingSession}>
            {launchingSession ? "Starting…" : "Start session"}
          </Button>
        </div>
      </div>

      <div className="detail-grid">
        <section className="card editor-card">
          <div className="section-header">
            <h3>Edit work item</h3>
            <Button
              variant="secondary"
              onClick={() =>
                onSave({
                  title: form.title,
                  description: form.description,
                  acceptance_criteria: form.acceptance_criteria || null,
                  work_item_type: form.work_item_type,
                  status: form.status,
                  priority: form.priority || null,
                })
              }
              disabled={saving}
            >
              {saving ? "Saving…" : "Save"}
            </Button>
          </div>

          <label className="field">
            <span>Title</span>
            <input
              value={form.title}
              onChange={(event) => setForm((current) => ({ ...current, title: event.target.value }))}
            />
          </label>

          <label className="field">
            <span>Description</span>
            <textarea
              value={form.description}
              onChange={(event) =>
                setForm((current) => ({ ...current, description: event.target.value }))
              }
            />
          </label>

          <label className="field">
            <span>Acceptance criteria</span>
            <textarea
              value={form.acceptance_criteria}
              onChange={(event) =>
                setForm((current) => ({ ...current, acceptance_criteria: event.target.value }))
              }
            />
          </label>

          <div className="field-row">
            <label className="field">
              <span>Type</span>
              <select
                value={form.work_item_type}
                onChange={(event) =>
                  setForm((current) => ({ ...current, work_item_type: event.target.value }))
                }
              >
                {WORK_ITEM_TYPES.map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Status</span>
              <select
                value={form.status}
                onChange={(event) => setForm((current) => ({ ...current, status: event.target.value }))}
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
                value={form.priority}
                onChange={(event) => setForm((current) => ({ ...current, priority: event.target.value }))}
              >
                {PRIORITIES.map((value) => (
                  <option key={value || "none"} value={value}>
                    {value || "none"}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </section>

        <section className="card session-card-list">
          <h3>Planning</h3>
          <div className="planning-row">
            <div>
              <strong>Today</strong>
              <p className="detail-copy">{dayKey}</p>
            </div>
            <div className="planning-actions">
              <span className={`status-chip ${dailyAssignment ? "live" : "neutral"}`}>
                {dailyAssignment ? "assigned" : "not assigned"}
              </span>
              <Button variant="secondary" onClick={onToggleDailyAssignment}>
                {dailyAssignment ? "Remove" : "Assign"}
              </Button>
            </div>
          </div>
          <div className="planning-row">
            <div>
              <strong>This week</strong>
              <p className="detail-copy">{weekKey}</p>
            </div>
            <div className="planning-actions">
              <span className={`status-chip ${weeklyAssignment ? "live" : "neutral"}`}>
                {weeklyAssignment ? "assigned" : "not assigned"}
              </span>
              <Button variant="secondary" onClick={onToggleWeeklyAssignment}>
                {weeklyAssignment ? "Remove" : "Assign"}
              </Button>
            </div>
          </div>
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

        <section className="card session-card-list">
          <h3>Workflow proposals</h3>
          {proposals.length > 0 ? (
            proposals.map((proposal) => {
              const applyDisabled =
                !["pending", "accepted"].includes(proposal.status) ||
                !["project", "work_item", "document"].includes(proposal.target_entity_type);
              const dismissDisabled = proposal.status !== "pending" && proposal.status !== "accepted";
              return (
                <div key={proposal.id} className="proposal-card">
                  <div className="proposal-header">
                    <strong>{proposal.proposal_type}</strong>
                    <div className="status-strip">
                      <span className="status-chip neutral">{proposal.status}</span>
                      <span className="status-chip neutral">{proposal.target_entity_type}</span>
                    </div>
                  </div>
                  <p className="detail-copy">{proposal.reason}</p>
                  <pre className="proposal-payload">
                    {JSON.stringify(proposal.proposed_change_payload, null, 2)}
                  </pre>
                  <div className="action-row">
                    <button onClick={() => onApplyProposal(proposal.id)} disabled={applyDisabled}>
                      {applyDisabled ? "Review only" : "Apply"}
                    </button>
                    <Button variant="secondary" onClick={() => onDismissProposal(proposal.id)} disabled={dismissDisabled}>
                      Dismiss
                    </Button>
                  </div>
                </div>
              );
            })
          ) : (
            <p className="detail-copy">No workflow proposals for this work item.</p>
          )}
        </section>
      </div>
    </div>
  );
}

export function DocumentPane({
  detail,
  loading,
  workItems,
  saving,
  onOpenWorkItem,
  onSave,
}: {
  detail: DocumentDetail | null;
  loading: boolean;
  workItems: WorkItemSummary[];
  saving: boolean;
  onOpenWorkItem: (workItemId: string, projectId: string) => void;
  onSave: (input: {
    work_item_id?: string | null;
    doc_type: string;
    title: string;
    slug?: string;
    status: string;
    content_markdown: string;
  }) => void;
}) {
  const [form, setForm] = useState({
    title: "",
    slug: "",
    doc_type: "note",
    status: "draft",
    work_item_id: "",
    content_markdown: "",
  });

  useEffect(() => {
    if (!detail) {
      return;
    }
    setForm({
      title: detail.title,
      slug: detail.slug,
      doc_type: detail.doc_type,
      status: detail.status,
      work_item_id: detail.work_item_id ?? "",
      content_markdown: detail.content_markdown,
    });
  }, [detail]);

  if (loading || !detail) {
    return <div className="empty-pane">Loading document…</div>;
  }

  const linkedWorkItem = workItems.find((item) => item.id === detail.work_item_id) ?? null;

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

      <section className="card editor-card">
        <div className="section-header">
          <h3>Edit document</h3>
          <Button
            variant="secondary"
            onClick={() =>
              onSave({
                work_item_id: form.work_item_id || null,
                doc_type: form.doc_type,
                title: form.title,
                slug: form.slug,
                status: form.status,
                content_markdown: form.content_markdown,
              })
            }
            disabled={saving}
          >
            {saving ? "Saving…" : "Save"}
          </Button>
        </div>

        <div className="field-row">
          <label className="field">
            <span>Title</span>
            <input value={form.title} onChange={(event) => setForm((current) => ({ ...current, title: event.target.value }))} />
          </label>
          <label className="field">
            <span>Slug</span>
            <input value={form.slug} onChange={(event) => setForm((current) => ({ ...current, slug: event.target.value }))} />
          </label>
        </div>

        <div className="field-row">
          <label className="field">
            <span>Type</span>
            <input
              value={form.doc_type}
              onChange={(event) => setForm((current) => ({ ...current, doc_type: event.target.value }))}
            />
          </label>
          <label className="field">
            <span>Status</span>
            <select
              value={form.status}
              onChange={(event) => setForm((current) => ({ ...current, status: event.target.value }))}
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
              value={form.work_item_id}
              onChange={(event) =>
                setForm((current) => ({ ...current, work_item_id: event.target.value }))
              }
            >
              <option value="">None</option>
              {workItems.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.callsign} · {item.title}
                </option>
              ))}
            </select>
          </label>
        </div>

        <label className="field">
          <span>Markdown</span>
          <textarea
            className="markdown-editor"
            value={form.content_markdown}
            onChange={(event) =>
              setForm((current) => ({ ...current, content_markdown: event.target.value }))
            }
          />
        </label>
      </section>

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
    </div>
  );
}

export function documentPreview(markdown: string): string {
  return markdown.replace(/\s+/g, " ").trim().slice(0, 120) || "No preview";
}
