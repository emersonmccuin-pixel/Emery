import { Suspense, lazy } from 'react'
import { useAppStore, useSelectedWorktree } from '../../store'
import { PanelFallback } from './shared'
import HistoryStage from './HistoryStage'
import TerminalStage from './TerminalStage'
import WorkItemsStage from './WorkItemsStage'

const ConfigurationPanel = lazy(() => import('../ConfigurationPanel'))
const WorktreeWorkItemPanel = lazy(() => import('../WorktreeWorkItemPanel'))

function WorktreeWorkItemStage() {
  const selectedWorktree = useSelectedWorktree()

  if (!selectedWorktree) {
    return null
  }

  return (
    <Suspense fallback={<PanelFallback label="Loading work item..." />}>
      <WorktreeWorkItemPanel worktree={selectedWorktree} />
    </Suspense>
  )
}

function WorkspaceActiveStage() {
  const activeView = useAppStore((s) => s.activeView)

  switch (activeView) {
    case 'terminal':
      return <TerminalStage />
    case 'history':
      return <HistoryStage />
    case 'configuration':
      return (
        <Suspense fallback={<PanelFallback label="Loading configuration..." />}>
          <ConfigurationPanel />
        </Suspense>
      )
    case 'workItems':
      return <WorkItemsStage />
    case 'worktreeWorkItem':
      return <WorktreeWorkItemStage />
    default:
      return null
  }
}

export default WorkspaceActiveStage
