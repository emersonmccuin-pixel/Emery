import { useSyncExternalStore } from "react";

export type NavigationLayer =
  | { layer: "home" }
  | { layer: "inbox"; projectId: string }
  | { layer: "project"; projectId: string }
  | { layer: "project-settings"; projectId: string }
  | { layer: "agent"; projectId: string; sessionId: string }
  | { layer: "document"; projectId: string; documentId: string }
  | { layer: "new-document"; projectId: string; workItemId?: string }
  | { layer: "work_item"; projectId: string; workItemId: string };

type NavState = {
  current: NavigationLayer;
  history: NavigationLayer[];
};

type Listener = () => void;

const listeners = new Set<Listener>();
let state: NavState = {
  current: { layer: "home" },
  history: [],
};

function emit() {
  for (const listener of listeners) listener();
}

function getState(): NavState {
  return state;
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export const navStore = {
  getState,
  subscribe,

  goHome() {
    if (state.current.layer === "home") return;
    state = { current: { layer: "home" }, history: [...state.history, state.current] };
    emit();
  },

  goToInbox(projectId: string) {
    state = {
      current: { layer: "inbox", projectId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goToProject(projectId: string) {
    state = {
      current: { layer: "project", projectId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goToProjectSettings(projectId: string) {
    state = {
      current: { layer: "project-settings", projectId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goToAgent(projectId: string, sessionId: string) {
    state = {
      current: { layer: "agent", projectId, sessionId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goToDocument(projectId: string, documentId: string) {
    state = {
      current: { layer: "document", projectId, documentId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goToNewDocument(projectId: string, workItemId?: string) {
    state = {
      current: { layer: "new-document", projectId, workItemId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goToWorkItem(projectId: string, workItemId: string) {
    state = {
      current: { layer: "work_item", projectId, workItemId },
      history: [...state.history, state.current],
    };
    emit();
  },

  goBack() {
    if (state.history.length === 0) return;
    const prev = state.history[state.history.length - 1];
    state = {
      current: prev,
      history: state.history.slice(0, -1),
    };
    emit();
  },

  /** Restore navigation from workspace persistence */
  restore(layer: NavigationLayer) {
    state = { current: layer, history: [] };
    emit();
  },

  breadcrumbs(): Array<{ label: string; layer: NavigationLayer }> {
    const crumbs: Array<{ label: string; layer: NavigationLayer }> = [
      { label: "EURI", layer: { layer: "home" } },
    ];
    const c = state.current;
    if (c.layer === "inbox") {
      crumbs.push({ label: "inbox", layer: c });
    }
    if (c.layer === "project" || c.layer === "project-settings" || c.layer === "agent" || c.layer === "document" || c.layer === "new-document" || c.layer === "work_item") {
      crumbs.push({ label: c.projectId, layer: { layer: "project", projectId: c.projectId } });
    }
    if (c.layer === "project-settings") {
      crumbs.push({ label: "settings", layer: c });
    }
    if (c.layer === "agent") {
      crumbs.push({ label: c.sessionId, layer: c });
    }
    if (c.layer === "document") {
      crumbs.push({ label: c.documentId, layer: c });
    }
    if (c.layer === "new-document") {
      crumbs.push({ label: "new document", layer: c });
    }
    if (c.layer === "work_item") {
      crumbs.push({ label: c.workItemId, layer: c });
    }
    return crumbs;
  },
};

export function useNavStore<T>(selector: (s: NavState) => T): T {
  return useSyncExternalStore(subscribe, () => selector(getState()));
}

export function useNavLayer(): NavigationLayer {
  return useNavStore((s) => s.current);
}
