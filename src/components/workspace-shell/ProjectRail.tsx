import { ChevronLeft, ChevronRight, Plus } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useAppStore } from '../../store'

function ProjectRail() {
  const { projects, selectedProjectId, isProjectRailCollapsed, isProjectCreateOpen } = useAppStore(
    useShallow((s) => ({
      projects: s.projects,
      selectedProjectId: s.selectedProjectId,
      isProjectRailCollapsed: s.isProjectRailCollapsed,
      isProjectCreateOpen: s.isProjectCreateOpen,
    })),
  )

  const {
    setIsProjectRailCollapsed,
    startCreateProject,
    cancelCreateProject,
    selectProject,
  } = useAppStore.getState()

  return (
    <aside
      className={`side-rail side-rail--projects ${
        isProjectRailCollapsed ? 'side-rail--collapsed' : ''
      }`}
    >
      {isProjectRailCollapsed ? (
        <div className="side-rail__collapsed h-full flex flex-col items-center py-4 gap-4">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setIsProjectRailCollapsed(false)}
            className="h-8 w-8 text-hud-cyan"
            title="Expand project rail (Ctrl/Cmd+B)"
            aria-label="Expand project rail"
            aria-keyshortcuts="Control+B Meta+B"
          >
            <ChevronRight size={20} />
          </Button>
        </div>
      ) : (
        <>
          <div className="side-rail__header">
            <div className="flex items-center justify-between">
              <span className="rail-label">PROJECTS</span>
              <div className="flex gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => (isProjectCreateOpen ? cancelCreateProject() : startCreateProject())}
                  className="h-5 w-5 hover:text-hud-cyan"
                >
                  <Plus size={14} />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setIsProjectRailCollapsed(true)}
                  className="h-5 w-5"
                  title="Collapse project rail (Ctrl/Cmd+B)"
                  aria-label="Collapse project rail"
                  aria-keyshortcuts="Control+B Meta+B"
                >
                  <ChevronLeft size={14} />
                </Button>
              </div>
            </div>
          </div>

          <ScrollArea className="flex-1 hud-scrollarea">
            <div className="project-list mt-2">
              {projects.length === 0 ? (
                <div className="text-[10px] uppercase opacity-60 text-center py-8">No Data.</div>
              ) : (
                projects.map((project) => (
                  <button
                    key={project.id}
                    className={`project-card--minimal w-full text-left truncate flex items-center justify-between group ${
                      project.id === selectedProjectId ? 'project-card--active' : ''
                    }`}
                    type="button"
                    onClick={() => selectProject(project.id)}
                  >
                    <span className="uppercase tracking-widest">{project.name}</span>
                    <div
                      className={`h-1.5 w-1.5 rounded-full ${
                        project.id === selectedProjectId
                          ? 'bg-hud-cyan animate-pulse shadow-[0_0_8px_rgba(58,240,224,0.8)]'
                          : 'bg-hud-cyan/20 group-hover:bg-hud-cyan/40'
                      }`}
                    />
                  </button>
                ))
              )}
            </div>
          </ScrollArea>
        </>
      )}
    </aside>
  )
}

export default ProjectRail
