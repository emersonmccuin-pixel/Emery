import { lazy, Suspense } from "react";
import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";

const ProjectCommandView = lazy(async () => {
  const module = await import("./views/project-command-view");
  return { default: module.ProjectCommandView };
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
          <ProjectCommandView projectId={layer.projectId} />
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
