export type ProjectSummary = {
  id: string;
  name: string;
  slug: string;
  sort_order: number;
  default_account_id: string | null;
  root_count: number;
  live_session_count: number;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
};

export type AccountSummary = {
  id: string;
  agent_kind: string;
  label: string;
  binary_path: string | null;
  config_root: string | null;
  env_preset_ref: string | null;
  is_default: boolean;
  status: string;
  created_at: number;
  updated_at: number;
};

export type BootstrapCounts = {
  project_count: number;
  account_count: number;
  live_session_count: number;
  restorable_workspace_count: number;
  interrupted_session_count: number;
};

export type HealthSnapshot = {
  supervisor_started_at: number;
  uptime_ms: number;
  app_data_root: string;
  artifact_root_available: boolean;
  live_session_count: number;
  app_db: {
    available: boolean;
    schema_version: string | null;
  };
  knowledge_db: {
    available: boolean;
    schema_version: string | null;
  };
};

export type HelloResult = {
  protocol_version: string;
  supervisor_version: string;
  min_supported_client_version: string;
  capabilities: string[];
  app_data_root: string;
  ipc_endpoint: string;
};

export type SessionRuntimeView = {
  runtime_state: string;
  attached_clients: number;
  started_at: number | null;
  created_at: number;
  updated_at: number;
  artifact_root: string;
  raw_log_path: string;
  replay_cursor: number;
  replay_byte_count: number;
};

export type SessionSummary = {
  id: string;
  session_spec_id: string;
  project_id: string;
  project_root_id: string | null;
  worktree_id: string | null;
  work_item_id: string | null;
  account_id: string;
  agent_kind: string;
  origin_mode: string;
  current_mode: string;
  title: string | null;
  title_source: string;
  runtime_state: string;
  status: string;
  activity_state: string;
  needs_input_reason: string | null;
  pty_owner_key: string | null;
  cwd: string;
  started_at: number | null;
  ended_at: number | null;
  last_output_at: number | null;
  last_attached_at: number | null;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
  live: boolean;
};

export type SessionDetail = {
  runtime: SessionRuntimeView | null;
} & SessionSummary;

export type EncodedTerminalChunk = {
  sequence: number;
  timestamp: number;
  encoding: "base64";
  data: string;
};

export type ReplaySnapshot = {
  oldest_sequence: number | null;
  latest_sequence: number;
  truncated_before_sequence: number | null;
  chunks: EncodedTerminalChunk[];
};

export type SessionAttachResponse = {
  attachment_id: string;
  session: SessionDetail;
  terminal_cols: number;
  terminal_rows: number;
  replay: ReplaySnapshot;
  output_cursor: number;
};

export type SessionOutputEvent = {
  session_id: string;
  sequence: number;
  timestamp: number;
  encoding: "base64";
  data: string;
};

export type SessionStateChangedEvent = {
  session_id: string;
  runtime_state: string;
  status: string;
  activity_state: string;
  needs_input_reason: string | null;
  attached_clients: number;
  started_at: number | null;
  last_output_at: number | null;
  last_attached_at: number | null;
  updated_at: number;
  live: boolean;
};

export type WorkspaceResource =
  | {
      resource_type: "project_home";
      project_id: string;
      resource_id: string;
    }
  | {
      resource_type: "session_terminal";
      session_id: string;
      resource_id: string;
    };

export type WorkspacePayload = {
  version: 1;
  selected_project_id: string | null;
  left_panel: "projects" | "sessions";
  open_resources: WorkspaceResource[];
  active_resource_id: string | null;
};

export type WorkspaceStateRecord = {
  id: string;
  scope: string;
  payload: WorkspacePayload;
  saved_at: number;
};

export type ShellBootstrap = {
  hello: HelloResult;
  health: HealthSnapshot;
  bootstrap: BootstrapCounts;
  projects: ProjectSummary[];
  accounts: AccountSummary[];
  sessions: SessionSummary[];
  workspace: WorkspaceStateRecord | null;
};

export type ConnectionStatusEvent = {
  state: "connected" | "disconnected" | "reconnecting";
  detail?: string;
};
