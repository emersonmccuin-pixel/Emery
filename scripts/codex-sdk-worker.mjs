import process from 'node:process'
import { Codex } from '@openai/codex-sdk'

const INBOX_BATCH_LIMIT = 20
const INBOX_WAIT_TIMEOUT_MS = 30_000
const WORKER_HEARTBEAT_INTERVAL_MS = 25_000
const MAX_AUTO_FORWARD_TEXT_LENGTH = 4000
const BROKER_MESSAGE_SENTINEL = 'PROJECT_COMMANDER_MESSAGE'
const PROJECT_COMMANDER_TOOL_NAMES = [
  'project-commander/send_message',
  'project-commander/get_messages',
  'project-commander/wait_for_messages',
  'project-commander/list_messages',
  'project-commander/search_work_items',
  'project-commander/get_work_item',
  'project-commander/list_work_items',
  'project-commander/update_work_item',
  'project-commander/close_work_item',
]
const VALID_DISPATCHER_MESSAGE_TYPES = new Set([
  'question',
  'blocked',
  'complete',
  'options',
  'status_update',
  'request_approval',
])

function requiredEnv(name) {
  const value = process.env[name]?.trim()

  if (!value) {
    throw new Error(`Missing required environment variable ${name}.`)
  }

  return value
}

function optionalEnv(name) {
  const value = process.env[name]
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null
}

function parseIntegerEnv(name) {
  const raw = requiredEnv(name)
  const parsed = Number.parseInt(raw, 10)

  if (!Number.isFinite(parsed)) {
    throw new Error(`Environment variable ${name} must be an integer.`)
  }

  return parsed
}

function envFlag(name) {
  return process.env[name]?.trim().toLowerCase() === 'true'
}

const config = {
  projectId: parseIntegerEnv('PROJECT_COMMANDER_PROJECT_ID'),
  sessionId: parseIntegerEnv('PROJECT_COMMANDER_SESSION_ID'),
  worktreeId: optionalEnv('PROJECT_COMMANDER_WORKTREE_ID'),
  rootPath: requiredEnv('PROJECT_COMMANDER_ROOT_PATH'),
  agentName: requiredEnv('PROJECT_COMMANDER_AGENT_NAME'),
  providerSessionId: optionalEnv('PROJECT_COMMANDER_PROVIDER_SESSION_ID'),
  supervisorPort: parseIntegerEnv('PROJECT_COMMANDER_SUPERVISOR_PORT'),
  supervisorToken: requiredEnv('PROJECT_COMMANDER_SUPERVISOR_TOKEN'),
  supervisorBinary: requiredEnv('PROJECT_COMMANDER_SUPERVISOR_BINARY'),
  bridgeSystemPrompt: requiredEnv('PROJECT_COMMANDER_BRIDGE_SYSTEM_PROMPT'),
  startupPrompt: optionalEnv('PROJECT_COMMANDER_STARTUP_PROMPT'),
  model: optionalEnv('PROJECT_COMMANDER_MODEL'),
  codexPathOverride: optionalEnv('PROJECT_COMMANDER_CODEX_PATH'),
  resumeExistingSession: envFlag('PROJECT_COMMANDER_RESUME_EXISTING_SESSION'),
}

if (config.resumeExistingSession && !config.providerSessionId) {
  throw new Error(
    'PROJECT_COMMANDER_PROVIDER_SESSION_ID is required when resuming a Codex SDK worker thread.',
  )
}

let partialLineOpen = false
let streamedAssistantText = false
let currentAssistantText = ''
let currentThreadId = config.providerSessionId

function writeHostLine(message) {
  ensureTrailingNewline()
  process.stdout.write(`[Project Commander Codex] ${message}\n`)
}

function ensureTrailingNewline() {
  if (partialLineOpen) {
    process.stdout.write('\n')
    partialLineOpen = false
  }
}

function writeTerminalDelta(text) {
  if (!text) {
    return
  }

  process.stdout.write(text)
  partialLineOpen = !text.endsWith('\n')
}

function writeAssistantDelta(text) {
  if (!text) {
    return
  }

  streamedAssistantText = true
  currentAssistantText += text
  writeTerminalDelta(text)
}

function resetTurnAssistantText() {
  streamedAssistantText = false
  currentAssistantText = ''
}

function normalizedAssistantText(...values) {
  for (const value of values) {
    if (typeof value === 'string' && value.trim().length > 0) {
      return value.trim()
    }
  }

  return ''
}

function buildSupervisorUrl(pathname) {
  return `http://127.0.0.1:${config.supervisorPort}${pathname}`
}

async function postSupervisor(pathname, payload) {
  const response = await fetch(buildSupervisorUrl(pathname), {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-project-commander-token': config.supervisorToken,
      'x-project-commander-source': 'codex_sdk_host',
      'x-project-commander-session-id': String(config.sessionId),
    },
    body: JSON.stringify(payload),
  })

  const envelope = await response.json().catch(() => null)

  if (!response.ok || !envelope?.ok) {
    const detail =
      envelope?.error ??
      envelope?.message ??
      `${response.status} ${response.statusText}`.trim()
    throw new Error(`Supervisor request ${pathname} failed: ${detail}`)
  }

  return envelope.data
}

function formatError(error) {
  return error instanceof Error ? error.message : String(error)
}

async function postSupervisorBestEffort(pathname, payload, label) {
  try {
    return await postSupervisor(pathname, payload)
  } catch (error) {
    writeHostLine(`${label} failed: ${formatError(error)}`)
    return null
  }
}

async function publishStatus(state, detail = null, options = {}) {
  return postSupervisorBestEffort(
    '/session/status',
    {
      state,
      detail,
      threadId: options.threadId ?? null,
      providerSessionId: currentThreadId ?? null,
      contextJson: options.contextJson ?? null,
    },
    `worker status ${state}`,
  )
}

async function heartbeat(detail = null, options = {}) {
  return postSupervisorBestEffort(
    '/session/heartbeat',
    {
      detail,
      contextJson: options.contextJson ?? null,
    },
    'worker heartbeat',
  )
}

async function markDone(summary, options = {}) {
  return postSupervisorBestEffort(
    '/session/mark-done',
    {
      summary,
      threadId: options.threadId ?? null,
      providerSessionId: currentThreadId ?? null,
      contextJson: options.contextJson ?? null,
    },
    'worker done marker',
  )
}

async function withWorkerHeartbeat(detail, options, callback) {
  const timer = setInterval(() => {
    void heartbeat(detail, options)
  }, WORKER_HEARTBEAT_INTERVAL_MS)

  try {
    return await callback()
  } finally {
    clearInterval(timer)
  }
}

async function waitForInbox() {
  const data = await postSupervisor('/message/wait', {
    projectId: config.projectId,
    agentName: config.agentName,
    limit: INBOX_BATCH_LIMIT,
    timeoutMs: INBOX_WAIT_TIMEOUT_MS,
  })

  const messages = Array.isArray(data?.messages) ? data.messages : []
  return {
    messages: [...messages].reverse(),
    timedOut: data?.timedOut === true,
  }
}

async function fetchLatestDispatcherReplyId(threadId = null, replyToMessageId = null) {
  const data = await postSupervisor('/message/list', {
    projectId: config.projectId,
    fromAgent: config.agentName,
    toAgent: 'dispatcher',
    threadId,
    replyToMessageId,
    limit: 1,
  })

  const messages = Array.isArray(data?.messages) ? data.messages : []
  return messages[0]?.id ?? null
}

async function fetchDispatcherReplyMarkers(originalMessage) {
  const [exact, threaded, global] = await Promise.all([
    fetchLatestDispatcherReplyId(originalMessage.threadId, originalMessage.id),
    fetchLatestDispatcherReplyId(originalMessage.threadId, null),
    fetchLatestDispatcherReplyId(),
  ])

  return {
    exact: exact ?? 0,
    threaded: threaded ?? 0,
    global: global ?? 0,
  }
}

function hasNewerDispatcherReply(after, before) {
  return (
    after.exact > before.exact ||
    after.threaded > before.threaded ||
    after.global > before.global
  )
}

async function ackMessages(messageIds) {
  if (messageIds.length === 0) {
    return
  }

  await postSupervisor('/message/ack', {
    projectId: config.projectId,
    all: false,
    messageIds,
  })
}

function buildSupervisorMcpArgs() {
  const args = [
    'mcp-stdio',
    '--port',
    String(config.supervisorPort),
    '--token',
    config.supervisorToken,
    '--project-id',
    String(config.projectId),
    '--session-id',
    String(config.sessionId),
  ]

  if (config.worktreeId) {
    args.push('--worktree-id', config.worktreeId)
  }

  return args
}

function buildCodexConfigOverrides() {
  return {
    show_raw_agent_reasoning: true,
    mcp_servers: {
      'project-commander': {
        command: config.supervisorBinary,
        args: buildSupervisorMcpArgs(),
      },
    },
  }
}

function buildThreadOptions() {
  return {
    model: config.model ?? undefined,
    sandboxMode: 'danger-full-access',
    approvalPolicy: 'never',
    workingDirectory: config.rootPath,
  }
}

function buildProjectCommanderToolInstructions() {
  return [
    'Project Commander MCP server is already connected for this session.',
    `Use only these Project Commander tools when you need tracker or broker actions: ${PROJECT_COMMANDER_TOOL_NAMES.join(', ')}.`,
    'For broker replies, call project-commander/send_message.',
    'For semantic tracker lookups, call project-commander/search_work_items.',
    'Never call wcp/*, wcp_*, or any non-project-commander tracker tool in this session.',
    'Do not search the repository to discover tool names.',
  ].join('\n')
}

function formatQueuedPrompt(message) {
  let prompt = [
    `[${message.fromAgent}] (${message.messageType}): ${message.body}`,
    '',
    `Conversation thread: ${message.threadId}`,
    `If you reply with send_message, preserve threadId="${message.threadId}" and set replyToMessageId=${message.id}.`,
    'The worker host will attach the current thread metadata automatically when you use the structured PROJECT_COMMANDER_MESSAGE fallback.',
    '',
    'Important: the dispatcher does NOT receive plain-text terminal replies.',
    'You must use the Project Commander send_message MCP tool for any response that should be delivered.',
    `If send_message is unavailable in your tool list, output exactly one line in this format instead: ${BROKER_MESSAGE_SENTINEL} {"messageType":"complete","body":"...","contextJson":{}}`,
    'When calling the tool, use project-commander/send_message.',
    'If the directive is fully self-contained, handle it directly instead of doing extra repository exploration first.',
  ].join('\n')

  if (typeof message.contextJson === 'string' && message.contextJson.trim().length > 0) {
    prompt += `\n\nStructured context:\n${message.contextJson}`
  }

  return prompt
}

function renderQueuedPrompt(message) {
  ensureTrailingNewline()
  process.stdout.write(`\n[${message.fromAgent}] (${message.messageType}): ${message.body}\n\n`)
}

async function persistProviderSessionIdIfNeeded(threadId) {
  if (!threadId || currentThreadId === threadId) {
    return
  }

  currentThreadId = threadId

  try {
    await postSupervisor('/session/provider-session-id', {
      providerSessionId: threadId,
    })
    writeHostLine(`Registered Codex thread ${threadId}.`)
  } catch (error) {
    process.stderr.write(
      `[Project Commander Codex] Failed to persist Codex thread id ${threadId}: ${error instanceof Error ? error.message : String(error)}\n`,
    )
  }
}

function summarizeFileChanges(changes) {
  if (!Array.isArray(changes) || changes.length === 0) {
    return 'file changes applied'
  }

  return changes
    .slice(0, 6)
    .map((change) => `${change.kind}:${change.path}`)
    .join(', ')
}

function summarizeTodoItems(items) {
  if (!Array.isArray(items) || items.length === 0) {
    return 'todo list updated'
  }

  const completed = items.filter((item) => item?.completed === true).length
  return `todo list ${completed}/${items.length} complete`
}

function itemTextDelta(itemState, itemId, nextText) {
  const previous = itemState.get(itemId) ?? ''

  if (!nextText) {
    itemState.set(itemId, '')
    return ''
  }

  itemState.set(itemId, nextText)
  return nextText.startsWith(previous) ? nextText.slice(previous.length) : nextText
}

function renderCodexItem(eventType, item, itemState) {
  if (!item || typeof item !== 'object') {
    return
  }

  switch (item.type) {
    case 'agent_message': {
      const text = typeof item.text === 'string' ? item.text : ''
      const delta = itemTextDelta(itemState.assistantText, item.id, text)
      if (delta) {
        writeAssistantDelta(delta)
      }
      if (eventType === 'item.completed') {
        ensureTrailingNewline()
      }
      break
    }
    case 'command_execution': {
      if (eventType === 'item.started') {
        writeHostLine(`Command: ${item.command}`)
      }
      const output = typeof item.aggregated_output === 'string' ? item.aggregated_output : ''
      const delta = itemTextDelta(itemState.commandOutput, item.id, output)
      if (delta) {
        writeTerminalDelta(delta)
      }
      if (eventType === 'item.completed') {
        ensureTrailingNewline()
        writeHostLine(
          `Command ${item.status}${typeof item.exit_code === 'number' ? ` (exit ${item.exit_code})` : ''}.`,
        )
      }
      break
    }
    case 'mcp_tool_call': {
      if (eventType === 'item.started') {
        writeHostLine(`Tool: ${item.server}/${item.tool}`)
      } else if (eventType === 'item.completed') {
        writeHostLine(
          item.status === 'completed'
            ? `Tool completed: ${item.server}/${item.tool}`
            : `Tool failed: ${item.server}/${item.tool} (${item.error?.message ?? 'unknown error'})`,
        )
      }
      break
    }
    case 'file_change': {
      if (eventType === 'item.completed') {
        writeHostLine(`Patch ${item.status}: ${summarizeFileChanges(item.changes)}`)
      }
      break
    }
    case 'reasoning': {
      if (eventType === 'item.completed' && typeof item.text === 'string' && item.text.trim()) {
        writeHostLine(`Reasoning: ${item.text.trim()}`)
      }
      break
    }
    case 'todo_list': {
      if (eventType !== 'item.started') {
        writeHostLine(summarizeTodoItems(item.items))
      }
      break
    }
    case 'web_search': {
      if (eventType === 'item.started') {
        writeHostLine(`Web search: ${item.query}`)
      }
      break
    }
    case 'error': {
      if (typeof item.message === 'string' && item.message.trim()) {
        process.stderr.write(`[Project Commander Codex] ${item.message.trim()}\n`)
      }
      break
    }
    default:
      break
  }
}

function trimAutoForwardText(text) {
  const normalized = normalizedAssistantText(text)
  if (!normalized) {
    return ''
  }

  if (normalized.length <= MAX_AUTO_FORWARD_TEXT_LENGTH) {
    return normalized
  }

  return `${normalized.slice(0, MAX_AUTO_FORWARD_TEXT_LENGTH).trimEnd()}\n\n[truncated]`
}

function inferAutoForwardMessageType(replyText) {
  const normalized = normalizedAssistantText(replyText).toLowerCase()
  if (!normalized) {
    return 'status_update'
  }

  if (
    normalized.includes('request approval') ||
    normalized.includes('need approval') ||
    normalized.includes('please approve')
  ) {
    return 'request_approval'
  }

  if (
    normalized.includes('blocked') ||
    normalized.includes('cannot') ||
    normalized.includes("can't") ||
    normalized.includes('unable') ||
    normalized.includes('not available') ||
    normalized.includes('missing')
  ) {
    return 'blocked'
  }

  if (
    normalized.includes('option a') ||
    normalized.includes('option b') ||
    normalized.includes('choose between') ||
    normalized.includes('multiple options')
  ) {
    return 'options'
  }

  if (
    normalized.endsWith('?') ||
    normalized.includes('could you') ||
    normalized.includes('can you') ||
    normalized.includes('would you') ||
    normalized.includes('please clarify')
  ) {
    return 'question'
  }

  if (
    normalized.startsWith('done') ||
    normalized.startsWith('complete') ||
    normalized.startsWith('completed') ||
    normalized.includes('task is done') ||
    normalized.includes('finished')
  ) {
    return 'complete'
  }

  return 'status_update'
}

function buildRecoveryPrompt(originalMessage, undeliveredReply) {
  const sections = [
    'Your previous turn finished without sending a Project Commander message to the dispatcher.',
    'Plain-text terminal output is not delivered to the dispatcher or the user.',
    `Original dispatcher directive:\n${formatQueuedPrompt(originalMessage)}`,
  ]

  if (undeliveredReply) {
    sections.push(`Your undelivered plain-text reply was:\n${undeliveredReply}`)
  }

  sections.push(
    'Now send exactly one dispatcher reply.',
    `Preferred path: call send_message(to="dispatcher", threadId="${originalMessage.threadId}", replyToMessageId=${originalMessage.id}, ...).`,
    `Fallback path if send_message is unavailable: output exactly one line in this format:\n${BROKER_MESSAGE_SENTINEL} {"messageType":"complete","body":"...","contextJson":{}}`,
    'Choose the correct messageType for your state: `question`, `blocked`, `options`, `request_approval`, `status_update`, or `complete`.',
    'Do not search the repository for tool names. Use the existing Project Commander tool directly.',
    'Do not answer in unstructured plain text.',
  )

  return sections.join('\n\n')
}

function buildCodexTurnPrompt(prompt, { includeStartupPrompt = false } = {}) {
  const sections = [config.bridgeSystemPrompt, buildProjectCommanderToolInstructions()]

  if (includeStartupPrompt && config.startupPrompt) {
    sections.push(`Launch-time instructions:\n${config.startupPrompt}`)
  }

  sections.push(prompt)
  return sections.join('\n\n')
}

function normalizeStructuredReplyEnvelope(candidate) {
  if (!candidate || typeof candidate !== 'object' || Array.isArray(candidate)) {
    return null
  }

  const messageType =
    typeof candidate.messageType === 'string' ? candidate.messageType.trim() : ''
  const body = typeof candidate.body === 'string' ? candidate.body.trim() : ''

  if (!VALID_DISPATCHER_MESSAGE_TYPES.has(messageType) || !body) {
    return null
  }

  let contextJson = null
  if (typeof candidate.contextJson === 'string' && candidate.contextJson.trim().length > 0) {
    contextJson = candidate.contextJson.trim()
  } else if (
    candidate.contextJson &&
    typeof candidate.contextJson === 'object' &&
    !Array.isArray(candidate.contextJson)
  ) {
    contextJson = JSON.stringify(candidate.contextJson)
  }

  return {
    messageType,
    body,
    contextJson,
  }
}

function extractStructuredDispatcherReply(text) {
  const normalized = normalizedAssistantText(text)
  if (!normalized) {
    return null
  }

  const lines = normalized
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)

  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]
    const markerIndex = line.indexOf(BROKER_MESSAGE_SENTINEL)
    if (markerIndex === -1) {
      continue
    }

    const rawEnvelope = line.slice(markerIndex + BROKER_MESSAGE_SENTINEL.length).trim()
    if (!rawEnvelope) {
      continue
    }

    try {
      const parsed = JSON.parse(rawEnvelope)
      const envelope = normalizeStructuredReplyEnvelope(parsed)
      if (envelope) {
        return envelope
      }
    } catch {
      // Ignore malformed fallback envelopes and continue scanning.
    }
  }

  return null
}

async function persistStructuredDispatcherReply(originalMessage, envelope) {
  await postSupervisor('/message/send', {
    projectId: config.projectId,
    toAgent: 'dispatcher',
    threadId: originalMessage.threadId,
    replyToMessageId: originalMessage.id,
    messageType: envelope.messageType,
    body: envelope.body,
    contextJson: envelope.contextJson,
  })
}

async function autoForwardUndeliveredReply(originalMessage, replyText) {
  const body = trimAutoForwardText(replyText)
  if (!body) {
    throw new Error(
      `Worker ${config.agentName} finished directive ${originalMessage.id} without a deliverable reply.`,
    )
  }

  const messageType = inferAutoForwardMessageType(body)
  writeHostLine(
    `No send_message detected after recovery turn. Auto-forwarding the terminal reply as ${messageType}.`,
  )

  await postSupervisor('/message/send', {
    projectId: config.projectId,
    toAgent: 'dispatcher',
    threadId: originalMessage.threadId,
    replyToMessageId: originalMessage.id,
    messageType,
    body: `[auto-forwarded terminal reply]\n\n${body}`,
    contextJson: JSON.stringify({
      autoForwarded: true,
      originalDirectiveMessageId: originalMessage.id,
      originalDirectiveType: originalMessage.messageType,
    }),
  })
}

async function ensureDispatcherReply(originalMessage, previousReplyId, firstTurnResult) {
  let latestReplyId = await fetchDispatcherReplyMarkers(originalMessage)

  if (hasNewerDispatcherReply(latestReplyId, previousReplyId)) {
    return
  }

  const firstTurnEnvelope = extractStructuredDispatcherReply(firstTurnResult.assistantText)
  if (firstTurnEnvelope) {
    writeHostLine('Delivering a structured dispatcher reply emitted by the worker host fallback.')
    await persistStructuredDispatcherReply(originalMessage, firstTurnEnvelope)
    return
  }

  writeHostLine(
    'The worker completed a turn without a dispatcher-visible reply. Requesting an explicit send_message follow-up.',
  )

  const recoveryResult = await runTurn(
    buildCodexTurnPrompt(buildRecoveryPrompt(originalMessage, firstTurnResult.assistantText)),
  )

  latestReplyId = await fetchDispatcherReplyMarkers(originalMessage)
  if (hasNewerDispatcherReply(latestReplyId, previousReplyId)) {
    return
  }

  const recoveryEnvelope = extractStructuredDispatcherReply(recoveryResult.assistantText)
  if (recoveryEnvelope) {
    writeHostLine('Delivering a structured dispatcher reply emitted during the recovery turn.')
    await persistStructuredDispatcherReply(originalMessage, recoveryEnvelope)
    return
  }

  await autoForwardUndeliveredReply(
    originalMessage,
    normalizedAssistantText(recoveryResult.assistantText, firstTurnResult.assistantText),
  )
}

const codex = new Codex({
  codexPathOverride: config.codexPathOverride ?? undefined,
  config: buildCodexConfigOverrides(),
})

const thread = config.resumeExistingSession
  ? codex.resumeThread(config.providerSessionId, buildThreadOptions())
  : codex.startThread(buildThreadOptions())

async function runTurn(prompt) {
  resetTurnAssistantText()
  const { events } = await thread.runStreamed(prompt)
  const itemState = {
    assistantText: new Map(),
    commandOutput: new Map(),
  }
  let finalAssistantText = ''
  let usage = null
  let turnFailure = null

  for await (const event of events) {
    switch (event.type) {
      case 'thread.started':
        await persistProviderSessionIdIfNeeded(event.thread_id)
        break
      case 'turn.started':
        writeHostLine('Turn started.')
        break
      case 'turn.completed':
        usage = event.usage
        break
      case 'turn.failed':
        turnFailure = event.error?.message ?? 'Codex turn failed.'
        break
      case 'item.started':
      case 'item.updated':
      case 'item.completed':
        renderCodexItem(event.type, event.item, itemState)
        if (event.item?.type === 'agent_message' && typeof event.item.text === 'string') {
          finalAssistantText = event.item.text
        }
        break
      case 'error':
        turnFailure = event.message || 'Codex stream error.'
        break
      default:
        break
    }
  }

  ensureTrailingNewline()

  if (turnFailure) {
    throw new Error(turnFailure)
  }

  if (!streamedAssistantText && finalAssistantText.trim()) {
    currentAssistantText = finalAssistantText.trim()
    process.stdout.write(`${currentAssistantText}\n`)
  }

  if (usage) {
    writeHostLine(
      `Turn complete (${usage.input_tokens} input, ${usage.cached_input_tokens} cached input, ${usage.output_tokens} output tokens).`,
    )
  } else {
    writeHostLine('Turn complete.')
  }

  return {
    assistantText: normalizedAssistantText(currentAssistantText, finalAssistantText),
  }
}

async function main() {
  process.chdir(config.rootPath)
  writeHostLine(`Watching inbox for ${config.agentName} in ${config.rootPath}`)
  await publishStatus('launching', 'Bootstrapping Codex SDK worker host.', {
    contextJson: {
      resumeExistingSession: config.resumeExistingSession,
      model: config.model,
    },
  })

  let idleLogged = false
  let startupPromptPending = Boolean(config.startupPrompt)

  await publishStatus('ready', 'Worker host ready and waiting for dispatcher directives.', {
    contextJson: {
      resumeExistingSession: config.resumeExistingSession,
      startupPromptPending,
    },
  })

  while (true) {
    const waitResult = await waitForInbox()
    const inbox = waitResult.messages

    if (inbox.length === 0) {
      if (!idleLogged && waitResult.timedOut) {
        writeHostLine('Idle. Waiting for dispatcher directives...')
        await publishStatus('idle', 'Waiting for dispatcher directives.', {
          contextJson: { waitTimedOut: true },
        })
        idleLogged = true
      }
      continue
    }

    idleLogged = false
    const nextMessage = inbox[0]
    const previousReplyId = await fetchDispatcherReplyMarkers(nextMessage)
    renderQueuedPrompt(nextMessage)
    await publishStatus('busy', `Handling dispatcher directive ${nextMessage.id}.`, {
      threadId: nextMessage.threadId,
      contextJson: {
        dispatcherMessageId: nextMessage.id,
        dispatcherMessageType: nextMessage.messageType,
        fromAgent: nextMessage.fromAgent,
        includeStartupPrompt: startupPromptPending,
      },
    })
    const turnResult = await withWorkerHeartbeat(
      `Processing dispatcher directive ${nextMessage.id}.`,
      {
        contextJson: {
          dispatcherMessageId: nextMessage.id,
        },
      },
      () =>
        runTurn(
          buildCodexTurnPrompt(formatQueuedPrompt(nextMessage), {
            includeStartupPrompt: startupPromptPending,
          }),
        ),
    )
    startupPromptPending = false
    await ensureDispatcherReply(nextMessage, previousReplyId, turnResult)
    await ackMessages([nextMessage.id])
    await markDone(`Processed dispatcher directive ${nextMessage.id}.`, {
      threadId: nextMessage.threadId,
      contextJson: {
        dispatcherMessageId: nextMessage.id,
        dispatcherMessageType: nextMessage.messageType,
        providerSessionId: currentThreadId,
      },
    })
    await publishStatus('ready', 'Waiting for dispatcher directives.', {
      contextJson: {
        startupPromptPending,
      },
    })
  }
}

main().catch(async (error) => {
  ensureTrailingNewline()
  await publishStatus('failed', formatError(error), {
    contextJson: {
      phase: 'worker_main',
      providerSessionId: currentThreadId,
    },
  })
  process.stderr.write(
    `[Project Commander Codex] ${error instanceof Error ? error.stack ?? error.message : String(error)}\n`,
  )
  process.exit(1)
})
