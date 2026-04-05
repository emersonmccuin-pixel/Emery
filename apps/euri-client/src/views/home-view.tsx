import { useAppStore } from "../store";
import { navStore } from "../nav-store";
import type { ProjectSummary, SessionSummary, MergeQueueEntry } from "../types";

export function HomeView() {
  const projects = useAppStore((s) => s.bootstrap?.projects ?? []);
  const sessions = useAppStore((s) => s.sessions);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);

  return (
    <div className="home-view">
      <div className="focus-card-grid">
        {projects.map((project) => (
          <FocusCard
            key={project.id}
            project={project}
            sessions={sessions}
            mergeQueueEntries={mergeQueueByProject[project.id] ?? []}
            onClick={() => navStore.goToProject(project.id)}
          />
        ))}
      </div>
      {projects.length === 0 ? (
        <div className="empty-pane">No projects yet. Create one to get started.</div>
      ) : null}
    </div>
  );
}

function FocusCard({
  project,
  sessions,
  mergeQueueEntries,
  onClick,
}: {
  project: ProjectSummary;
  sessions: SessionSummary[];
  mergeQueueEntries: MergeQueueEntry[];
  onClick: () => void;
}) {
  const liveAgents = sessions.filter(
    (s) => s.project_id === project.id && s.live,
  );
  const pendingMerges = mergeQueueEntries.filter(
    (e) => e.status !== "merged",
  ).length;

  const status = liveAgents.length > 0
    ? "active"
    : pendingMerges > 0
      ? "queued"
      : "idle";

  return (
    <button className={`focus-card focus-card-${status}`} onClick={onClick}>
      <div className="focus-card-header">
        <span className="focus-card-name">{project.name}</span>
      </div>
      <div className="focus-card-indicators">
        {liveAgents.length > 0 ? (
          <span className="focus-card-agents">
            {liveAgents.slice(0, 5).map((_, i) => (
              <span key={i} className="agent-dot pulsing" />
            ))}
            {liveAgents.length > 5 ? (
              <span className="agent-dot-overflow">+{liveAgents.length - 5}</span>
            ) : null}
          </span>
        ) : null}
        {pendingMerges > 0 ? (
          <span className="focus-card-merges">
            {pendingMerges} merge{pendingMerges !== 1 ? "s" : ""}
          </span>
        ) : null}
      </div>
      <div className="focus-card-status">
        <span className={`focus-card-badge focus-card-badge-${status}`}>
          {status}
        </span>
      </div>
    </button>
  );
}
