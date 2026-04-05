export function ModelBadge({ agentKind }: { agentKind: string }) {
  const short = agentKind.includes("claude") ? "Claude" : agentKind;
  return <span className="model-badge">{short}</span>;
}
