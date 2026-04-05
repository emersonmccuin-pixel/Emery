import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";
import { AgentView } from "./views/agent-view";

function ProjectCommandStub({ projectId }: { projectId: string }) {
  return (
    <div className="layer-stub">
      <h2>Project Command</h2>
      <p>Project: {projectId}</p>
      <p>This view will be built in VS-CC-2.</p>
    </div>
  );
}

export function LayerRouter() {
  const layer = useNavLayer();

  switch (layer.layer) {
    case "home":
      return <HomeView />;
    case "project":
      return <ProjectCommandStub projectId={layer.projectId} />;
    case "agent":
      return <AgentView projectId={layer.projectId} sessionId={layer.sessionId} />;
  }
}
