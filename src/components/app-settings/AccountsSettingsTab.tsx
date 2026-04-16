import type { FormEvent } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAppStore } from "@/store";
import {
  CLAUDE_AGENT_SDK_PROVIDER,
  CLAUDE_CODE_PROVIDER,
  CODEX_SDK_PROVIDER,
  getLaunchProfileProviderLabel,
  isWorkerLaunchProfileProvider,
} from "@/store/utils";

function AccountsSettingsTab() {
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
              . For tools that expect a temp file path instead of the raw secret
              value, add
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

export default AccountsSettingsTab;
