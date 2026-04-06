export type ProjectSummary = {
  id: string;
  name: string;
  slug: string;
  sort_order: number;
  default_account_id: string | null;
  project_type: string | null;
  model_defaults_json: string | null;
  wcp_namespace: string | null;
  root_count: number;
  live_session_count: number;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
};

export type ProjectRootSummary = {
  id: string;
  project_id: string;
  label: string;
  path: string;
  git_root_path: string | null;
  remote_url: string | null;
  root_kind: string;
  sort_order: number;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
};

export type ProjectDetail = {
  id: string;
  name: string;
  slug: string;
  sort_order: number;
  default_account_id: string | null;
  project_type: string | null;
  model_defaults_json: string | null;
  agent_safety_overrides_json: string | null;
  wcp_namespace: string | null;
  dispatch_item_callsign: string | null;
  settings_json: string | null;
  instructions_md: string | null;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
  roots: ProjectRootSummary[];
};

export type AgentTemplateSummary = {
  id: string;
  project_id: string;
  template_key: string;
  label: string;
  origin_mode: string;
  default_model: string | null;
  instructions_md: string | null;
  stop_rules_json: string | null;
  sort_order: number;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
};

export type AgentTemplateDetail = {
  summary: AgentTemplateSummary;
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
  default_safety_mode: string | null;
  default_launch_args_json: string | null;
  default_model: string | null;
  created_at: number;
  updated_at: number;
};

export type McpServerSummary = {
  id: string;
  name: string;
  server_type: "stdio" | "http";
  command: string | null;
  args_json: string | null;
  env_json: string | null;
  url: string | null;
  is_builtin: boolean;
  enabled: boolean;
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
  diagnostics_enabled: boolean;
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
  worktree_branch: string | null;
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
  dispatch_group: string | null;
  live: boolean;
};

export type SessionDetail = {
  runtime: SessionRuntimeView | null;
} & SessionSummary;

export type WorktreeSummary = {
  id: string;
  project_id: string;
  project_root_id: string;
  branch_name: string;
  head_commit: string | null;
  base_ref: string | null;
  path: string;
  status: string;
  created_by_session_id: string | null;
  last_used_at: number | null;
  sort_order: number;
  created_at: number;
  updated_at: number;
  closed_at: number | null;
  active_session_count: number;
  has_uncommitted_changes: boolean;
};

export type CloseWorktreeResult = {
  worktree_id: string;
  merge_queue_id: string | null;
  committed: boolean;
  merged: boolean;
  conflicts: string[];
  status: string;
};

export type ConflictWarning = {
  item_a: string;
  item_b: string;
  overlapping_files: string[];
};

export type PendingDispatch =
  | { mode: "single"; workItemId: string; projectId: string; originMode: string }
  | { mode: "multi"; workItemIds: string[]; projectId: string };

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

export type SessionResyncEvent = {
  session_id: string;
  reason: string;
  last_available_seq: number;
};

export type SessionStateChangedEvent = {
  session_id: string;
  runtime_state: string;
  status: string;
  activity_state: string;
  needs_input_reason: string | null;
  tab_status: string | null;
  attached_clients: number;
  started_at: number | null;
  last_output_at: number | null;
  last_attached_at: number | null;
  updated_at: number;
  live: boolean;
};

export type WorkItemSummary = {
  id: string;
  project_id: string;
  parent_id: string | null;
  root_work_item_id: string | null;
  callsign: string;
  child_sequence: number | null;
  title: string;
  description: string;
  acceptance_criteria: string | null;
  work_item_type: string;
  status: string;
  priority: string | null;
  created_by: string | null;
  created_at: number;
  updated_at: number;
  closed_at: number | null;
  child_count: number;
};

export type WorkItemDetail = WorkItemSummary;

export type PlanningAssignmentSummary = {
  id: string;
  work_item_id: string;
  cadence_type: string;
  cadence_key: string;
  created_by: string;
  created_at: number;
  updated_at: number;
  removed_at: number | null;
};

export type PlanningAssignmentDetail = PlanningAssignmentSummary;

export type WorkflowReconciliationProposalSummary = {
  id: string;
  source_session_id: string;
  work_item_id: string | null;
  target_entity_type: string;
  target_entity_id: string | null;
  proposal_type: string;
  proposed_change_payload: Record<string, unknown>;
  reason: string;
  confidence: number;
  status: string;
  created_at: number;
  updated_at: number;
  resolved_at: number | null;
};

export type WorkflowReconciliationProposalDetail = WorkflowReconciliationProposalSummary;

export type DocumentSummary = {
  id: string;
  project_id: string;
  work_item_id: string | null;
  session_id: string | null;
  doc_type: string;
  title: string;
  slug: string;
  status: string;
  content_markdown: string;
  created_at: number;
  updated_at: number;
  archived_at: number | null;
};

export type DocumentDetail = DocumentSummary;

export type MergeQueueDiffStat = {
  files_changed: number;
  insertions: number;
  deletions: number;
  raw: string;
};

export type MergeQueueEntry = {
  id: string;
  project_id: string;
  session_id: string;
  worktree_id: string;
  branch_name: string;
  base_ref: string;
  position: number;
  status: "pending" | "ready" | "merging" | "merged" | "conflict" | "parked";
  diff_stat: MergeQueueDiffStat | null;
  conflict_files: string[] | null;
  has_uncommitted_changes: boolean;
  queued_at: number;
  merged_at: number | null;
  session_title: string | null;
  work_item_callsign: string | null;
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
    }
  | {
      resource_type: "work_item_detail";
      work_item_id: string;
      project_id: string;
      resource_id: string;
    }
  | {
      resource_type: "document_detail";
      document_id: string;
      project_id: string;
      resource_id: string;
    }
  | {
      resource_type: "merge_queue";
      project_id: string;
      resource_id: string;
    };

export type WorkspacePayloadV1 = {
  version: 1;
  selected_project_id: string | null;
  left_panel: "sessions" | "workbench";
  open_resources: WorkspaceResource[];
  active_resource_id: string | null;
};

export type WorkspacePayloadV2 = {
  version: 2;
  navigation: { layer: string; projectId?: string; sessionId?: string };
  focus_project_ids: string[];
  planning_view_mode: string;
};

export type WorkspacePayloadV3 = {
  version: 3;
  main_navigation: { layer: string; projectId?: string; sessionId?: string; documentId?: string; workItemId?: string };
  focus_project_ids: string[];
  sidebar_collapsed: boolean;
};

export type WorkspacePayload = WorkspacePayloadV1 | WorkspacePayloadV2 | WorkspacePayloadV3;

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

export type DiagnosticsBundleResult = {
  bundle_path: string;
};

export type GitHealthStatus = {
  has_remote: boolean;
  is_clean: boolean;
  is_pushed: boolean | null;
  is_behind: boolean | null;
  last_sync_at: number | null;
};

export type VaultEntry = {
  id: string;
  scope: string;
  key: string;
  description: string | null;
  created_at: number;
  updated_at: number;
};

export type VaultLockStatus = {
  unlocked: boolean;
  unlocked_at: number | null;
  unlock_expires_at: number | null;
};

export type VaultAuditEntry = {
  id: string;
  action: string;
  actor: string | null;
  key: string;
  scope: string;
  timestamp: number;
};
