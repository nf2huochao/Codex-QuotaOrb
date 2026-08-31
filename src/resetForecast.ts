export const RESET_FORECAST_SOURCE_URL = 'https://codex.lunarwerx.com/'

const RESET_FORECAST_ENDPOINT = 'https://codex.lunarwerx.com/cnx/aireset/summary/t/'
const REQUEST_TIMEOUT_MS = 20_000

export interface ResetForecast {
  status: 'loading' | 'fresh' | 'error'
  sourceUrl: string
  fetchedAt?: number
  probability24h?: number
  elapsedHours?: number
  resets30d?: number
  averageWaitDays?: number
  lastResetAt?: string
  error?: string
}

function finite(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

export async function fetchResetForecast(): Promise<ResetForecast> {
  const result: ResetForecast = { status: 'loading', sourceUrl: RESET_FORECAST_SOURCE_URL }
  const bucket = Math.floor(Date.now() / 300_000)
  const controller = new AbortController()
  const timeout = window.setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS)
  try {
    const response = await fetch(`${RESET_FORECAST_ENDPOINT}${bucket}`, {
      cache: 'no-store',
      signal: controller.signal,
    })
    if (!response.ok) throw new Error(`HTTP ${response.status}`)
    const raw = await response.json() as Record<string, unknown>
    const stats = raw.stats && typeof raw.stats === 'object' ? raw.stats as Record<string, unknown> : {}
    return {
      status: 'fresh',
      sourceUrl: RESET_FORECAST_SOURCE_URL,
      fetchedAt: Date.now(),
      probability24h: finite(raw.chanceToday),
      elapsedHours: finite(raw.hoursSinceLast),
      resets30d: finite(stats.last30Days),
      averageWaitDays: finite(stats.meanGapDays),
      lastResetAt: typeof raw.lastReset === 'string' ? raw.lastReset : undefined,
    }
  } catch (error) {
    return {
      ...result,
      status: 'error',
      error: error instanceof DOMException && error.name === 'AbortError' ? '读取超时' : '暂时无法获取',
    }
  } finally {
    window.clearTimeout(timeout)
  }
}
