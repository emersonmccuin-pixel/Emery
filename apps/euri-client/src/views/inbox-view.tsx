import { useEffect, useState } from "react";
import { useAppStore, appStore } from "../store";
import type { InboxEntrySummary } from "../lib";

type StatusFilter = "all" | "success" | "needs_review";

function formatTimestamp(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function InboxEntryRow({
  entry,
  projectId,
}: {
  entry: InboxEntrySummary;
  projectId: string;
}) {
  const [expanded, setExpanded] = useState(false);

  function handleMarkRead() {
    const nowSecs = Math.floor(Date.now() / 1000);
    void appStore.handleUpdateInboxEntry(
      entry.id,
      { read_at: nowSecs },
      projectId,
    );
  }

  function handleMarkResolved() {
    const nowSecs = Math.floor(Date.now() / 1000);
    void appStore.handleUpdateInboxEntry(
      entry.id,
      { status: "resolved", resolved_at: nowSecs },
      projectId,
    );
  }

  const isUnread = entry.read_at === null;
  const statusClass = entry.status === "success"
    ? "inbox-badge-success"
    : entry.status === "needs_review"
    ? "inbox-badge-needs-review"
    : entry.status === "resolved"
    ? "inbox-badge-resolved"
    : "inbox-badge-default";

  return (
    <div className={`inbox-entry${isUnread ? " inbox-entry-unread" : ""}`}>
      <div className="inbox-entry-header">
        <button
          className="inbox-entry-toggle"
          onClick={() => setExpanded((e) => !e)}
          aria-expanded={expanded}
        >
          <span className="inbox-entry-expand">{expanded ? "▾" : "▸"}</span>
          {isUnread && <span className="inbox-unread-dot" title="Unread" />}
          <span className="inbox-entry-title">{entry.title}</span>
        </button>
        <span className={`inbox-badge ${statusClass}`}>{entry.status.replace("_", " ")}</span>
        {entry.work_item_callsign && (
          <span className="inbox-callsign">{entry.work_item_callsign}</span>
        )}
        <span className="inbox-timestamp">{formatTimestamp(entry.created_at)}</span>
        <div className="inbox-actions">
          {isUnread && (
            <button className="inbox-action-btn" onClick={handleMarkRead} title="Mark as read">
              mark read
            </button>
          )}
          {entry.status !== "resolved" && (
            <button className="inbox-action-btn" onClick={handleMarkResolved} title="Mark as resolved">
              resolve
            </button>
          )}
        </div>
      </div>
      {expanded && (
        <div className="inbox-entry-body">
          {entry.summary ? (
            <p className="inbox-summary-text">{entry.summary}</p>
          ) : (
            <p className="inbox-summary-empty">No summary provided.</p>
          )}
          {entry.branch_name && (
            <div className="inbox-meta-row">
              <span className="inbox-meta-label">branch</span>
              <code className="inbox-branch">{entry.branch_name}</code>
            </div>
          )}
          {entry.session_title && (
            <div className="inbox-meta-row">
              <span className="inbox-meta-label">session</span>
              <span className="inbox-meta-value">{entry.session_title}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function InboxView({ projectId }: { projectId: string }) {
  const inboxEntriesByProject = useAppStore((s) => s.inboxEntriesByProject);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");

  const isLoading = loadingKeys[`inbox:${projectId}`] ?? false;
  const allEntries = inboxEntriesByProject[projectId] ?? [];

  useEffect(() => {
    void appStore.handleLoadInboxEntries(projectId);
    void appStore.handleLoadInboxUnreadCount(projectId);
  }, [projectId]);

  const filteredEntries = allEntries.filter((entry) => {
    if (statusFilter === "all") return true;
    if (statusFilter === "success") return entry.status === "success";
    if (statusFilter === "needs_review") return entry.status === "needs_review";
    return true;
  });

  // Sort by created_at DESC
  const sortedEntries = [...filteredEntries].sort((a, b) => b.created_at - a.created_at);

  return (
    <div className="inbox-view">
      <div className="inbox-header">
        <h2 className="inbox-title">Inbox</h2>
        <span className="inbox-count">{allEntries.length} entries</span>
      </div>
      <div className="inbox-filters">
        {(["all", "success", "needs_review"] as StatusFilter[]).map((f) => (
          <button
            key={f}
            className={`inbox-filter-pill${statusFilter === f ? " active" : ""}`}
            onClick={() => setStatusFilter(f)}
          >
            {f === "all" ? "All" : f === "success" ? "Success" : "Needs Review"}
          </button>
        ))}
      </div>
      {isLoading ? (
        <div className="inbox-loading">Loading…</div>
      ) : sortedEntries.length === 0 ? (
        <div className="inbox-empty">
          <p>No inbox entries{statusFilter !== "all" ? ` with status "${statusFilter.replace("_", " ")}"` : ""}.</p>
        </div>
      ) : (
        <div className="inbox-list">
          {sortedEntries.map((entry) => (
            <InboxEntryRow key={entry.id} entry={entry} projectId={projectId} />
          ))}
        </div>
      )}
    </div>
  );
}
