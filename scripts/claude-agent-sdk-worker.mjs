import process from 'node:process'
import { query } from '@anthropic-ai/claude-agent-sdk'

const INBOX_BATCH_LIMIT = 20
const INBOX_WAIT_TIMEOUT_MS = 30_000
const MAX_AUTO_FORWARD_TEXT_LENGTH = 4000
const BROKER_MESSAGE_SENTINEL = 'PROJECT_COMMANDER_MESSAGE'
const PROJECT_COMMANDER_TOOL_SELECTORS = [
  'mcp__project-commander__send_message',
  'mcp__project-commander__get_messages',
  'mcp__project-commander__get_work_item',
  'mcp__project-commander__list_work_items',
  'mcp__project-commander__update_work_item',
  'mcp__project-commander__close_work_item',
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
  return typeof value === 'string' && value.trim().length > 0 ? value : null
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
  providerSessionId: requiredEnv('PROJECT_COMMANDER_PROVIDER_SESSION_ID'),
  supervisorPort: parseIntegerEnv('PROJECT_COMMANDER_SUPERVISOR_PORT'),
  supervisorToken: requiredEnv('PROJECT_COMMANDER_SUPERVISOR_TOKEN'),
  bridgeSystemPrompt: requiredEnv('PROJECT_COMMANDER_BRIDGE_SYSTEM_PROMPT'),
  startupPrompt: optionalEnv('PROJECT_COMMANDER_STARTUP_PROMPT'),
  model: optionalEnv('PROJECT_COMMANDER_MODEL'),
  resumeExistingSession: envFlag('PROJECT_COMMANDER_RESUME_EXISTING_SESSION'),
}

let partialLineOpen = false
let streamedAssistantText = false
let currentAssistantText = ''

function writeHostLine(message) {
  process.stdout.write(`[Project Commander SDK] ${message}\n`)
}

function ensureTrailingNewline() {
  if (partialLineOpen) {
    process.stdout.write('\n')
    partialLineOpen = false
  }
}

function writeAssistantDelta(text) {
  if (!text) {
    return
  }

  streamedAssistantText = true
  currentAssistantText += text
  process.stdout.write(text)
  partialLineOpen = !text.endsWith('\n')
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
      'x-project-commander-source': 'claude_agent_sdk_host',
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

function buildMcpServerConfig() {
  const headers = {
    'x-project-commander-token': config.supervisorToken,
    'x-project-commander-project-id': String(config.projectId),
    'x-project-commander-session-id': String(config.sessionId),
    'x-project-commander-source': 'claude_agent_sdk_mcp',
  }

  if (config.worktreeId) {
    headers['x-project-commander-worktree-id'] = config.worktreeId
  }

  return {
    'project-commander': {
      type: 'http',
      url: buildSupervisorUrl('/mcp'),
      headers,
    },
  }
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
    'Do NOT search the repository to discover Project Commander tools. They are already available in your Claude tool list.',
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

function renderSdkMessage(message) {
  switch (message.type) {
    case 'system': {
      switch (message.subtype) {
        case 'init':
          writeHostLine(
            `Session ${message.session_id} ready with ${message.model} (${message.apiKeySource}).`,
          )
          break
        case 'session_state_changed':
          writeHostLine(`Session state: ${message.state}`)
          break
        case 'task_started':
          writeHostLine(`Task started: ${message.description}`)
          break
        case 'task_progress':
          writeHostLine(message.summary ?? message.description)
          break
        case 'task_notification':
          writeHostLine(`${message.status}: ${message.summary}`)
          break
        default:
          break
      }
      break
    }
    case 'tool_progress':
      writeHostLine(
        `Tool ${message.tool_name} running (${message.elapsed_time_seconds.toFixed(1)}s)`,
      )
      break
    case 'tool_use_summary':
      writeHostLine(message.summary)
      break
    case 'stream_event': {
      const event = message.event

      if (
        event?.type === 'content_block_delta' &&
        event.delta?.type === 'text_delta' &&
        typeof event.delta.text === 'string'
      ) {
        writeAssistantDelta(event.delta.text)
      } else if (event?.type === 'content_block_stop') {
        ensureTrailingNewline()
      }
      break
    }
    case 'result':
      ensureTrailingNewline()
      if (message.subtype === 'success') {
        if (
          !streamedAssistantText &&
          typeof message.result === 'string' &&
          message.result.trim().length > 0
        ) {
          currentAssistantText += message.result
          process.stdout.write(`${message.result.trim()}\n`)
        }
        writeHostLine(
          `Turn complete in ${message.duration_ms} ms (${message.num_turns} turns, $${message.total_cost_usd.toFixed(4)}).`,
        )
        return
      }

      for (const error of message.errors ?? []) {
        process.stderr.write(`[Project Commander SDK] ${error}\n`)
      }
      throw new Error(
        `Claude Agent SDK turn failed (${message.subtype}) after ${message.duration_ms} ms.`,
      )
    default:
      break
  }
}

function buildQueryOptions(useResume) {
  return {
    cwd: config.rootPath,
    includePartialMessages: true,
    permissionMode: 'bypassPermissions',
    allowDangerouslySkipPermissions: true,
    model: config.model ?? undefined,
    mcpServers: buildMcpServerConfig(),
    systemPrompt: {
      type: 'preset',
      preset: 'claude_code',
      append: config.bridgeSystemPrompt,
    },
    ...(useResume
      ? { resume: config.providerSessionId }
      : { sessionId: config.providerSessionId }),
  }
}

async function runTurn(prompt, useResume) {
  resetTurnAssistantText()
  const stream = query({
    prompt,
    options: buildQueryOptions(useResume),
  })

  let sawResult = false
  let resultText = ''

  for await (const message of stream) {
    renderSdkMessage(message)
    if (message.type === 'result') {
      sawResult = true
      if (typeof message.result === 'string') {
        resultText = message.result
      }
    }
  }

  if (!sawResult) {
    throw new Error('Claude Agent SDK turn ended without a result message.')
  }

  return {
    assistantText: normalizedAssistantText(currentAssistantText, resultText),
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

function buildToolBootstrapPrompt() {
  const selectors = PROJECT_COMMANDER_TOOL_SELECTORS.map((name) => `- select:${name}`).join('\n')

  return [
    'Before handling dispatcher work, load the core Project Commander MCP tools into this session.',
    'Use the ToolSearch tool for each of the following selectors so the tool schemas become available:',
    selectors,
    'Do not inspect the repository. Do not send any Project Commander messages. Do not modify files.',
    'After the tools are loaded, stop. A short plain-text confirmation is fine.',
  ].join('\n\n')
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
    buildRecoveryPrompt(originalMessage, firstTurnResult.assistantText),
    true,
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

async function main() {
  process.chdir(config.rootPath)
  writeHostLine(`Watching inbox for ${config.agentName} in ${config.rootPath}`)

  let hasExecutedTurn = config.resumeExistingSession
  let idleLogged = false

  if (!config.resumeExistingSession) {
    writeHostLine('Loading Project Commander MCP tools into the fresh SDK session.')
    await runTurn(buildToolBootstrapPrompt(), false)
    hasExecutedTurn = true
  }

  if (config.startupPrompt) {
    writeHostLine('Processing startup prompt.')
    await runTurn(config.startupPrompt, hasExecutedTurn || config.resumeExistingSession)
    hasExecutedTurn = true
  }

  while (true) {
    const waitResult = await waitForInbox()
    const inbox = waitResult.messages

    if (inbox.length === 0) {
      if (!idleLogged && waitResult.timedOut) {
        writeHostLine('Idle. Waiting for dispatcher directives...')
        idleLogged = true
      }
      continue
    }

    idleLogged = false
    const nextMessage = inbox[0]
    const prompt = formatQueuedPrompt(nextMessage)
    const previousReplyId = await fetchDispatcherReplyMarkers(nextMessage)
    renderQueuedPrompt(nextMessage)
    const turnResult = await runTurn(prompt, hasExecutedTurn || config.resumeExistingSession)
    await ensureDispatcherReply(nextMessage, previousReplyId, turnResult)
    await ackMessages([nextMessage.id])
    hasExecutedTurn = true
  }
}

main().catch((error) => {
  ensureTrailingNewline()
  process.stderr.write(
    `[Project Commander SDK] ${error instanceof Error ? error.stack ?? error.message : String(error)}\n`,
  )
  process.exit(1)
})
