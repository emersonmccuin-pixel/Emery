import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";
import { ProjectCommandView } from "./views/project-command-view";
import { AgentView } from "./views/agent-view";
import { DocumentView } from "./views/document-view";

export function LayerRouter() {
  const layer = useNavLayer();

  switch (layer.layer) {
    case "home":
      return <HomeView />;
    case "project":
      return <ProjectCommandView projectId={layer.projectId} />;
    case "agent":
      return <AgentView projectId={layer.projectId} sessionId={layer.sessionId} />;
    case "document":
      return <DocumentView documentId={layer.documentId} projectId={layer.projectId} />;
  }
}
