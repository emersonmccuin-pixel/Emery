import React, { useState } from "react";
import type {
  AccountSummary,
  ConflictWarning,
  ProjectDetail,
  WorkItemSummary,
} from "./types";
import { useAppStore, appStore } from "./store";
import { Button } from "./components/ui";

export function DispatchSheet() {
  const pendingDispatch = useAppStore((s) => s.pendingDispatch);
  const projectDetails = useAppStore((s) => s.projectDetails);
  const workItemDetails = useAppStore((s) => s.workItemDetails);
  const bootstrap = useAppStore((s) => s.bootstrap);
  const dispatchConflicts = useAppStore((s) => s.dispatchConflicts);

  if (!pendingDispatch) return null;

  const project = projectDetails[pendingDispatch.projectId];
  if (!project) return null;

  const accounts = bootstrap?.accounts ?? [];
  const defaultAccount =
    accounts.find((a) => a.id === project.default_account_id) ?? accounts[0] ?? null;

  if (pendingDispatch.mode === "single") {
    const workItem = workItemDetails[pendingDispatch.workItemId];
    if (!workItem) return null;

    return (
      <SingleDispatchSheet
        workItem={workItem}
        project={project}
        account={defaultAccount}
        originMode={pendingDispatch.originMode}
        onConfirm={(opts) => void appStore.confirmDispatch(opts)}
        onCancel={() => appStore.cancelDispatch()}
      />
    );
  }

  // Multi-dispatch
  const workItems = pendingDispatch.workItemIds
    .map((id) => workItemDetails[id])
    .filter(Boolean) as WorkItemSummary[];

  return (
    <MultiDispatchSheet
      workItems={workItems}
      project={project}
      accounts={accounts}
      defaultAccount={defaultAccount}
      conflicts={dispatchConflicts}
      onConfirm={(dispatches) => void appStore.confirmMultiDispatch(dispatches)}
      onCancel={() => appStore.cancelDispatch()}
    />
  );
}

// --- Stop rules helper ---

function getDefaultStopRules(originMode: string): string[] {
  switch (originMode) {
    case "planning":
      return ["No file writes", "No shell execution", "Return plan as text only"];
    case "research":
      return ["No file writes", "Read-only tools only", "Return findings as text"];
    case "execution":
      return ["Implement only assigned task", "No merging/pushing", "No external system updates"];
    case "follow_up":
      return ["Address only the follow-up issue", "No scope expansion"];
    default:
      return [];
  }
}

// --- Single dispatch (existing behavior) ---

const SAFETY_MODE_DESCRIPTIONS: Record<string, string> = {
  full: "Agent reads, writes, and executes without asking",
  permissive: "Agent asks before destructive operations",
  none: "Agent can read files but cannot write or execute",
};

function safetyModeDescription(mode: string): string {
  return SAFETY_MODE_DESCRIPTIONS[mode] ?? "";
}

function SingleDispatchSheet({
  workItem,
  project,
  account,
  originMode,
  onConfirm,
  onCancel,
}: {
  workItem: WorkItemSummary;
  project: ProjectDetail;
  account: AccountSummary | null;
  originMode: string;
  onConfirm: (opts: { autoWorktree: boolean; originMode: string; safetyMode?: string; model?: string }) => void;
  onCancel: () => void;
}) {
  const [safetyMode, setSafetyMode] = useState<string>("");
  const [model, setModel] = useState<string>("");
  const [confirmClose, setConfirmClose] = useState(false);
  const branchName = `euri/${workItem.callsign.toLowerCase()}`;
  const root = project.roots[0] ?? null;
  const isExecution = originMode === "execution";
  const resolvedDefault = account?.default_safety_mode ?? "full";
  const resolvedDefaultModel = isExecution ? "sonnet" : "opus";

  const isDirty = safetyMode !== "" || model !== "";

  function handleBackdropClick() {
    if (!isDirty) {
      onCancel();
      return;
    }
    if (confirmClose) {
      onCancel();
    } else {
      setConfirmClose(true);
      setTimeout(() => setConfirmClose(false), 3000);
    }
  }

  return (
    <div className="dispatch-overlay" onClick={handleBackdropClick}>
      <div className="dispatch-sheet" onClick={(e: React.MouseEvent<HTMLDivElement>) => e.stopPropagation()}>
        <h3>Launch Session</h3>
        <div className="dispatch-details">
          <div className="dispatch-row">
            <span className="dispatch-label">Work Item</span>
            <span>{workItem.callsign} &middot; {workItem.title}</span>
          </div>
          {isExecution ? (
            <>
              <div className="dispatch-row">
                <span className="dispatch-label">Branch</span>
                <code>{branchName}</code>
              </div>
              <div className="dispatch-row">
                <span className="dispatch-label">Location</span>
                <span>Worktree (new branch from HEAD)</span>
              </div>
            </>
          ) : (
            <div className="dispatch-row">
              <span className="dispatch-label">Location</span>
              <span>Project root (main)</span>
            </div>
          )}
          <div className="dispatch-row">
            <span className="dispatch-label">Account</span>
            <span>{account?.label ?? account?.agent_kind ?? "\u2014"}</span>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Base</span>
            <span>Current HEAD of {root?.path ?? "project root"}</span>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Safety Mode</span>
            <div>
              <select value={safetyMode} onChange={(e) => setSafetyMode(e.target.value)}>
                <option value="">Default ({resolvedDefault})</option>
                <option value="full">Autonomous</option>
                <option value="permissive">Supervised</option>
                <option value="none">Read Only</option>
              </select>
              {(safetyMode || resolvedDefault) && (
                <div style={{ fontSize: "0.72rem", color: "var(--text-secondary)", marginTop: "2px" }}>
                  {safetyModeDescription(safetyMode || resolvedDefault)}
                </div>
              )}
            </div>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Model</span>
            <select value={model} onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setModel(e.target.value)}>
              <option value="">Default ({resolvedDefaultModel})</option>
              <option value="opus">Opus</option>
              <option value="sonnet">Sonnet</option>
              <option value="haiku">Haiku</option>
            </select>
          </div>
          {getDefaultStopRules(originMode).length > 0 && (
            <div className="dispatch-row">
              <span className="dispatch-label">Stop Rules</span>
              <span className="dispatch-stop-rules">
                {getDefaultStopRules(originMode).map((rule, i) => (
                  <span key={i} className="dispatch-stop-rule">{rule}</span>
                ))}
              </span>
            </div>
          )}
        </div>
        <div className="dispatch-actions">
          <Button
            className={confirmClose ? "dispatch-discard-confirm" : undefined}
            variant="secondary"
            onClick={() => {
              if (isDirty && !confirmClose) {
                setConfirmClose(true);
                setTimeout(() => setConfirmClose(false), 3000);
              } else {
                onCancel();
              }
            }}
          >
            {confirmClose ? "Discard changes?" : "Cancel"}
          </Button>
          {account ? (
            <Button variant="secondary" onClick={() => onConfirm({ autoWorktree: isExecution, originMode, safetyMode: safetyMode || undefined, model: model || undefined })}>
              {isExecution ? "Launch on Branch" : "Launch on Main"}
            </Button>
          ) : (
            <span className="dispatch-no-accounts">No accounts available. Add one in Settings &gt; Accounts.</span>
          )}
        </div>
      </div>
    </div>
  );
}

// --- Multi dispatch (new) ---

function MultiDispatchSheet({
  workItems,
  accounts,
  defaultAccount,
  conflicts,
  onConfirm,
  onCancel,
}: {
  workItems: WorkItemSummary[];
  project: ProjectDetail;
  accounts: AccountSummary[];
  defaultAccount: AccountSummary | null;
  conflicts: ConflictWarning[];
  onConfirm: (dispatches: Array<{ workItemId: string; accountId: string; agentKind: string; safetyMode?: string; model?: string }>) => void;
  onCancel: () => void;
}) {
  const [itemAccounts, setItemAccounts] = useState<Record<string, string>>(() => {
    const initial: Record<string, string> = {};
    for (const item of workItems) {
      initial[item.id] = defaultAccount?.id ?? "";
    }
    return initial;
  });
  const [safetyMode, setSafetyMode] = useState<string>("");
  const [model, setModel] = useState<string>("");
  const resolvedDefault = defaultAccount?.default_safety_mode ?? "full";

  function handleConfirm() {
    const dispatches = workItems.map((item) => {
      const accountId = itemAccounts[item.id] ?? defaultAccount?.id ?? "";
      const account = accounts.find((a) => a.id === accountId);
      return {
        workItemId: item.id,
        accountId,
        agentKind: account?.agent_kind ?? "claude-code",
        safetyMode: safetyMode || undefined,
        model: model || undefined,
      };
    });
    onConfirm(dispatches);
  }

  return (
    <div className="dispatch-overlay" onClick={onCancel}>
      <div className="dispatch-sheet dispatch-sheet-multi" onClick={(e) => e.stopPropagation()}>
        <h3>Launch {workItems.length} Sessions</h3>

        {conflicts.length > 0 ? (
          <div className="dispatch-conflicts">
            <span className="dispatch-conflict-header">File overlap detected:</span>
            {conflicts.map((c, i) => (
              <div key={i} className="dispatch-conflict-row">
                <strong>{c.item_a}</strong> and <strong>{c.item_b}</strong> both touch:{" "}
                {c.overlapping_files.map((f) => (
                  <code key={f} className="dispatch-conflict-file">{f}</code>
                ))}
              </div>
            ))}
          </div>
        ) : null}

        <div className="dispatch-item-list">
          {workItems.map((item) => (
            <div key={item.id} className="dispatch-item-row">
              <div className="dispatch-item-info">
                <span className="dispatch-item-callsign">{item.callsign}</span>
                <span className="dispatch-item-title">{item.title}</span>
                <code className="dispatch-item-branch">euri/{item.callsign.toLowerCase()}</code>
              </div>
              <select
                className="dispatch-item-account"
                value={itemAccounts[item.id] ?? ""}
                onChange={(e) =>
                  setItemAccounts((prev) => ({ ...prev, [item.id]: e.target.value }))
                }
              >
                {accounts.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.label}
                  </option>
                ))}
              </select>
            </div>
          ))}
        </div>

        <div className="dispatch-details">
          <div className="dispatch-row">
            <span className="dispatch-label">Safety Mode</span>
            <div>
              <select value={safetyMode} onChange={(e) => setSafetyMode(e.target.value)}>
                <option value="">Default ({resolvedDefault})</option>
                <option value="full">Autonomous</option>
                <option value="permissive">Supervised</option>
                <option value="none">Read Only</option>
              </select>
              {(safetyMode || resolvedDefault) && (
                <div style={{ fontSize: "0.72rem", color: "var(--text-secondary)", marginTop: "2px" }}>
                  {safetyModeDescription(safetyMode || resolvedDefault)}
                </div>
              )}
            </div>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Model</span>
            <select value={model} onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setModel(e.target.value)}>
              <option value="">Default (execution → sonnet)</option>
              <option value="opus">Opus</option>
              <option value="sonnet">Sonnet</option>
              <option value="haiku">Haiku</option>
            </select>
          </div>
        </div>

        <div className="dispatch-actions">
          <Button variant="secondary" onClick={onCancel}>Cancel</Button>
          <Button variant="secondary" onClick={handleConfirm}>
            Launch All ({workItems.length} sessions)
          </Button>
        </div>
      </div>
    </div>
  );
}
