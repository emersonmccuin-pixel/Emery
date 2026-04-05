import { useSyncExternalStore } from "react";

export type Toast = {
  id: string;
  message: string;
  type: "success" | "error" | "info";
  action?: { label: string; onClick: () => void };
  timeout?: number; // ms, default 5000
};

type ToastState = Toast[];

type Listener = () => void;

const listeners = new Set<Listener>();
let state: ToastState = [];

function emit() {
  for (const listener of listeners) listener();
}

function getState(): ToastState {
  return state;
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

let idCounter = 0;

export const toastStore = {
  getState,
  subscribe,

  addToast(toast: Omit<Toast, "id"> & { id?: string }): string {
    const id = toast.id ?? `toast-${++idCounter}-${Date.now()}`;
    const full: Toast = { timeout: 5000, ...toast, id };
    state = [...state, full];
    emit();
    window.setTimeout(() => {
      toastStore.removeToast(id);
    }, full.timeout ?? 5000);
    return id;
  },

  removeToast(id: string) {
    state = state.filter((t) => t.id !== id);
    emit();
  },
};

export function useToastStore(): ToastState {
  return useSyncExternalStore(subscribe, getState);
}
