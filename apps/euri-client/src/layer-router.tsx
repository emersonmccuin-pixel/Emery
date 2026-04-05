import { useNavLayer } from "./nav-store";
import { HomeView } from "./views/home-view";
import { InboxView } from "./views/inbox-view";
import { ProjectCommandView } from "./views/project-command-view";
import { AgentView } from "./views/agent-view";
import { DocumentView } from "./views/document-view";
import { WorkItemView } from "./views/work-item-view";

export function LayerRouter() {
  const layer = useNavLayer();

  switch (layer.layer) {
    case "home":
      return <HomeView />;
    case "inbox":
      return <InboxView projectId={layer.projectId} />;
    case "project":
      return <ProjectCommandView projectId={layer.projectId} />;
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
