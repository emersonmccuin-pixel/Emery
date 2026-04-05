import { useSyncExternalStore } from "react";

// ── Main channel (the primary content area) ────────────────────────────────

export type MainLayer =
  | { layer: "home" }
  | { layer: "project"; projectId: string }
  | { layer: "agent"; projectId: string; sessionId: string }
  | { layer: "document"; projectId: string; documentId: string }
  | { layer: "new-document"; projectId: string; workItemId?: string }
  | { layer: "work_item"; projectId: string; workItemId: string }
  | { layer: "project-settings"; projectId: string }
  | { layer: "settings" };

/** Backwards-compatible alias — same shape as before. */
export type NavigationLayer = MainLayer;

// ── Modal channel (centered overlay) ───────────────────────────────────────

export type ModalLayer =
  | null
  | { modal: "dispatch_single"; projectId: string; workItemId: string; originMode: string }
  | { modal: "dispatch_multi"; projectId: string; workItemIds: string[] }
  | { modal: "create_work_item"; projectId: string; parentId?: string }
  | { modal: "create_project" }
  | { modal: "work_item_detail"; projectId: string; workItemId: string }
  | { modal: "confirm"; title: string; message: string; onConfirm: () => void };

// ── State ──────────────────────────────────────────────────────────────────

type NavState = {
  main: MainLayer;
  modal: ModalLayer;
  mainHistory: MainLayer[];
};

type Listener = () => void;

const listeners = new Set<Listener>();
let state: NavState = {
  main: { layer: "home" },
  modal: null,
  mainHistory: [],
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

/**
 * Push current main layer to history and set new main.
 */
function navigateMain(next: MainLayer) {
  state = {
    main: next,
    modal: state.modal,
    mainHistory: [...state.mainHistory, state.main],
  };
  emit();
}

export const navStore = {
  getState,
  subscribe,

  // ── Main navigation ──────────────────────────────────────────────────────

  goHome() {
    if (state.main.layer === "home") return;
    navigateMain({ layer: "home" });
  },

  goToProject(projectId: string) {
    navigateMain({ layer: "project", projectId });
  },

  goToProjectSettings(projectId: string) {
    navigateMain({ layer: "project-settings", projectId });
  },

  goToAgent(projectId: string, sessionId: string) {
    navigateMain({ layer: "agent", projectId, sessionId });
  },

  goToDocument(projectId: string, documentId: string) {
    navigateMain({ layer: "document", projectId, documentId });
  },

  goToNewDocument(projectId: string, workItemId?: string) {
    navigateMain({ layer: "new-document", projectId, workItemId });
  },

  goToWorkItem(projectId: string, workItemId: string) {
    navigateMain({ layer: "work_item", projectId, workItemId });
  },

  goToSettings() {
    navigateMain({ layer: "settings" });
  },

  goBack() {
    if (state.mainHistory.length === 0) return;
    const prev = state.mainHistory[state.mainHistory.length - 1];
    state = {
      main: prev,
      modal: state.modal,
      mainHistory: state.mainHistory.slice(0, -1),
    };
    emit();
  },

  // ── Modal channel ────────────────────────────────────────────────────────

  openModal(modal: NonNullable<ModalLayer>) {
    state = { ...state, modal };
    emit();
  },

  closeModal() {
    if (state.modal === null) return;
    state = { ...state, modal: null };
    emit();
  },

  // ── Restore (workspace persistence) ──────────────────────────────────────

  /** Restore navigation from workspace persistence. Modal always starts closed. */
  restore(layer: MainLayer) {
    state = { main: layer, modal: null, mainHistory: [] };
    emit();
  },

  // ── Breadcrumbs (reads from main channel) ────────────────────────────────

  breadcrumbs(): Array<{ label: string; layer: MainLayer }> {
    const crumbs: Array<{ label: string; layer: MainLayer }> = [
      { label: "EURI", layer: { layer: "home" } },
    ];
    const c = state.main;
    if (c.layer === "settings") {
      crumbs.push({ label: "settings", layer: c });
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

// ── Hooks ──────────────────────────────────────────────────────────────────

export function useNavStore<T>(selector: (s: NavState) => T): T {
  return useSyncExternalStore(subscribe, () => selector(getState()));
}

/** Returns the current main layer (backwards-compatible with old useNavLayer). */
export function useNavLayer(): NavigationLayer {
  return useNavStore((s) => s.main);
}

/** Returns the current modal layer (null when closed). */
export function useModalLayer(): ModalLayer {
  return useNavStore((s) => s.modal);
}
