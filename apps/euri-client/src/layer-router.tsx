import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";
import { ProjectCommandView } from "./views/project-command-view";
import { ProjectSettingsView } from "./views/project-settings-view";
import { AgentView } from "./views/agent-view";
import { DocumentView } from "./views/document-view";
import { WorkItemView } from "./views/work-item-view";

export function LayerRouter() {
  const layer = useNavLayer();

  switch (layer.layer) {
    case "home":
      return <HomeView />;
    case "project":
      return <ProjectCommandView projectId={layer.projectId} />;
    case "project-settings":
      return <ProjectSettingsView projectId={layer.projectId} />;
    case "agent":
      return <AgentView projectId={layer.projectId} sessionId={layer.sessionId} />;
    case "document":
      return <DocumentView documentId={layer.documentId} projectId={layer.projectId} />;
    case "new-document":
      return <DocumentView documentId="new" projectId={layer.projectId} workItemId={layer.workItemId} />;
    case "work_item":
      return <WorkItemView projectId={layer.projectId} workItemId={layer.workItemId} />;
  }
}
