import { Suspense, lazy, useState } from "react";
import { PanelLoadingState } from "@/components/ui/panel-state";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import AppSettingsBanner from "@/components/app-settings/AppSettingsBanner";
import AppearanceSettingsTab from "@/components/app-settings/AppearanceSettingsTab";
import "./panel-surfaces.css";
import "./app-settings.css";

type AppSettingsTab =
  | "appearance"
  | "accounts"
  | "defaults"
  | "integrations"
  | "vault"
  | "diagnostics";

type Props = {
  initialTab?: AppSettingsTab;
};

const AccountsSettingsTab = lazy(
  () => import("@/components/app-settings/AccountsSettingsTab"),
);
const DefaultsSettingsTab = lazy(
  () => import("@/components/app-settings/DefaultsSettingsTab"),
);
const VaultSettingsTab = lazy(
  () => import("@/components/app-settings/VaultSettingsTab"),
);
const DiagnosticsSettingsTab = lazy(
  () => import("@/components/app-settings/DiagnosticsSettingsTab"),
);
const IntegrationsTab = lazy(() => import("@/components/IntegrationsTab"));

function SettingsTabFallback({
  detail,
  eyebrow,
  title,
}: {
  detail: string;
  eyebrow: string;
  title: string;
}) {
  return (
    <PanelLoadingState
      className="min-h-[18rem]"
      detail={detail}
      eyebrow={eyebrow}
      title={title}
      tone="cyan"
    />
  );
}

function AppSettingsPanel({ initialTab = "appearance" }: Props) {
  const [activeTab, setActiveTab] = useState<AppSettingsTab>(initialTab);

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => setActiveTab(value as AppSettingsTab)}
      className="h-full"
    >
      <nav className="workspace-tabs--shell flex items-center h-10 px-4 shrink-0">
        <TabsList>
          <TabsTrigger value="appearance">Appearance</TabsTrigger>
          <TabsTrigger value="accounts">Accounts</TabsTrigger>
          <TabsTrigger value="defaults">Defaults</TabsTrigger>
          <TabsTrigger value="integrations">Integrations</TabsTrigger>
          <TabsTrigger value="vault">Vault</TabsTrigger>
          <TabsTrigger value="diagnostics">Diagnostics</TabsTrigger>
        </TabsList>
      </nav>
      <div className="flex-1 min-h-0 overflow-auto scrollbar-thin p-6">
        <AppSettingsBanner />
        <TabsContent value="appearance">
          <AppearanceSettingsTab />
        </TabsContent>
        <TabsContent value="accounts">
          <Suspense
            fallback={
              <SettingsTabFallback
                detail="Loading launch-profile controls."
                eyebrow="Accounts"
                title="Opening accounts"
              />
            }
          >
            <AccountsSettingsTab />
          </Suspense>
        </TabsContent>
        <TabsContent value="defaults">
          <Suspense
            fallback={
              <SettingsTabFallback
                detail="Loading app-default settings."
                eyebrow="Defaults"
                title="Opening defaults"
              />
            }
          >
            <DefaultsSettingsTab />
          </Suspense>
        </TabsContent>
        <TabsContent value="integrations">
          <Suspense
            fallback={
              <SettingsTabFallback
                detail="Loading integration and backup settings."
                eyebrow="Integrations"
                title="Opening integrations"
              />
            }
          >
            <IntegrationsTab />
          </Suspense>
        </TabsContent>
        <TabsContent value="vault">
          <Suspense
            fallback={
              <SettingsTabFallback
                detail="Loading vault settings."
                eyebrow="Vault"
                title="Opening secret catalog"
              />
            }
          >
            <VaultSettingsTab />
          </Suspense>
        </TabsContent>
        <TabsContent value="diagnostics">
          <Suspense
            fallback={
              <SettingsTabFallback
                detail="Loading the diagnostics console."
                eyebrow="Diagnostics"
                title="Opening diagnostics"
              />
            }
          >
            <DiagnosticsSettingsTab isActive={activeTab === "diagnostics"} />
          </Suspense>
        </TabsContent>
      </div>
    </Tabs>
  );
}

export default AppSettingsPanel;
