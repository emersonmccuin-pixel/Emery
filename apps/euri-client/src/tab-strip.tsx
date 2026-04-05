import { appStore, useAppStore, sessionTone, resourceLabel } from "./store";

export function TabStrip() {
  const openResources = useAppStore((s) => s.openResources);
  const activeResourceId = useAppStore((s) => s.activeResourceId);
  const sessions = useAppStore((s) => s.sessions);
  const workItemDetails = useAppStore((s) => s.workItemDetails);
  const documentDetails = useAppStore((s) => s.documentDetails);
  const bootstrap = useAppStore((s) => s.bootstrap);

  return (
    <div className="tab-strip">
      {openResources.map((resource) => {
        const isSession = resource.resource_type === "session_terminal";
        const sessionForTab = isSession ? sessions.find((s) => s.id === resource.session_id) : null;
        const tabTone = sessionForTab ? sessionTone(sessionForTab) : null;
        return (
          <button
            key={resource.resource_id}
            className={`tab ${resource.resource_id === activeResourceId ? "active" : ""}`}
            onClick={() => appStore.setActiveResourceId(resource.resource_id)}
          >
            {tabTone ? <span className={`indicator ${tabTone}`} /> : null}
            {resourceLabel(
              resource,
              sessions,
              workItemDetails,
              documentDetails,
              bootstrap?.projects.find(
                (project) => resource.resource_type === "project_home" && project.id === resource.project_id,
              )?.name,
            )}
            <span
              className="tab-close"
              onClick={(event) => {
                event.stopPropagation();
                appStore.closeResource(resource);
              }}
            >
              ×
            </span>
          </button>
        );
      })}
    </div>
  );
}
