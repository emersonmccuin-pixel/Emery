import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionDetail, SessionStateChangedEvent, ShellBootstrap } from "./types";

const libMocks = vi.hoisted(() => ({
  archiveProject: vi.fn(),
  deleteProject: vi.fn(),
  bootstrapShell: vi.fn(),
  checkDispatchConflicts: vi.fn(),
  closeWorktree: vi.fn(),
  createDocument: vi.fn(),
  createProject: vi.fn(),
  createProjectRoot: vi.fn(),
  removeProjectRoot: vi.fn(),
  gitInitProjectRoot: vi.fn(),
  setProjectRootRemote: vi.fn(),
  createSession: vi.fn(),
  createSessionBatch: vi.fn(),
  createWorkItem: vi.fn(),
  getMergeQueueDiff: vi.fn(),
  getProject: vi.fn(),
  getSession: vi.fn(),
  getDocument: vi.fn(),
  getWorkItem: vi.fn(),
  interruptSession: vi.fn(),
  listDocuments: vi.fn(),
  listMergeQueue: vi.fn(),
  listWorkflowReconciliationProposals: vi.fn(),
  listAccounts: vi.fn(),
  listWorktrees: vi.fn(),
  createAccount: vi.fn(),
  updateAccount: vi.fn(),
  listWorkItems: vi.fn(),
  mergeQueueCheckConflicts: vi.fn(),
  mergeQueueMerge: vi.fn(),
  mergeQueuePark: vi.fn(),
  reorderWorktrees: vi.fn(),
  terminateSession: vi.fn(),
  updateDocument: vi.fn(),
  updateProject: vi.fn(),
  updateWorkflowReconciliationProposal: vi.fn(),
  updateWorkItem: vi.fn(),
  watchLiveSessions: vi.fn(),
  unwatchLiveSessions: vi.fn(),
  ensureCommandCenterProject: vi.fn(),
  getProjectRootGitStatus: vi.fn(),
}));

const diagnosticsMocks = vi.hoisted(() => ({
  makeClientEvent: vi.fn(() => ({ type: "test" })),
  newCorrelationId: vi.fn((scope: string) => `${scope}-corr`),
  recordClientEvent: vi.fn(),
}));

const toastMocks = vi.hoisted(() => ({
  addToast: vi.fn(),
  removeToast: vi.fn(),
}));

const navMocks = vi.hoisted(() => ({
  goToAgent: vi.fn(),
  closeModal: vi.fn(),
}));

vi.mock("./lib", () => ({ ...libMocks }));
vi.mock("./diagnostics", () => ({ ...diagnosticsMocks }));
vi.mock("./toast-store", () => ({ toastStore: toastMocks, useToastStore: vi.fn(() => []) }));
vi.mock("./nav-store", () => ({ navStore: navMocks }));

function makeDetail(overrides: Partial<SessionDetail> = {}): SessionDetail {
  return {
    id: "ses_test",
    session_spec_id: "sspec_test",
    project_id: "proj_test",
    project_root_id: null,
    worktree_id: null,
    worktree_branch: null,
    work_item_id: null,
    account_id: "acct_test",
    agent_kind: "claude",
    origin_mode: "execution",
    current_mode: "execution",
    title: "Test Session",
    title_source: "manual",
    runtime_state: "running",
    status: "active",
    activity_state: "working",
    needs_input_reason: null,
    pty_owner_key: "pty_test",
    cwd: "C:\\repo",
    started_at: 100,
    ended_at: null,
    last_output_at: null,
    last_attached_at: null,
    created_at: 100,
    updated_at: 100,
    archived_at: null,
    dispatch_group: null,
    live: true,
    runtime: {
      runtime_state: "running",
      attached_clients: 1,
      started_at: 100,
      created_at: 100,
      updated_at: 100,
      artifact_root: "C:\\repo\\.artifacts",
      raw_log_path: "C:\\repo\\raw.log",
      replay_cursor: 0,
      replay_byte_count: 0,
    },
    ...overrides,
  };
}

function makeStateChangedEvent(
  overrides: Partial<SessionStateChangedEvent> = {},
): SessionStateChangedEvent {
  return {
    session_id: "ses_test",
    runtime_state: "running",
    status: "active",
    activity_state: "working",
    needs_input_reason: null,
    tab_status: null,
    attached_clients: 1,
    started_at: 100,
    last_output_at: null,
    last_attached_at: null,
    updated_at: 101,
    live: true,
    ...overrides,
  };
}

function makeBootstrap(overrides: Partial<ShellBootstrap> = {}): ShellBootstrap {
  return {
    hello: {
      protocol_version: "1",
      supervisor_version: "0.1.0",
      min_supported_client_version: "0.1.0",
      capabilities: [],
      app_data_root: "C:\\repo",
      ipc_endpoint: "ipc://test",
      diagnostics_enabled: false,
    },
    health: {
      supervisor_started_at: 1,
      uptime_ms: 1,
      app_data_root: "C:\\repo",
      artifact_root_available: true,
      live_session_count: 0,
      app_db: { available: true, schema_version: "1" },
      knowledge_db: { available: true, schema_version: "1" },
    },
    bootstrap: {
      project_count: 1,
      account_count: 1,
      live_session_count: 0,
      restorable_workspace_count: 0,
      interrupted_session_count: 0,
    },
    projects: [],
    accounts: [],
    sessions: [],
    workspace: null,
    ...overrides,
  };
}

async function loadFreshStoreModule() {
  const storeModule = await import("./store");
  const sessionStoreModule = await import("./session-store");
  return {
    appStore: storeModule.appStore,
    sessionStore: sessionStoreModule.sessionStore,
  };
}

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  localStorage.clear();
  vi.useFakeTimers();
});

afterEach(() => {
  localStorage.clear();
  vi.useRealTimers();
});

describe("AppStore session integration", () => {
  it("ensureSessionSnapshot fetches and seeds a missing session snapshot", async () => {
    const detail = makeDetail({ id: "ses_missing", title: "Recovered Session" });
    libMocks.getSession.mockResolvedValue(detail);

    const { appStore, sessionStore } = await loadFreshStoreModule();

    expect(sessionStore.getSnapshot("ses_missing")).toBeUndefined();

    await appStore.ensureSessionSnapshot("ses_missing");

    expect(libMocks.getSession).toHaveBeenCalledWith("ses_missing", "session-reconcile-corr");
    expect(sessionStore.getSnapshot("ses_missing")).toMatchObject({
      runtime_state: "running",
      title: "Recovered Session",
      current_mode: "execution",
    });
    expect(appStore.getState().sessions[0]).toMatchObject({
      id: "ses_missing",
      title: "Recovered Session",
    });
  });

  it("ensureSessionSnapshot skips fetching when a snapshot already exists", async () => {
    const detail = makeDetail({ id: "ses_existing" });
    const { appStore } = await loadFreshStoreModule();

    appStore.applySessionDetail(detail);
    await appStore.ensureSessionSnapshot("ses_existing");

    expect(libMocks.getSession).not.toHaveBeenCalled();
  });

  it("applySessionStateChange updates session snapshots and unwatches ended sessions", async () => {
    libMocks.unwatchLiveSessions.mockResolvedValue(undefined);
    const { appStore, sessionStore } = await loadFreshStoreModule();

    appStore.applySessionDetail(makeDetail());
    appStore.applySessionStateChange(
      makeStateChangedEvent({
        runtime_state: "exited",
        status: "completed",
        activity_state: "idle",
        live: false,
        updated_at: 200,
      }),
    );

    expect(appStore.getState().sessions[0]).toMatchObject({
      runtime_state: "exited",
      status: "completed",
      live: false,
      updated_at: 200,
    });
    expect(sessionStore.getSnapshot("ses_test")).toMatchObject({
      runtime_state: "exited",
      status: "completed",
      live: false,
    });
    expect(libMocks.unwatchLiveSessions).toHaveBeenCalledWith(
      ["ses_test"],
      "session-unwatch-corr",
    );
    expect(toastMocks.addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "success",
        message: "Test Session completed",
      }),
    );
  });

  it("rebootstrap reseeds sessions, rewatches live sessions, and refreshes selected project reads", async () => {
    const payload = makeBootstrap({
      sessions: [
        makeDetail({
          id: "ses_live",
          title: "Live Session",
          live: true,
          runtime_state: "running",
          status: "active",
          activity_state: "working",
          runtime: null,
        }),
        makeDetail({
          id: "ses_done",
          title: "Done Session",
          live: false,
          runtime_state: "exited",
          status: "completed",
          activity_state: "idle",
          runtime: null,
        }),
      ],
    });
    libMocks.bootstrapShell.mockResolvedValue(payload);
    libMocks.watchLiveSessions.mockResolvedValue(undefined);
    libMocks.getProject.mockResolvedValue({
      id: "proj_test",
      name: "Project",
      slug: "project",
      sort_order: 0,
      default_account_id: null,
      project_type: "scratch",
      model_defaults_json: null,
      agent_safety_overrides_json: null,
      wcp_namespace: null,
      dispatch_item_callsign: null,
      settings_json: null,
      instructions_md: null,
      created_at: 1,
      updated_at: 1,
      archived_at: null,
      roots: [],
    });
    libMocks.listWorkItems.mockResolvedValue([]);
    libMocks.listDocuments.mockResolvedValue([]);

    const { appStore, sessionStore } = await loadFreshStoreModule();
    appStore.setSelectedProjectId("proj_test");

    await appStore.rebootstrap();

    expect(libMocks.bootstrapShell).toHaveBeenCalledWith("rebootstrap-corr");
    expect(libMocks.watchLiveSessions).toHaveBeenCalledWith(["ses_live"], "rebootstrap-corr");
    expect(appStore.getState().connectionState).toBe("connected");
    expect(appStore.getState().sessions).toHaveLength(2);
    expect(sessionStore.getSnapshot("ses_live")).toMatchObject({
      runtime_state: "running",
      title: "Live Session",
      live: true,
    });
    expect(libMocks.getProject).toHaveBeenCalledWith("proj_test", "project-load-corr");
  });

  it("unknown session refresh is debounced and triggers rebootstrap once", async () => {
    const payload = makeBootstrap({
      sessions: [makeDetail({ id: "ses_unknown", title: "Unknown Session", runtime: null })],
    });
    libMocks.bootstrapShell.mockResolvedValue(payload);
    libMocks.watchLiveSessions.mockResolvedValue(undefined);
    libMocks.listWorktrees.mockResolvedValue([]);

    const { appStore } = await loadFreshStoreModule();
    appStore.setSelectedProjectId("proj_test");
    appStore.applySessionStateChange(
      makeStateChangedEvent({
        session_id: "ses_unknown",
      }),
    );
    appStore.applySessionStateChange(
      makeStateChangedEvent({
        session_id: "ses_unknown",
      }),
    );

    expect(libMocks.bootstrapShell).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(500);
    await Promise.resolve();

    expect(libMocks.bootstrapShell).toHaveBeenCalledTimes(1);
    expect(libMocks.listWorktrees).toHaveBeenCalledTimes(1);
    expect(appStore.getState().sessions[0]).toMatchObject({
      id: "ses_unknown",
      title: "Unknown Session",
    });
  });
});
