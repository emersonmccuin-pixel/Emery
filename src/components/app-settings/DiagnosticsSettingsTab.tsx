import { Suspense, lazy } from "react";
import { PanelLoadingState } from "@/components/ui/panel-state";

const DiagnosticsConsole = lazy(() => import("@/components/DiagnosticsConsole"));

function DiagnosticsSettingsTab({ isActive }: { isActive: boolean }) {
  return (
    <Suspense
      fallback={
        <PanelLoadingState
          className="min-h-[18rem]"
          detail="Loading the diagnostics console."
          eyebrow="Diagnostics"
          title="Opening diagnostics"
          tone="cyan"
        />
      }
    >
      <DiagnosticsConsole isActive={isActive} />
    </Suspense>
  );
}

export default DiagnosticsSettingsTab;
