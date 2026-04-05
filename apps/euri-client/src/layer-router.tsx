import { lazy, Suspense } from "react";
import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";

const ProjectCommandView = lazy(async () => {
  const module = await import("./views/project-command-view");
  return { default: module.ProjectCommandView };
});

const ProjectSettingsView = lazy(async () => {
  const module = await import("./views/project-settings-view");
  return { default: module.ProjectSettingsView };
});

const AgentView = lazy(async () => {
  const module = await import("./views/agent-view");
  return { default: module.AgentView };
});

const DocumentView = lazy(async () => {
  const module = await import("./views/document-view");
  return { default: module.DocumentView };
});

const WorkItemView = lazy(async () => {
  const module = await import("./views/work-item-view");
  return { default: module.WorkItemView };
});

const SettingsView = lazy(async () => {
  const module = await import("./views/settings-view");
  return { default: module.SettingsView };
});

export function LayerRouter() {
  const layer = useNavLayer();

  if (layer.layer === "home") {
    return <HomeView />;
  }

  switch (layer.layer) {
    case "project":
      return (
        <Suspense fallback={<div className="layer-stub">Loading project...</div>}>
          <ProjectCommandView projectId={layer.projectId} />
        </Suspense>
      );
    case "project-settings":
      return (
        <Suspense fallback={<div className="layer-stub">Loading settings...</div>}>
          <ProjectSettingsView projectId={layer.projectId} />
        </Suspense>
      );
    case "agent":
      return (
        <Suspense fallback={<div className="layer-stub">Loading agent...</div>}>
          <AgentView projectId={layer.projectId} sessionId={layer.sessionId} />
        </Suspense>
      );
    case "document":
      return (
        <Suspense fallback={<div className="layer-stub">Loading document...</div>}>
          <DocumentView documentId={layer.documentId} projectId={layer.projectId} />
        </Suspense>
      );
    case "new-document":
      return (
        <Suspense fallback={<div className="layer-stub">Loading document...</div>}>
          <DocumentView documentId="new" projectId={layer.projectId} workItemId={layer.workItemId} />
        </Suspense>
      );
    case "work_item":
      return (
        <Suspense fallback={<div className="layer-stub">Loading work item...</div>}>
          <WorkItemView projectId={layer.projectId} workItemId={layer.workItemId} />
        </Suspense>
      );
    case "settings":
      return (
        <Suspense fallback={<div className="layer-stub">Loading settings...</div>}>
          <SettingsView />
        </Suspense>
      );
  }
}
