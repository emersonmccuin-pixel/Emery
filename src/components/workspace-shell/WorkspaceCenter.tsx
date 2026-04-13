import { Settings } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { PanelBanner, PanelEmptyState } from '@/components/ui/panel-state'
import { useAppStore } from '../../store'
import RecoveryBannerHost from './RecoveryBannerHost'
import WorkspaceActiveStage from './WorkspaceActiveStage'
import WorkspaceNav from './WorkspaceNav'

function WorkspaceCenter() {
  const hasSelectedProject = useAppStore((s) => s.selectedProjectId !== null)
  const sessionError = useAppStore((s) => s.sessionError)

  return (
    <section className="center-stage flex flex-col">
      {!hasSelectedProject ? (
        <div className="flex-1 min-h-0 p-8">
          <PanelEmptyState
            action={
              <Button
                variant="outline"
                className="hud-button--cyan"
                onClick={() => useAppStore.getState().startCreateProject()}
              >
                INITIATE NEW PROJECT
              </Button>
            }
            className="h-full min-h-[28rem]"
            detail="Select an existing workspace or register a new repository root to load dispatchers, worktrees, backlog items, and recovery history."
            eyebrow="Project Commander"
            icon={<Settings className="h-6 w-6" />}
            title="No project selected"
            tone="cyan"
          />
        </div>
      ) : (
        <>
          <WorkspaceNav />
          <RecoveryBannerHost />

          <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
            {sessionError ? (
              <PanelBanner
                className="rounded-none border-x-0 border-t-0"
                message={`Session alert: ${sessionError}`}
              />
            ) : null}

            <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
              <WorkspaceActiveStage />
            </div>
          </div>
        </>
      )}
    </section>
  )
}

export default WorkspaceCenter
