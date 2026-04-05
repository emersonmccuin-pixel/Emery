export function MergeDiffViewer({
  diff,
  loading,
}: {
  diff: string | undefined;
  loading: boolean;
}) {
  if (loading) return <div className="mq-diff-loading">Loading diff…</div>;
  if (diff) return <pre className="mq-diff">{diff}</pre>;
  return <div className="mq-diff-loading">No diff available</div>;
}
