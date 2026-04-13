import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import {
  createDiagnosticsCorrelationId,
  recordDiagnosticsEntry,
  summarizeDiagnosticsValue,
} from '@/diagnostics'

const SLOW_TAURI_INVOKE_MS = 500

function summarizeInvokeResult(value: unknown) {
  if (Array.isArray(value)) {
    return `Array(${value.length})`
  }

  if (value && typeof value === 'object') {
    return summarizeDiagnosticsValue(value)
  }

  return summarizeDiagnosticsValue(value)
}

export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const startedAt = performance.now()
  const invokeId = createDiagnosticsCorrelationId('invoke')

  try {
    const result = await tauriInvoke<T>(command, args)
    const durationMs = performance.now() - startedAt

    recordDiagnosticsEntry({
      event: 'tauri.invoke',
      source: 'invoke',
      severity: durationMs >= SLOW_TAURI_INVOKE_MS ? 'warn' : 'info',
      summary: `${command} completed`,
      durationMs,
      metadata: {
        invokeId,
        command,
        status: 'ok',
        args,
        result: summarizeInvokeResult(result),
      },
    })

    return result
  } catch (error) {
    const durationMs = performance.now() - startedAt

    recordDiagnosticsEntry({
      event: 'tauri.invoke',
      source: 'invoke',
      severity: 'error',
      summary: `${command} failed`,
      durationMs,
      metadata: {
        invokeId,
        command,
        status: 'error',
        args,
        error,
      },
    })

    throw error
  }
}
