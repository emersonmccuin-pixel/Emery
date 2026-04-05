import { useEffect, useMemo } from "react";
import { appStore, useAppStore } from "./store";
import { navStore } from "./nav-store";
import type { PeekLayer } from "./nav-store";
import { StatusBadge } from "./components/status-badge";
import { MergeDiffViewer } from "./components/merge-diff-viewer";
import { DiffStats } from "./components/diff-stats";
import { renderMarkdown } from "./utils/markdown";
import { Button } from "@/components/ui/button";

// ── Shared peek header ────────────────────────────────────────────────────────

function PeekHeader({
  title,
  onPopOut,
}: {
  title: string;
  onPopOut?: () => void;
}) {
  return (
    <div className="peek-header">
      <span className="peek-title">{title}</span>
      <div className="peek-actions">
        {onPopOut && (
          <button onClick={onPopOut} title="Open in main view">
            &#x2B08;
          </button>
        )}
        <button onClick={() => navStore.closePeek()} title="Close">
          &#x2715;
        </button>
      </div>
    </div>
  );
}

// ── Utility ───────────────────────────────────────────────────────────────────

function formatTimestamp(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function formatDuration(startedAt: number | null, endedAt: number | null): string {
  if (!startedAt) return "\u2014";
  const end = endedAt ?? Date.now() / 1000;
  const totalSeconds = Math.floor(end - startedAt);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) return `${seconds}s`;
  return `${minutes}m ${seconds}s`;
}

// ── Peek: Work Item ───────────────────────────────────────────────────────────

function PeekWorkItem({ projectId, workItemId }: { projectId: string; workItemId: string }) {
  const workItem = useAppStore((s) => s.workItemDetails[workItemId]);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const loading = loadingKeys[`work-item-bundle:${workItemId}`] ?? false;

  useEffect(() => {
    void appStore.fetchWorkItemBundle(projectId, workItemId);
  }, [projectId, workItemId]);

  if (!workItem && loading) {
    return (
      <>
        <PeekHeader title="Work Item" />
        <div className="peek-body peek-loading">Loading...</div>
      </>
    );
  }
  if (!workItem) {
    return (
      <>
        <PeekHeader title="Work Item" />
        <div className="peek-body peek-empty">Work item not found.</div>
      </>
    );
  }

  function handlePopOut() {
    navStore.closePeek();
    navStore.goToWorkItem(projectId, workItemId);
  }

  return (
    <>
      <PeekHeader title={workItem.callsign} onPopOut={handlePopOut} />
      <div className="peek-body">
        <h3 className="peek-wi-title">{workItem.title}</h3>
        <div className="peek-wi-meta">
          <StatusBadge status={workItem.status} />
          {workItem.priority && (
            <span className={`peek-wi-priority priority-${workItem.priority}`}>
              {workItem.priority}
            </span>
          )}
          <span className="peek-wi-type">{workItem.work_item_type}</span>
        </div>

        {workItem.description && (
          <div className="peek-section">
            <div className="peek-section-label">Description</div>
            <div
              className="peek-markdown"
              dangerouslySetInnerHTML={{ __html: renderMarkdown(workItem.description) }}
            />
          </div>
        )}

        {workItem.acceptance_criteria && (
          <div className="peek-section">
            <div className="peek-section-label">Acceptance Criteria</div>
            <div
              className="peek-markdown"
              dangerouslySetInnerHTML={{ __html: renderMarkdown(workItem.acceptance_criteria) }}
            />
          </div>
        )}

        <div className="peek-action-bar">
          <Button
            size="sm"
            onClick={() => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
          >
            Launch Agent
          </Button>
        </div>
      </div>
    </>
  );
}

// ── Peek: Inbox ───────────────────────────────────────────────────────────────

function PeekInbox({ projectId }: { projectId: string }) {
  const entries = useAppStore((s) => s.inboxEntriesByProject[projectId] ?? []);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const isLoading = loadingKeys[`inbox:${projectId}`] ?? false;

  useEffect(() => {
    void appStore.handleLoadInboxEntries(projectId);
  }, [projectId]);

  function handlePopOut() {
    navStore.closePeek();
    navStore.goToInbox(projectId);
  }

  const sortedEntries = useMemo(
    () => [...entries].sort((a, b) => b.created_at - a.created_at).slice(0, 20),
    [entries],
  );

  return (
    <>
      <PeekHeader title={`Inbox (${entries.length})`} onPopOut={handlePopOut} />
      <div className="peek-body">
        {isLoading && entries.length === 0 && (
          <div className="peek-loading">Loading...</div>
        )}
        {!isLoading && sortedEntries.length === 0 && (
          <div className="peek-empty">No inbox entries.</div>
        )}
        <div className="peek-inbox-list">
          {sortedEntries.map((entry) => {
            const isUnread = entry.read_at === null;
            const statusClass =
              entry.status === "success"
                ? "inbox-badge-success"
                : entry.status === "needs_review"
                  ? "inbox-badge-needs-review"
                  : "inbox-badge-default";

            return (
              <div key={entry.id} className={`peek-inbox-entry${isUnread ? " peek-inbox-unread" : ""}`}>
                <div className="peek-inbox-entry-header">
                  {isUnread && <span className="inbox-unread-dot" />}
                  <span className="peek-inbox-entry-title">{entry.title}</span>
                  <span className={`inbox-badge ${statusClass}`}>
                    {entry.status.replace("_", " ")}
                  </span>
                </div>
                <div className="peek-inbox-entry-meta">
                  {entry.work_item_callsign && (
                    <span className="inbox-callsign">{entry.work_item_callsign}</span>
                  )}
                  <span className="peek-inbox-timestamp">
                    {formatTimestamp(entry.created_at)}
                  </span>
                </div>
                {entry.summary && (
                  <p className="peek-inbox-summary">{entry.summary}</p>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}

// ── Peek: Document ────────────────────────────────────────────────────────────

function PeekDocument({ projectId, documentId }: { projectId: string; documentId: string }) {
  const doc = useAppStore((s) => s.documentDetails[documentId] ?? null);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const isLoading = !!loadingKeys[`document:${documentId}`];

  useEffect(() => {
    void appStore.ensureDocumentDetail(documentId);
  }, [documentId]);

  function handlePopOut() {
    navStore.closePeek();
    navStore.goToDocument(projectId, documentId);
  }

  if (isLoading && !doc) {
    return (
      <>
        <PeekHeader title="Document" />
        <div className="peek-body peek-loading">Loading...</div>
      </>
    );
  }
  if (!doc) {
    return (
      <>
        <PeekHeader title="Document" />
        <div className="peek-body peek-empty">Document not found.</div>
      </>
    );
  }

  return (
    <>
      <PeekHeader title={doc.title} onPopOut={handlePopOut} />
      <div className="peek-body">
        <div className="peek-doc-meta">
          <span className="doc-type-chip">{doc.doc_type}</span>
          <span className="doc-status-badge">{doc.status}</span>
        </div>
        <div
          className="peek-markdown"
          dangerouslySetInnerHTML={{ __html: renderMarkdown(doc.content_markdown) }}
        />
      </div>
    </>
  );
}

// ── Peek: Session Detail ──────────────────────────────────────────────────────

function PeekSessionDetail({ projectId, sessionId }: { projectId: string; sessionId: string }) {
  const session = useAppStore((s) => s.sessions.find((ss) => ss.id === sessionId) ?? null);

  function handlePopOut() {
    navStore.closePeek();
    navStore.goToAgent(projectId, sessionId);
  }

  if (!session) {
    return (
      <>
        <PeekHeader title="Session" />
        <div className="peek-body peek-empty">Session not found.</div>
      </>
    );
  }

  return (
    <>
      <PeekHeader title={session.title ?? "Session"} onPopOut={handlePopOut} />
      <div className="peek-body">
        <div className="peek-session-meta">
          <div className="peek-meta-row">
            <span className="peek-meta-label">Status</span>
            <StatusBadge status={session.runtime_state} />
          </div>
          <div className="peek-meta-row">
            <span className="peek-meta-label">Mode</span>
            <span className="peek-meta-value">{session.current_mode}</span>
          </div>
          {session.worktree_branch && (
            <div className="peek-meta-row">
              <span className="peek-meta-label">Branch</span>
              <code className="peek-meta-value">{session.worktree_branch}</code>
            </div>
          )}
          {session.started_at && (
            <div className="peek-meta-row">
              <span className="peek-meta-label">Started</span>
              <span className="peek-meta-value">{formatTimestamp(session.started_at)}</span>
            </div>
          )}
          <div className="peek-meta-row">
            <span className="peek-meta-label">Duration</span>
            <span className="peek-meta-value">
              {formatDuration(session.started_at, session.ended_at)}
            </span>
          </div>
          {session.work_item_id && (
            <div className="peek-meta-row">
              <span className="peek-meta-label">Work Item</span>
              <button
                className="peek-link"
                onClick={() =>
                  navStore.openPeek({
                    peek: "work_item",
                    projectId,
                    workItemId: session.work_item_id!,
                  })
                }
              >
                {session.work_item_id.slice(0, 8)}...
              </button>
            </div>
          )}
        </div>

        <div className="peek-action-bar">
          <Button size="sm" variant="secondary" onClick={handlePopOut}>
            View Terminal
          </Button>
        </div>
      </div>
    </>
  );
}

// ── Peek: Merge Diff ──────────────────────────────────────────────────────────

function PeekMergeDiff({ projectId, entryId }: { projectId: string; entryId: string }) {
  const mergeEntries = useAppStore((s) => s.mergeQueueByProject[projectId] ?? []);
  const diff = useAppStore((s) => s.mergeQueueDiffs[entryId]);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const diffLoading = loadingKeys[`merge-diff:${entryId}`] ?? false;
  const merging = loadingKeys[`merge:${entryId}`] ?? false;

  const entry = useMemo(
    () => mergeEntries.find((e) => e.id === entryId) ?? null,
    [mergeEntries, entryId],
  );

  useEffect(() => {
    if (diff === undefined) {
      void appStore.handleLoadMergeQueueDiff(entryId);
    }
  }, [entryId, diff]);

  const title = entry
    ? entry.work_item_callsign
      ? `${entry.work_item_callsign}: ${entry.session_title ?? entry.branch_name}`
      : entry.session_title ?? entry.branch_name
    : "Merge Diff";

  return (
    <>
      <PeekHeader title={title} />
      <div className="peek-body">
        {entry && (
          <div className="peek-merge-meta">
            <code className="peek-merge-branch">{entry.branch_name}</code>
            <StatusBadge status={entry.status} className={`mq-badge mq-badge-${entry.status}`} />
            <DiffStats stat={entry.diff_stat} />
          </div>
        )}

        <div className="peek-merge-diff-container">
          <MergeDiffViewer diff={diff} loading={diffLoading} />
        </div>

        {entry && (
          <div className="peek-action-bar">
            {entry.status === "ready" && (
              <Button
                size="sm"
                onClick={() => void appStore.handleMergeQueueMerge(entryId, projectId)}
                disabled={merging}
              >
                {merging ? "Merging..." : "Merge"}
              </Button>
            )}
            {entry.status !== "parked" && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => void appStore.handleMergeQueuePark(entryId, projectId)}
              >
                Park
              </Button>
            )}
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void appStore.handleMergeQueueCheckConflicts(entryId, projectId)}
            >
              Check Conflicts
            </Button>
          </div>
        )}
      </div>
    </>
  );
}

// ── Peek: Project Settings ────────────────────────────────────────────────────

function PeekProjectSettings({ projectId }: { projectId: string }) {
  const project = useAppStore(
    (s) => s.bootstrap?.projects.find((p) => p.id === projectId) ?? null,
  );

  function handlePopOut() {
    navStore.closePeek();
    navStore.goToProjectSettings(projectId);
  }

  return (
    <>
      <PeekHeader title={project ? `${project.name} Settings` : "Project Settings"} onPopOut={handlePopOut} />
      <div className="peek-body">
        {project ? (
          <div className="peek-session-meta">
            <div className="peek-meta-row">
              <span className="peek-meta-label">Name</span>
              <span className="peek-meta-value">{project.name}</span>
            </div>
            <div className="peek-meta-row">
              <span className="peek-meta-label">Slug</span>
              <code className="peek-meta-value">{project.slug}</code>
            </div>
            <div className="peek-meta-row">
              <span className="peek-meta-label">Created</span>
              <span className="peek-meta-value">{formatTimestamp(project.created_at)}</span>
            </div>
          </div>
        ) : (
          <div className="peek-empty">Project not found.</div>
        )}
        <div className="peek-action-bar">
          <Button size="sm" variant="secondary" onClick={handlePopOut}>
            Open Settings
          </Button>
        </div>
      </div>
    </>
  );
}

// ── Router ────────────────────────────────────────────────────────────────────

export function PeekRouter({ peek }: { peek: NonNullable<PeekLayer> }) {
  switch (peek.peek) {
    case "work_item":
      return <PeekWorkItem projectId={peek.projectId} workItemId={peek.workItemId} />;
    case "inbox":
      return <PeekInbox projectId={peek.projectId} />;
    case "document":
      return <PeekDocument projectId={peek.projectId} documentId={peek.documentId} />;
    case "session_detail":
      return <PeekSessionDetail projectId={peek.projectId} sessionId={peek.sessionId} />;
    case "merge_diff":
      return <PeekMergeDiff projectId={peek.projectId} entryId={peek.entryId} />;
    case "project_settings":
      return <PeekProjectSettings projectId={peek.projectId} />;
  }
}
