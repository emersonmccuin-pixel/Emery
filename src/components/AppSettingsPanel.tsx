import { open } from "@tauri-apps/plugin-dialog";
import { Suspense, lazy, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from "@/components/ui/panel-state";
import { invoke } from "@/lib/tauri";
import { useAppStore } from "../store";
import {
  CLAUDE_AGENT_SDK_PROVIDER,
  CLAUDE_CODE_PROVIDER,
  CODEX_SDK_PROVIDER,
  getDispatcherLaunchProfiles,
  getLaunchProfileProviderLabel,
  getWorkerLaunchProfiles,
  isWorkerLaunchProfileProvider,
} from "../store/utils";
import { themes } from "../themes";
import type { VaultEntryRecord, VaultSnapshot } from "../types";
import "./panel-surfaces.css";
import "./app-settings.css";

type AppSettingsTab =
  | "appearance"
  | "accounts"
  | "defaults"
  | "vault"
  | "diagnostics";

type Props = {
  initialTab?: AppSettingsTab;
};

const DiagnosticsConsole = lazy(
  () => import("@/components/DiagnosticsConsole"),
);

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
          <TabsTrigger value="vault">Vault</TabsTrigger>
          <TabsTrigger value="diagnostics">Diagnostics</TabsTrigger>
        </TabsList>
      </nav>
      <div className="flex-1 min-h-0 overflow-auto scrollbar-thin p-6">
        <Banner />
        <TabsContent value="appearance">
          <AppearanceTab />
        </TabsContent>
        <TabsContent value="accounts">
          <AccountsTab />
        </TabsContent>
        <TabsContent value="defaults">
          <DefaultsTab />
        </TabsContent>
        <TabsContent value="vault">
          <VaultTab />
        </TabsContent>
        <TabsContent value="diagnostics">
          <DiagnosticsTab isActive={activeTab === "diagnostics"} />
        </TabsContent>
      </div>
    </Tabs>
  );
}

function Banner() {
  const settingsError = useAppStore((s) => s.settingsError);
  const settingsMessage = useAppStore((s) => s.settingsMessage);
  if (!settingsError && !settingsMessage) return null;
  return (
    <>
      {settingsError ? (
        <p className="form-error settings-banner">{settingsError}</p>
      ) : null}
      {settingsMessage ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {settingsMessage}
        </p>
      ) : null}
    </>
  );
}

function AppearanceTab() {
  const activeThemeId = useAppStore((s) => s.activeThemeId);

  return (
    <article className="overview-card">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Appearance</p>
          <strong>Theme</strong>
        </div>
      </div>
      <div className="theme-picker-grid">
        {Object.entries(themes).map(([id, theme]) => {
          const isActive = id === activeThemeId;
          return (
            <button
              key={id}
              type="button"
              className={`theme-card${isActive ? " theme-card--active" : ""}`}
              style={
                isActive
                  ? {
                      borderColor: theme["--center-tint"],
                      boxShadow: `0 0 0 1px ${theme["--center-tint"]}, 0 0 16px color-mix(in srgb, ${theme["--center-tint"]} 40%, transparent)`,
                    }
                  : undefined
              }
              onClick={() => useAppStore.getState().setActiveThemeId(id)}
            >
              {/* Mini 3-panel preview */}
              <div
                className="theme-card__preview"
                style={{ background: theme["--hud-bg"] }}
              >
                <div
                  className="theme-card__preview-panel"
                  style={{
                    background: theme["--hud-panel-bg"],
                    borderColor: theme["--rail-projects-tint"],
                  }}
                />
                <div
                  className="theme-card__preview-panel theme-card__preview-panel--center"
                  style={{
                    background: theme["--hud-panel-bg"],
                    borderColor: theme["--center-tint"],
                  }}
                />
                <div
                  className="theme-card__preview-panel"
                  style={{
                    background: theme["--hud-panel-bg"],
                    borderColor: theme["--rail-sessions-tint"],
                  }}
                />
              </div>

              {/* Theme name + swatches row */}
              <div className="theme-card__footer">
                <span className="theme-card__label">{theme.label}</span>
                <div className="theme-card__swatches">
                  {(
                    [
                      "--rail-projects-tint",
                      "--center-tint",
                      "--rail-sessions-tint",
                      "--hud-amber",
                      "--hud-purple",
                    ] as const
                  ).map((key) => (
                    <span
                      key={key}
                      className="theme-card__swatch"
                      style={{ backgroundColor: theme[key] }}
                    />
                  ))}
                </div>
              </div>

              {/* Active checkmark */}
              {isActive && (
                <div
                  className="theme-card__check"
                  style={{ color: theme["--center-tint"] }}
                >
                  ✓
                </div>
              )}
            </button>
          );
        })}
      </div>
    </article>
  );
}

function DefaultsTab() {
  const launchProfiles = useAppStore((s) => s.launchProfiles);
  const defaultLaunchProfileSettingId = useAppStore(
    (s) => s.defaultLaunchProfileSettingId,
  );
  const defaultWorkerLaunchProfileSettingId = useAppStore(
    (s) => s.defaultWorkerLaunchProfileSettingId,
  );
  const sdkClaudeConfigDirSetting = useAppStore(
    (s) => s.sdkClaudeConfigDirSetting,
  );
  const autoRepairSafeCleanupOnStartup = useAppStore(
    (s) => s.autoRepairSafeCleanupOnStartup,
  );
  const isSavingAppSettings = useAppStore((s) => s.isSavingAppSettings);
  const dispatcherProfiles = getDispatcherLaunchProfiles(launchProfiles);
  const workerProfiles = getWorkerLaunchProfiles(launchProfiles);
  const {
    setDefaultLaunchProfileSettingId,
    setDefaultWorkerLaunchProfileSettingId,
    setSdkClaudeConfigDirSetting,
    setAutoRepairSafeCleanupOnStartup,
    submitAppSettings,
  } = useAppStore.getState();

  return (
    <article className="overview-card">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">App defaults</p>
          <strong>Supervisor-backed settings</strong>
        </div>
      </div>

      <form
        className="stack-form"
        onSubmit={(event) =>
          void submitAppSettings(event as FormEvent<HTMLFormElement>)
        }
      >
        <label className="field">
          <span>Default dispatcher profile</span>
          <select
            value={
              defaultLaunchProfileSettingId === null
                ? ""
                : String(defaultLaunchProfileSettingId)
            }
            onChange={(event) =>
              setDefaultLaunchProfileSettingId(
                event.target.value === "" ? null : Number(event.target.value),
              )
            }
          >
            <option value="">Use first available profile</option>
            {dispatcherProfiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.label}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span>Default worktree agent profile</span>
          <select
            value={
              defaultWorkerLaunchProfileSettingId === null
                ? ""
                : String(defaultWorkerLaunchProfileSettingId)
            }
            onChange={(event) =>
              setDefaultWorkerLaunchProfileSettingId(
                event.target.value === "" ? null : Number(event.target.value),
              )
            }
          >
            <option value="">
              {workerProfiles.length > 0
                ? "Prefer the first SDK worker profile"
                : "Use first available profile"}
            </option>
            {launchProfiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.label} ·{" "}
                {getLaunchProfileProviderLabel(profile.provider)}
              </option>
            ))}
          </select>
          <p className="stack-form__note">
            Worktree agents launch from this profile, consume dispatcher
            directives from the Project Commander inbox, and expose a watch-only
            console in the worktree.
          </p>
        </label>

        <label className="field">
          <span>SDK personal Claude config dir</span>
          <div className="field-grid">
            <Input
              value={sdkClaudeConfigDirSetting}
              onChange={(event) =>
                setSdkClaudeConfigDirSetting(event.target.value)
              }
              placeholder="C:\\Users\\you\\.claude-personal"
              className="hud-input"
            />
            <Button
              type="button"
              variant="outline"
              onClick={async () => {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title:
                    "Select personal Claude config directory for SDK workers",
                });

                if (typeof selected === "string") {
                  setSdkClaudeConfigDirSetting(selected);
                }
              }}
            >
              Browse
            </Button>
          </div>
          <p className="stack-form__note">
            Claude Agent SDK workers always use this Claude config directory and
            ignore competing auth env vars like API keys or cloud-provider
            overrides. Leave it blank only if your personal account already
            lives in the default
            <code> ~/.claude </code>
            home.
          </p>
        </label>

        <label className="settings-toggle">
          <div>
            <span>Repair safe cleanup items on supervisor startup</span>
            <p className="stack-form__note">
              Automatically clear stale runtime artifacts, managed worktree
              directories, and missing-path worktree records when they are
              classified as safe repairs.
            </p>
          </div>
          <input
            checked={autoRepairSafeCleanupOnStartup}
            onChange={(event) =>
              setAutoRepairSafeCleanupOnStartup(event.target.checked)
            }
            type="checkbox"
          />
        </label>

        <div className="action-row">
          <Button
            variant="default"
            disabled={isSavingAppSettings}
            type="submit"
          >
            {isSavingAppSettings ? "Saving..." : "Save app settings"}
          </Button>
        </div>
      </form>
    </article>
  );
}

function AccountsTab() {
  const appSettings = useAppStore((s) => s.appSettings);
  const selectedLaunchProfileId = useAppStore((s) => s.selectedLaunchProfileId);
  const launchProfiles = useAppStore((s) => s.launchProfiles);
  const profileLabel = useAppStore((s) => s.profileLabel);
  const profileProvider = useAppStore((s) => s.profileProvider);
  const profileExecutable = useAppStore((s) => s.profileExecutable);
  const profileArgs = useAppStore((s) => s.profileArgs);
  const profileEnvJson = useAppStore((s) => s.profileEnvJson);
  const profileError = useAppStore((s) => s.profileError);
  const isProfileFormOpen = useAppStore((s) => s.isProfileFormOpen);
  const editingLaunchProfileId = useAppStore((s) => s.editingLaunchProfileId);
  const isCreatingProfile = useAppStore((s) => s.isCreatingProfile);
  const activeDeleteLaunchProfileId = useAppStore(
    (s) => s.activeDeleteLaunchProfileId,
  );

  const {
    setSelectedLaunchProfileId,
    setProfileLabel,
    setProfileProvider,
    setProfileExecutable,
    setProfileArgs,
    setProfileEnvJson,
    submitLaunchProfile,
    startCreateLaunchProfile,
    startEditLaunchProfile,
    cancelLaunchProfileEditor,
    deleteLaunchProfile,
  } = useAppStore.getState();

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Accounts</p>
          <strong>Manage launch profiles</strong>
        </div>
        <Button
          variant="outline"
          size="sm"
          type="button"
          onClick={() => startCreateLaunchProfile()}
        >
          {isProfileFormOpen && editingLaunchProfileId === null
            ? "Adding profile"
            : "Add account"}
        </Button>
      </div>

      <div className="settings-profile-list">
        {launchProfiles.length === 0 ? (
          <div className="empty-state empty-state--rail">
            No launch profiles configured yet.
          </div>
        ) : (
          launchProfiles.map((profile) => (
            <article key={profile.id} className="settings-profile-card">
              <div className="settings-profile-card__header">
                <div className="settings-profile-card__title">
                  <strong>{profile.label}</strong>
                  <div className="settings-profile-card__badges">
                    {selectedLaunchProfileId === profile.id &&
                    profile.provider === CLAUDE_CODE_PROVIDER ? (
                      <Badge variant="running">Selected</Badge>
                    ) : null}
                    {appSettings.defaultLaunchProfileId === profile.id ? (
                      <Badge className="rounded-full border border-border px-1.5 py-0.5">
                        Dispatcher default
                      </Badge>
                    ) : null}
                    {appSettings.defaultWorkerLaunchProfileId === profile.id ? (
                      <Badge className="rounded-full border border-border px-1.5 py-0.5">
                        Worker default
                      </Badge>
                    ) : null}
                    <Badge className="rounded-full border border-border px-1.5 py-0.5">
                      {getLaunchProfileProviderLabel(profile.provider)}
                    </Badge>
                  </div>
                </div>
                <div className="overview-inline-meta">
                  <code>{profile.executable}</code>
                  <code>{profile.args || "(no args)"}</code>
                </div>
              </div>

              <p className="stack-form__note">
                {isWorkerLaunchProfileProvider(profile.provider)
                  ? "This profile launches the SDK worker host. Env JSON may include vault-backed bindings."
                  : "Env JSON is injected at launch. Individual vars may point at Vault entries instead of literal values."}
              </p>

              <div className="action-row">
                <Button
                  variant="outline"
                  disabled={profile.provider !== CLAUDE_CODE_PROVIDER}
                  type="button"
                  onClick={() => setSelectedLaunchProfileId(profile.id)}
                >
                  {profile.provider !== CLAUDE_CODE_PROVIDER
                    ? "Worker-only"
                    : selectedLaunchProfileId === profile.id
                      ? "In use"
                      : "Use now"}
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => startEditLaunchProfile(profile)}
                >
                  Edit
                </Button>
                <Button
                  variant="destructive"
                  disabled={activeDeleteLaunchProfileId === profile.id}
                  type="button"
                  onClick={() => void deleteLaunchProfile(profile)}
                >
                  {activeDeleteLaunchProfileId === profile.id
                    ? "Deleting..."
                    : "Delete"}
                </Button>
              </div>
            </article>
          ))
        )}
      </div>

      {isProfileFormOpen ? (
        <form
          className="stack-form settings-profile-form"
          onSubmit={(event) =>
            void submitLaunchProfile(event as FormEvent<HTMLFormElement>)
          }
        >
          <div className="stack-form__header">
            <h3>
              {editingLaunchProfileId === null
                ? "Add launch account"
                : "Edit launch account"}
            </h3>
          </div>

          <label className="field">
            <span>Label</span>
            <Input
              value={profileLabel}
              onChange={(event) => setProfileLabel(event.target.value)}
              placeholder="Claude Code / Work"
              className="hud-input"
            />
          </label>

          <label className="field">
            <span>Provider</span>
            <select
              value={profileProvider}
              onChange={(event) => setProfileProvider(event.target.value)}
            >
              <option value={CLAUDE_CODE_PROVIDER}>Claude Code CLI</option>
              <option value={CLAUDE_AGENT_SDK_PROVIDER}>
                Claude Agent SDK
              </option>
              <option value={CODEX_SDK_PROVIDER}>Codex SDK</option>
            </select>
          </label>

          <div className="field-grid">
            <label className="field">
              <span>Executable</span>
              <Input
                value={profileExecutable}
                onChange={(event) => setProfileExecutable(event.target.value)}
                placeholder={
                  isWorkerLaunchProfileProvider(profileProvider)
                    ? "node"
                    : "claude"
                }
                className="hud-input"
              />
            </label>

            <label className="field">
              <span>Args</span>
              <Input
                value={profileArgs}
                onChange={(event) => setProfileArgs(event.target.value)}
                placeholder={
                  isWorkerLaunchProfileProvider(profileProvider)
                    ? "--no-warnings"
                    : "--dangerously-skip-permissions"
                }
                className="hud-input"
              />
            </label>
          </div>

          <label className="field">
            <span>Environment JSON</span>
            <textarea
              rows={5}
              value={profileEnvJson}
              onChange={(event) => setProfileEnvJson(event.target.value)}
              placeholder={`{"ANTHROPIC_API_KEY":{"source":"vault","vault":"Anthropic Key","scopeTags":["anthropic:api"]}}`}
            />
            <p className="stack-form__note">
              Literal values still work. Vault example:
              <code>
                {" "}
                {"{\"OPENAI_API_KEY\":{\"source\":\"vault\",\"vault\":\"OpenAI Key\",\"scopeTags\":[\"openai:api\"]}}"}
              </code>
              . For tools that expect a temp file path instead of the raw secret value, add
              <code>
                {" "}
                {"\"delivery\":\"file\""}
              </code>
              . Values are resolved only by the supervisor at launch.
            </p>
          </label>

          {profileError ? <p className="form-error">{profileError}</p> : null}

          <div className="action-row">
            <Button
              variant="default"
              disabled={isCreatingProfile}
              type="submit"
            >
              {isCreatingProfile
                ? "Saving..."
                : editingLaunchProfileId === null
                  ? "Create account"
                  : "Save account"}
            </Button>
            <Button
              variant="outline"
              type="button"
              onClick={() => cancelLaunchProfileEditor()}
            >
              Cancel
            </Button>
          </div>
        </form>
      ) : null}
    </article>
  );
}

const EMPTY_VAULT_FORM = {
  id: null as number | null,
  name: "",
  kind: "token",
  description: "",
  scopeTags: "",
  gatePolicy: "confirm_session",
  value: "",
};

function parseScopeTags(value: string) {
  return value
    .split(/[\n,]/)
    .map((token) => token.trim())
    .filter(Boolean);
}

function formatGatePolicyLabel(policy: string) {
  switch (policy) {
    case "auto":
      return "Auto";
    case "confirm_each_use":
      return "Confirm Each Use";
    case "confirm_session":
    default:
      return "Confirm Per Session";
  }
}

function vaultDiagnosticsArgs(form: typeof EMPTY_VAULT_FORM) {
  return {
    input: {
      id: form.id,
      name: form.name,
      kind: form.kind,
      description: form.description,
      scopeTags: parseScopeTags(form.scopeTags),
      gatePolicy: form.gatePolicy,
      value: form.value ? "<redacted>" : undefined,
    },
  };
}

function VaultTab() {
  const [snapshot, setSnapshot] = useState<VaultSnapshot | null>(null);
  const [form, setForm] = useState(EMPTY_VAULT_FORM);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [activeDeleteId, setActiveDeleteId] = useState<number | null>(null);

  useEffect(() => {
    void loadVaultEntries();
  }, []);

  async function loadVaultEntries() {
    setIsLoading(true);
    setError(null);

    try {
      const next = await invoke<VaultSnapshot>("list_vault_entries");
      setSnapshot(next);
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to load vault entries",
      );
    } finally {
      setIsLoading(false);
    }
  }

  function startEdit(entry: VaultEntryRecord) {
    setForm({
      id: entry.id,
      name: entry.name,
      kind: entry.kind,
      description: entry.description,
      scopeTags: entry.scopeTags.join(", "),
      gatePolicy: entry.gatePolicy,
      value: "",
    });
    setMessage(null);
    setError(null);
  }

  function resetForm() {
    setForm(EMPTY_VAULT_FORM);
  }

  async function submitVaultEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setError(null);
    setMessage(null);

    try {
      const next = await invoke<VaultSnapshot>(
        "upsert_vault_entry",
        {
          input: {
            id: form.id,
            name: form.name,
            kind: form.kind,
            description: form.description,
            scopeTags: parseScopeTags(form.scopeTags),
            gatePolicy: form.gatePolicy,
            value: form.value || undefined,
          },
        },
        { diagnosticsArgs: vaultDiagnosticsArgs(form) },
      );

      setSnapshot(next);
      setMessage(
        form.id === null
          ? "Vault entry saved to Stronghold."
          : form.value
            ? "Vault entry updated and rotated."
            : "Vault entry metadata updated.",
      );
      resetForm();
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to save vault entry",
      );
    } finally {
      setIsSaving(false);
    }
  }

  async function deleteEntry(entry: VaultEntryRecord) {
    if (!confirm(`Delete vault entry "${entry.name}"?`)) {
      return;
    }

    setActiveDeleteId(entry.id);
    setError(null);
    setMessage(null);

    try {
      const next = await invoke<VaultSnapshot>("delete_vault_entry", {
        input: { id: entry.id },
      });
      setSnapshot(next);
      if (form.id === entry.id) {
        resetForm();
      }
      setMessage(`Deleted vault entry "${entry.name}".`);
    } catch (err) {
      setError(
        typeof err === "string"
          ? err
          : (err as { message?: string })?.message ??
              "Failed to delete vault entry",
      );
    } finally {
      setActiveDeleteId(null);
    }
  }

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Vault</p>
          <strong>Stronghold-backed secrets</strong>
        </div>
        <Button
          variant="outline"
          size="sm"
          type="button"
          onClick={() => {
            resetForm();
            setMessage(null);
            setError(null);
          }}
        >
          New entry
        </Button>
      </div>

      <p className="stack-form__note">
        Secret values are deposited from this trusted settings surface and stored
        in the backend vault snapshot. The UI never reads them back after save.
      </p>

      {error ? <PanelBanner className="mb-4" message={error} /> : null}
      {message ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {message}
        </p>
      ) : null}

      {snapshot ? (
        <div className="settings-path-list mb-4">
          <div className="settings-path-row">
            <span>Vault root</span>
            <code>{snapshot.vaultRoot}</code>
          </div>
          <div className="settings-path-row">
            <span>Snapshot path</span>
            <code>{snapshot.snapshotPath}</code>
          </div>
        </div>
      ) : null}

      <form className="stack-form settings-profile-form" onSubmit={submitVaultEntry}>
        <div className="field-grid">
          <label className="field">
            <span>Name</span>
            <Input
              value={form.name}
              onChange={(event) =>
                setForm((current) => ({ ...current, name: event.target.value }))
              }
              placeholder="GitHub Token"
              className="hud-input"
              required
            />
          </label>

          <label className="field">
            <span>Kind</span>
            <select
              value={form.kind}
              onChange={(event) =>
                setForm((current) => ({ ...current, kind: event.target.value }))
              }
            >
              <option value="token">Token</option>
              <option value="password">Password</option>
              <option value="json">JSON blob</option>
              <option value="pem">PEM / key</option>
              <option value="generic">Generic</option>
            </select>
          </label>

          <label className="field">
            <span>Scope tags</span>
            <Input
              value={form.scopeTags}
              onChange={(event) =>
                setForm((current) => ({
                  ...current,
                  scopeTags: event.target.value,
                }))
              }
              placeholder="gh:repo, github:read"
              className="hud-input"
            />
          </label>

          <label className="field">
            <span>Gate policy</span>
            <select
              value={form.gatePolicy}
              onChange={(event) =>
                setForm((current) => ({
                  ...current,
                  gatePolicy: event.target.value,
                }))
              }
            >
              <option value="confirm_session">Confirm per session</option>
              <option value="auto">Auto approve</option>
              <option value="confirm_each_use">Confirm each use</option>
            </select>
          </label>
        </div>

        <label className="field">
          <span>Description</span>
          <Input
            value={form.description}
            onChange={(event) =>
              setForm((current) => ({
                ...current,
                description: event.target.value,
              }))
            }
            placeholder="Used for repository and issue operations."
            className="hud-input"
          />
        </label>

        <label className="field">
          <span>{form.id === null ? "Secret value" : "Rotate value (optional)"}</span>
          <textarea
            rows={6}
            value={form.value}
            onChange={(event) =>
              setForm((current) => ({ ...current, value: event.target.value }))
            }
            placeholder={
              form.id === null
                ? "Paste the secret value once."
                : "Leave blank to keep the current value, or paste a new one to rotate."
            }
            className="hud-input min-h-[8rem] font-mono"
          />
        </label>

        <div className="action-row">
          <Button variant="default" disabled={isSaving} type="submit">
            {isSaving
              ? "Saving..."
              : form.id === null
                ? "Save vault entry"
                : "Update vault entry"}
          </Button>
          {form.id !== null ? (
            <Button variant="outline" type="button" onClick={() => resetForm()}>
              Cancel edit
            </Button>
          ) : null}
        </div>
      </form>

      {isLoading ? (
        <PanelLoadingState
          className="min-h-[18rem]"
          detail="Loading vault metadata from the local Stronghold snapshot."
          eyebrow="Vault"
          title="Opening secret catalog"
          tone="cyan"
        />
      ) : snapshot?.entries.length ? (
        <div className="settings-profile-list">
          {snapshot.entries.map((entry) => (
            <article key={entry.id} className="settings-profile-card">
              <div className="settings-profile-card__header">
                <div className="settings-profile-card__title">
                  <strong>{entry.name}</strong>
                  <div className="settings-profile-card__badges">
                    <Badge className="rounded-full border border-border px-1.5 py-0.5">
                      {entry.kind}
                    </Badge>
                    <Badge className="rounded-full border border-border px-1.5 py-0.5">
                      {formatGatePolicyLabel(entry.gatePolicy)}
                    </Badge>
                  </div>
                </div>
              </div>

              {entry.description ? (
                <p className="stack-form__note">{entry.description}</p>
              ) : null}

              <div className="overview-inline-meta">
                <code>{entry.scopeTags.length > 0 ? entry.scopeTags.join(", ") : "No scope tags yet"}</code>
                <code>Updated {entry.updatedAt}</code>
              </div>

              <div className="action-row">
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => startEdit(entry)}
                >
                  Edit metadata
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => startEdit(entry)}
                >
                  Rotate value
                </Button>
                <Button
                  variant="destructive"
                  disabled={activeDeleteId === entry.id}
                  type="button"
                  onClick={() => void deleteEntry(entry)}
                >
                  {activeDeleteId === entry.id ? "Deleting..." : "Delete"}
                </Button>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <PanelEmptyState
          className="min-h-[18rem]"
          detail="Create the first vault entry here before wiring integrations or workflow-stage secret requirements."
          eyebrow="Vault"
          title="No secrets stored yet"
          tone="cyan"
        />
      )}
    </article>
  );
}

function DiagnosticsTab({ isActive }: { isActive: boolean }) {
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

export default AppSettingsPanel;
