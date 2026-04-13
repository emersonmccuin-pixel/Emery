import { Suspense, lazy } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useAppStore, useSelectedProject } from '../../store'
import { PanelFallback } from './shared'

const WorkItemsPanel = lazy(() => import('../WorkItemsPanel'))

function WorkItemsStage() {
  const selectedProject = useSelectedProject()
  const { workItems, workItemError, isLoadingWorkItems, startingWorkItemId } = useAppStore(
    useShallow((s) => ({
      workItems: s.workItems,
      workItemError: s.workItemError,
      isLoadingWorkItems: s.isLoadingWorkItems,
      startingWorkItemId: s.startingWorkItemId,
    })),
  )

  const { createWorkItem, deleteWorkItem, startWorkItemInTerminal, updateWorkItem } =
    useAppStore.getState()

  return (
    <Suspense fallback={<PanelFallback label="Loading backlog..." />}>
      <WorkItemsPanel
        error={workItemError}
        isLoading={isLoadingWorkItems}
        onCreate={createWorkItem}
        onDelete={deleteWorkItem}
        onStartInTerminal={startWorkItemInTerminal}
        onUpdate={updateWorkItem}
        project={selectedProject}
        startingWorkItemId={startingWorkItemId}
        workItems={workItems}
      />
    </Suspense>
  )
}

export default WorkItemsStage
