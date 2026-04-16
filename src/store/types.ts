import type { FormEvent } from "react";
import type {
  AppSettings,
  CleanupCandidate,
  CrashRecoveryManifest,
  DocumentRecord,
  LaunchProfileRecord,
  ProjectWorkflowCatalog,
  ProjectWorkflowRunSnapshot,
  ProjectRecord,
  SessionEventRecord,
  SessionRecord,
  SessionRecoveryDetails,
  SessionSnapshot,
  StorageInfo,
  TerminalExitEvent,
  WorkflowRunRecord,
  WorkflowLibrarySnapshot,
  WorkItemRecord,
  WorkItemStatus,
  WorktreeRecord,
} from "../types";

export type WorkspaceView =
  | "overview"
  | "files"
  | "terminal"
  | "history"
  | "configuration"
  | "workflows"
  | "workItems"
  | "worktreeWorkItem";

export type TerminalPromptDraft = {
  label: string;
  prompt: string;
};

export type LiveProjectSession = {
  project: ProjectRecord;
  snapshot: SessionSnapshot;
};

export type WorktreeSessionEntry = {
  worktree: WorktreeRecord;
  snapshot: SessionSnapshot | null;
};

export type CreateWorkItemInput = {
  title: string;
  body: string;
  itemType: string;
  status: WorkItemStatus;
  parentWorkItemId: number | null;
};

export type UpdateWorkItemInput = {
  id: number;
  title: string;
  body: string;
  itemType: string;
  status: WorkItemStatus;
};

export type CreateDocumentInput = {
  title: string;
  body: string;
  workItemId: number | null;
};

export type UpdateDocumentInput = {
  id: number;
  title: string;
  body: string;
  workItemId: number | null;
};

export type LaunchSessionOptions = {
  startupPrompt?: string;
  resumeSessionId?: string | null;
  launchProfileId?: number | null;
  worktreeId?: number | null;
  worktree?: WorktreeRecord | null;
  attachSnapshot?: boolean;
  activateTerminal?: boolean;
};

export type ResumeSessionRecordOptions = {
  openTarget?: boolean;
  attachSnapshot?: boolean;
  activateTerminal?: boolean;
  successMessage?: string | null;
};

export type SessionAutoRestartStatus = "countdown" | "restarting" | "blocked";

export type SessionAutoRestartEntry = {
  projectId: number;
  worktreeId: number | null;
  status: SessionAutoRestartStatus;
  headline: string;
  restartAt: number | null;
  recentCrashCount: number;
  failedSessionId: number | null;
  replacementSessionId: number | null;
  blockedReason: string | null;
  exitCode: number | null;
};

export type ProjectSlice = {
  projects: ProjectRecord[];
  launchProfiles: LaunchProfileRecord[];
  appSettings: AppSettings;
  storageInfo: StorageInfo | null;
  selectedProjectId: number | null;
  selectedLaunchProfileId: number | null;

  projectName: string;
  projectRootPath: string;
  projectError: string | null;
  isProjectCreateOpen: boolean;
  isCreatingProject: boolean;

  editProjectName: string;
  editProjectRootPath: string;
  editProjectBaseBranch: string;
  projectUpdateError: string | null;
  isProjectEditorOpen: boolean;
  isUpdatingProject: boolean;

  profileLabel: string;
  profileProvider: string;
  profileExecutable: string;
  profileArgs: string;
  profileEnvJson: string;
  profileError: string | null;
  isProfileFormOpen: boolean;
  editingLaunchProfileId: number | null;
  isCreatingProfile: boolean;
  activeDeleteLaunchProfileId: number | null;

  settingsError: string | null;
  settingsMessage: string | null;
  defaultLaunchProfileSettingId: number | null;
  defaultWorkerLaunchProfileSettingId: number | null;
  sdkClaudeConfigDirSetting: string;
  autoRepairSafeCleanupOnStartup: boolean;
  isSavingAppSettings: boolean;

  setProjectName: (value: string) => void;
  setProjectRootPath: (value: string) => void;
  setProjectError: (value: string | null) => void;
  setProjectUpdateError: (value: string | null) => void;
  setSelectedLaunchProfileId: (value: number | null) => void;
  setEditProjectName: (value: string) => void;
  setEditProjectRootPath: (value: string) => void;
  setEditProjectBaseBranch: (value: string) => void;
  setIsProjectEditorOpen: (value: boolean) => void;
  setIsProjectCreateOpen: (value: boolean) => void;
  setDefaultLaunchProfileSettingId: (value: number | null) => void;
  setDefaultWorkerLaunchProfileSettingId: (value: number | null) => void;
  setSdkClaudeConfigDirSetting: (value: string) => void;
  setAutoRepairSafeCleanupOnStartup: (value: boolean) => void;
  setSettingsError: (value: string | null) => void;
  setProfileLabel: (value: string) => void;
  setProfileProvider: (value: string) => void;
  setProfileExecutable: (value: string) => void;
  setProfileArgs: (value: string) => void;
  setProfileEnvJson: (value: string) => void;
  setIsProfileFormOpen: (value: boolean) => void;

  bootstrap: () => Promise<void>;
  selectProject: (projectId: number) => void;
  startCreateProject: () => void;
  cancelCreateProject: () => void;
  browseForProjectFolder: (
    applyPath: (value: string) => void,
    setError: (value: string | null) => void,
  ) => Promise<void>;
  submitProject: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  submitProjectUpdate: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  submitAppSettings: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  submitLaunchProfile: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  startCreateLaunchProfile: () => void;
  startEditLaunchProfile: (profile: LaunchProfileRecord) => void;
  cancelLaunchProfileEditor: () => void;
  deleteLaunchProfile: (profile: LaunchProfileRecord) => Promise<void>;
  projectCreated: (project: ProjectRecord) => void;
  adjustProjectWorkItemCount: (projectId: number, delta: number) => void;
  adjustProjectDocumentCount: (projectId: number, delta: number) => void;
};

export type SessionSlice = {
  sessionSnapshot: SessionSnapshot | null;
  liveSessionSnapshots: SessionSnapshot[];
  sessionError: string | null;
  isLaunchingSession: boolean;
  isStoppingSession: boolean;
  selectedTerminalWorktreeId: number | null;
  terminalPromptDraft: TerminalPromptDraft | null;
  agentPromptMessage: string | null;
  terminatedSessions: Set<string>;
  sessionAutoRestart: Record<string, SessionAutoRestartEntry>;

  setTerminalPromptDraft: (value: TerminalPromptDraft | null) => void;

  fetchSessionSnapshot: (
    projectId: number,
    worktreeId?: number | null,
  ) => Promise<SessionSnapshot | null>;
  refreshLiveSessions: (projectId: number) => Promise<SessionSnapshot[]>;
  refreshSelectedSessionSnapshot: () => Promise<SessionSnapshot | null>;
  launchSession: (
    options?: LaunchSessionOptions,
  ) => Promise<SessionSnapshot | null>;
  stopSession: () => Promise<void>;
  resumeSessionRecord: (
    record: SessionRecord,
    options?: ResumeSessionRecordOptions,
  ) => Promise<SessionSnapshot | null>;
  handleSessionExit: (event: TerminalExitEvent) => void;
  cancelSessionAutoRestart: (
    projectId: number,
    worktreeId: number | null,
  ) => void;
  focusTerminalTarget: (
    worktreeId: number | null,
    preferredView?: WorkspaceView,
  ) => void;
  restartSessionTargetNow: (
    projectId: number,
    worktreeId: number | null,
  ) => Promise<void>;
  selectMainTerminal: () => void;
  selectWorktreeTerminal: (worktreeId: number) => void;
  sendAgentStartupPrompt: () => Promise<void>;
  copyAgentStartupPrompt: () => Promise<void>;
  copyTerminalOutput: () => Promise<void>;
  launchWorkspaceGuide: () => Promise<void>;
  sendPromptToSession: (
    projectId: number,
    worktreeId: number | null,
    prompt: string,
    successMessage: string,
  ) => Promise<void>;
};

export type WorkItemSlice = {
  workItems: WorkItemRecord[];
  workItemError: string | null;
  isLoadingWorkItems: boolean;
  startingWorkItemId: number | null;
  documents: DocumentRecord[];
  documentError: string | null;
  isLoadingDocuments: boolean;
  isDocumentsManagerOpen: boolean;

  setIsDocumentsManagerOpen: (value: boolean) => void;

  refreshWorkItems: (projectId: number) => Promise<WorkItemRecord[]>;
  refreshDocuments: (projectId: number) => Promise<DocumentRecord[]>;
  loadWorkItems: (projectId: number) => Promise<void>;
  loadDocuments: (projectId: number) => Promise<void>;
  createWorkItem: (input: CreateWorkItemInput) => Promise<void>;
  updateWorkItem: (input: UpdateWorkItemInput) => Promise<void>;
  deleteWorkItem: (id: number) => Promise<void>;
  createDocument: (input: CreateDocumentInput) => Promise<void>;
  updateDocument: (input: UpdateDocumentInput) => Promise<void>;
  deleteDocument: (id: number) => Promise<void>;
  startWorkItemInTerminal: (workItemId: number) => Promise<void>;
};

export type WorktreeSlice = {
  worktrees: WorktreeRecord[];
  stagedWorktrees: WorktreeRecord[];
  worktreeError: string | null;
  worktreeMessage: string | null;
  activeWorktreeActionId: number | null;
  activeWorktreeActionKind: "remove" | "recreate" | "cleanup" | "pin" | null;
  worktreeRequestId: number;
  isLoadingWorktrees: boolean;

  refreshWorktrees: (projectId: number) => Promise<WorktreeRecord[]>;
  upsertTrackedWorktree: (worktree: WorktreeRecord) => void;
  dropTrackedWorktree: (worktreeId: number) => void;
  removeWorktree: (worktree: WorktreeRecord) => Promise<void>;
  recreateWorktree: (worktree: WorktreeRecord) => Promise<void>;
  cleanupWorktree: (worktree: WorktreeRecord) => Promise<void>;
  pinWorktree: (worktree: WorktreeRecord, pinned: boolean) => Promise<void>;
  syncWorktreeLifecycleState: (projectId: number) => Promise<void>;
};

export type HistorySlice = {
  sessionRecords: SessionRecord[];
  sessionEvents: SessionEventRecord[];
  selectedHistorySessionId: number | null;
  historyError: string | null;
  isLoadingHistory: boolean;
  orphanedSessions: SessionRecord[];
  cleanupCandidates: CleanupCandidate[];
  activeOrphanSessionId: number | null;
  activeCleanupPath: string | null;
  isRepairingCleanup: boolean;

  setSelectedHistorySessionId: (value: number | null) => void;

  refreshSessionHistory: (projectId: number) => Promise<void>;
  refreshOrphanedSessions: (projectId: number) => Promise<SessionRecord[]>;
  refreshCleanupCandidates: () => Promise<CleanupCandidate[]>;
  loadSessionHistory: (projectId: number) => Promise<void>;
  loadOrphanedSessions: (projectId: number) => Promise<void>;
  loadCleanupCandidates: () => Promise<void>;
  openHistoryForSession: (sessionId: number | null) => void;
  openSessionTarget: (record: SessionRecord) => void;
  resumeRecoverableSession: (
    record: SessionRecord,
  ) => Promise<SessionSnapshot | null>;
  terminateRecoveredSession: (sessionId: number) => Promise<void>;
  recoverOrphanedSession: (record: SessionRecord) => Promise<SessionSnapshot | null>;
  removeStaleArtifact: (candidate: CleanupCandidate) => Promise<void>;
  repairCleanupCandidates: () => Promise<void>;
};

export type RecoveryResult = "pending" | "resumed" | "skipped" | "failed";
export type RecoveryDetailsStatus = "idle" | "loading" | "ready" | "error";

export type RecoverySlice = {
  crashManifest: CrashRecoveryManifest | null;
  recoveryInProgress: boolean;
  recoveryResults: Record<number, RecoveryResult>;
  sessionRecoveryDetails: Record<number, SessionRecoveryDetails>;
  sessionRecoveryStatus: Record<number, RecoveryDetailsStatus>;

  loadCrashManifest: () => Promise<void>;
  dismissRecovery: () => void;
  skipSession: (sessionId: number) => void;
  fetchSessionRecoveryDetails: (
    projectId: number,
    sessionId: number,
  ) => Promise<SessionRecoveryDetails | null>;
};

export type AppSettingsTab =
  | "appearance"
  | "accounts"
  | "defaults"
  | "workflows"
  | "vault"
  | "diagnostics";

export type ProjectRefreshTarget =
  | "workItems"
  | "documents"
  | "worktrees"
  | "workflowRuns"
  | "liveSessions"
  | "sessionSnapshot"
  | "history"
  | "orphanedSessions"
  | "cleanupCandidates";

export type UiSlice = {
  activeView: WorkspaceView;
  activeThemeId: string;
  isProjectRailCollapsed: boolean;
  isSessionRailCollapsed: boolean;
  isAgentGuideOpen: boolean;
  isAppSettingsOpen: boolean;
  appSettingsInitialTab: AppSettingsTab;

  setActiveView: (value: WorkspaceView) => void;
  setActiveThemeId: (id: string) => void;
  setIsProjectRailCollapsed: (value: boolean) => void;
  setIsSessionRailCollapsed: (value: boolean) => void;
  setIsAgentGuideOpen: (value: boolean) => void;
  openAppSettings: (tab?: AppSettingsTab) => void;
  closeAppSettings: () => void;
  refreshSelectedProjectData: (
    targets: ProjectRefreshTarget[],
  ) => Promise<void>;
};

export type WorkflowEntityType = "workflow" | "pod";

export type WorkflowCatalogSlice = {
  workflowLibrary: WorkflowLibrarySnapshot | null;
  projectWorkflowCatalog: ProjectWorkflowCatalog | null;
  workflowRuns: ProjectWorkflowRunSnapshot | null;
  workflowError: string | null;
  isLoadingWorkflowCatalog: boolean;
  isLoadingWorkflowRuns: boolean;
  activeWorkflowActionKey: string | null;
  activeWorkflowRunKey: string | null;

  refreshWorkflowLibrary: () => Promise<WorkflowLibrarySnapshot>;
  refreshProjectWorkflowCatalog: (
    projectId: number,
  ) => Promise<ProjectWorkflowCatalog>;
  refreshProjectWorkflowRuns: (
    projectId: number,
  ) => Promise<ProjectWorkflowRunSnapshot>;
  loadWorkflowCatalog: (projectId: number) => Promise<void>;
  loadWorkflowRuns: (projectId: number) => Promise<void>;
  adoptProjectCatalogEntry: (
    projectId: number,
    entityType: WorkflowEntityType,
    slug: string,
    mode?: "linked" | "forked",
  ) => Promise<void>;
  upgradeProjectCatalogAdoption: (
    projectId: number,
    entityType: WorkflowEntityType,
    slug: string,
  ) => Promise<void>;
  detachProjectCatalogAdoption: (
    projectId: number,
    entityType: WorkflowEntityType,
    slug: string,
  ) => Promise<void>;
  startWorkflowRun: (
    projectId: number,
    workflowSlug: string,
    rootWorkItemId: number,
    rootWorktreeId?: number | null,
  ) => Promise<WorkflowRunRecord | null>;
};

export type AppStore = ProjectSlice &
  SessionSlice &
  WorkItemSlice &
  WorktreeSlice &
  HistorySlice &
  UiSlice &
  RecoverySlice &
  WorkflowCatalogSlice;
