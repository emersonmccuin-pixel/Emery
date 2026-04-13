import { Suspense, lazy, useState } from 'react'
import type { FormEvent } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { PanelLoadingState } from '@/components/ui/panel-state'
import { useAppStore } from '../store'
import { themes } from '../themes'

type AppSettingsTab = 'appearance' | 'accounts' | 'defaults' | 'diagnostics'

type Props = {
  initialTab?: AppSettingsTab
}

const DiagnosticsConsole = lazy(() => import('@/components/DiagnosticsConsole'))

function AppSettingsPanel({ initialTab = 'appearance' }: Props) {
  const [activeTab, setActiveTab] = useState<AppSettingsTab>(initialTab)

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
        <TabsContent value="diagnostics">
          <DiagnosticsTab isActive={activeTab === 'diagnostics'} />
        </TabsContent>
      </div>
    </Tabs>
  )
}

function Banner() {
  const settingsError = useAppStore((s) => s.settingsError)
  const settingsMessage = useAppStore((s) => s.settingsMessage)
  if (!settingsError && !settingsMessage) return null
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
  )
}

function AppearanceTab() {
  const activeThemeId = useAppStore((s) => s.activeThemeId)

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
          const isActive = id === activeThemeId
          return (
            <button
              key={id}
              type="button"
              className={`theme-card${isActive ? ' theme-card--active' : ''}`}
              style={
                isActive
                  ? {
                      borderColor: theme['--center-tint'],
                      boxShadow: `0 0 0 1px ${theme['--center-tint']}, 0 0 16px color-mix(in srgb, ${theme['--center-tint']} 40%, transparent)`,
                    }
                  : undefined
              }
              onClick={() => useAppStore.getState().setActiveThemeId(id)}
            >
              {/* Mini 3-panel preview */}
              <div
                className="theme-card__preview"
                style={{ background: theme['--hud-bg'] }}
              >
                <div
                  className="theme-card__preview-panel"
                  style={{
                    background: theme['--hud-panel-bg'],
                    borderColor: theme['--rail-projects-tint'],
                  }}
                />
                <div
                  className="theme-card__preview-panel theme-card__preview-panel--center"
                  style={{
                    background: theme['--hud-panel-bg'],
                    borderColor: theme['--center-tint'],
                  }}
                />
                <div
                  className="theme-card__preview-panel"
                  style={{
                    background: theme['--hud-panel-bg'],
                    borderColor: theme['--rail-sessions-tint'],
                  }}
                />
              </div>

              {/* Theme name + swatches row */}
              <div className="theme-card__footer">
                <span className="theme-card__label">{theme.label}</span>
                <div className="theme-card__swatches">
                  {(
                    [
                      '--rail-projects-tint',
                      '--center-tint',
                      '--rail-sessions-tint',
                      '--hud-amber',
                      '--hud-purple',
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
                  style={{ color: theme['--center-tint'] }}
                >
                  ✓
                </div>
              )}
            </button>
          )
        })}
      </div>
    </article>
  )
}

function DefaultsTab() {
  const launchProfiles = useAppStore((s) => s.launchProfiles)
  const defaultLaunchProfileSettingId = useAppStore((s) => s.defaultLaunchProfileSettingId)
  const autoRepairSafeCleanupOnStartup = useAppStore((s) => s.autoRepairSafeCleanupOnStartup)
  const isSavingAppSettings = useAppStore((s) => s.isSavingAppSettings)
  const {
    setDefaultLaunchProfileSettingId,
    setAutoRepairSafeCleanupOnStartup,
    submitAppSettings,
  } = useAppStore.getState()

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
          <span>Default launch profile</span>
          <select
            value={
              defaultLaunchProfileSettingId === null
                ? ''
                : String(defaultLaunchProfileSettingId)
            }
            onChange={(event) =>
              setDefaultLaunchProfileSettingId(
                event.target.value === '' ? null : Number(event.target.value),
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
          <Button
            variant="default"
            disabled={isSavingAppSettings}
            type="submit"
          >
            {isSavingAppSettings ? 'Saving...' : 'Save app settings'}
          </Button>
        </div>
      </form>
    </article>
  )
}

function AccountsTab() {
  const appSettings = useAppStore((s) => s.appSettings)
  const selectedLaunchProfileId = useAppStore((s) => s.selectedLaunchProfileId)
  const launchProfiles = useAppStore((s) => s.launchProfiles)
  const profileLabel = useAppStore((s) => s.profileLabel)
  const profileExecutable = useAppStore((s) => s.profileExecutable)
  const profileArgs = useAppStore((s) => s.profileArgs)
  const profileEnvJson = useAppStore((s) => s.profileEnvJson)
  const profileError = useAppStore((s) => s.profileError)
  const isProfileFormOpen = useAppStore((s) => s.isProfileFormOpen)
  const editingLaunchProfileId = useAppStore((s) => s.editingLaunchProfileId)
  const isCreatingProfile = useAppStore((s) => s.isCreatingProfile)
  const activeDeleteLaunchProfileId = useAppStore((s) => s.activeDeleteLaunchProfileId)

  const {
    setSelectedLaunchProfileId,
    setProfileLabel,
    setProfileExecutable,
    setProfileArgs,
    setProfileEnvJson,
    submitLaunchProfile,
    startCreateLaunchProfile,
    startEditLaunchProfile,
    cancelLaunchProfileEditor,
    deleteLaunchProfile,
  } = useAppStore.getState()

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Accounts</p>
          <strong>Manage Claude launch accounts</strong>
        </div>
        <Button
          variant="outline"
          size="sm"
          type="button"
          onClick={() => startCreateLaunchProfile()}
        >
          {isProfileFormOpen && editingLaunchProfileId === null
            ? 'Adding profile'
            : 'Add account'}
        </Button>
      </div>

      <div className="settings-profile-list">
        {launchProfiles.length === 0 ? (
          <div className="empty-state empty-state--rail">
            No accounts configured yet.
          </div>
        ) : (
          launchProfiles.map((profile) => (
            <article key={profile.id} className="settings-profile-card">
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
                Environment JSON is stored with this profile and injected into
                newly launched sessions.
              </p>

              <div className="action-row">
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => setSelectedLaunchProfileId(profile.id)}
                >
                  {selectedLaunchProfileId === profile.id ? 'In use' : 'Use now'}
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
                    ? 'Deleting...'
                    : 'Delete'}
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
                ? 'Add launch account'
                : 'Edit launch account'}
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

          <div className="field-grid">
            <label className="field">
              <span>Executable</span>
              <Input
                value={profileExecutable}
                onChange={(event) => setProfileExecutable(event.target.value)}
                placeholder="claude"
                className="hud-input"
              />
            </label>

            <label className="field">
              <span>Args</span>
              <Input
                value={profileArgs}
                onChange={(event) => setProfileArgs(event.target.value)}
                placeholder="--dangerously-skip-permissions"
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
              placeholder='{"ANTHROPIC_API_KEY":"..."}'
            />
          </label>

          {profileError ? <p className="form-error">{profileError}</p> : null}

          <div className="action-row">
            <Button
              variant="default"
              disabled={isCreatingProfile}
              type="submit"
            >
              {isCreatingProfile
                ? 'Saving...'
                : editingLaunchProfileId === null
                  ? 'Create account'
                  : 'Save account'}
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
  )
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
  )
}

export default AppSettingsPanel
