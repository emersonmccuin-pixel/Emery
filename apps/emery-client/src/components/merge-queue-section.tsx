import type { MergeQueueEntry as MQEntry } from "../types";
import { MergeQueueEntry } from "./merge-queue-entry";

const STATUS_ORDER: Record<string, number> = {
  ready: 0, pending: 1, conflict: 2, merging: 3, parked: 4, merged: 5,
};

type MergeQueueSectionProps = {
  entries: MQEntry[];
  diffs: Record<string, string>;
  loadingKeys: Record<string, boolean>;
  onMerge: (id: string) => void;
  onPark: (id: string) => void;
  onLoadDiff: (id: string) => void;
  onCheckConflicts: (id: string) => void;
  onPeekDiff?: (id: string) => void;
};

export function MergeQueueSection({
  entries,
  diffs,
  loadingKeys,
  onMerge,
  onPark,
  onLoadDiff,
  onCheckConflicts,
  onPeekDiff,
}: MergeQueueSectionProps) {
  const sorted = [...entries].sort(
    (a, b) => (STATUS_ORDER[a.status] ?? 99) - (STATUS_ORDER[b.status] ?? 99) || a.position - b.position,
  );
  const active = sorted.filter((e) => e.status !== "merged");
  const merged = sorted.filter((e) => e.status === "merged");

  if (entries.length === 0) return null;

  return (
    <section className="project-section merge-queue-section">
      <div className="section-header">
        <h3>Merge Queue</h3>
        <span className="section-count">{active.length} pending</span>
      </div>
      {active.length > 0 ? (
        <div className="mq-list">
          {active.map((entry) => (
            <MergeQueueEntry
              key={entry.id}
              entry={entry}
              diff={diffs[entry.id]}
              diffLoading={Boolean(loadingKeys[`merge-diff:${entry.id}`])}
              merging={Boolean(loadingKeys[`merge:${entry.id}`])}
              checking={Boolean(loadingKeys[`merge-conflicts:${entry.id}`])}
              onMerge={() => onMerge(entry.id)}
              onPark={() => onPark(entry.id)}
              onLoadDiff={() => onLoadDiff(entry.id)}
              onCheckConflicts={() => onCheckConflicts(entry.id)}
              onPeekDiff={onPeekDiff ? () => onPeekDiff(entry.id) : undefined}
            />
          ))}
        </div>
      ) : null}
      {merged.length > 0 ? (
        <details className="mq-merged-section">
          <summary className="mq-merged-summary">{merged.length} merged</summary>
          <div className="mq-list">
            {merged.map((entry) => (
              <div key={entry.id} className="mq-entry mq-entry-merged">
                <span className="mq-entry-title">
                  {entry.work_item_callsign ? `${entry.work_item_callsign} \u00B7 ` : ""}
                  {entry.session_title ?? entry.branch_name}
                </span>
              </div>
            ))}
          </div>
        </details>
      ) : null}
    </section>
  );
}
