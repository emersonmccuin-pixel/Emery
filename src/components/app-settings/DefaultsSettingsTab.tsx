import { open } from "@tauri-apps/plugin-dialog";
import type { FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAppStore } from "@/store";
import {
  getDispatcherLaunchProfiles,
  getLaunchProfileProviderLabel,
  getWorkerLaunchProfiles,
} from "@/store/utils";

function DefaultsSettingsTab() {
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

export default DefaultsSettingsTab;
