import { useSyncExternalStore } from "react";

// ── Main channel (the primary content area) ────────────────────────────────

export type MainLayer =
  | { layer: "home" }
  | { layer: "project"; projectId: string }
  | { layer: "agent"; projectId: string; sessionId: string }
  | { layer: "document"; projectId: string; documentId: string }
  | { layer: "new-document"; projectId: string; workItemId?: string }
  | { layer: "work_item"; projectId: string; workItemId: string }
  | { layer: "inbox"; projectId: string }
  | { layer: "project-settings"; projectId: string }
  | { layer: "settings" }
  | { layer: "vault" };

/** Backwards-compatible alias — same shape as before. */
export type NavigationLayer = MainLayer;

// ── Peek channel (right-side panel) ────────────────────────────────────────

export type PeekLayer =
  | null
  | { peek: "work_item"; projectId: string; workItemId: string }
  | { peek: "inbox"; projectId: string }
  | { peek: "document"; projectId: string; documentId: string }
  | { peek: "session_detail"; projectId: string; sessionId: string }
  | { peek: "merge_diff"; projectId: string; entryId: string }
  | { peek: "project_settings"; projectId: string };

// ── Modal channel (centered overlay) ───────────────────────────────────────

export type ModalLayer =
  | null
  | { modal: "dispatch_single"; projectId: string; workItemId: string; originMode: string }
  | { modal: "dispatch_multi"; projectId: string; workItemIds: string[] }
  | { modal: "create_work_item"; projectId: string; parentId?: string }
  | { modal: "create_project" }
  | { modal: "confirm"; title: string; message: string; onConfirm: () => void };

// ── State ──────────────────────────────────────────────────────────────────

type NavState = {
  main: MainLayer;
  peek: PeekLayer;
  modal: ModalLayer;
  mainHistory: MainLayer[];
};

type Listener = () => void;

const listeners = new Set<Listener>();
let state: NavState = {
  main: { layer: "home" },
  peek: null,
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
 * Push current main layer to history, set new main, and auto-close peek.
 */
function navigateMain(next: MainLayer) {
  state = {
    main: next,
    peek: null, // main nav change auto-closes peek
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

  goToInbox(projectId: string) {
    navigateMain({ layer: "inbox", projectId });
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

  goToVault() {
    navigateMain({ layer: "vault" });
  },

  goBack() {
    if (state.mainHistory.length === 0) return;
    const prev = state.mainHistory[state.mainHistory.length - 1];
    state = {
      main: prev,
      peek: null, // close peek on back navigation too
      modal: state.modal,
      mainHistory: state.mainHistory.slice(0, -1),
    };
    emit();
  },

  // ── Peek channel ─────────────────────────────────────────────────────────

  openPeek(peek: NonNullable<PeekLayer>) {
    state = { ...state, peek };
    emit();
  },

  closePeek() {
    if (state.peek === null) return;
    state = { ...state, peek: null };
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

  /** Restore navigation from workspace persistence. Peek & modal always start closed. */
  restore(layer: MainLayer) {
    state = { main: layer, peek: null, modal: null, mainHistory: [] };
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
    if (c.layer === "vault") {
      crumbs.push({ label: "vault", layer: c });
    }
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

// ── Hooks ──────────────────────────────────────────────────────────────────

export function useNavStore<T>(selector: (s: NavState) => T): T {
  return useSyncExternalStore(subscribe, () => selector(getState()));
}

/** Returns the current main layer (backwards-compatible with old useNavLayer). */
export function useNavLayer(): NavigationLayer {
  return useNavStore((s) => s.main);
}

/** Returns the current peek layer (null when closed). */
export function usePeekLayer(): PeekLayer {
  return useNavStore((s) => s.peek);
}

/** Returns the current modal layer (null when closed). */
export function useModalLayer(): ModalLayer {
  return useNavStore((s) => s.modal);
}
