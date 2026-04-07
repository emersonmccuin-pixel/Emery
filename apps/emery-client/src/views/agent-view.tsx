import { useEffect, useMemo } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { useSessionSnapshot } from "../session-store";
import { AgentInfoBar } from "../components/agent-info-bar";
import { AgentTerminal } from "../components/agent-terminal";
import { AgentControls } from "../components/agent-controls";

export function AgentView({
  projectId,
  sessionId,
}: {
  projectId: string;
  sessionId: string;
}) {
  const sessionSnapshot = useSessionSnapshot(sessionId);

  const sessionSummary = useAppStore((s) =>
    s.sessions.find((entry) => entry.id === sessionId) ?? null,
  );
  const workItemDetails = useAppStore((s) => s.workItemDetails);

  const callsign = useMemo(() => {
    if (!sessionSummary?.work_item_id) return null;
    const wi = workItemDetails[sessionSummary.work_item_id];
    return wi?.callsign ?? null;
  }, [sessionSummary, workItemDetails]);

  const branch = sessionSummary?.worktree_branch ?? null;

  useEffect(() => {
    if (
      sessionSummary?.work_item_id &&
      !workItemDetails[sessionSummary.work_item_id]
    ) {
      void appStore.ensureWorkItemDetail(sessionSummary.work_item_id);
    }
  }, [sessionSummary?.work_item_id, workItemDetails]);

  useEffect(() => {
    if (!sessionSnapshot) {
      void appStore.ensureSessionSnapshot(sessionId);
    }
  }, [sessionId, sessionSnapshot]);

  if (!sessionSnapshot) {
    return <div className="empty-pane">Loading session…</div>;
  }

  const live = sessionSnapshot.live;

  return (
    <div className="content-frame-full">
    <div className="agent-view">
      <AgentInfoBar
        callsign={callsign}
        title={sessionSnapshot.title}
        branch={branch}
        agentKind={sessionSummary?.agent_kind ?? "unknown"}
        startedAt={sessionSummary?.started_at ?? null}
        runtimeState={sessionSnapshot.runtime_state}
        activityState={sessionSnapshot.activity_state}
        needsInputReason={sessionSnapshot.needs_input_reason}
        tabStatus={sessionSnapshot.tab_status}
        live={live}
        endedAt={sessionSummary?.ended_at ?? null}
        projectId={projectId}
        workItemId={sessionSummary?.work_item_id ?? null}
      />

      <div className="agent-terminal-area">
        <AgentTerminal sessionId={sessionId} live={live} />
        <span className="agent-escape-hint">Double-Escape to exit</span>
      </div>

      <AgentControls
        live={live}
        runtimeState={sessionSnapshot.runtime_state}
        onInterrupt={() => void appStore.handleInterruptSession(sessionId)}
        onTerminate={() => void appStore.handleTerminateSession(sessionId)}
        onDetach={() => navStore.goBack()}
      />
    </div>
    </div>
  );
}
