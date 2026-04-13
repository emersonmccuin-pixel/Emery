import { Suspense, lazy } from 'react'
import { useAppStore } from '../../store'

const CreateProjectModal = lazy(() => import('../CreateProjectModal'))

function ProjectCreateModalHost() {
  const isProjectCreateOpen = useAppStore((s) => s.isProjectCreateOpen)

  if (!isProjectCreateOpen) {
    return null
  }

  return (
    <Suspense fallback={null}>
      <CreateProjectModal />
    </Suspense>
  )
}

export default ProjectCreateModalHost
