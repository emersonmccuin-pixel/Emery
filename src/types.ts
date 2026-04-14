export type RuntimeStatus = "loading" | "ready" | "error";

export type StorageInfo = {
  appDataDir: string;
  dbDir: string;
  dbPath: string;
};

export type DiagnosticsLogTail = {
  path: string;
  exists: boolean;
  tail: string;
  truncated: boolean;
  readError?: string | null;
};

export type DiagnosticsRuntimeContext = {
  appRunId: string;
  appStartedAt: string;
  appRuntimeStatePath: string;
  lastUnexpectedShutdown?: AppUnexpectedShutdown | null;
};

export type DiagnosticsStreamPayload = {
  at: string;
  appRunId: string;
  event: string;
  source: "app" | "supervisor-log";
  severity: "info" | "warn" | "error";
  summary: string;
  path?: string | null;
  line?: string | null;
};

export type AppUnexpectedShutdown = {
  appRunId: string;
  appStartedAt: string;
  appEndedAt?: string | null;
  processId: number;
  statePath: string;
  detectedAt: string;
};

export type DiagnosticsSnapshot = {
  capturedAt: string;
  appRunId: string;
  appStartedAt: string;
  appRuntimeStatePath: string;
  lastUnexpectedShutdown?: AppUnexpectedShutdown | null;
  appDataDir: string;
  dbDir: string;
  dbPath: string;
  runtimeDir: string;
  worktreesDir: string;
  logsDir: string;
  sessionOutputDir: string;
  crashReportsDir: string;
  diagnosticsLogPath: string;
  previousDiagnosticsLogPath: string;
  supervisorLog: DiagnosticsLogTail;
  previousSupervisorLog: DiagnosticsLogTail;
};

export type DiagnosticsBundleExportResult = {
  path: string;
  appRunId: string;
  includedFiles: string[];
  truncatedFiles: string[];
};

export type AppSettings = {
  defaultLaunchProfileId: number | null;
  defaultWorkerLaunchProfileId: number | null;
  sdkClaudeConfigDir: string | null;
  autoRepairSafeCleanupOnStartup: boolean;
};

export type ProjectRecord = {
  id: number;
  name: string;
  rootPath: string;
  rootAvailable: boolean;
  createdAt: string;
  updatedAt: string;
  workItemCount: number;
  documentCount: number;
  sessionCount: number;
  workItemPrefix: string | null;
  systemPrompt: string;
  baseBranch: string | null;
};

export type LaunchProfileRecord = {
  id: number;
  label: string;
  provider: string;
  executable: string;
  args: string;
  envJson: string;
  createdAt: string;
  updatedAt: string;
};

export type BootstrapData = {
  storage: StorageInfo;
  settings: AppSettings;
  projects: ProjectRecord[];
  launchProfiles: LaunchProfileRecord[];
};

export type SessionSnapshot = {
  sessionId: number;
  projectId: number;
  worktreeId?: number | null;
  launchProfileId: number;
  profileLabel: string;
  rootPath: string;
  isRunning: boolean;
  startedAt: string;
  output: string;
  outputCursor?: number;
  exitCode?: number | null;
  exitSuccess?: boolean | null;
};

export type TerminalOutputEvent = {
  projectId: number;
  worktreeId?: number | null;
  data: string;
};

export type TerminalExitEvent = {
  projectId: number;
  worktreeId?: number | null;
  exitCode: number;
  success: boolean;
  error?: string | null;
};

export type WorkItemStatus =
  | "backlog"
  | "in_progress"
  | "blocked"
  | "parked"
  | "done";

export type WorkItemType = "bug" | "task" | "feature" | "note";

export type WorkItemRecord = {
  id: number;
  projectId: number;
  parentWorkItemId: number | null;
  callSign: string;
  sequenceNumber: number;
  childNumber: number | null;
  title: string;
  body: string;
  itemType: WorkItemType;
  status: WorkItemStatus;
  createdAt: string;
  updatedAt: string;
};

export type DocumentRecord = {
  id: number;
  projectId: number;
  workItemId: number | null;
  title: string;
  body: string;
  createdAt: string;
  updatedAt: string;
};

export type WorktreeRecord = {
  id: number;
  projectId: number;
  workItemId: number;
  workItemCallSign: string;
  workItemTitle: string;
  workItemStatus: string;
  branchName: string;
  shortBranchName: string;
  worktreePath: string;
  pathAvailable: boolean;
  hasUncommittedChanges: boolean;
  hasUnmergedCommits: boolean;
  pinned: boolean;
  isCleanupEligible: boolean;
  pendingSignalCount: number;
  agentName: string;
  sessionSummary: string;
  createdAt: string;
  updatedAt: string;
};

export type AgentSignalRecord = {
  id: number;
  projectId: number;
  worktreeId?: number | null;
  workItemId?: number | null;
  sessionId?: number | null;
  signalType: string;
  message: string;
  contextJson: string;
  status: string;
  response?: string | null;
  respondedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type SessionRecord = {
  id: number;
  projectId: number;
  launchProfileId?: number | null;
  worktreeId?: number | null;
  processId?: number | null;
  supervisorPid?: number | null;
  provider: string;
  providerSessionId?: string | null;
  profileLabel: string;
  rootPath: string;
  state: string;
  startupPrompt: string;
  startedAt: string;
  endedAt?: string | null;
  exitCode?: number | null;
  exitSuccess?: boolean | null;
  createdAt: string;
  updatedAt: string;
  lastHeartbeatAt?: string | null;
};

export type CrashRecoveryManifest = {
  wasCrash: boolean;
  interruptedSessions: SessionRecord[];
  orphanedSessions: SessionRecord[];
  affectedWorktrees: WorktreeRecord[];
  affectedWorkItems: WorkItemRecord[];
};

export type SessionCrashReport = {
  sessionId: number;
  projectId: number;
  worktreeId?: number | null;
  launchProfileId?: number | null;
  profileLabel: string;
  rootPath: string;
  startedAt: string;
  endedAt?: string | null;
  exitCode?: number | null;
  exitSuccess?: boolean | null;
  error?: string | null;
  headline?: string | null;
  lastActivity?: string | null;
  startupPrompt?: string | null;
  lastOutput?: string | null;
  outputLogPath?: string | null;
  crashReportPath?: string | null;
  bunReportUrl?: string | null;
};

export type SessionRecoveryDetails = {
  session: SessionRecord;
  crashReport?: SessionCrashReport | null;
};

export type SessionEventRecord = {
  id: number;
  projectId: number;
  sessionId?: number | null;
  eventType: string;
  entityType?: string | null;
  entityId?: number | null;
  source: string;
  payloadJson: string;
  createdAt: string;
};

export type SessionHistoryOutput = {
  sessions: SessionRecord[];
  events: SessionEventRecord[];
};

export type CleanupCandidate = {
  kind: string;
  path: string;
  projectId?: number | null;
  worktreeId?: number | null;
  sessionId?: number | null;
  reason: string;
};

export type CleanupActionOutput = {
  removed: boolean;
  candidate: CleanupCandidate;
};

export type CleanupRepairOutput = {
  actions: CleanupActionOutput[];
};

export type WorktreeLaunchOutput = {
  worktree: WorktreeRecord;
  session: SessionSnapshot;
};

export type CheckProjectFolderResult = {
  isGitRepo: boolean;
  gitBranch: string | null;
  hasClaudeMd: boolean;
};

export type WorkflowCategoryRecord = {
  id: number;
  name: string;
  description: string;
  isShipped: boolean;
  createdAt: string;
};

export type WorkflowStageRetryPolicyRecord = {
  maxAttempts: number;
  onFailFeedbackTo?: string | null;
};

export type WorkflowArtifactContractRecord = {
  artifactType: string;
  label: string;
  description: string;
  requiredFrontmatterFields: string[];
  requiredMarkdownSections: string[];
};

export type WorkflowProducedArtifactRecord = {
  type: string;
  title?: string | null;
  summary?: string | null;
  bodyMarkdown?: string | null;
  frontmatter: Record<string, unknown>;
};

export type VaultAccessBindingRequest = {
  envVar: string;
  entryName: string;
  scopeTags: string[];
  delivery: "env" | "file";
};

export type VaultIntegrationTemplateKind = "http_broker" | "cli" | "mcp";

export type VaultIntegrationSecretPlacement =
  | "authorization_bearer"
  | "header"
  | "env_var";

export type VaultIntegrationSecretSlotTemplate = {
  slotName: string;
  label: string;
  description: string;
  requiredScopeTags: string[];
  placement: VaultIntegrationSecretPlacement;
  envVar?: string | null;
  headerName?: string | null;
  headerPrefix?: string | null;
};

export type VaultIntegrationTemplateRecord = {
  slug: string;
  name: string;
  description: string;
  kind: VaultIntegrationTemplateKind;
  source: string;
  command?: string | null;
  defaultArgs: string[];
  defaultEnv: Record<string, string>;
  baseUrl?: string | null;
  egressDomains: string[];
  supportedMethods: string[];
  defaultHeaders: Record<string, string>;
  secretSlots: VaultIntegrationSecretSlotTemplate[];
};

export type VaultIntegrationBindingRecord = {
  slotName: string;
  entryName: string;
};

export type VaultIntegrationInstallationRecord = {
  id: number;
  templateSlug: string;
  label: string;
  enabled: boolean;
  bindings: VaultIntegrationBindingRecord[];
  createdAt: string;
  updatedAt: string;
  ready: boolean;
  missingBindings: string[];
  template?: VaultIntegrationTemplateRecord | null;
};

export type VaultIntegrationSnapshot = {
  templates: VaultIntegrationTemplateRecord[];
  installations: VaultIntegrationInstallationRecord[];
};

export type WorkflowStageRecord = {
  name: string;
  role: string;
  podRef?: string | null;
  provider?: string | null;
  model?: string | null;
  promptTemplateRef?: string | null;
  inputs: string[];
  outputs: string[];
  inputContracts: WorkflowArtifactContractRecord[];
  outputContracts: WorkflowArtifactContractRecord[];
  needsSecrets: string[];
  vaultEnvBindings: VaultAccessBindingRequest[];
  retryPolicy?: WorkflowStageRetryPolicyRecord | null;
  retrySummary?: string | null;
};

export type WorkflowRecord = {
  id: number;
  slug: string;
  name: string;
  kind: string;
  version: number;
  description: string;
  source: string;
  template: boolean;
  categories: string[];
  tags: string[];
  stages: WorkflowStageRecord[];
  podRefs: string[];
  yaml: string;
  filePath: string;
  updatedAt: string;
};

export type PodRecord = {
  id: number;
  slug: string;
  name: string;
  role: string;
  version: number;
  description: string;
  provider: string;
  model?: string | null;
  promptTemplateRef?: string | null;
  categories: string[];
  tags: string[];
  toolAllowlist: string[];
  secretScopes: string[];
  defaultPolicyJson: string;
  yaml: string;
  source: string;
  filePath: string;
  updatedAt: string;
};

export type WorkflowLibrarySnapshot = {
  libraryRoot: string;
  workflowDir: string;
  podDir: string;
  categories: WorkflowCategoryRecord[];
  workflows: WorkflowRecord[];
  pods: PodRecord[];
};

export type AdoptionRecord = {
  slug: string;
  pinnedVersion: number;
  latestVersion?: number | null;
  mode: string;
  isOutdated: boolean;
  updatedAt: string;
};

export type ProjectWorkflowRecord = WorkflowRecord & {
  adoption: AdoptionRecord;
};

export type ProjectPodRecord = PodRecord & {
  adoption: AdoptionRecord;
};

export type ProjectWorkflowCatalog = {
  projectId: number;
  workflows: ProjectWorkflowRecord[];
  pods: ProjectPodRecord[];
};

export type ProjectWorkflowOverrideDocument = {
  projectId: number;
  workflowSlug: string;
  filePath: string;
  exists: boolean;
  source: string;
  yaml: string;
  hasOverrides: boolean;
  stageOverrideCount: number;
  validationError?: string | null;
};

export type ResolvedWorkflowStageRecord = {
  ordinal: number;
  name: string;
  role: string;
  podSlug?: string | null;
  podVersion?: number | null;
  provider: string;
  model?: string | null;
  promptTemplateRef?: string | null;
  toolAllowlist: string[];
  secretScopes: string[];
  defaultPolicyJson: string;
  inputs: string[];
  outputs: string[];
  inputContracts: WorkflowArtifactContractRecord[];
  outputContracts: WorkflowArtifactContractRecord[];
  needsSecrets: string[];
  vaultEnvBindings: VaultAccessBindingRequest[];
  retryPolicy?: WorkflowStageRetryPolicyRecord | null;
};

export type ResolvedWorkflowRecord = {
  slug: string;
  name: string;
  kind: string;
  version: number;
  description: string;
  source: string;
  template: boolean;
  categories: string[];
  tags: string[];
  adoptionMode: string;
  hasOverrides: boolean;
  stages: ResolvedWorkflowStageRecord[];
};

export type WorkflowRunStageRecord = {
  id: number;
  runId: number;
  stageOrdinal: number;
  stageName: string;
  stageRole: string;
  podSlug?: string | null;
  podVersion?: number | null;
  provider: string;
  model?: string | null;
  worktreeId?: number | null;
  sessionId?: number | null;
  agentName?: string | null;
  threadId?: string | null;
  directiveMessageId?: number | null;
  responseMessageId?: number | null;
  status: string;
  attempt: number;
  completionMessageType?: string | null;
  completionSummary?: string | null;
  completionContextJson: string;
  producedArtifacts: WorkflowProducedArtifactRecord[];
  artifactValidationStatus?: string | null;
  artifactValidationError?: string | null;
  retrySourceStageName?: string | null;
  retryFeedbackSummary?: string | null;
  retryFeedbackContextJson: string;
  retryRequestedAt?: string | null;
  failureReason?: string | null;
  createdAt: string;
  startedAt?: string | null;
  completedAt?: string | null;
  updatedAt: string;
  resolvedStage: ResolvedWorkflowStageRecord;
};

export type WorkflowRunRecord = {
  id: number;
  projectId: number;
  workflowSlug: string;
  workflowName: string;
  workflowKind: string;
  workflowVersion: number;
  rootWorkItemId: number;
  rootWorkItemCallSign: string;
  rootWorktreeId?: number | null;
  sourceAdoptionMode: string;
  status: string;
  hasOverrides: boolean;
  failureReason?: string | null;
  createdAt: string;
  startedAt: string;
  completedAt?: string | null;
  updatedAt: string;
  resolvedWorkflow: ResolvedWorkflowRecord;
  stages: WorkflowRunStageRecord[];
};

export type ProjectWorkflowRunSnapshot = {
  projectId: number;
  runs: WorkflowRunRecord[];
};

export type VaultEntryRecord = {
  id: number;
  name: string;
  kind: string;
  description: string;
  scopeTags: string[];
  gatePolicy: string;
  createdAt: string;
  updatedAt: string;
  lastAccessedAt?: string | null;
};

export type VaultSnapshot = {
  vaultRoot: string;
  snapshotPath: string;
  entries: VaultEntryRecord[];
};
