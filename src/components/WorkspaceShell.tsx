import { useEffect } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useAppStore } from '../store'
import { mergeWorktreesForProject } from '../worktrees'
import AppSettingsModal from './workspace-shell/AppSettingsModal'
import ProjectCreateModalHost from './workspace-shell/ProjectCreateModalHost'
import ProjectRail from './workspace-shell/ProjectRail'
import SessionRail from './workspace-shell/SessionRail'
import WorkspaceCenter from './workspace-shell/WorkspaceCenter'

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  const tagName = target.tagName

  return (
    target.isContentEditable ||
    tagName === 'INPUT' ||
    tagName === 'TEXTAREA' ||
    tagName === 'SELECT' ||
    Boolean(target.closest('[contenteditable="true"]'))
  )
}

function resolveWorkspaceViewShortcut(key: string, hasSelectedWorktree: boolean) {
  switch (key) {
    case '1':
      return 'overview'
    case '2':
      return 'terminal'
    case '3':
      return hasSelectedWorktree ? 'worktreeWorkItem' : 'workItems'
    case '4':
      return hasSelectedWorktree ? null : 'history'
    case '5':
      return hasSelectedWorktree ? null : 'configuration'
    case '6':
      return hasSelectedWorktree ? null : 'workflows'
    default:
      return null
  }
}

function WorkspaceShell() {
  const { isProjectRailCollapsed, isSessionRailCollapsed } = useAppStore(
    useShallow((s) => ({
      isProjectRailCollapsed: s.isProjectRailCollapsed,
      isSessionRailCollapsed: s.isSessionRailCollapsed,
    })),
  )

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || isEditableTarget(event.target)) {
        return
      }

      const key = event.key.toLowerCase()
      const mod = event.metaKey || event.ctrlKey
      const state = useAppStore.getState()

      if (state.isProjectCreateOpen || state.isAppSettingsOpen) {
        return
      }

      if (mod && !event.altKey && !event.shiftKey && key === ',') {
        event.preventDefault()
        state.openAppSettings()
        return
      }

      if (mod && !event.altKey && !event.shiftKey && key === 'b') {
        event.preventDefault()
        state.setIsProjectRailCollapsed(!state.isProjectRailCollapsed)
        return
      }

      if (mod && !event.altKey && event.shiftKey && key === 'b') {
        event.preventDefault()
        state.setIsSessionRailCollapsed(!state.isSessionRailCollapsed)
        return
      }

      if (state.selectedProjectId === null) {
        return
      }

      if (mod && !event.altKey && !event.shiftKey) {
        const nextView = resolveWorkspaceViewShortcut(key, state.selectedTerminalWorktreeId !== null)
        if (nextView !== null) {
          event.preventDefault()
          state.setActiveView(nextView)
          return
        }
      }

      if (event.altKey && !mod && !event.shiftKey && (key === 'arrowup' || key === 'arrowdown')) {
        if (state.projects.length < 2) {
          return
        }

        event.preventDefault()
        const currentProjectIndex = state.projects.findIndex(
          (project) => project.id === state.selectedProjectId,
        )
        const direction = key === 'arrowdown' ? 1 : -1
        const nextProjectIndex =
          currentProjectIndex === -1
            ? 0
            : (currentProjectIndex + direction + state.projects.length) % state.projects.length

        state.selectProject(state.projects[nextProjectIndex].id)
        return
      }

      if (
        event.altKey &&
        !mod &&
        !event.shiftKey &&
        (key === 'arrowleft' || key === 'arrowright')
      ) {
        const visibleWorktrees = mergeWorktreesForProject(
          state.selectedProjectId,
          state.worktrees,
          state.stagedWorktrees,
        )
        const targets = [null, ...visibleWorktrees.map((worktree) => worktree.id)]

        if (targets.length < 2) {
          return
        }

        event.preventDefault()
        const currentTargetIndex = targets.indexOf(state.selectedTerminalWorktreeId)
        const direction = key === 'arrowright' ? 1 : -1
        const nextTargetIndex =
          currentTargetIndex === -1
            ? 0
            : (currentTargetIndex + direction + targets.length) % targets.length
        const nextTarget = targets[nextTargetIndex]

        if (nextTarget === null) {
          state.selectMainTerminal()
          return
        }

        state.selectWorktreeTerminal(nextTarget)
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  return (
    <main className="terminal-app">
      <section
        className={`terminal-layout ${
          isProjectRailCollapsed ? 'terminal-layout--left-collapsed' : ''
        } ${isSessionRailCollapsed ? 'terminal-layout--right-collapsed' : ''}`}
      >
        <ProjectRail />
        <WorkspaceCenter />
        <SessionRail />
      </section>

      <AppSettingsModal />
      <ProjectCreateModalHost />
    </main>
  )
}

export default WorkspaceShell
