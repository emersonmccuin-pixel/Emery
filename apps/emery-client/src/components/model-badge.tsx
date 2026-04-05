export function ModelBadge({ agentKind }: { agentKind: string }) {
  const short = agentKind.includes("claude") ? "Claude"
    : agentKind === "codex" ? "Codex"
    : agentKind === "gemini" ? "Gemini"
    : agentKind;
  return <span className="model-badge">{short}</span>;
}
