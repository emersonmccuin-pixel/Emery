import { useSyncExternalStore } from "react";

export type SessionSnapshot = {
  runtime_state: string;
  status: string;
  activity_state: string;
  needs_input_reason: string | null;
  live: boolean;
  title: string | null;
  current_mode: string;
  agent_kind: string;
  cwd: string;
  attached_clients: number;
};

type Listener = () => void;

class SessionStore {
  private snapshots = new Map<string, SessionSnapshot>();
  private listeners = new Set<Listener>();
  private seqState = new Map<string, { lastSeenSeq: number }>();

  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private notify() {
    for (const listener of this.listeners) {
      listener();
    }
  }

  getSnapshot = (sessionId: string): SessionSnapshot | undefined => {
    return this.snapshots.get(sessionId);
  };

  seedSession(sessionId: string, snap: SessionSnapshot) {
    this.snapshots.set(sessionId, snap);
  }

  seedComplete() {
    this.notify();
  }

  // --- Output sequence tracking ---

  recordOutputSeq(sessionId: string, seq: number): { gap: boolean; isDuplicate: boolean } {
    const state = this.seqState.get(sessionId);
    const lastSeenSeq = state?.lastSeenSeq ?? 0;
    if (seq <= lastSeenSeq) {
      // Duplicate or replayed event (e.g. arrives after a resync already advanced the cursor)
      return { gap: false, isDuplicate: true };
    }
    const gap = lastSeenSeq > 0 && seq !== lastSeenSeq + 1;
    this.seqState.set(sessionId, { lastSeenSeq: seq });
    return { gap, isDuplicate: false };
  }

  setLastSeenSeq(sessionId: string, seq: number) {
    this.seqState.set(sessionId, { lastSeenSeq: seq });
  }

  clearSeqState(sessionId: string) {
    this.seqState.delete(sessionId);
  }

  updateSession(
    sessionId: string,
    fields: {
      runtime_state: string;
      status: string;
      activity_state: string;
      needs_input_reason: string | null;
      live: boolean;
      attached_clients: number;
    },
  ) {
    const existing = this.snapshots.get(sessionId);
    if (
      existing &&
      existing.runtime_state === fields.runtime_state &&
      existing.status === fields.status &&
      existing.activity_state === fields.activity_state &&
      existing.needs_input_reason === fields.needs_input_reason &&
      existing.live === fields.live &&
      existing.attached_clients === fields.attached_clients
    ) {
      return; // no visible change — skip re-render
    }

    if (existing) {
      this.snapshots.set(sessionId, { ...existing, ...fields });
    } else {
      // Unknown session — create a minimal snapshot
      this.snapshots.set(sessionId, {
        runtime_state: fields.runtime_state,
        status: fields.status,
        activity_state: fields.activity_state,
        needs_input_reason: fields.needs_input_reason,
        live: fields.live,
        attached_clients: fields.attached_clients,
        title: null,
        current_mode: "",
        agent_kind: "",
        cwd: "",
      });
    }
    this.notify();
  }
}

export const sessionStore = new SessionStore();

export function useSessionSnapshot(sessionId: string): SessionSnapshot | undefined {
  return useSyncExternalStore(
    sessionStore.subscribe,
    () => sessionStore.getSnapshot(sessionId),
  );
}

export type DisplayState =
  | "starting"
  | "actively_working"
  | "idle_live"
  | "waiting_for_input"
  | "stopping"
  | "ended"
  | "error";

export function deriveDisplayState(snap: SessionSnapshot): DisplayState {
  if (snap.runtime_state === "failed") return "error";
  if (!snap.live) return "ended";
  if (snap.runtime_state === "stopping") return "stopping";
  if (snap.runtime_state === "starting") return "starting";

  if (snap.runtime_state === "running") {
    switch (snap.activity_state) {
      case "working": return "actively_working";
      case "needs_input": return "waiting_for_input";
      case "idle":
      default: return "idle_live";
    }
  }

  return "ended";
}

export function useDisplayState(sessionId: string): DisplayState {
  const snap = useSessionSnapshot(sessionId);
  if (!snap) return "ended";
  return deriveDisplayState(snap);
}
