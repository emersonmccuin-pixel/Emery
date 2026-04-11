import type {
  DocumentRecord,
  ProjectRecord,
  SessionSnapshot,
  WorkItemRecord,
  WorkItemStatus,
  WorktreeRecord,
} from '../types'

export const WORK_ITEM_STATUS_ORDER: Record<WorkItemStatus, number> = {
  in_progress: 0,
  blocked: 1,
  backlog: 2,
  done: 3,
}

export const PROJECT_COMMANDER_TOOLS = [
  'session_brief()',
  'list_work_items(status?, itemType?, parentOnly?, openOnly?)',
  'get_work_item(id)',
  'create_work_item(...)',
  'update_work_item(...)',
  'close_work_item(id)',
  'list_worktrees()',
  'launch_worktree_agent(workItemId, launchProfileId?)',
  'list_documents(workItemId?)',
  'create_document(...)',
  'update_document(...)',
] as const

export const DEFAULT_APP_SETTINGS = {
  defaultLaunchProfileId: null,
  autoRepairSafeCleanupOnStartup: false,
} as const

export const SESSION_EVENT_HISTORY_LIMIT = 120

export const DEFAULT_PROFILE_LABEL = 'Claude Code / YOLO'
export const DEFAULT_PROFILE_EXECUTABLE = 'claude'
export const DEFAULT_PROFILE_ARGS = '--dangerously-skip-permissions'
export const DEFAULT_PROFILE_ENV_JSON = '{}'

export function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    try {
      const parsed = JSON.parse(error) as { error?: unknown; message?: unknown }

      if (typeof parsed.error === 'string' && parsed.error.trim()) {
        return parsed.error
      }

      if (typeof parsed.message === 'string' && parsed.message.trim()) {
        return parsed.message
      }
    } catch {
      return error
    }

    return error
  }

  if (error && typeof error === 'object') {
    const candidate = error as { error?: unknown; message?: unknown }

    if (typeof candidate.error === 'string' && candidate.error.trim()) {
      return candidate.error
    }

    if (typeof candidate.message === 'string' && candidate.message.trim()) {
      return candidate.message
    }
  }

  return fallback
}

export function sortWorkItems(items: WorkItemRecord[]) {
  return [...items].sort((left, right) => {
    const statusDelta = WORK_ITEM_STATUS_ORDER[left.status] - WORK_ITEM_STATUS_ORDER[right.status]

    if (statusDelta !== 0) {
      return statusDelta
    }

    return right.updatedAt.localeCompare(left.updatedAt)
  })
}

export function sortDocuments(documents: DocumentRecord[]) {
  return [...documents].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
}

export function areSessionSnapshotListsEqual(left: SessionSnapshot[], right: SessionSnapshot[]) {
  if (left.length !== right.length) {
    return false
  }

  return left.every((snapshot, index) => {
    const candidate = right[index]

    return (
      snapshot.sessionId === candidate.sessionId &&
      snapshot.projectId === candidate.projectId &&
      (snapshot.worktreeId ?? null) === (candidate.worktreeId ?? null) &&
      snapshot.launchProfileId === candidate.launchProfileId &&
      snapshot.profileLabel === candidate.profileLabel &&
      snapshot.rootPath === candidate.rootPath &&
      snapshot.isRunning === candidate.isRunning &&
      snapshot.startedAt === candidate.startedAt &&
      (snapshot.exitCode ?? null) === (candidate.exitCode ?? null) &&
      (snapshot.exitSuccess ?? null) === (candidate.exitSuccess ?? null)
    )
  })
}

export function areWorktreeListsEqual(left: WorktreeRecord[], right: WorktreeRecord[]) {
  if (left.length !== right.length) {
    return false
  }

  return left.every((worktree, index) => {
    const candidate = right[index]

    return (
      worktree.id === candidate.id &&
      worktree.projectId === candidate.projectId &&
      worktree.workItemId === candidate.workItemId &&
      worktree.workItemCallSign === candidate.workItemCallSign &&
      worktree.workItemTitle === candidate.workItemTitle &&
      worktree.branchName === candidate.branchName &&
      worktree.shortBranchName === candidate.shortBranchName &&
      worktree.worktreePath === candidate.worktreePath &&
      worktree.pathAvailable === candidate.pathAvailable &&
      worktree.hasUncommittedChanges === candidate.hasUncommittedChanges &&
      worktree.hasUnmergedCommits === candidate.hasUnmergedCommits &&
      worktree.sessionSummary === candidate.sessionSummary &&
      worktree.createdAt === candidate.createdAt &&
      worktree.updatedAt === candidate.updatedAt
    )
  })
}

export function buildAgentStartupPrompt(
  project: ProjectRecord | null,
  workItems: WorkItemRecord[],
  documents: DocumentRecord[],
) {
  if (!project) {
    return ''
  }

  const workItemCallSigns = new Map(workItems.map((item) => [item.id, item.callSign]))
  const workItemLines =
    workItems.length === 0
      ? ['- No work items yet. Create them with project-commander-cli when needed.']
      : workItems
          .slice(0, 5)
          .map(
            (item) =>
              `- ${item.callSign} [${item.status}/${item.itemType}] ${item.title}`,
          )

  const documentLines =
    documents.length === 0
      ? ['- No documents yet.']
      : documents.slice(0, 5).map((document) => {
          const linkedCallSign =
            document.workItemId === null ? null : workItemCallSigns.get(document.workItemId) ?? null
          const linkedLabel =
            document.workItemId === null
              ? 'project-level'
              : `linked to ${linkedCallSign ?? `work item #${document.workItemId}`}`

          return `- #${document.id} [${linkedLabel}] ${document.title}`
        })

  return [
    'Project Commander dispatcher startup context.',
    `Project: ${project.name}`,
    `Root path: ${project.rootPath}`,
    'Project Commander MCP tools are attached to this session.',
    'Required first action: call session_brief.',
    'You are the repository dispatcher for this project.',
    'Use the Project Commander MCP tools as the source of truth for project context, work items, documents, and focused worktree launches.',
    'If MCP tools are unavailable, fall back to project-commander-cli.',
    'Key tools:',
    '- session_brief()',
    '- list_work_items(status?, itemType?, parentOnly?, openOnly?)',
    '- list_worktrees()',
    '- launch_worktree_agent(workItemId, launchProfileId?)',
    '- create_work_item(...)',
    '- update_work_item(...)',
    '- close_work_item(id)',
    '- list_documents(workItemId?)',
    'Current work items:',
    ...workItemLines,
    'Current documents:',
    ...documentLines,
    'After reading this context, inspect open bugs/features or create the next work item, then coordinate focused worktree execution when appropriate.',
  ].join('\n')
}

export function flattenPromptForTerminal(prompt: string) {
  return `${prompt.replace(/\s+/g, ' ').trim()}\r`
}
