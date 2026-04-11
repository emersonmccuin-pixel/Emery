import type { FormEvent } from 'react'
import { Badge } from '@/components/ui/badge'
import { useAppStore, useSelectedProject } from '../store'

function SettingsPanel() {
  const selectedProject = useSelectedProject()

  const storageInfo = useAppStore((s) => s.storageInfo)
  const appSettings = useAppStore((s) => s.appSettings)
  const selectedLaunchProfileId = useAppStore((s) => s.selectedLaunchProfileId)
  const launchProfiles = useAppStore((s) => s.launchProfiles)
  const settingsError = useAppStore((s) => s.settingsError)
  const settingsMessage = useAppStore((s) => s.settingsMessage)
  const defaultLaunchProfileSettingId = useAppStore((s) => s.defaultLaunchProfileSettingId)
  const autoRepairSafeCleanupOnStartup = useAppStore((s) => s.autoRepairSafeCleanupOnStartup)
  const editProjectName = useAppStore((s) => s.editProjectName)
  const editProjectRootPath = useAppStore((s) => s.editProjectRootPath)
  const projectUpdateError = useAppStore((s) => s.projectUpdateError)
  const isProjectEditorOpen = useAppStore((s) => s.isProjectEditorOpen)
  const profileLabel = useAppStore((s) => s.profileLabel)
  const profileExecutable = useAppStore((s) => s.profileExecutable)
  const profileArgs = useAppStore((s) => s.profileArgs)
  const profileEnvJson = useAppStore((s) => s.profileEnvJson)
  const profileError = useAppStore((s) => s.profileError)
  const isProfileFormOpen = useAppStore((s) => s.isProfileFormOpen)
  const editingLaunchProfileId = useAppStore((s) => s.editingLaunchProfileId)
  const isSavingAppSettings = useAppStore((s) => s.isSavingAppSettings)
  const isUpdatingProject = useAppStore((s) => s.isUpdatingProject)
  const isCreatingProfile = useAppStore((s) => s.isCreatingProfile)
  const activeDeleteLaunchProfileId = useAppStore((s) => s.activeDeleteLaunchProfileId)

  const {
    setSelectedLaunchProfileId,
    setDefaultLaunchProfileSettingId,
    setAutoRepairSafeCleanupOnStartup,
    setEditProjectName,
    setEditProjectRootPath,
    setIsProjectEditorOpen,
    setProfileLabel,
    setProfileExecutable,
    setProfileArgs,
    setProfileEnvJson,
    setProjectUpdateError,
    browseForProjectFolder,
    submitProjectUpdate,
    submitAppSettings,
    submitLaunchProfile,
    startCreateLaunchProfile,
    startEditLaunchProfile,
    cancelLaunchProfileEditor,
    deleteLaunchProfile,
  } = useAppStore.getState()

  if (!selectedProject) {
    return null
  }

  const runtimeDir = storageInfo ? `${storageInfo.appDataDir}\\runtime` : null
  const worktreeDir = storageInfo ? `${storageInfo.appDataDir}\\worktrees` : null

  return (
    <section className="overview-view">
      {settingsError ? (
        <p className="form-error settings-banner">{settingsError}</p>
      ) : null}
      {settingsMessage ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {settingsMessage}
        </p>
      ) : null}

      <div className="overview-grid">
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
              <span>Default launch profile</span>
              <select
                value={
                  defaultLaunchProfileSettingId === null
                    ? ''
                    : String(defaultLaunchProfileSettingId)
                }
                onChange={(event) =>
                  setDefaultLaunchProfileSettingId(
                    event.target.value === ''
                      ? null
                      : Number(event.target.value),
                  )
                }
              >
                <option value="">Use first available profile</option>
                {launchProfiles.map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.label}
                  </option>
                ))}
              </select>
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
              <button
                className="button button--primary"
                disabled={isSavingAppSettings}
                type="submit"
              >
                {isSavingAppSettings ? 'Saving...' : 'Save app settings'}
              </button>
            </div>
          </form>
        </article>

        <article className="overview-card">
          <div className="overview-card__header">
            <div>
              <p className="panel__eyebrow">Project settings</p>
              <strong>
                {selectedProject.rootAvailable ? 'Edit project' : 'Rebind root'}
              </strong>
            </div>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() =>
                setIsProjectEditorOpen(!isProjectEditorOpen)
              }
            >
              {isProjectEditorOpen
                ? 'Hide'
                : selectedProject.rootAvailable
                  ? 'Edit project'
                  : 'Rebind root'}
            </button>
          </div>

          {isProjectEditorOpen ? (
            <form
              className="stack-form"
              onSubmit={(event) =>
                void submitProjectUpdate(event as FormEvent<HTMLFormElement>)
              }
            >
              {!selectedProject.rootAvailable ? (
                <p className="form-error">
                  The registered root is missing. Pick the new folder and save
                  to repair launch.
                </p>
              ) : null}

              <div className="field-grid">
                <label className="field">
                  <span>Name</span>
                  <input
                    value={editProjectName}
                    onChange={(event) => setEditProjectName(event.target.value)}
                    placeholder="Project name"
                  />
                </label>

                <label className="field">
                  <span>Root folder</span>
                  <div className="input-row">
                    <input
                      value={editProjectRootPath}
                      onChange={(event) =>
                        setEditProjectRootPath(event.target.value)
                      }
                      placeholder="E:\\Projects\\Example"
                    />
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() =>
                        browseForProjectFolder(
                          setEditProjectRootPath,
                          setProjectUpdateError,
                        )
                      }
                    >
                      Browse
                    </button>
                  </div>
                </label>
              </div>

              {projectUpdateError ? (
                <p className="form-error">{projectUpdateError}</p>
              ) : null}

              <div className="action-row">
                <button
                  className="button button--primary"
                  disabled={isUpdatingProject}
                  type="submit"
                >
                  {isUpdatingProject
                    ? 'Saving...'
                    : selectedProject.rootAvailable
                      ? 'Save changes'
                      : 'Rebind project'}
                </button>
              </div>
            </form>
          ) : (
            <p className="stack-form__note">
              Keep the project name clean and rebind the root here if the folder
              moves.
            </p>
          )}
        </article>

        <article className="overview-card overview-card--full">
          <div className="overview-card__header">
            <div>
              <p className="panel__eyebrow">Launch profiles</p>
              <strong>Manage Claude launch accounts</strong>
            </div>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => startCreateLaunchProfile()}
            >
              {isProfileFormOpen && editingLaunchProfileId === null
                ? 'Adding profile'
                : 'Add profile'}
            </button>
          </div>

          <div className="settings-profile-list">
            {launchProfiles.length === 0 ? (
              <div className="empty-state empty-state--rail">
                No launch profiles yet.
              </div>
            ) : (
              launchProfiles.map((profile) => (
                <article
                  key={profile.id}
                  className="settings-profile-card"
                >
                  <div className="settings-profile-card__header">
                    <div className="settings-profile-card__title">
                        <strong>{profile.label}</strong>
                      <div className="settings-profile-card__badges">
                        {selectedLaunchProfileId === profile.id ? (
                          <Badge variant="running">Selected</Badge>
                        ) : null}
                        {appSettings.defaultLaunchProfileId === profile.id ? (
                          <Badge className="rounded-full border border-border px-1.5 py-0.5">
                            Default
                          </Badge>
                        ) : null}
                        <Badge className="rounded-full border border-border px-1.5 py-0.5">
                          Claude Code
                        </Badge>
                      </div>
                    </div>
                    <div className="overview-inline-meta">
                      <code>{profile.executable}</code>
                      <code>{profile.args || '(no args)'}</code>
                    </div>
                  </div>

                  <p className="stack-form__note">
                    Environment JSON is stored with this profile and injected
                    into newly launched sessions.
                  </p>

                  <div className="action-row">
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() => setSelectedLaunchProfileId(profile.id)}
                    >
                      {selectedLaunchProfileId === profile.id
                        ? 'In use'
                        : 'Use now'}
                    </button>
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() => startEditLaunchProfile(profile)}
                    >
                      Edit
                    </button>
                    <button
                      className="button button--danger"
                      disabled={activeDeleteLaunchProfileId === profile.id}
                      type="button"
                      onClick={() => void deleteLaunchProfile(profile)}
                    >
                      {activeDeleteLaunchProfileId === profile.id
                        ? 'Deleting...'
                        : 'Delete'}
                    </button>
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
                    ? 'Add launch profile'
                    : 'Edit launch profile'}
                </h3>
              </div>

              <label className="field">
                <span>Label</span>
                <input
                  value={profileLabel}
                  onChange={(event) => setProfileLabel(event.target.value)}
                  placeholder="Claude Code / Work"
                />
              </label>

              <div className="field-grid">
                <label className="field">
                  <span>Executable</span>
                  <input
                    value={profileExecutable}
                    onChange={(event) =>
                      setProfileExecutable(event.target.value)
                    }
                    placeholder="claude"
                  />
                </label>

                <label className="field">
                  <span>Args</span>
                  <input
                    value={profileArgs}
                    onChange={(event) => setProfileArgs(event.target.value)}
                    placeholder="--dangerously-skip-permissions"
                  />
                </label>
              </div>

              <label className="field">
                <span>Environment JSON</span>
                <textarea
                  rows={5}
                  value={profileEnvJson}
                  onChange={(event) => setProfileEnvJson(event.target.value)}
                  placeholder='{"ANTHROPIC_API_KEY":"..."}'
                />
              </label>

              {profileError ? <p className="form-error">{profileError}</p> : null}

              <div className="action-row">
                <button
                  className="button button--primary"
                  disabled={isCreatingProfile}
                  type="submit"
                >
                  {isCreatingProfile
                    ? 'Saving...'
                    : editingLaunchProfileId === null
                      ? 'Create profile'
                      : 'Save profile'}
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => cancelLaunchProfileEditor()}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : null}
        </article>

        <article className="overview-card">
          <div className="overview-card__header">
            <div>
              <p className="panel__eyebrow">Diagnostics</p>
              <strong>Storage and runtime paths</strong>
            </div>
          </div>

          {storageInfo ? (
            <div className="settings-path-list">
              <div className="settings-path-row">
                <span>App data</span>
                <code>{storageInfo.appDataDir}</code>
              </div>
              <div className="settings-path-row">
                <span>Database dir</span>
                <code>{storageInfo.dbDir}</code>
              </div>
              <div className="settings-path-row">
                <span>Database file</span>
                <code>{storageInfo.dbPath}</code>
              </div>
              {runtimeDir ? (
                <div className="settings-path-row">
                  <span>Runtime dir</span>
                  <code>{runtimeDir}</code>
                </div>
              ) : null}
              {worktreeDir ? (
                <div className="settings-path-row">
                  <span>Managed worktrees</span>
                  <code>{worktreeDir}</code>
                </div>
              ) : null}
            </div>
          ) : (
            <div className="empty-state empty-state--rail">
              Storage info is not available yet.
            </div>
          )}
        </article>
      </div>
    </section>
  )
}

export default SettingsPanel
