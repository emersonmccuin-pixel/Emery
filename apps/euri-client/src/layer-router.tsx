import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";
import { ProjectCommandView } from "./views/project-command-view";

function AgentViewStub({ sessionId }: { sessionId: string }) {
  return (
    <div className="layer-stub">
      <h2>Agent View</h2>
      <p>Session: {sessionId}</p>
      <p>This view will be built in VS-CC-3.</p>
    </div>
  );
}

export function LayerRouter() {
  const layer = useNavLayer();

  switch (layer.layer) {
    case "home":
      return <HomeView />;
    case "project":
      return <ProjectCommandView projectId={layer.projectId} />;
    case "agent":
      return <AgentViewStub sessionId={layer.sessionId} />;
  }
}
