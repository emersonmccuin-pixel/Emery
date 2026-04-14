import type { StateCreator } from "zustand";
import { invoke } from "@/lib/tauri";
import type {
  ProjectWorkflowCatalog,
  ProjectWorkflowRunSnapshot,
  WorkflowLibrarySnapshot,
  WorkflowRunRecord,
} from "../types";
import type {
  AppStore,
  WorkflowCatalogSlice,
  WorkflowEntityType,
} from "./types";
import { getErrorMessage } from "./utils";

function workflowActionKey(
  projectId: number,
  entityType: WorkflowEntityType,
  slug: string,
  action: string,
) {
  return `${projectId}:${entityType}:${slug}:${action}`;
}

function workflowRunKey(
  projectId: number,
  workflowSlug: string,
  rootWorkItemId: number,
  rootWorktreeId?: number | null,
) {
  return `${projectId}:${workflowSlug}:${rootWorkItemId}:${rootWorktreeId ?? "auto"}`;
}

export const createWorkflowSlice: StateCreator<
  AppStore,
  [],
  [],
  WorkflowCatalogSlice
> = (set, get) => ({
  workflowLibrary: null,
  projectWorkflowCatalog: null,
  workflowRuns: null,
  workflowError: null,
  isLoadingWorkflowCatalog: false,
  isLoadingWorkflowRuns: false,
  activeWorkflowActionKey: null,
  activeWorkflowRunKey: null,

  refreshWorkflowLibrary: async () => {
    const library = await invoke<WorkflowLibrarySnapshot>("list_workflow_library");
    set({ workflowLibrary: library });
    return library;
  },

  refreshProjectWorkflowCatalog: async (projectId) => {
    const catalog = await invoke<ProjectWorkflowCatalog>(
      "list_project_workflow_catalog",
      { projectId },
    );
    set((state) => ({
      projectWorkflowCatalog:
        state.selectedProjectId === projectId ? catalog : state.projectWorkflowCatalog,
    }));
    return catalog;
  },

  refreshProjectWorkflowRuns: async (projectId) => {
    const workflowRuns = await invoke<ProjectWorkflowRunSnapshot>(
      "list_project_workflow_runs",
      { projectId },
    );
    set((state) => ({
      workflowRuns:
        state.selectedProjectId === projectId ? workflowRuns : state.workflowRuns,
    }));
    return workflowRuns;
  },

  loadWorkflowCatalog: async (projectId) => {
    set({ workflowError: null, isLoadingWorkflowCatalog: true });
    try {
      const [workflowLibrary, projectWorkflowCatalog] = await Promise.all([
        get().refreshWorkflowLibrary(),
        get().refreshProjectWorkflowCatalog(projectId),
      ]);

      set((state) => ({
        workflowLibrary,
        projectWorkflowCatalog:
          state.selectedProjectId === projectId
            ? projectWorkflowCatalog
            : state.projectWorkflowCatalog,
      }));
    } catch (error) {
      set({
        workflowError: getErrorMessage(
          error,
          "Failed to load workflow library.",
        ),
      });
    } finally {
      set({ isLoadingWorkflowCatalog: false });
    }
  },

  loadWorkflowRuns: async (projectId) => {
    set({ workflowError: null, isLoadingWorkflowRuns: true });
    try {
      const workflowRuns = await get().refreshProjectWorkflowRuns(projectId);
      set((state) => ({
        workflowRuns:
          state.selectedProjectId === projectId ? workflowRuns : state.workflowRuns,
      }));
    } catch (error) {
      set({
        workflowError: getErrorMessage(
          error,
          "Failed to load workflow runs.",
        ),
      });
    } finally {
      set({ isLoadingWorkflowRuns: false });
    }
  },

  adoptProjectCatalogEntry: async (projectId, entityType, slug, mode = "linked") => {
    const actionKey = workflowActionKey(projectId, entityType, slug, "adopt");
    set({ workflowError: null, activeWorkflowActionKey: actionKey });
    try {
      const [workflowLibrary, projectWorkflowCatalog] = await Promise.all([
        get().refreshWorkflowLibrary(),
        invoke<ProjectWorkflowCatalog>("adopt_project_catalog_entry", {
          input: { projectId, entityType, slug, mode },
        }),
      ]);

      set((state) => ({
        workflowLibrary,
        projectWorkflowCatalog:
          state.selectedProjectId === projectId
            ? projectWorkflowCatalog
            : state.projectWorkflowCatalog,
      }));
    } catch (error) {
      set({
        workflowError: getErrorMessage(
          error,
          `Failed to adopt ${entityType} '${slug}'.`,
        ),
      });
    } finally {
      set({ activeWorkflowActionKey: null });
    }
  },

  upgradeProjectCatalogAdoption: async (projectId, entityType, slug) => {
    const actionKey = workflowActionKey(projectId, entityType, slug, "upgrade");
    set({ workflowError: null, activeWorkflowActionKey: actionKey });
    try {
      const [workflowLibrary, projectWorkflowCatalog] = await Promise.all([
        get().refreshWorkflowLibrary(),
        invoke<ProjectWorkflowCatalog>("upgrade_project_catalog_adoption", {
          input: { projectId, entityType, slug },
        }),
      ]);

      set((state) => ({
        workflowLibrary,
        projectWorkflowCatalog:
          state.selectedProjectId === projectId
            ? projectWorkflowCatalog
            : state.projectWorkflowCatalog,
      }));
    } catch (error) {
      set({
        workflowError: getErrorMessage(
          error,
          `Failed to upgrade ${entityType} '${slug}'.`,
        ),
      });
    } finally {
      set({ activeWorkflowActionKey: null });
    }
  },

  detachProjectCatalogAdoption: async (projectId, entityType, slug) => {
    const actionKey = workflowActionKey(projectId, entityType, slug, "detach");
    set({ workflowError: null, activeWorkflowActionKey: actionKey });
    try {
      const projectWorkflowCatalog = await invoke<ProjectWorkflowCatalog>(
        "detach_project_catalog_adoption",
        {
          input: { projectId, entityType, slug },
        },
      );

      set((state) => ({
        projectWorkflowCatalog:
          state.selectedProjectId === projectId
            ? projectWorkflowCatalog
            : state.projectWorkflowCatalog,
      }));
    } catch (error) {
      set({
        workflowError: getErrorMessage(
          error,
          `Failed to detach ${entityType} '${slug}'.`,
        ),
      });
    } finally {
      set({ activeWorkflowActionKey: null });
    }
  },

  startWorkflowRun: async (
    projectId,
    workflowSlug,
    rootWorkItemId,
    rootWorktreeId,
  ) => {
    const runKey = workflowRunKey(
      projectId,
      workflowSlug,
      rootWorkItemId,
      rootWorktreeId,
    );
    set({ workflowError: null, activeWorkflowRunKey: runKey });

    try {
      const run = await invoke<WorkflowRunRecord>("start_workflow_run", {
        input: {
          projectId,
          workflowSlug,
          rootWorkItemId,
          rootWorktreeId,
        },
      });

      await Promise.all([
        get().refreshProjectWorkflowRuns(projectId),
        get().refreshWorktrees(projectId),
        get().refreshLiveSessions(projectId),
      ]);

      return run;
    } catch (error) {
      set({
        workflowError: getErrorMessage(
          error,
          `Failed to start workflow '${workflowSlug}'.`,
        ),
      });
      return null;
    } finally {
      set({ activeWorkflowRunKey: null });
    }
  },
});
