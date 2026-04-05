import { useState } from "react";
import type { MergeQueueEntry } from "./types";

const STATUS_ORDER: Record<string, number> = {
  ready: 0,
  pending: 1,
  conflict: 2,
  merging: 3,
  parked: 4,
  merged: 5,
};

function statusBadgeClass(status: string): string {
  switch (status) {
    case "ready":
      return "mq-badge mq-badge-ready";
    case "pending":
      return "mq-badge mq-badge-pending";
    case "conflict":
      return "mq-badge mq-badge-conflict";
    case "merging":
      return "mq-badge mq-badge-merging";
    case "merged":
      return "mq-badge mq-badge-merged";
    case "parked":
      return "mq-badge mq-badge-parked";
    default:
      return "mq-badge";
  }
}

export function MergeQueue({
  entries,
  diffs,
  loading,
  loadingKeys,
  onMerge,
  onPark,
  onLoadDiff,
  onCheckConflicts,
}: {
  entries: MergeQueueEntry[];
  diffs: Record<string, string>;
  loading: boolean;
  loadingKeys: Record<string, boolean>;
  onMerge: (id: string) => void;
  onPark: (id: string) => void;
  onLoadDiff: (id: string) => void;
  onCheckConflicts: (id: string) => void;
}) {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const sorted = [...entries].sort(
    (a, b) => (STATUS_ORDER[a.status] ?? 99) - (STATUS_ORDER[b.status] ?? 99) || a.position - b.position,
  );

  const activeEntries = sorted.filter((e) => e.status !== "merged");
  const mergedEntries = sorted.filter((e) => e.status === "merged");

  if (loading && entries.length === 0) {
    return <div className="empty-pane">Loading merge queue...</div>;
  }

  if (entries.length === 0) {
    return <div className="empty-pane">No branches queued for merge.</div>;
  }

  function toggleExpand(entryId: string) {
    if (expandedId === entryId) {
      setExpandedId(null);
    } else {
      setExpandedId(entryId);
      if (!diffs[entryId]) {
        onLoadDiff(entryId);
      }
    }
  }

  return (
    <div className="merge-queue">
      <div className="mq-header">
        <h2>Merge queue</h2>
        <span className="mq-count">{activeEntries.length} pending</span>
      </div>

      {activeEntries.length > 0 ? (
        <div className="mq-list">
          {activeEntries.map((entry) => (
            <div key={entry.id} className="mq-entry">
              <div className="mq-entry-header">
                <button className="mq-entry-toggle" onClick={() => toggleExpand(entry.id)}>
                  <span className="mq-entry-expand">{expandedId === entry.id ? "\u25BC" : "\u25B6"}</span>
                  <span className="mq-entry-title">
                    {entry.work_item_callsign ? `${entry.work_item_callsign} \u00B7 ` : ""}
                    {entry.session_title ?? entry.branch_name}
                  </span>
                </button>

                <span className="mq-entry-branch">{entry.branch_name}</span>
                <span className={statusBadgeClass(entry.status)}>{entry.status}</span>

                {entry.diff_stat ? (
                  <span className="mq-diff-stat">
                    <span className="mq-files">{entry.diff_stat.files_changed} file{entry.diff_stat.files_changed !== 1 ? "s" : ""}</span>
                    {entry.diff_stat.insertions > 0 ? (
                      <span className="mq-ins">+{entry.diff_stat.insertions}</span>
                    ) : null}
                    {entry.diff_stat.deletions > 0 ? (
                      <span className="mq-del">-{entry.diff_stat.deletions}</span>
                    ) : null}
                  </span>
                ) : null}

                {entry.has_uncommitted_changes ? (
                  <span className="mq-warning" title="Worktree has uncommitted changes">!</span>
                ) : null}

                <div className="mq-actions">
                  {entry.status === "ready" ? (
                    <button
                      className="mq-action-button mq-merge-button"
                      onClick={() => onMerge(entry.id)}
                      disabled={Boolean(loadingKeys[`merge:${entry.id}`])}
                    >
                      {loadingKeys[`merge:${entry.id}`] ? "Merging..." : "Merge"}
                    </button>
                  ) : null}
                  <button
                    className="mq-action-button"
                    onClick={() => onCheckConflicts(entry.id)}
                    disabled={Boolean(loadingKeys[`merge-conflicts:${entry.id}`])}
                  >
                    Check
                  </button>
                  {entry.status !== "parked" ? (
                    <button className="mq-action-button" onClick={() => onPark(entry.id)}>
                      Park
                    </button>
                  ) : null}
                </div>
              </div>

              {entry.conflict_files && entry.conflict_files.length > 0 ? (
                <div className="mq-conflicts">
                  <span className="mq-conflict-label">Conflicts:</span>
                  {entry.conflict_files.map((file) => (
                    <span key={file} className="mq-conflict-file">{file}</span>
                  ))}
                </div>
              ) : null}

              {expandedId === entry.id ? (
                <div className="mq-diff-container">
                  {loadingKeys[`merge-diff:${entry.id}`] ? (
                    <div className="mq-diff-loading">Loading diff...</div>
                  ) : diffs[entry.id] ? (
                    <pre className="mq-diff">{diffs[entry.id]}</pre>
                  ) : (
                    <div className="mq-diff-loading">No diff available</div>
                  )}
                </div>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}

      {mergedEntries.length > 0 ? (
        <details className="mq-merged-section">
          <summary className="mq-merged-summary">
            {mergedEntries.length} merged
          </summary>
          <div className="mq-list">
            {mergedEntries.map((entry) => (
              <div key={entry.id} className="mq-entry mq-entry-merged">
                <div className="mq-entry-header">
                  <span className="mq-entry-title">
                    {entry.work_item_callsign ? `${entry.work_item_callsign} \u00B7 ` : ""}
                    {entry.session_title ?? entry.branch_name}
                  </span>
                  <span className={statusBadgeClass(entry.status)}>merged</span>
                </div>
              </div>
            ))}
          </div>
        </details>
      ) : null}
    </div>
  );
}
