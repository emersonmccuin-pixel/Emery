import { useState } from "react";
import type { MergeQueueEntry as MQEntry } from "../types";
import { DiffStats } from "./diff-stats";
import { StatusBadge } from "./status-badge";
import { MergeDiffViewer } from "./merge-diff-viewer";

type MergeQueueEntryProps = {
  entry: MQEntry;
  diff: string | undefined;
  diffLoading: boolean;
  merging: boolean;
  checking: boolean;
  onMerge: () => void;
  onPark: () => void;
  onLoadDiff: () => void;
  onCheckConflicts: () => void;
};

export function MergeQueueEntry({
  entry,
  diff,
  diffLoading,
  merging,
  checking,
  onMerge,
  onPark,
  onLoadDiff,
  onCheckConflicts,
}: MergeQueueEntryProps) {
  const [expanded, setExpanded] = useState(false);

  function toggleExpand() {
    if (!expanded && diff === undefined) onLoadDiff();
    setExpanded(!expanded);
  }

  return (
    <div className="mq-entry">
      <div className="mq-entry-header">
        <button className="mq-entry-toggle" onClick={toggleExpand}>
          <span className="mq-entry-expand">{expanded ? "\u25BC" : "\u25B6"}</span>
          <span className="mq-entry-title">
            {entry.work_item_callsign ? `${entry.work_item_callsign} \u00B7 ` : ""}
            {entry.session_title ?? entry.branch_name}
          </span>
        </button>
        <code className="mq-entry-branch">{entry.branch_name}</code>
        <StatusBadge status={entry.status} className={`mq-badge mq-badge-${entry.status}`} />
        <DiffStats stat={entry.diff_stat} />
        {entry.has_uncommitted_changes ? (
          <span className="mq-warning" title="Worktree has uncommitted changes">!</span>
        ) : null}
        <div className="mq-actions">
          {entry.status === "ready" ? (
            <button className="mq-action-button mq-merge-button" onClick={onMerge} disabled={merging}>
              {merging ? "Merging…" : "Merge"}
            </button>
          ) : null}
          <button className="mq-action-button" onClick={onCheckConflicts} disabled={checking}>
            Check
          </button>
          {entry.status !== "parked" ? (
            <button className="mq-action-button" onClick={onPark}>Park</button>
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
      {expanded ? (
        <div className="mq-diff-container">
          <MergeDiffViewer diff={diff} loading={diffLoading} />
        </div>
      ) : null}
    </div>
  );
}
