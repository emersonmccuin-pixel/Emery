import type { StateCreator } from 'zustand'
import { flushSync } from 'react-dom'
import { invoke } from '@/lib/tauri'
import type {
  DocumentRecord,
  ProjectRecord,
  SessionSnapshot,
  WorkItemRecord,
  WorkItemStatus,
  WorktreeLaunchOutput,
} from '../types'
import type { AppStore, WorkItemSlice } from './types'
import {
  areDocumentListsEqual,
  areWorkItemListsEqual,
  getErrorMessage,
  sortDocuments,
  sortWorkItems,
} from './utils'

function syncProjectCount(
  projects: ProjectRecord[],
  projectId: number,
  field: 'workItemCount' | 'documentCount',
  value: number,
) {
  let changed = false
  const nextProjects = projects.map((project) => {
    if (project.id !== projectId || project[field] === value) {
      return project
    }

    changed = true
    return { ...project, [field]: value }
  })

  return changed ? nextProjects : projects
}

export const createWorkItemSlice: StateCreator<AppStore, [], [], WorkItemSlice> = (set, get) => ({
  workItems: [],
  workItemError: null,
  isLoadingWorkItems: false,
  startingWorkItemId: null,
  documents: [],
  documentError: null,
  isLoadingDocuments: false,
  isDocumentsManagerOpen: false,

  setIsDocumentsManagerOpen: (value) => set({ isDocumentsManagerOpen: value }),

  refreshWorkItems: async (projectId) => {
    try {
      const items = await invoke<WorkItemRecord[]>('list_work_items', { projectId })
      const nextWorkItems = sortWorkItems(items)

      set((state) => ({
        workItems: areWorkItemListsEqual(state.workItems, nextWorkItems)
          ? state.workItems
          : nextWorkItems,
        projects: syncProjectCount(state.projects, projectId, 'workItemCount', items.length),
      }))

      return nextWorkItems
    } catch (error) {
      set({ workItemError: getErrorMessage(error, 'Failed to load work items.') })
      return []
    }
  },

  refreshDocuments: async (projectId) => {
    try {
      const items = await invoke<DocumentRecord[]>('list_documents', { projectId })
      const nextDocuments = sortDocuments(items)

      set((state) => ({
        documents: areDocumentListsEqual(state.documents, nextDocuments)
          ? state.documents
          : nextDocuments,
        projects: syncProjectCount(state.projects, projectId, 'documentCount', items.length),
      }))

      return nextDocuments
    } catch (error) {
      set({ documentError: getErrorMessage(error, 'Failed to load documents.') })
      return []
    }
  },

  loadWorkItems: async (projectId) => {
    set({ isLoadingWorkItems: true, workItemError: null })

    try {
      const items = await invoke<WorkItemRecord[]>('list_work_items', { projectId })
      const nextWorkItems = sortWorkItems(items)

      set((state) => ({
        workItems: areWorkItemListsEqual(state.workItems, nextWorkItems)
          ? state.workItems
          : nextWorkItems,
        isLoadingWorkItems: false,
        projects: syncProjectCount(state.projects, projectId, 'workItemCount', items.length),
      }))
    } catch (error) {
      set({
        workItemError: getErrorMessage(error, 'Failed to load work items.'),
        isLoadingWorkItems: false,
      })
    }
  },

  loadDocuments: async (projectId) => {
    set({ isLoadingDocuments: true, documentError: null })

    try {
      const items = await invoke<DocumentRecord[]>('list_documents', { projectId })
      const nextDocuments = sortDocuments(items)

      set((state) => ({
        documents: areDocumentListsEqual(state.documents, nextDocuments)
          ? state.documents
          : nextDocuments,
        isLoadingDocuments: false,
        projects: syncProjectCount(state.projects, projectId, 'documentCount', items.length),
      }))
    } catch (error) {
      set({
        documentError: getErrorMessage(error, 'Failed to load documents.'),
        isLoadingDocuments: false,
      })
    }
  },

  createWorkItem: async (input) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ workItemError: null })

    try {
      const item = await invoke<WorkItemRecord>('create_work_item', {
        input: {
          projectId: selectedProject.id,
          title: input.title,
          body: input.body,
          itemType: input.itemType,
          status: input.status,
          parentWorkItemId: input.parentWorkItemId,
        },
      })

      set((s) => ({ workItems: sortWorkItems([item, ...s.workItems]) }))
      get().adjustProjectWorkItemCount(selectedProject.id, 1)
    } catch (error) {
      set({ workItemError: getErrorMessage(error, 'Failed to create work item.') })
      throw error
    }
  },

  updateWorkItem: async (input) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ workItemError: null })

    try {
      const item = await invoke<WorkItemRecord>('update_work_item', {
        input: {
          projectId: selectedProject.id,
          id: input.id,
          title: input.title,
          body: input.body,
          itemType: input.itemType,
          status: input.status,
        },
      })

      set((s) => ({
        workItems: sortWorkItems(s.workItems.map((w) => (w.id === item.id ? item : w))),
      }))
      await get().refreshSelectedProjectData(['worktrees'])
    } catch (error) {
      set({ workItemError: getErrorMessage(error, 'Failed to update work item.') })
      throw error
    }
  },

  deleteWorkItem: async (id) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ workItemError: null })

    try {
      await invoke('delete_work_item', { input: { projectId: selectedProject.id, id } })

      set((s) => ({
        workItems: s.workItems.filter((w) => w.id !== id),
        documents: s.documents.map((d) => (d.workItemId === id ? { ...d, workItemId: null } : d)),
      }))
      get().adjustProjectWorkItemCount(selectedProject.id, -1)
      await get().refreshSelectedProjectData(['worktrees'])
    } catch (error) {
      set({ workItemError: getErrorMessage(error, 'Failed to delete work item.') })
      throw error
    }
  },

  createDocument: async (input) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ documentError: null })

    try {
      const document = await invoke<DocumentRecord>('create_document', {
        input: {
          projectId: selectedProject.id,
          workItemId: input.workItemId,
          title: input.title,
          body: input.body,
        },
      })

      set((s) => ({ documents: sortDocuments([document, ...s.documents]) }))
      get().adjustProjectDocumentCount(selectedProject.id, 1)
    } catch (error) {
      set({ documentError: getErrorMessage(error, 'Failed to create document.') })
      throw error
    }
  },

  updateDocument: async (input) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ documentError: null })

    try {
      const document = await invoke<DocumentRecord>('update_document', {
        input: {
          projectId: selectedProject.id,
          id: input.id,
          workItemId: input.workItemId,
          clearWorkItem: input.workItemId === null,
          title: input.title,
          body: input.body,
        },
      })

      set((s) => ({
        documents: sortDocuments(s.documents.map((d) => (d.id === document.id ? document : d))),
      }))
    } catch (error) {
      set({ documentError: getErrorMessage(error, 'Failed to update document.') })
      throw error
    }
  },

  deleteDocument: async (id) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ documentError: null })

    try {
      await invoke('delete_document', { input: { projectId: selectedProject.id, id } })
      set((s) => ({ documents: s.documents.filter((d) => d.id !== id) }))
      get().adjustProjectDocumentCount(selectedProject.id, -1)
    } catch (error) {
      set({ documentError: getErrorMessage(error, 'Failed to delete document.') })
      throw error
    }
  },

  startWorkItemInTerminal: async (workItemId) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null
    const workItem = state.workItems.find((item) => item.id === workItemId)

    if (!selectedProject || !workItem) {
      return
    }

    set({ startingWorkItemId: workItem.id, workItemError: null, sessionError: null })

    try {
      let targetWorkItem = workItem

      if (workItem.status !== 'in_progress' && workItem.status !== 'done') {
        try {
          const updatedWorkItem = await invoke<WorkItemRecord>('update_work_item', {
            input: {
              projectId: selectedProject.id,
              id: workItem.id,
              title: workItem.title,
              body: workItem.body,
              itemType: workItem.itemType,
              status: 'in_progress' as WorkItemStatus,
            },
          })

          targetWorkItem = updatedWorkItem
          set((s) => ({
            workItems: sortWorkItems(
              s.workItems.map((w) => (w.id === updatedWorkItem.id ? updatedWorkItem : w)),
            ),
          }))
        } catch (error) {
          set({ workItemError: getErrorMessage(error, 'Failed to update work item status.') })
          return
        }
      }

      const launch = await invoke<WorktreeLaunchOutput>('launch_worktree_agent', {
        input: {
          projectId: selectedProject.id,
          workItemId: targetWorkItem.id,
        },
      })
      const { worktree, session } = launch

      flushSync(() => {
        get().upsertTrackedWorktree(worktree)
        set({
          selectedTerminalWorktreeId: worktree.id,
          sessionSnapshot: session,
          terminalPromptDraft: null,
          activeView: 'terminal',
        })
      })
      await get().refreshWorktrees(selectedProject.id)
      await get().refreshLiveSessions(selectedProject.id)
      await get().refreshSessionHistory(selectedProject.id)
      flushSync(() => {
        set({
          selectedTerminalWorktreeId: worktree.id,
          sessionSnapshot: session as SessionSnapshot,
          activeView: 'terminal',
        })
      })
      set({
        agentPromptMessage: `Focused worktree ${worktree.shortBranchName} opened for ${targetWorkItem.callSign}. The SDK worker is live and waiting for dispatcher directives.`,
      })
    } catch (error) {
      set({ sessionError: getErrorMessage(error, 'Failed to hand work item off to the terminal.') })
    } finally {
      set({ startingWorkItemId: null })
    }
  },
})
