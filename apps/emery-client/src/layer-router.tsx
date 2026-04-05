import { lazy, Suspense } from "react";
import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";

const ProjectDetailView = lazy(async () => {
  const module = await import("./views/project-detail-view");
  return { default: module.ProjectDetailView };
});

const AgentView = lazy(async () => {
  const module = await import("./views/agent-view");
  return { default: module.AgentView };
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
          <ProjectDetailView projectId={layer.projectId} />
        </Suspense>
      );
    case "agent":
      return (
        <Suspense fallback={<div className="layer-stub">Loading agent...</div>}>
          <AgentView projectId={layer.projectId} sessionId={layer.sessionId} />
        </Suspense>
      );
  }
}
