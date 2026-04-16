import { Settings } from 'lucide-react'
import type { WorkspaceView } from '@/store/types'
import { normalizeWorkspaceViewForTarget } from '@/store/workspaceRouting'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  useAppStore,
  useBridgeReady,
  useOpenWorkItemCount,
  useRecoverableSessionCount,
  useSelectedProject,
  useSelectedWorktree,
} from '../../store'

const WORKTREE_TABS: readonly [WorkspaceView, string][] = [
  ['overview', 'OVERVIEW'],
  ['terminal', 'CONSOLE'],
  ['worktreeWorkItem', 'WORK ITEM'],
]

const PROJECT_TABS: readonly [WorkspaceView, string][] = [
  ['overview', 'OVERVIEW'],
  ['files', 'FILES'],
  ['terminal', 'CONSOLE'],
  ['workItems', 'BACKLOG'],
  ['history', 'HISTORY'],
  ['configuration', 'CONFIGURATION'],
  ['workflows', 'WORKFLOWS'],
]

function WorkspaceNav() {
  const selectedProject = useSelectedProject()
  const selectedWorktree = useSelectedWorktree()
  const bridgeReady = useBridgeReady()
  const openWorkItemCount = useOpenWorkItemCount()
  const recoverableSessionCount = useRecoverableSessionCount()
  const activeView = useAppStore((s) => s.activeView)
  const selectedTerminalWorktreeId = useAppStore((s) => s.selectedTerminalWorktreeId)

  const { setActiveView, openAppSettings } = useAppStore.getState()
  const tabs = selectedTerminalWorktreeId !== null ? WORKTREE_TABS : PROJECT_TABS
  const normalizedActiveView = normalizeWorkspaceViewForTarget(
    activeView,
    selectedTerminalWorktreeId,
  )

  if (!selectedProject) {
    return null
  }

  return (
    <Tabs
      value={normalizedActiveView}
      onValueChange={(value) => setActiveView(value as typeof activeView)}
        className="contents"
    >
      <nav className="workspace-tabs--shell flex items-center justify-between h-10 px-4 shrink-0">
        <TabsList>
          {tabs.map(([value, label]) => (
            <TabsTrigger key={value} value={value}>
              {label}
              {value === 'workItems' && openWorkItemCount > 0 ? (
                <span className="ml-2 px-1 rounded-sm bg-hud-green/10 text-[9px] text-hud-green font-bold border border-hud-green/40">
                  {openWorkItemCount}
                </span>
              ) : null}
              {value === 'history' && recoverableSessionCount > 0 ? (
                <span className="ml-2 px-1 rounded-sm bg-hud-magenta/10 text-[9px] text-hud-magenta font-bold border border-hud-magenta/40">
                  {recoverableSessionCount}
                </span>
              ) : null}
            </TabsTrigger>
          ))}
        </TabsList>
        <div className="flex items-center gap-3">
          <button
            type="button"
            className="h-6 w-6 inline-flex items-center justify-center rounded text-hud-cyan/60 hover:text-hud-cyan hover:bg-hud-cyan/10 transition-colors"
            aria-label="Settings"
            onClick={() => openAppSettings()}
          >
            <Settings size={13} />
          </button>
          <span className="text-[10px] font-black uppercase tracking-[0.15em] text-hud-cyan truncate max-w-[200px]">
            {selectedTerminalWorktreeId !== null && selectedWorktree
              ? selectedWorktree.workItemCallSign || selectedWorktree.shortBranchName.toUpperCase()
              : selectedProject.name}
          </span>
          <div
            className={`h-1.5 w-1.5 rounded-full ${
              bridgeReady ? 'bg-hud-green shadow-[0_0_8px_rgba(116,243,161,0.6)]' : 'bg-destructive'
            }`}
          />
        </div>
      </nav>
    </Tabs>
  )
}

export default WorkspaceNav
