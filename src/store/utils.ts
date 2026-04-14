import type {
  CleanupCandidate,
  DocumentRecord,
  LaunchProfileRecord,
  ProjectRecord,
  SessionEventRecord,
  SessionRecord,
  SessionSnapshot,
  WorkItemRecord,
  WorkItemStatus,
  WorktreeRecord,
} from "../types";

export const WORK_ITEM_STATUS_ORDER: Record<WorkItemStatus, number> = {
  in_progress: 0,
  blocked: 1,
  backlog: 2,
  parked: 3,
  done: 4,
};

export const PROJECT_COMMANDER_TOOLS = [
  "list_work_items(status?, itemType?, parentOnly?, openOnly?)",
  "get_work_item(id)",
  "create_work_item(...)",
  "update_work_item(...)",
  "close_work_item(id)",
  "list_worktrees()",
  "launch_worktree_agent(workItemId, launchProfileId?)",
  "list_documents(workItemId?)",
  "create_document(...)",
  "update_document(...)",
] as const;

export const DEFAULT_APP_SETTINGS = {
  defaultLaunchProfileId: null,
  defaultWorkerLaunchProfileId: null,
  sdkClaudeConfigDir: null,
  autoRepairSafeCleanupOnStartup: false,
} as const;

export const SESSION_EVENT_HISTORY_LIMIT = 120;
export const SESSION_RECORD_HISTORY_LIMIT = 200;

export const DEFAULT_PROFILE_LABEL = "Claude Code / YOLO";
export const DEFAULT_PROFILE_PROVIDER = "claude_code";
export const DEFAULT_PROFILE_EXECUTABLE = "claude";
export const DEFAULT_PROFILE_ARGS = "--dangerously-skip-permissions";
export const DEFAULT_PROFILE_ENV_JSON = "{}";
export const CLAUDE_CODE_PROVIDER = "claude_code";
export const CLAUDE_AGENT_SDK_PROVIDER = "claude_agent_sdk";
export const CODEX_SDK_PROVIDER = "codex_sdk";
const WORKER_LAUNCH_PROFILE_PROVIDERS = new Set([
  CLAUDE_AGENT_SDK_PROVIDER,
  CODEX_SDK_PROVIDER,
]);

export function getLaunchProfileProviderLabel(provider: string) {
  switch (provider) {
    case CLAUDE_CODE_PROVIDER:
      return "Claude Code CLI";
    case CLAUDE_AGENT_SDK_PROVIDER:
      return "Claude Agent SDK";
    case CODEX_SDK_PROVIDER:
      return "Codex SDK";
    default:
      return provider;
  }
}

export function isDispatcherLaunchProfile(
  profile?: LaunchProfileRecord | null,
) {
  return profile?.provider === CLAUDE_CODE_PROVIDER;
}

export function getDispatcherLaunchProfiles(profiles: LaunchProfileRecord[]) {
  return profiles.filter(
    (profile) => profile.provider === CLAUDE_CODE_PROVIDER,
  );
}

export function isWorkerLaunchProfileProvider(provider?: string | null) {
  return provider != null && WORKER_LAUNCH_PROFILE_PROVIDERS.has(provider);
}

export function getWorkerLaunchProfiles(profiles: LaunchProfileRecord[]) {
  return profiles.filter(
    (profile) => isWorkerLaunchProfileProvider(profile.provider),
  );
}

export function getFirstDispatcherLaunchProfile(
  profiles: LaunchProfileRecord[],
) {
  return getDispatcherLaunchProfiles(profiles)[0] ?? profiles[0] ?? null;
}

export function getFirstWorkerLaunchProfile(profiles: LaunchProfileRecord[]) {
  return getWorkerLaunchProfiles(profiles)[0] ?? profiles[0] ?? null;
}

export function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (typeof error === "string" && error.trim()) {
    try {
      const parsed = JSON.parse(error) as {
        error?: unknown;
        message?: unknown;
      };

      if (typeof parsed.error === "string" && parsed.error.trim()) {
        return parsed.error;
      }

      if (typeof parsed.message === "string" && parsed.message.trim()) {
        return parsed.message;
      }
    } catch {
      return error;
    }

    return error;
  }

  if (error && typeof error === "object") {
    const candidate = error as { error?: unknown; message?: unknown };

    if (typeof candidate.error === "string" && candidate.error.trim()) {
      return candidate.error;
    }

    if (typeof candidate.message === "string" && candidate.message.trim()) {
      return candidate.message;
    }
  }

  return fallback;
}

export function sortWorkItems(items: WorkItemRecord[]) {
  return [...items].sort((left, right) => {
    const statusDelta =
      WORK_ITEM_STATUS_ORDER[left.status] -
      WORK_ITEM_STATUS_ORDER[right.status];

    if (statusDelta !== 0) {
      return statusDelta;
    }

    return right.updatedAt.localeCompare(left.updatedAt);
  });
}

export function sortDocuments(documents: DocumentRecord[]) {
  return [...documents].sort((left, right) =>
    right.updatedAt.localeCompare(left.updatedAt),
  );
}

export function areSessionSnapshotListsEqual(
  left: SessionSnapshot[],
  right: SessionSnapshot[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((snapshot, index) => {
    const candidate = right[index];

    return (
      snapshot.sessionId === candidate.sessionId &&
      snapshot.projectId === candidate.projectId &&
      (snapshot.worktreeId ?? null) === (candidate.worktreeId ?? null) &&
      snapshot.launchProfileId === candidate.launchProfileId &&
      snapshot.profileLabel === candidate.profileLabel &&
      snapshot.rootPath === candidate.rootPath &&
      snapshot.isRunning === candidate.isRunning &&
      snapshot.startedAt === candidate.startedAt &&
      (snapshot.exitCode ?? null) === (candidate.exitCode ?? null) &&
      (snapshot.exitSuccess ?? null) === (candidate.exitSuccess ?? null)
    );
  });
}

export function areWorkItemListsEqual(
  left: WorkItemRecord[],
  right: WorkItemRecord[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((workItem, index) => {
    const candidate = right[index];

    return (
      workItem.id === candidate.id &&
      workItem.projectId === candidate.projectId &&
      workItem.parentWorkItemId === candidate.parentWorkItemId &&
      workItem.callSign === candidate.callSign &&
      workItem.sequenceNumber === candidate.sequenceNumber &&
      workItem.childNumber === candidate.childNumber &&
      workItem.title === candidate.title &&
      workItem.body === candidate.body &&
      workItem.itemType === candidate.itemType &&
      workItem.status === candidate.status &&
      workItem.createdAt === candidate.createdAt &&
      workItem.updatedAt === candidate.updatedAt
    );
  });
}

export function areDocumentListsEqual(
  left: DocumentRecord[],
  right: DocumentRecord[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((document, index) => {
    const candidate = right[index];

    return (
      document.id === candidate.id &&
      document.projectId === candidate.projectId &&
      document.workItemId === candidate.workItemId &&
      document.title === candidate.title &&
      document.body === candidate.body &&
      document.createdAt === candidate.createdAt &&
      document.updatedAt === candidate.updatedAt
    );
  });
}

export function areWorktreeListsEqual(
  left: WorktreeRecord[],
  right: WorktreeRecord[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((worktree, index) => {
    const candidate = right[index];

    return (
      worktree.id === candidate.id &&
      worktree.projectId === candidate.projectId &&
      worktree.workItemId === candidate.workItemId &&
      worktree.workItemCallSign === candidate.workItemCallSign &&
      worktree.workItemTitle === candidate.workItemTitle &&
      worktree.workItemStatus === candidate.workItemStatus &&
      worktree.branchName === candidate.branchName &&
      worktree.shortBranchName === candidate.shortBranchName &&
      worktree.worktreePath === candidate.worktreePath &&
      worktree.pathAvailable === candidate.pathAvailable &&
      worktree.hasUncommittedChanges === candidate.hasUncommittedChanges &&
      worktree.hasUnmergedCommits === candidate.hasUnmergedCommits &&
      worktree.pinned === candidate.pinned &&
      worktree.isCleanupEligible === candidate.isCleanupEligible &&
      worktree.pendingSignalCount === candidate.pendingSignalCount &&
      worktree.agentName === candidate.agentName &&
      worktree.sessionSummary === candidate.sessionSummary &&
      worktree.createdAt === candidate.createdAt &&
      worktree.updatedAt === candidate.updatedAt
    );
  });
}

export function areSessionRecordListsEqual(
  left: SessionRecord[],
  right: SessionRecord[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((record, index) => {
    const candidate = right[index];

    return (
      record.id === candidate.id &&
      record.projectId === candidate.projectId &&
      record.launchProfileId === candidate.launchProfileId &&
      record.worktreeId === candidate.worktreeId &&
      record.processId === candidate.processId &&
      record.supervisorPid === candidate.supervisorPid &&
      record.provider === candidate.provider &&
      record.providerSessionId === candidate.providerSessionId &&
      record.profileLabel === candidate.profileLabel &&
      record.rootPath === candidate.rootPath &&
      record.state === candidate.state &&
      record.startupPrompt === candidate.startupPrompt &&
      record.startedAt === candidate.startedAt &&
      record.endedAt === candidate.endedAt &&
      record.exitCode === candidate.exitCode &&
      record.exitSuccess === candidate.exitSuccess &&
      record.createdAt === candidate.createdAt &&
      record.updatedAt === candidate.updatedAt &&
      record.lastHeartbeatAt === candidate.lastHeartbeatAt
    );
  });
}

export function areSessionEventListsEqual(
  left: SessionEventRecord[],
  right: SessionEventRecord[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((event, index) => {
    const candidate = right[index];

    return (
      event.id === candidate.id &&
      event.projectId === candidate.projectId &&
      event.sessionId === candidate.sessionId &&
      event.eventType === candidate.eventType &&
      event.entityType === candidate.entityType &&
      event.entityId === candidate.entityId &&
      event.source === candidate.source &&
      event.payloadJson === candidate.payloadJson &&
      event.createdAt === candidate.createdAt
    );
  });
}

export function areCleanupCandidateListsEqual(
  left: CleanupCandidate[],
  right: CleanupCandidate[],
) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((candidate, index) => {
    const nextCandidate = right[index];

    return (
      candidate.kind === nextCandidate.kind &&
      candidate.path === nextCandidate.path &&
      candidate.projectId === nextCandidate.projectId &&
      candidate.worktreeId === nextCandidate.worktreeId &&
      candidate.sessionId === nextCandidate.sessionId &&
      candidate.reason === nextCandidate.reason
    );
  });
}

export function buildAgentStartupPrompt(
  project: ProjectRecord | null,
  workItems: WorkItemRecord[],
  documents: DocumentRecord[],
) {
  if (!project) {
    return "";
  }

  const workItemCallSigns = new Map(
    workItems.map((item) => [item.id, item.callSign]),
  );
  const workItemLines =
    workItems.length === 0
      ? [
          "- No work items yet. Create them with project-commander-cli when needed.",
        ]
      : workItems
          .slice(0, 5)
          .map(
            (item) =>
              `- ${item.callSign} [${item.status}/${item.itemType}] ${item.title}`,
          );

  const documentLines =
    documents.length === 0
      ? ["- No documents yet."]
      : documents.slice(0, 5).map((document) => {
          const linkedCallSign =
            document.workItemId === null
              ? null
              : (workItemCallSigns.get(document.workItemId) ?? null);
          const linkedLabel =
            document.workItemId === null
              ? "project-level"
              : `linked to ${linkedCallSign ?? `work item #${document.workItemId}`}`;

          return `- #${document.id} [${linkedLabel}] ${document.title}`;
        });

  return [
    "Project Commander dispatcher startup context.",
    `Project: ${project.name}`,
    `Root path: ${project.rootPath}`,
    "Use this root path for all shell commands. Do not convert to WSL-style /mnt/ paths.",
    "Project Commander MCP tools are attached to this session.",
    "You are the repository dispatcher for this project.",
    "Use the Project Commander MCP tools as the source of truth for project context, work items, documents, and focused worktree launches.",
    "If MCP tools are unavailable, fall back to project-commander-cli.",
    "Key tools:",
    "- list_work_items(status?, itemType?, parentOnly?, openOnly?)",
    "- list_worktrees()",
    "- launch_worktree_agent(workItemId, launchProfileId?)",
    "- create_work_item(...)",
    "- update_work_item(...)",
    "- close_work_item(id)",
    "- list_documents(workItemId?)",
    "Current work items:",
    ...workItemLines,
    "Current documents:",
    ...documentLines,
    `Required first action: call get_work_item for ${project.workItemPrefix ?? project.name}-0 to load current project state, priorities, and operating procedures. Then list_worktrees to check for active agents.`,
  ].join("\n");
}

export function flattenPromptForTerminal(prompt: string) {
  return `${prompt.replace(/\s+/g, " ").trim()}\r`;
}
