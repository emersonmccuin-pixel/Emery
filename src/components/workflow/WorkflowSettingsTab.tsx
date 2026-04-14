import { Suspense, lazy } from "react";
import { PanelLoadingState } from "@/components/ui/panel-state";

const WorkflowBuilderSettings = lazy(
  () => import("@/components/workflow/WorkflowBuilderSettings"),
);

function WorkflowSettingsTab() {
  return (
    <Suspense
      fallback={
        <PanelLoadingState
          className="min-h-[24rem]"
          detail="Loading workflow templates and builder controls."
          eyebrow="Settings"
          title="Opening workflow builder"
          tone="cyan"
        />
      }
    >
      <WorkflowBuilderSettings />
    </Suspense>
  );
}

export default WorkflowSettingsTab;
