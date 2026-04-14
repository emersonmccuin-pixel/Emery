const providerModes = {
  claude: [
    { value: 'default', label: 'default' },
    { value: 'acceptEdits', label: 'acceptEdits' },
    { value: 'auto', label: 'auto' },
    { value: 'dontAsk', label: 'dontAsk' },
    { value: 'bypassPermissions', label: 'bypassPermissions' },
  ],
  codex: [
    { value: 'default', label: 'default' },
    { value: 'full-auto', label: 'full-auto' },
    { value: 'yolo', label: 'yolo' },
  ],
}

const state = {
  providers: {},
  runs: [],
  selectedRunId: null,
  stream: null,
}

const elements = {
  provider: document.querySelector('#provider'),
  cwd: document.querySelector('#cwd'),
  model: document.querySelector('#model'),
  runMode: document.querySelector('#runMode'),
  prompt: document.querySelector('#prompt'),
  allowedTools: document.querySelector('#allowedTools'),
  systemPrompt: document.querySelector('#systemPrompt'),
  bare: document.querySelector('#bare'),
  skipGitRepoCheck: document.querySelector('#skipGitRepoCheck'),
  runForm: document.querySelector('#runForm'),
  followUpForm: document.querySelector('#followUpForm'),
  followUpPrompt: document.querySelector('#followUpPrompt'),
  providers: document.querySelector('#providers'),
  runs: document.querySelector('#runs'),
  selectedRun: document.querySelector('#selectedRun'),
  selectedMeta: document.querySelector('#selectedMeta'),
  eventStream: document.querySelector('#eventStream'),
  refreshProviders: document.querySelector('#refreshProviders'),
  refreshRuns: document.querySelector('#refreshRuns'),
  allowedToolsWrap: document.querySelector('#allowedToolsWrap'),
  systemPromptWrap: document.querySelector('#systemPromptWrap'),
  bareWrap: document.querySelector('#bareWrap'),
  skipGitRepoCheckWrap: document.querySelector('#skipGitRepoCheckWrap'),
}

function providerLabel(provider) {
  return provider === 'claude' ? 'Claude Code' : 'Codex CLI'
}

function escapeHtml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
}

function setProviderModeOptions(provider) {
  const modes = providerModes[provider] || []
  elements.runMode.innerHTML = modes
    .map((mode) => `<option value="${mode.value}">${mode.label}</option>`)
    .join('')
}

function syncProviderFields() {
  const provider = elements.provider.value
  setProviderModeOptions(provider)

  const isClaude = provider === 'claude'
  elements.allowedToolsWrap.classList.toggle('hidden', !isClaude)
  elements.systemPromptWrap.classList.toggle('hidden', !isClaude)
  elements.bareWrap.classList.toggle('hidden', !isClaude)
  elements.skipGitRepoCheckWrap.classList.toggle('hidden', isClaude)
}

function reportError(error) {
  const message = error instanceof Error ? error.message : String(error)
  window.alert(message)
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, {
    headers: {
      'Content-Type': 'application/json',
      ...(options.headers || {}),
    },
    ...options,
  })

  const payload = await response.json()
  if (!response.ok) {
    throw new Error(payload.error || `Request failed: ${response.status}`)
  }

  return payload
}

async function loadProviders() {
  const payload = await fetchJson('/api/providers')
  state.providers = payload.providers

  elements.providers.innerHTML = Object.values(payload.providers)
    .map((provider) => {
      const tone = provider.available ? 'ok' : 'bad'
      const label = provider.available ? 'Available' : 'Missing'
      return `
        <article class="provider-card">
          <div class="provider-row">
            <h3>${provider.label}</h3>
            <span class="pill ${tone}">${label}</span>
          </div>
          <p class="provider-version">${escapeHtml(provider.version || 'Not detected')}</p>
          <p class="provider-detail">${escapeHtml(provider.details || 'No details.')}</p>
        </article>
      `
    })
    .join('')
}

function renderRuns() {
  if (!state.runs.length) {
    elements.runs.textContent = 'No runs yet.'
    elements.runs.classList.add('empty-state')
    return
  }

  elements.runs.classList.remove('empty-state')
  elements.runs.innerHTML = state.runs
    .map((run) => {
      const selected = run.id === state.selectedRunId ? 'selected' : ''
      const tone =
        run.status === 'completed' ? 'ok' : run.status === 'failed' ? 'bad' : 'warn'
      const resume = run.resumeId
        ? `<span class="pill ok">resume ${escapeHtml(run.resumeId)}</span>`
        : ''
      const branch = run.git?.branch
        ? `<span class="meta-line">${escapeHtml(run.git.branch)}</span>`
        : ''

      return `
        <button class="run-card ${selected}" data-run-id="${run.id}" type="button">
          <div class="provider-row">
            <strong>${providerLabel(run.provider)}</strong>
            <span class="pill ${tone}">${escapeHtml(run.status)}</span>
          </div>
          <div class="meta-line">${escapeHtml(run.cwd)}</div>
          ${branch}
          <div class="meta-line">${escapeHtml(run.prompt)}</div>
          <div class="run-card-footer">
            <span>${new Date(run.createdAt).toLocaleString()}</span>
            ${resume}
          </div>
        </button>
      `
    })
    .join('')

  for (const button of elements.runs.querySelectorAll('[data-run-id]')) {
    button.addEventListener('click', () => {
      void selectRun(button.dataset.runId)
    })
  }
}

function formatEvent(event) {
  if (event.kind === 'snapshot') {
    return `SNAPSHOT\n${JSON.stringify(event.run, null, 2)}`
  }

  if (event.kind === 'status') {
    return `[${event.at}] STATUS ${event.status}\n${JSON.stringify(event, null, 2)}`
  }

  if (event.kind === 'json') {
    return `[${event.at}] ${event.stream.toUpperCase()}\n${JSON.stringify(event.payload, null, 2)}`
  }

  return `[${event.at}] ${event.stream.toUpperCase()}\n${event.text}`
}

function renderSelectedRun(run) {
  if (!run) {
    elements.selectedMeta.textContent = 'None selected'
    elements.selectedRun.textContent =
      'Pick a run to inspect its event stream and launch a follow-up turn.'
    elements.selectedRun.classList.add('empty-state')
    elements.followUpForm.classList.add('hidden')
    elements.eventStream.textContent = 'No stream selected.'
    elements.eventStream.classList.add('empty-state')
    return
  }

  elements.selectedMeta.textContent = `${providerLabel(run.provider)} · ${run.status}`
  elements.selectedRun.classList.remove('empty-state')
  elements.selectedRun.innerHTML = `
    <dl class="detail-list">
      <div><dt>Path</dt><dd>${escapeHtml(run.cwd)}</dd></div>
      <div><dt>Mode</dt><dd>${escapeHtml(run.mode)}</dd></div>
      <div><dt>Run mode</dt><dd>${escapeHtml(run.runMode || 'default')}</dd></div>
      <div><dt>Resume token</dt><dd>${escapeHtml(run.resumeId || 'None')}</dd></div>
      <div><dt>Branch</dt><dd>${escapeHtml(run.git?.branch || 'N/A')}</dd></div>
      <div><dt>Git root</dt><dd>${escapeHtml(run.git?.gitRoot || 'Not a git repo')}</dd></div>
      <div><dt>Command</dt><dd>${escapeHtml([run.command, ...(run.args || [])].join(' '))}</dd></div>
      <div><dt>Latest text</dt><dd>${escapeHtml(run.latestText || 'None')}</dd></div>
      <div><dt>Latest error</dt><dd>${escapeHtml(run.latestError || 'None')}</dd></div>
    </dl>
  `

  elements.followUpForm.classList.toggle('hidden', !run.resumeId)
  elements.eventStream.classList.remove('empty-state')
  elements.eventStream.textContent = (run.events || []).map(formatEvent).join('\n\n')
  elements.eventStream.scrollTop = elements.eventStream.scrollHeight
}

async function loadRuns() {
  const payload = await fetchJson('/api/runs')
  state.runs = payload.runs
  renderRuns()

  if (state.selectedRunId) {
    const selected = state.runs.find((run) => run.id === state.selectedRunId)
    if (selected) {
      await selectRun(selected.id, { refreshOnly: true })
      return
    }
  }

  renderSelectedRun(null)
}

async function selectRun(runId, options = {}) {
  state.selectedRunId = runId
  const payload = await fetchJson(`/api/runs/${runId}`)
  const run = payload.run
  const index = state.runs.findIndex((candidate) => candidate.id === run.id)

  if (index >= 0) {
    state.runs[index] = run
  } else {
    state.runs.unshift(run)
  }

  renderRuns()
  renderSelectedRun(run)

  elements.provider.value = run.provider
  syncProviderFields()
  elements.cwd.value = run.cwd
  elements.model.value = run.model || ''
  if ([...elements.runMode.options].some((option) => option.value === run.runMode)) {
    elements.runMode.value = run.runMode
  }

  if (!options.refreshOnly) {
    elements.followUpPrompt.value = ''
  }

  if (state.stream) {
    state.stream.close()
  }

  state.stream = new EventSource(`/api/runs/${runId}/stream`)
  state.stream.onmessage = (message) => {
    const event = JSON.parse(message.data)
    if (event.kind === 'snapshot') {
      renderSelectedRun(event.run)
      return
    }

    const selected = state.runs.find((candidate) => candidate.id === runId)
    if (!selected) {
      return
    }

    selected.events = selected.events || []
    selected.events.push(event)
    if (selected.events.length > 400) {
      selected.events.shift()
    }

    if (event.kind === 'status') {
      selected.status = event.status
      selected.exitCode = event.exitCode ?? selected.exitCode
      if (event.resumeId) {
        selected.resumeId = event.resumeId
      }
    }

    if (event.kind === 'json') {
      const payloadValue = event.payload || {}
      const maybeResume =
        payloadValue.session_id ||
        payloadValue.sessionId ||
        payloadValue.thread_id ||
        payloadValue.threadId

      if (typeof maybeResume === 'string' && maybeResume) {
        selected.resumeId = maybeResume
      }
    }

    renderRuns()
    renderSelectedRun(selected)
  }
}

async function startRun(body) {
  const payload = await fetchJson('/api/runs', {
    method: 'POST',
    body: JSON.stringify(body),
  })

  const run = payload.run
  state.runs.unshift(run)
  renderRuns()
  await selectRun(run.id)
}

elements.provider.addEventListener('change', syncProviderFields)

elements.runForm.addEventListener('submit', async (event) => {
  event.preventDefault()
  try {
    await startRun({
      provider: elements.provider.value,
      cwd: elements.cwd.value,
      model: elements.model.value,
      runMode: elements.runMode.value,
      prompt: elements.prompt.value,
      allowedTools: elements.allowedTools.value,
      systemPrompt: elements.systemPrompt.value,
      bare: elements.bare.checked,
      skipGitRepoCheck: elements.skipGitRepoCheck.checked,
    })
  } catch (error) {
    reportError(error)
  }
})

elements.followUpForm.addEventListener('submit', async (event) => {
  event.preventDefault()
  const selected = state.runs.find((run) => run.id === state.selectedRunId)
  if (!selected || !selected.resumeId) {
    return
  }

  try {
    await startRun({
      provider: selected.provider,
      cwd: selected.cwd,
      model: selected.model,
      runMode: selected.runMode,
      prompt: elements.followUpPrompt.value,
      allowedTools: elements.allowedTools.value,
      systemPrompt: elements.systemPrompt.value,
      bare: elements.bare.checked,
      skipGitRepoCheck: elements.skipGitRepoCheck.checked,
      resumeId: selected.resumeId,
    })
  } catch (error) {
    reportError(error)
  }
})

elements.refreshProviders.addEventListener('click', () => {
  void loadProviders()
})

elements.refreshRuns.addEventListener('click', () => {
  void loadRuns()
})

syncProviderFields()
elements.cwd.value = 'e:\\Claude Code Projects\\Personal\\Project Commander'
elements.prompt.value =
  'Inspect the repository root, summarize the top-level folders, and stop.'

try {
  await loadProviders()
  await loadRuns()
} catch (error) {
  reportError(error)
}
