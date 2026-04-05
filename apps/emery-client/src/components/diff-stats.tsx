import type { MergeQueueDiffStat } from "../types";

export function DiffStats({ stat }: { stat: MergeQueueDiffStat | null }) {
  if (!stat) return null;
  return (
    <span className="diff-stats">
      <span className="diff-files">
        {stat.files_changed} file{stat.files_changed !== 1 ? "s" : ""}
      </span>
      {stat.insertions > 0 ? <span className="diff-ins">+{stat.insertions}</span> : null}
      {stat.deletions > 0 ? <span className="diff-del">-{stat.deletions}</span> : null}
    </span>
  );
}
