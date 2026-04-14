import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HOST = process.env.AGENT_LAB_HOST || '127.0.0.1'
const PORT = Number.parseInt(process.env.AGENT_LAB_PORT || '43117', 10)
const MAX_EVENT_COUNT = 400
const STATIC_DIR = path.join(path.dirname(fileURLToPath(import.meta.url)), 'static')

const runs = new Map()

const PROVIDERS = {
  claude: {
    label: 'Claude Code',
    command: 'claude',
    supportsNativeSteer: false,
    defaultMode: 'default',
  },
  codex: {
    label: 'Codex CLI',
    command: 'codex',
    supportsNativeSteer: false,
    defaultMode: 'default',
  },
}

function writeJson(res, status, value) {
  const body = JSON.stringify(value)
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Content-Length': Buffer.byteLength(body),
    'Cache-Control': 'no-store',
  })
  res.end(body)
}

function writeText(res, status, body, contentType = 'text/plain; charset=utf-8') {
  res.writeHead(status, {
    'Content-Type': contentType,
    'Content-Length': Buffer.byteLength(body),
    'Cache-Control': 'no-store',
  })
  res.end(body)
}

async function readJsonBody(req) {
  const chunks = []

  for await (const chunk of req) {
    chunks.push(chunk)
  }

  if (chunks.length === 0) {
    return {}
  }

  return JSON.parse(Buffer.concat(chunks).toString('utf8'))
}

function sseWrite(client, payload) {
  client.write(`data: ${JSON.stringify(payload)}\n\n`)
}

function emitRunEvent(run, payload) {
  const event = {
    id: `${run.id}:${run.eventSequence++}`,
    at: new Date().toISOString(),
    ...payload,
  }

  run.events.push(event)
  if (run.events.length > MAX_EVENT_COUNT) {
    run.events.shift()
  }

  for (const client of run.clients) {
    sseWrite(client, event)
  }
}

function sanitizeRun(run, includeEvents = false) {
  return {
    id: run.id,
    provider: run.provider,
    providerLabel: PROVIDERS[run.provider]?.label ?? run.provider,
    mode: run.mode,
    cwd: run.cwd,
    prompt: run.prompt,
    model: run.model,
    runMode: run.runMode,
    status: run.status,
    createdAt: run.createdAt,
    startedAt: run.startedAt,
    endedAt: run.endedAt,
    pid: run.pid,
    exitCode: run.exitCode,
    command: run.command,
    args: run.args,
    git: run.git,
    resumeId: run.resumeId,
    sessionId: run.sessionId,
    threadId: run.threadId,
    latestText: run.latestText,
    latestError: run.latestError,
    supportsNativeSteer: PROVIDERS[run.provider]?.supportsNativeSteer ?? false,
    eventCount: run.events.length,
    events: includeEvents ? run.events : undefined,
  }
}

function deepFindStringByKeys(value, keys) {
  if (value == null) {
    return null
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const result = deepFindStringByKeys(item, keys)
      if (result !== null) {
        return result
      }
    }
    return null
  }

  if (typeof value !== 'object') {
    return null
  }

  for (const key of keys) {
    const candidate = value[key]
    if (typeof candidate === 'string' && candidate.trim()) {
      return candidate.trim()
    }
  }

  for (const candidate of Object.values(value)) {
    const result = deepFindStringByKeys(candidate, keys)
    if (result !== null) {
      return result
    }
  }

  return null
}

function extractLikelyText(value) {
  if (typeof value === 'string' && value.trim()) {
    return value.trim()
  }

  if (value == null || typeof value !== 'object') {
    return null
  }

  for (const key of ['text', 'delta', 'content', 'message', 'result', 'summary', 'output']) {
    const candidate = value[key]
    if (typeof candidate === 'string' && candidate.trim()) {
      return candidate.trim()
    }
  }

  return null
}

function updateRunFromJsonEvent(run, parsed) {
  if (run.provider === 'claude') {
    const sessionId = deepFindStringByKeys(parsed, ['session_id', 'sessionId'])
    if (sessionId) {
      run.sessionId = sessionId
      run.resumeId = sessionId
    }
  }

  if (run.provider === 'codex') {
    const threadId = deepFindStringByKeys(parsed, ['thread_id', 'threadId'])
    if (threadId) {
      run.threadId = threadId
      run.resumeId = threadId
    }
  }

  const text = extractLikelyText(parsed)
  if (text) {
    run.latestText = text
  }

  const errorText = deepFindStringByKeys(parsed, ['error', 'message'])
  if (parsed?.type === 'turn.failed' || parsed?.type === 'error') {
    run.latestError = errorText || 'Provider reported an error.'
  }
}

function processOutputLine(run, stream, line) {
  if (!line.trim()) {
    return
  }

  let parsed = null
  if (stream === 'stdout') {
    try {
      parsed = JSON.parse(line)
    } catch {
      parsed = null
    }
  }

  if (parsed) {
    updateRunFromJsonEvent(run, parsed)
    emitRunEvent(run, { kind: 'json', stream, payload: parsed })
    return
  }

  if (stream === 'stderr') {
    run.latestError = line.trim()
  } else {
    run.latestText = line.trim()
  }

  emitRunEvent(run, { kind: 'text', stream, text: line })
}

function wireProcessStream(run, streamName, source) {
  source.setEncoding('utf8')
  source.on('data', (chunk) => {
    run.streamBuffers[streamName] += chunk
    let newlineIndex = run.streamBuffers[streamName].indexOf('\n')

    while (newlineIndex !== -1) {
      const line = run.streamBuffers[streamName]
        .slice(0, newlineIndex)
        .replace(/\r$/, '')
      run.streamBuffers[streamName] = run.streamBuffers[streamName].slice(newlineIndex + 1)
      processOutputLine(run, streamName, line)
      newlineIndex = run.streamBuffers[streamName].indexOf('\n')
    }
  })
}

async function runCommand(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env ?? process.env,
      stdio: ['ignore', 'pipe', 'pipe'],
    })

    let stdout = ''
    let stderr = ''

    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      stdout += chunk
    })
    child.stderr.on('data', (chunk) => {
      stderr += chunk
    })

    child.on('error', (error) => {
      resolve({
        ok: false,
        exitCode: null,
        stdout,
        stderr: stderr || String(error),
      })
    })

    child.on('close', (exitCode) => {
      resolve({
        ok: exitCode === 0,
        exitCode,
        stdout,
        stderr,
      })
    })
  })
}

async function detectGitContext(cwd) {
  const inside = await runCommand('git', ['rev-parse', '--is-inside-work-tree'], { cwd })
  if (!inside.ok || inside.stdout.trim() !== 'true') {
    return {
      isGitRepo: false,
      gitRoot: null,
      branch: null,
      worktreePath: null,
    }
  }

  const [gitRoot, branch, worktreePath] = await Promise.all([
    runCommand('git', ['rev-parse', '--show-toplevel'], { cwd }),
    runCommand('git', ['branch', '--show-current'], { cwd }),
    runCommand('git', ['rev-parse', '--path-format=absolute', '--git-common-dir'], { cwd }),
  ])

  return {
    isGitRepo: true,
    gitRoot: gitRoot.ok ? gitRoot.stdout.trim() : null,
    branch: branch.ok ? branch.stdout.trim() || '(detached)' : null,
    worktreePath: worktreePath.ok ? worktreePath.stdout.trim() : null,
  }
}

function buildClaudeArgs(input) {
  const args = [
    '--print',
    '--verbose',
    '--output-format',
    'stream-json',
    '--include-partial-messages',
  ]

  if (input.model) {
    args.push('--model', input.model)
  }

  if (input.runMode && input.runMode !== 'default') {
    args.push('--permission-mode', input.runMode)
  }

  if (input.systemPrompt) {
    args.push('--append-system-prompt', input.systemPrompt)
  }

  if (input.allowedTools) {
    args.push('--allowedTools', input.allowedTools)
  }

  if (input.bare) {
    args.push('--bare')
  }

  if (input.resumeId) {
    args.push('--resume', input.resumeId)
  }

  args.push(input.prompt)
  return args
}

function buildCodexArgs(input) {
  const args = ['exec']

  if (input.resumeId) {
    args.push('resume', input.resumeId)
  }

  if (input.model) {
    args.push('--model', input.model)
  }

  if (input.runMode === 'full-auto') {
    args.push('--full-auto')
  } else if (input.runMode === 'yolo') {
    args.push('--dangerously-bypass-approvals-and-sandbox')
  }

  if (input.skipGitRepoCheck) {
    args.push('--skip-git-repo-check')
  }

  args.push('--json')
  args.push(input.prompt)
  return args
}

function buildProviderCommand(input) {
  if (input.provider === 'claude') {
    return { command: 'claude', args: buildClaudeArgs(input) }
  }

  if (input.provider === 'codex') {
    return { command: 'codex', args: buildCodexArgs(input) }
  }

  throw new Error(`Unsupported provider: ${input.provider}`)
}

async function createRun(input) {
  const provider = input.provider
  if (!PROVIDERS[provider]) {
    throw new Error(`Unsupported provider: ${provider}`)
  }

  if (typeof input.cwd !== 'string' || !input.cwd.trim()) {
    throw new Error('A working directory is required.')
  }

  if (typeof input.prompt !== 'string' || !input.prompt.trim()) {
    throw new Error('A prompt is required.')
  }

  const cwd = input.cwd.trim()
  const prompt = input.prompt.trim()
  const runId = randomUUID()
  const git = await detectGitContext(cwd)
  const commandSpec = buildProviderCommand({ ...input, cwd, prompt })

  const run = {
    id: runId,
    provider,
    mode: input.resumeId ? 'resume' : 'start',
    cwd,
    prompt,
    model: input.model?.trim() || '',
    runMode: input.runMode || PROVIDERS[provider].defaultMode,
    command: commandSpec.command,
    args: commandSpec.args,
    git,
    createdAt: new Date().toISOString(),
    startedAt: null,
    endedAt: null,
    status: 'launching',
    pid: null,
    exitCode: null,
    sessionId: null,
    threadId: null,
    resumeId: input.resumeId?.trim() || null,
    latestText: null,
    latestError: null,
    eventSequence: 0,
    events: [],
    clients: new Set(),
    streamBuffers: {
      stdout: '',
      stderr: '',
    },
  }

  runs.set(run.id, run)

  emitRunEvent(run, {
    kind: 'status',
    stream: 'meta',
    status: 'launching',
    command: run.command,
    args: run.args,
    git: run.git,
  })

  const child = spawn(run.command, run.args, {
    cwd,
    env: process.env,
    stdio: ['ignore', 'pipe', 'pipe'],
  })

  child.on('error', (error) => {
    run.status = 'failed'
    run.endedAt = new Date().toISOString()
    run.latestError = String(error)
    emitRunEvent(run, {
      kind: 'status',
      stream: 'meta',
      status: 'failed',
      error: String(error),
    })
  })

  child.on('spawn', () => {
    run.startedAt = new Date().toISOString()
    run.status = 'running'
    run.pid = child.pid ?? null
    emitRunEvent(run, { kind: 'status', stream: 'meta', status: 'running', pid: run.pid })
  })

  wireProcessStream(run, 'stdout', child.stdout)
  wireProcessStream(run, 'stderr', child.stderr)

  child.on('close', (exitCode) => {
    if (run.streamBuffers.stdout.trim()) {
      processOutputLine(run, 'stdout', run.streamBuffers.stdout.trim())
      run.streamBuffers.stdout = ''
    }
    if (run.streamBuffers.stderr.trim()) {
      processOutputLine(run, 'stderr', run.streamBuffers.stderr.trim())
      run.streamBuffers.stderr = ''
    }

    run.exitCode = exitCode
    run.endedAt = new Date().toISOString()
    run.status = exitCode === 0 ? 'completed' : 'failed'

    emitRunEvent(run, {
      kind: 'status',
      stream: 'meta',
      status: run.status,
      exitCode,
      resumeId: run.resumeId,
    })
  })

  return run
}

async function getProviderStatuses() {
  const checks = await Promise.all(
    Object.entries(PROVIDERS).map(async ([provider, config]) => {
      const version = await runCommand(config.command, ['--version'])
      const helpArgs = provider === 'codex' ? ['app-server', '--help'] : ['--help']
      const extra = await runCommand(config.command, helpArgs)

      return [
        provider,
        {
          provider,
          label: config.label,
          command: config.command,
          available: version.ok,
          version: version.ok
            ? version.stdout.trim() || version.stderr.trim() || 'available'
            : null,
          details: extra.ok
            ? extra.stdout.trim().split(/\r?\n/)[0] || 'ok'
            : extra.stderr.trim() || null,
          supportsStructuredStream: true,
          supportsResume: true,
          supportsNativeSteer: config.supportsNativeSteer,
        },
      ]
    }),
  )

  return Object.fromEntries(checks)
}

async function handleApi(req, res, pathname) {
  if (req.method === 'GET' && pathname === '/api/providers') {
    writeJson(res, 200, { providers: await getProviderStatuses() })
    return
  }

  if (req.method === 'GET' && pathname === '/api/runs') {
    const list = [...runs.values()]
      .map((run) => sanitizeRun(run))
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
    writeJson(res, 200, { runs: list })
    return
  }

  if (req.method === 'POST' && pathname === '/api/runs') {
    const body = await readJsonBody(req)
    const run = await createRun(body)
    writeJson(res, 201, { run: sanitizeRun(run, true) })
    return
  }

  const runMatch = pathname.match(/^\/api\/runs\/([^/]+)$/)
  if (req.method === 'GET' && runMatch) {
    const run = runs.get(runMatch[1])
    if (!run) {
      writeJson(res, 404, { error: 'Run not found.' })
      return
    }

    writeJson(res, 200, { run: sanitizeRun(run, true) })
    return
  }

  const streamMatch = pathname.match(/^\/api\/runs\/([^/]+)\/stream$/)
  if (req.method === 'GET' && streamMatch) {
    const run = runs.get(streamMatch[1])
    if (!run) {
      writeJson(res, 404, { error: 'Run not found.' })
      return
    }

    res.writeHead(200, {
      'Content-Type': 'text/event-stream; charset=utf-8',
      'Cache-Control': 'no-store',
      Connection: 'keep-alive',
    })
    res.write('\n')

    run.clients.add(res)
    sseWrite(res, { kind: 'snapshot', run: sanitizeRun(run, true) })

    req.on('close', () => {
      run.clients.delete(res)
    })
    return
  }

  writeJson(res, 404, { error: 'Not found.' })
}

async function serveStatic(res, pathname) {
  const relativePath = pathname === '/' ? 'index.html' : pathname.slice(1)
  const filePath = path.join(STATIC_DIR, relativePath)
  const file = await readFile(filePath)
  const extension = path.extname(filePath)
  const contentType =
    extension === '.html'
      ? 'text/html; charset=utf-8'
      : extension === '.js'
        ? 'text/javascript; charset=utf-8'
        : extension === '.css'
          ? 'text/css; charset=utf-8'
          : 'application/octet-stream'

  writeText(res, 200, file.toString('utf8'), contentType)
}

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url || '/', `http://${HOST}:${PORT}`)

    if (url.pathname.startsWith('/api/')) {
      await handleApi(req, res, url.pathname)
      return
    }

    await serveStatic(res, url.pathname)
  } catch (error) {
    writeJson(res, 500, {
      error: error instanceof Error ? error.message : String(error),
    })
  }
})

server.listen(PORT, HOST, () => {
  console.log(
    `[agent-lab] listening on http://${HOST}:${PORT} (Claude ${PROVIDERS.claude.command}, Codex ${PROVIDERS.codex.command})`,
  )
})
