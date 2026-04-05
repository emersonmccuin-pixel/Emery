export type ClientDiagnosticEvent = {
  timestamp_unix_ms: number;
  subsystem: string;
  event: string;
  correlation_id?: string;
  request_id?: string;
  session_id?: string;
  project_id?: string;
  work_item_id?: string;
  payload?: Record<string, unknown>;
};

const MAX_EVENTS = 500;
const events: ClientDiagnosticEvent[] = [];
let enabled = false;

export function configureDiagnostics(nextEnabled: boolean) {
  enabled = nextEnabled;
}

export function diagnosticsEnabled() {
  return enabled;
}

export function newCorrelationId(prefix = "ui") {
  return `${prefix}-${Date.now().toString(36)}-${crypto.randomUUID().slice(0, 8)}`;
}

export function recordClientEvent(event: ClientDiagnosticEvent) {
  if (!enabled) {
    return;
  }

  events.push(event);
  if (events.length > MAX_EVENTS) {
    events.splice(0, events.length - MAX_EVENTS);
  }
  console.debug("[emery:diag]", event);
}

export function snapshotClientDiagnostics() {
  return [...events];
}

export function makeClientEvent(
  subsystem: string,
  event: string,
  extras: Omit<ClientDiagnosticEvent, "timestamp_unix_ms" | "subsystem" | "event"> = {},
): ClientDiagnosticEvent {
  return {
    timestamp_unix_ms: Date.now(),
    subsystem,
    event,
    ...extras,
  };
}
