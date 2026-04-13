import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { startTrackedPerfSpan, withPerfSpan } from '../perf'

import type {
  AppSettings,
  BootstrapData,
  LaunchProfileRecord,
  ProjectRecord,
  StorageInfo,
} from '../types'
import type { AppStore, ProjectSlice } from './types'
import {
  DEFAULT_APP_SETTINGS,
  DEFAULT_PROFILE_ARGS,
  DEFAULT_PROFILE_ENV_JSON,
  DEFAULT_PROFILE_EXECUTABLE,
  DEFAULT_PROFILE_LABEL,
  getErrorMessage,
} from './utils'

export const createProjectSlice: StateCreator<AppStore, [], [], ProjectSlice> = (set, get) => ({
  projects: [],
  launchProfiles: [],
  appSettings: DEFAULT_APP_SETTINGS,
  storageInfo: null,
  selectedProjectId: null,
  selectedLaunchProfileId: null,

  projectName: '',
  projectRootPath: '',
  projectError: null,
  isProjectCreateOpen: false,
  isCreatingProject: false,

  editProjectName: '',
  editProjectRootPath: '',
  projectUpdateError: null,
  isProjectEditorOpen: false,
  isUpdatingProject: false,

  profileLabel: DEFAULT_PROFILE_LABEL,
  profileExecutable: DEFAULT_PROFILE_EXECUTABLE,
  profileArgs: DEFAULT_PROFILE_ARGS,
  profileEnvJson: DEFAULT_PROFILE_ENV_JSON,
  profileError: null,
  isProfileFormOpen: false,
  editingLaunchProfileId: null,
  isCreatingProfile: false,
  activeDeleteLaunchProfileId: null,

  settingsError: null,
  settingsMessage: null,
  defaultLaunchProfileSettingId: null,
  autoRepairSafeCleanupOnStartup: false,
  isSavingAppSettings: false,

  setProjectName: (value) => set({ projectName: value }),
  setProjectRootPath: (value) => set({ projectRootPath: value }),
  setProjectError: (value) => set({ projectError: value }),
  setProjectUpdateError: (value) => set({ projectUpdateError: value }),
  setSelectedLaunchProfileId: (value) => set({ selectedLaunchProfileId: value }),
  setEditProjectName: (value) => set({ editProjectName: value }),
  setEditProjectRootPath: (value) => set({ editProjectRootPath: value }),
  setIsProjectEditorOpen: (value) => set({ isProjectEditorOpen: value }),
  setIsProjectCreateOpen: (value) => set({ isProjectCreateOpen: value }),
  setDefaultLaunchProfileSettingId: (value) => set({ defaultLaunchProfileSettingId: value }),
  setAutoRepairSafeCleanupOnStartup: (value) => set({ autoRepairSafeCleanupOnStartup: value }),
  setSettingsError: (value) => set({ settingsError: value }),
  setProfileLabel: (value) => set({ profileLabel: value }),
  setProfileExecutable: (value) => set({ profileExecutable: value }),
  setProfileArgs: (value) => set({ profileArgs: value }),
  setProfileEnvJson: (value) => set({ profileEnvJson: value }),
  setIsProfileFormOpen: (value) => set({ isProfileFormOpen: value }),

  bootstrap: async () => {
    try {
      const [, bootstrap, storage] = await withPerfSpan(
        'bootstrap_state',
        {},
        () =>
          Promise.all([
            invoke<string>('health_check'),
            invoke<BootstrapData>('bootstrap_app_state'),
            invoke<StorageInfo>('get_storage_info'),
          ]),
      )

      set((state) => ({
        storageInfo: storage,
        appSettings: bootstrap.settings,
        projects: bootstrap.projects,
        launchProfiles: bootstrap.launchProfiles,
        selectedProjectId: state.selectedProjectId ?? bootstrap.projects[0]?.id ?? null,
        selectedLaunchProfileId:
          state.selectedLaunchProfileId ??
          bootstrap.settings.defaultLaunchProfileId ??
          bootstrap.launchProfiles[0]?.id ??
          null,
        defaultLaunchProfileSettingId: bootstrap.settings.defaultLaunchProfileId,
        autoRepairSafeCleanupOnStartup: bootstrap.settings.autoRepairSafeCleanupOnStartup,
        isProjectCreateOpen: bootstrap.projects.length === 0,
      }))
    } catch (error) {
      set({ sessionError: getErrorMessage(error, 'The Rust runtime did not respond.') })
    }
  },

  selectProject: (projectId) => {
    const state = get()
    const project = state.projects.find((candidate) => candidate.id === projectId) ?? null

    startTrackedPerfSpan('project-switch', 'project_switch', {
      projectId,
      projectName: project?.name ?? 'unknown',
    })

    set({
      selectedProjectId: projectId,
      selectedTerminalWorktreeId: null,
      activeView: 'terminal',
      terminalPromptDraft: null,
      worktreeError: null,
      worktreeMessage: null,
      activeWorktreeActionId: null,
      activeWorktreeActionKind: null,
      selectedHistorySessionId: null,
    })

    if (project) {
      set({
        editProjectName: project.name,
        editProjectRootPath: project.rootPath,
        projectUpdateError: null,
        isProjectEditorOpen: !project.rootAvailable,
      })
    }
  },

  startCreateProject: () => {
    set({
      projectName: '',
      projectRootPath: '',
      projectError: null,
      isProjectCreateOpen: true,
    })
  },

  cancelCreateProject: () => {
    set({
      projectName: '',
      projectRootPath: '',
      projectError: null,
      isProjectCreateOpen: false,
    })
  },

  browseForProjectFolder: async (applyPath, setError) => {
    setError(null)

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select project root folder',
      })

      if (typeof selected === 'string') {
        applyPath(selected)
      }
    } catch (error) {
      setError(getErrorMessage(error, 'Failed to open folder picker.'))
    }
  },

  submitProject: async (event) => {
    event.preventDefault()
    const { projectName, projectRootPath } = get()
    set({ projectError: null, isCreatingProject: true })

    try {
      const project = await invoke<ProjectRecord>('create_project', {
        input: { name: projectName, rootPath: projectRootPath },
      })

      set((state) => ({
        projects: [project, ...state.projects.filter((p) => p.id !== project.id)],
        projectName: '',
        projectRootPath: '',
        projectError: null,
        isProjectCreateOpen: false,
        selectedProjectId: project.id,
        selectedTerminalWorktreeId: null,
        activeView: 'terminal',
      }))
    } catch (error) {
      set({ projectError: getErrorMessage(error, 'Failed to create project.') })
    } finally {
      set({ isCreatingProject: false })
    }
  },

  submitProjectUpdate: async (event) => {
    event.preventDefault()
    const { selectedProjectId: projectId, editProjectName, editProjectRootPath } = get()

    if (projectId === null) {
      return
    }

    set({ projectUpdateError: null, isUpdatingProject: true })

    try {
      const project = await invoke<ProjectRecord>('update_project', {
        input: { id: projectId, name: editProjectName, rootPath: editProjectRootPath },
      })

      set((state) => ({
        projects: [project, ...state.projects.filter((p) => p.id !== project.id)],
        selectedProjectId: project.id,
        editProjectName: project.name,
        editProjectRootPath: project.rootPath,
        isProjectEditorOpen: false,
        sessionError:
          state.sessionError ===
          'selected project root folder no longer exists. Rebind the project before launching.'
            ? null
            : state.sessionError,
      }))
      await get().refreshSelectedProjectData([
        'documents',
        'worktrees',
        'liveSessions',
        'sessionSnapshot',
        'history',
        'orphanedSessions',
        'cleanupCandidates',
        'workItems',
      ])
    } catch (error) {
      set({ projectUpdateError: getErrorMessage(error, 'Failed to update project.') })
    } finally {
      set({ isUpdatingProject: false })
    }
  },

  submitAppSettings: async (event) => {
    event.preventDefault()
    const { defaultLaunchProfileSettingId, autoRepairSafeCleanupOnStartup } = get()
    set({ settingsError: null, settingsMessage: null, isSavingAppSettings: true })

    try {
      const settings = await invoke<AppSettings>('update_app_settings', {
        input: { defaultLaunchProfileId: defaultLaunchProfileSettingId, autoRepairSafeCleanupOnStartup },
      })

      set({
        appSettings: settings,
        settingsMessage: 'Settings saved.',
        selectedLaunchProfileId:
          settings.defaultLaunchProfileId !== null
            ? settings.defaultLaunchProfileId
            : get().selectedLaunchProfileId,
      })
    } catch (error) {
      set({ settingsError: getErrorMessage(error, 'Failed to save app settings.') })
    } finally {
      set({ isSavingAppSettings: false })
    }
  },

  submitLaunchProfile: async (event) => {
    event.preventDefault()
    const {
      editingLaunchProfileId,
      profileLabel,
      profileExecutable,
      profileArgs,
      profileEnvJson,
    } = get()
    set({ profileError: null, settingsMessage: null, isCreatingProfile: true })

    try {
      const profile =
        editingLaunchProfileId === null
          ? await invoke<LaunchProfileRecord>('create_launch_profile', {
              input: { label: profileLabel, executable: profileExecutable, args: profileArgs, envJson: profileEnvJson },
            })
          : await invoke<LaunchProfileRecord>('update_launch_profile', {
              input: {
                id: editingLaunchProfileId,
                label: profileLabel,
                executable: profileExecutable,
                args: profileArgs,
                envJson: profileEnvJson,
              },
            })

      set((state) => ({
        launchProfiles:
          editingLaunchProfileId === null
            ? [...state.launchProfiles, profile]
            : state.launchProfiles.map((p) => (p.id === profile.id ? profile : p)),
        selectedLaunchProfileId: profile.id,
        settingsMessage: editingLaunchProfileId === null ? 'Launch profile created.' : 'Launch profile updated.',
        editingLaunchProfileId: null,
        profileLabel: DEFAULT_PROFILE_LABEL,
        profileExecutable: DEFAULT_PROFILE_EXECUTABLE,
        profileArgs: DEFAULT_PROFILE_ARGS,
        profileEnvJson: DEFAULT_PROFILE_ENV_JSON,
        profileError: null,
        isProfileFormOpen: false,
      }))
    } catch (error) {
      set({
        profileError: getErrorMessage(
          error,
          editingLaunchProfileId === null ? 'Failed to create launch profile.' : 'Failed to update launch profile.',
        ),
      })
    } finally {
      set({ isCreatingProfile: false })
    }
  },

  startCreateLaunchProfile: () => {
    set({
      editingLaunchProfileId: null,
      profileLabel: DEFAULT_PROFILE_LABEL,
      profileExecutable: DEFAULT_PROFILE_EXECUTABLE,
      profileArgs: DEFAULT_PROFILE_ARGS,
      profileEnvJson: DEFAULT_PROFILE_ENV_JSON,
      profileError: null,
      settingsMessage: null,
      isProfileFormOpen: true,
      isAppSettingsOpen: true,
      appSettingsInitialTab: 'accounts',
    })
  },

  startEditLaunchProfile: (profile) => {
    set({
      editingLaunchProfileId: profile.id,
      profileLabel: profile.label,
      profileExecutable: profile.executable,
      profileArgs: profile.args,
      profileEnvJson: profile.envJson,
      profileError: null,
      settingsMessage: null,
      isProfileFormOpen: true,
      isAppSettingsOpen: true,
      appSettingsInitialTab: 'accounts',
    })
  },

  cancelLaunchProfileEditor: () => {
    set({
      editingLaunchProfileId: null,
      profileLabel: DEFAULT_PROFILE_LABEL,
      profileExecutable: DEFAULT_PROFILE_EXECUTABLE,
      profileArgs: DEFAULT_PROFILE_ARGS,
      profileEnvJson: DEFAULT_PROFILE_ENV_JSON,
      profileError: null,
      isProfileFormOpen: false,
    })
  },

  deleteLaunchProfile: async (profile) => {
    if (
      !window.confirm(
        `Delete launch profile "${profile.label}"? Existing session records will be preserved.`,
      )
    ) {
      return
    }

    set({ profileError: null, settingsError: null, settingsMessage: null, activeDeleteLaunchProfileId: profile.id })

    try {
      await invoke('delete_launch_profile', { id: profile.id })

      set((state) => {
        const remainingProfiles = state.launchProfiles.filter((p) => p.id !== profile.id)
        const nextSelectedId =
          state.selectedLaunchProfileId === profile.id
            ? state.appSettings.defaultLaunchProfileId === profile.id
              ? remainingProfiles[0]?.id ?? null
              : state.appSettings.defaultLaunchProfileId ?? remainingProfiles[0]?.id ?? null
            : state.selectedLaunchProfileId

        return {
          launchProfiles: remainingProfiles,
          selectedLaunchProfileId: nextSelectedId,
          appSettings:
            state.appSettings.defaultLaunchProfileId === profile.id
              ? { ...state.appSettings, defaultLaunchProfileId: null }
              : state.appSettings,
          editingLaunchProfileId:
            state.editingLaunchProfileId === profile.id ? null : state.editingLaunchProfileId,
          isProfileFormOpen:
            state.editingLaunchProfileId === profile.id ? false : state.isProfileFormOpen,
          settingsMessage: 'Launch profile deleted.',
        }
      })
    } catch (error) {
      set({ settingsError: getErrorMessage(error, 'Failed to delete launch profile.') })
    } finally {
      set({ activeDeleteLaunchProfileId: null })
    }
  },

  projectCreated: (project) => {
    set((state) => ({
      projects: [project, ...state.projects.filter((p) => p.id !== project.id)],
      selectedProjectId: project.id,
      selectedTerminalWorktreeId: null,
      activeView: 'terminal',
      isProjectCreateOpen: false,
      projectName: '',
      projectRootPath: '',
      projectError: null,
    }))
  },

  adjustProjectWorkItemCount: (projectId, delta) => {
    set((state) => ({
      projects: state.projects.map((p) =>
        p.id === projectId ? { ...p, workItemCount: Math.max(0, p.workItemCount + delta) } : p,
      ),
    }))
  },

  adjustProjectDocumentCount: (projectId, delta) => {
    set((state) => ({
      projects: state.projects.map((p) =>
        p.id === projectId ? { ...p, documentCount: Math.max(0, p.documentCount + delta) } : p,
      ),
    }))
  },
})
