import { Snapshot, STATUS_COLOR, taskStatusCounts, TaskStatus } from '../domain'
import { ApprovalDecision, renderTaskList } from './TaskList'
import { MountedView } from './FloatingIsland'
import { RESET_FORECAST_SOURCE_URL, RESET_FORECAST_STALE_AFTER_MS, ResetForecast } from '../resetForecast'
import { Language, getLanguage, t, toggleLanguage } from '../i18n'

function formatRecentTime(epoch?: number, language: Language = getLanguage()) { return epoch ? new Date(epoch * 1000).toLocaleString(language, { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false }) : '--' }
function formatResetTime(epoch?: number, language: Language = getLanguage()) { return epoch ? new Date(epoch * 1000).toLocaleString(language, { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }) : '--' }
function formatResetDate(epoch?: number, language: Language = getLanguage()) { return epoch ? new Date(epoch * 1000).toLocaleString(language, { month: 'numeric', day: 'numeric' }) : '--' }
function tokenLabel(snapshot: Snapshot) { const now = new Date(); const date = snapshot.usageDate?.slice(5) ?? `${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`; return `Token · ${date}` }
function value(value: unknown) { return value === undefined || value === null ? '--' : String(value) }
function isPlusPlan(snapshot: Snapshot) { return snapshot.plan?.trim().toLowerCase() === 'plus' }
function quotaPercent(snapshot: Snapshot) { return isPlusPlan(snapshot) ? snapshot.fiveHourRemainingPercent : snapshot.quotaRemainingPercent }
function escapeHtml(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }
function taskSignature(snapshot: Snapshot) { return snapshot.tasks.map((task) => `${task.id}:${task.status}:${task.acknowledged}:${task.title}:${task.activity ?? ''}:${task.waitingReason ?? ''}:${task.approvalRequestId ?? ''}:${task.updatedAt}:${task.tokenCount ?? ''}`).join('|') }
function sourceLabel(source: Snapshot['source'], language: Language = getLanguage()) {
  const labels: Record<string, string> = {
    'app-server-event': t('sourceRealtime', language),
    'task-watch': t('sourceTaskWatch', language),
    'permission-hook': t('sourceApprovalWatch', language),
    'metrics-poll': t('sourceMetricsPoll', language),
    'full-poll': t('sourceFullCalibration', language),
    'manual-refresh': t('sourceManualRefresh', language),
    'local-cache': t('sourceLocalCache', language),
  }
  return source ? labels[source] ?? '' : ''
}
function formatTokens(tokens?: number) {
  if (tokens === undefined) return '--'
  if (tokens < 10_000) return String(tokens)
  if (tokens >= 100_000_000) {
    const value = tokens / 100_000_000
    return `${Number(value.toFixed(tokens >= 1_000_000_000 ? 0 : 1))}亿`
  }
  const value = tokens / 10_000
  return `${Number(value.toFixed(tokens >= 1_000_000 ? 0 : 1))}万`
}

function formatForecastElapsed(hours?: number, language: Language = getLanguage()) {
  if (hours === undefined) return '--'
  const totalMinutes = Math.max(0, Math.round(hours * 60))
  const days = Math.floor(totalMinutes / (24 * 60))
  const remainingHours = Math.floor((totalMinutes % (24 * 60)) / 60)
  const minutes = totalMinutes % 60
  if (language === 'en-US') {
    if (days > 0) return `${days}${t('forecastDays', language)} ${remainingHours}${t('forecastHours', language)}`
    if (remainingHours > 0) return `${remainingHours}${t('forecastHours', language)} ${minutes}${t('forecastMinutes', language)}`
    return `${minutes}${t('forecastMinutes', language)}`
  }
  if (days > 0) return `${days}天${remainingHours}小时`
  if (remainingHours > 0) return `${remainingHours}小时${minutes}分`
  return `${minutes}分`
}

function formatForecastReset(value?: string, language: Language = getLanguage()) {
  if (!value) return '--'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? '--' : date.toLocaleString(language, { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false })
}

function renderResetForecast(root: HTMLElement, forecast: ResetForecast, language: Language = getLanguage()) {
  const stale = forecast.status === 'stale' || (forecast.status === 'fresh' && forecast.fetchedAt !== undefined && Date.now() - forecast.fetchedAt > RESET_FORECAST_STALE_AFTER_MS)
  const statusText = stale ? t('forecastStale', language) : forecast.status === 'fresh' ? t('updated', language) : forecast.status === 'loading' ? t('loading', language) : t('unavailable', language)
  const metrics = forecast.status === 'fresh' || forecast.status === 'stale'
    ? `<div><small>${t('probability24h', language)}</small><strong>${forecast.probability24h === undefined ? '--' : `${Math.round(forecast.probability24h * 100)}%`}</strong></div>
       <div><small>${t('sinceLastReset', language)}</small><strong>${formatForecastElapsed(forecast.elapsedHours, language)}</strong></div>
       <div><small>${t('resets30d', language)}</small><strong>${forecast.resets30d === undefined ? '--' : `${forecast.resets30d}${t('resetCountSuffix', language)}`}</strong></div>
       <div><small>${t('averageWait', language)}</small><strong>${forecast.averageWaitDays === undefined ? '--' : `${forecast.averageWaitDays}${t('forecastDays', language)}`}</strong></div>
       <div><small>${t('latestReset', language)}</small><strong>${formatForecastReset(forecast.lastResetAt, language)}</strong></div>`
    : `<p class="reset-forecast-empty">${forecast.status === 'loading' ? t('readingForecast', language) : (forecast.error ?? t('forecastUnavailable', language))}</p>`
  root.dataset.status = stale ? 'stale' : forecast.status
  const updatedAt = forecast.fetchedAt ? `${t('forecastUpdatedAt', language)} ${formatRecentTime(Math.floor(forecast.fetchedAt / 1000), language)}` : ''
  root.innerHTML = `<div class="reset-forecast-head"><div><strong>${t('forecast', language)}</strong><small>${t('publicData', language)}${updatedAt ? ` · ${updatedAt}` : ''}</small></div><span class="reset-forecast-status">${statusText}</span></div><div class="reset-forecast-grid">${metrics}</div><div class="reset-forecast-source"><a href="${RESET_FORECAST_SOURCE_URL}" target="_blank" rel="noreferrer">${t('source', language)}</a><a class="reset-forecast-evidence" href="${RESET_FORECAST_SOURCE_URL}" target="_blank" rel="noreferrer">${t('evidence', language)}</a></div>`
}
const HISTORY_DAYS = 7
const HOURS_PER_DAY = 24
type HistoryView = 'current' | 'previous'

function historyHourKey(date: Date) {
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}-${date.getHours()}`
}

function renderHistory(root: HTMLElement, snapshot: Snapshot, view: HistoryView, language: Language = getLanguage()) {
  const now = new Date()
  const today = new Date(now)
  today.setHours(0, 0, 0, 0)
  const source = view === 'previous' ? snapshot.previousHistory : snapshot.history
  const sorted = [...source]
    .filter((point) => point.quotaRemainingPercent !== undefined)
    .sort((left, right) => left.at - right.at)
  const points = new Map<string, { at: number; quota: number }>()
  for (const point of sorted) {
    const date = new Date(point.at * 1000)
    const key = historyHourKey(date)
    const previous = points.get(key)
    if (!previous || point.at >= previous.at) points.set(key, { at: point.at, quota: point.quotaRemainingPercent! })
  }
  const chartPoints: Array<{ label: string; quota?: number; inferred: boolean; future?: boolean }> = []
  const values: number[] = []
  const cycleKey = view === 'previous' ? snapshot.previousHistoryCycleKey : snapshot.historyCycleKey
  // The cycle key is derived from the API's next reset boundary. Anchor the
  // visible seven-day window to the reset that started this cycle (next reset
  // minus one week); use the live reset only when no cycle key was persisted.
  const cycleReference = view === 'current' ? (cycleKey ?? snapshot.quotaResetsAt) : cycleKey
  const validCycleReference = cycleReference !== undefined && cycleReference > 1_000_000_000
  const cycleStart = validCycleReference
    ? new Date((cycleReference! - HISTORY_DAYS * HOURS_PER_DAY * 3600) * 1000)
    : (() => {
        const fallback = new Date(today)
        fallback.setDate(today.getDate() - (HISTORY_DAYS - 1))
        return fallback
      })()
  cycleStart.setMinutes(0, 0, 0)
  let carried: { at: number; quota: number } | undefined = view === 'current'
    ? { at: Math.floor(cycleStart.getTime() / 1000), quota: 100 }
    : undefined
  const nowAt = Date.now()
  for (let index = 0; index < HISTORY_DAYS * HOURS_PER_DAY; index += 1) {
    const cellDate = new Date(cycleStart.getTime() + index * 3600 * 1000)
    const label = `${cellDate.getMonth() + 1}-${cellDate.getDate()} ${String(cellDate.getHours()).padStart(2, '0')}:00`
    const point = points.get(historyHourKey(cellDate))
    const future = cellDate.getTime() > nowAt
    if (point && index === 0 && view === 'current') {
      chartPoints.push({ label, quota: 100, inferred: true })
    } else if (point && !future) {
      carried = point
      values.push(point.quota)
      chartPoints.push({ label, quota: point.quota, inferred: false })
    } else if (carried) {
      chartPoints.push({ label, quota: carried.quota, inferred: true, future })
    } else {
      chartPoints.push({ label, inferred: true })
    }
  }
  const first = values[0]
  const last = values.at(-1)
  const summaryLabel = values.length ? (view === 'previous' ? t('savedPrevious', language) : t('savedCurrent', language)) : (view === 'previous' ? t('noData', language) : t('noSuccessfulSample', language))
  const summaryStart = view === 'current' && (first === undefined || first !== 100) ? 100 : first
  const cycleLabel = view === 'previous' ? t('previousCycle', language) : t('currentCycle', language)
  const width = 1000
  const height = 150
  const plotTop = 18
  const plotBottom = 126
  const x = (index: number) => (index / (chartPoints.length - 1)) * width
  const y = (quota: number) => plotBottom - (Math.max(0, Math.min(100, quota)) / 100) * (plotBottom - plotTop)
  const lineSegments: Array<{ path: string; future: boolean }> = []
  let segment: string[] = []
  let segmentFuture = false
  const flushSegment = () => {
    if (segment.length > 1) lineSegments.push({ path: segment.join(' '), future: segmentFuture })
    segment = []
  }
  chartPoints.forEach((point, index) => {
    if (point.quota === undefined) {
      flushSegment()
      return
    }
    const future = point.future === true
    if (segment.length && future !== segmentFuture) flushSegment()
    segmentFuture = future
    segment.push(`${segment.length ? 'L' : 'M'} ${x(index).toFixed(2)} ${y(point.quota).toFixed(2)}`)
  })
  flushSegment()
  const dividers = Array.from({ length: HISTORY_DAYS + 1 }, (_, day) => {
    const index = Math.min(chartPoints.length - 1, day * HOURS_PER_DAY)
    return `<line class="history-day-divider" x1="${x(index).toFixed(2)}" y1="${plotTop}" x2="${x(index).toFixed(2)}" y2="${plotBottom}" />`
  }).join('')
  const dayLabels = Array.from({ length: HISTORY_DAYS }, (_, day) => {
    const index = day * HOURS_PER_DAY
    const point = chartPoints[index]
    return point ? `<text class="history-day-label" x="${(x(index) + 5).toFixed(2)}" y="${height - 5}">${escapeHtml(point.label.split(' ')[0])}</text>` : ''
  }).join('')
  const hoverPoints = chartPoints.map((point, index) => point.quota === undefined
    ? `<circle class="history-hover-point is-empty" cx="${x(index).toFixed(2)}" cy="${plotBottom}" r="3"><title>${escapeHtml(point.label)} · ${t('noSample', language)}</title></circle>`
    : `<circle class="history-hover-point${point.inferred ? ' is-inferred' : ''}${point.future ? ' is-future' : ''}" cx="${x(index).toFixed(2)}" cy="${y(point.quota).toFixed(2)}" r="${point.future ? 3 : 7}"><title>${escapeHtml(point.label)} · ${t('weeklyPrefix', language)} ${point.quota}%${point.inferred ? ` · ${t('carriedSample', language)}` : ''}</title></circle>`).join('')
  const sampledPoints = chartPoints.map((point, index) => point.quota === undefined || point.inferred ? '' : `<circle class="history-sample-point" cx="${x(index).toFixed(2)}" cy="${y(point.quota).toFixed(2)}" r="2.8" aria-hidden="true" />`).join('')
  const paths = lineSegments.map(({ path, future }) => `<path class="history-line${future ? ' is-future' : ''}" d="${path}" />`).join('')
  root.innerHTML = `<div class="history-summary"><span>${t('weeklyPrefix', language)} ${summaryStart ?? '--'}% → ${last ?? '--'}%</span><span>${summaryLabel}</span></div><div class="history-chart-wrap"><svg class="history-chart" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none" role="img" aria-label="${cycleLabel} ${t('weeklyQuotaTrend', language)}"><line class="history-baseline" x1="0" y1="${plotBottom}" x2="${width}" y2="${plotBottom}" />${dividers}<g class="history-day-labels">${dayLabels}</g><g class="history-lines">${paths}</g><g class="history-points">${sampledPoints}${hoverPoints}</g></svg></div>`
}

function renderTaskCounts(root: HTMLElement, snapshot: Snapshot, language: Language = getLanguage()) {
  const counts = taskStatusCounts(snapshot.tasks)
  const entries: Array<{ status: TaskStatus; label: string; count: number }> = [
    { status: 'needs_action', label: t('needsAction', language), count: counts.needs_action },
    { status: 'running', label: t('running', language), count: counts.running },
    { status: 'completed', label: t('readyForReview', language), count: counts.completed },
  ].filter((entry) => entry.count > 0)
  if (!entries.length) {
    root.innerHTML = `<span class="task-count task-count-empty"><i class="task-count-dot" style="--status-color:${STATUS_COLOR.none}"></i><b>0</b></span>`
    return
  }
  root.innerHTML = entries.map((entry) => `<span class="task-count" aria-label="${entry.label}${entry.count}"><i class="task-count-dot" style="--status-color:${STATUS_COLOR[entry.status]}"></i><b>${entry.count}</b></span>`).join('')
}

export interface MountedDetailsView extends MountedView {
  setPairingSettingsOpen(value: boolean): void
  setPairingInfo(value: { address: string; code: string }): void
  setResetForecast(value: ResetForecast): void
}

export function mountDetailsPanel(
  root: HTMLElement,
  onRefresh: () => void,
  onAcknowledge: (taskId: string) => void,
  onClose: () => void,
  pairingInfo?: { address: string; code: string },
  onAdvance?: () => void,
  pairingSettingsOpen = false,
  onTogglePairing?: () => void,
  onResetPairing?: () => void,
  onApproval?: (taskId: string, decision: ApprovalDecision) => void,
  onTestPairing?: () => Promise<boolean> | boolean,
  resetForecast: ResetForecast = { status: 'loading', sourceUrl: RESET_FORECAST_SOURCE_URL },
  onOpenSettings?: () => void,
): MountedDetailsView {
  root.innerHTML = `<section class="details-panel" aria-label="额度详情">
    <header class="details-drag-region" data-tauri-drag-region><div data-tauri-drag-region><small class="quota-status-label" data-tauri-drag-region>CODEX 额度状态</small><h1 class="details-title" data-tauri-drag-region></h1><small class="weekly-quota-note" data-tauri-drag-region></small></div><div class="details-actions"><button class="close-button language-toggle" type="button"></button><button class="close-button pairing-settings-button" type="button"></button><button class="close-button settings-button" type="button"></button></div></header>
    <div class="detail-grid"><div><small class="reset-label">下次重置</small><strong class="reset-value"></strong></div><div><small class="plan-label">当前套餐</small><strong class="plan-value"></strong></div><div><small class="credits-label">可用重置机会</small><strong class="credits-value"></strong></div><div><small class="token-detail-label"></small><strong class="tokens-value"></strong></div></div>
    <div class="freshness"></div>
    <section class="reset-forecast-card" aria-label="Codex 重置预测"></section>
    <div class="history-card" aria-label="近期趋势"><div class="history-head"><strong class="history-title">近期趋势</strong><div class="history-head-actions"><small class="history-window"></small><button class="history-toggle" type="button" hidden></button></div></div><div class="history-content"></div></div>
    <div class="pairing-settings" aria-label="配对设置" hidden></div>
    <div class="task-header"><strong class="task-header-label">任务状态</strong><span class="task-count"></span></div><ul class="task-list"></ul>
    <div class="refresh-feedback" aria-live="polite"></div><button class="refresh-button" type="button"></button>
  </section>`

  const panel = root.querySelector<HTMLElement>('.details-panel')!
  const title = root.querySelector<HTMLElement>('.details-title')!
  const quotaStatusLabel = root.querySelector<HTMLElement>('.quota-status-label')!
  const weeklyQuotaNote = root.querySelector<HTMLElement>('.weekly-quota-note')!
  const resetLabel = root.querySelector<HTMLElement>('.reset-label')!
  const resetValue = root.querySelector<HTMLElement>('.reset-value')!
  const planValue = root.querySelector<HTMLElement>('.plan-value')!
  const planLabel = root.querySelector<HTMLElement>('.plan-label')!
  const creditsValue = root.querySelector<HTMLElement>('.credits-value')!
  const creditsLabel = root.querySelector<HTMLElement>('.credits-label')!
  const tokenDetailLabel = root.querySelector<HTMLElement>('.token-detail-label')!
  const tokensValue = root.querySelector<HTMLElement>('.tokens-value')!
  const freshness = root.querySelector<HTMLElement>('.freshness')!
  const resetForecastCard = root.querySelector<HTMLElement>('.reset-forecast-card')!
  const historyWindow = root.querySelector<HTMLElement>('.history-window')!
  const historyToggle = root.querySelector<HTMLButtonElement>('.history-toggle')!
  const historyContent = root.querySelector<HTMLElement>('.history-content')!
  const historyCard = root.querySelector<HTMLElement>('.history-card')!
  const historyTitle = root.querySelector<HTMLElement>('.history-title')!
  const pairingSettings = root.querySelector<HTMLElement>('.pairing-settings')!
  const pairingButton = root.querySelector<HTMLButtonElement>('.pairing-settings-button')!
  const settingsButton = root.querySelector<HTMLButtonElement>('.settings-button')!
  const languageButton = root.querySelector<HTMLButtonElement>('.language-toggle')!
  const taskHeaderLabel = root.querySelector<HTMLElement>('.task-header-label')!
  const taskCount = root.querySelector<HTMLElement>('.task-count')!
  const taskList = root.querySelector<HTMLElement>('.task-list')!
  const refreshButton = root.querySelector<HTMLButtonElement>('.refresh-button')!
  const refreshFeedback = root.querySelector<HTMLElement>('.refresh-feedback')!
  let lastTaskSignature = ''
  let currentPairingOpen = pairingSettingsOpen
  let currentPairingInfo = pairingInfo
  let language = getLanguage()
  let historySnapshot: Snapshot | undefined
  let historyView: HistoryView = 'current'
  let currentResetForecast = resetForecast
  let latestStatus: Snapshot['status'] = 'error'
  let refreshRequested = false

  pairingButton.addEventListener('click', () => onTogglePairing?.())
  settingsButton.addEventListener('click', () => onOpenSettings?.())
  languageButton.addEventListener('click', () => toggleLanguage())
  historyToggle.addEventListener('click', () => {
    historyView = historyView === 'current' ? 'previous' : 'current'
    if (historySnapshot) {
      historyWindow.textContent = historyView === 'previous' ? t('previousCycle', language) : t('currentCycle', language)
      historyToggle.textContent = historyView === 'previous' ? t('backToCurrent', language) : t('viewPrevious', language)
      renderHistory(historyContent, historySnapshot, historyView, language)
    }
  })
  refreshButton.addEventListener('click', onRefresh)
  if (onAdvance) panel.addEventListener('dblclick', (event) => {
    if ((event.target as HTMLElement).closest('button')) return
    onAdvance()
  })

  const updatePairing = () => {
    pairingButton.textContent = t('pairing', language)
    pairingButton.setAttribute('aria-label', t('pairingSettings', language))
    pairingButton.setAttribute('aria-expanded', String(currentPairingOpen))
    pairingSettings.hidden = !currentPairingOpen
    pairingSettings.innerHTML = currentPairingOpen
      ? currentPairingInfo
        ? `<div class="pairing-card"><small>${t('sameWifiAddress', language)}</small><code>${escapeHtml(currentPairingInfo.address)}</code><small>${t('pairingHint', language)} <b>${escapeHtml(currentPairingInfo.code)}</b>${language === 'zh-CN' ? '，' : '. '}${t('pairingRemembered', language)}</small><div class="pairing-actions"><button class="pairing-copy-button" type="button">${t('copyPairingCode', language)}</button><button class="pairing-test-button" type="button">${t('testPairing', language)}</button><button class="pairing-reset-button" type="button">${t('resetPairing', language)}</button></div><small class="pairing-feedback" aria-live="polite"></small></div>`
        : `<div class="pairing-card"><small>${t('pairing', language)}</small><strong>${t('pairingUnavailable', language)}</strong></div>`
      : ''
  }
  const updateLanguageChrome = () => {
    panel.setAttribute('aria-label', t('quotaDetails', language))
    resetForecastCard.setAttribute('aria-label', t('forecast', language))
    quotaStatusLabel.textContent = t('quotaStatus', language)
    planLabel.textContent = t('currentPlan', language)
    creditsLabel.textContent = t('resetCredits', language)
    historyTitle.textContent = t('recentTrend', language)
    taskHeaderLabel.textContent = t('taskStatus', language)
    historyCard.setAttribute('aria-label', t('recentTrend', language))
    pairingSettings.setAttribute('aria-label', t('pairingSettings', language))
    settingsButton.textContent = t('settings', language)
    settingsButton.setAttribute('aria-label', t('settings', language))
    languageButton.textContent = language === 'zh-CN' ? 'EN' : '中'
    languageButton.setAttribute('aria-label', language === 'zh-CN' ? t('switchToEnglish', language) : t('switchToChinese', language))
    updatePairing()
    refreshButton.textContent = t('refreshData', language)
  }
  updatePairing()
  updateLanguageChrome()
  renderResetForecast(resetForecastCard, currentResetForecast)
  pairingSettings.addEventListener('click', async (event) => {
    const target = event.target as HTMLElement
    const feedback = pairingSettings.querySelector<HTMLElement>('.pairing-feedback')
    if (target.closest('.pairing-reset-button')) onResetPairing?.()
    if (target.closest('.pairing-copy-button') && currentPairingInfo) {
      try {
        await navigator.clipboard.writeText(currentPairingInfo.code)
        if (feedback) feedback.textContent = t('pairingCopied', language)
      } catch {
        if (feedback) feedback.textContent = t('pairingTestFailed', language)
      }
    }
    const testButton = target.closest<HTMLButtonElement>('.pairing-test-button')
    if (testButton) {
      testButton.disabled = true
      if (feedback) feedback.textContent = t('testingPairing', language)
      try {
        const ok = await onTestPairing?.()
        if (feedback) feedback.textContent = ok === false ? t('pairingTestFailed', language) : t('pairingTestSuccess', language)
      } catch {
        if (feedback) feedback.textContent = t('pairingTestFailed', language)
      } finally {
        testButton.disabled = false
      }
    }
  })

  return {
    update(snapshot) {
      latestStatus = snapshot.status
      const plus = isPlusPlan(snapshot)
      const percent = quotaPercent(snapshot)
      title.textContent = `${plus ? t('fiveHourRemaining', language) : t('weeklyRemaining', language)} ${value(percent)}${percent === undefined ? '' : '%'}`
      resetLabel.textContent = plus ? t('fiveHourReset', language) : t('nextReset', language)
      resetValue.textContent = formatResetTime(plus ? snapshot.fiveHourResetsAt : snapshot.quotaResetsAt, language)
      weeklyQuotaNote.textContent = plus ? `${t('weeklyRemaining', language)} ${value(snapshot.quotaRemainingPercent)}${snapshot.quotaRemainingPercent === undefined ? '' : '%'}, ${t('quotaResetAt', language)} ${formatResetDate(snapshot.quotaResetsAt, language)}` : ''
      planValue.textContent = value(snapshot.plan)
      creditsValue.textContent = value(snapshot.resetCredits)
      tokenDetailLabel.textContent = tokenLabel(snapshot)
      tokensValue.textContent = formatTokens(snapshot.todayTokens)
      freshness.className = `freshness ${snapshot.status}`
      const source = sourceLabel(snapshot.source, language)
      freshness.textContent = snapshot.status === 'fresh'
        ? `${t('connected', language)} · ${t('recentlyUpdated', language)} ${formatRecentTime(snapshot.fetchedAt, language)}${source ? ` · ${source}` : ''}`
        : snapshot.status === 'stale'
          ? `${snapshot.error ?? t('dataExpired', language)} · ${t('reconnecting', language)} · ${t('keepLastData', language)} · ${t('lastSuccess', language)} ${formatRecentTime(snapshot.fetchedAt, language)}${source ? ` · ${source}` : ''}`
          : `${snapshot.error ?? t('temporarilyUnavailable', language)} · ${t('keepLastData', language)} · ${t('lastSuccess', language)} ${formatRecentTime(snapshot.fetchedAt, language)}`
      renderResetForecast(resetForecastCard, currentResetForecast, language)
      const signature = taskSignature(snapshot)
      if (signature !== lastTaskSignature) {
        renderTaskList(taskList, snapshot.tasks, onAcknowledge, onApproval, language)
        lastTaskSignature = signature
      }
      renderTaskCounts(taskCount, snapshot, language)
      historySnapshot = snapshot
      historyWindow.textContent = historyView === 'previous' ? t('previousCycle', language) : t('currentCycle', language)
      historyToggle.hidden = false
      historyToggle.textContent = historyView === 'previous' ? t('backToCurrent', language) : t('viewPrevious', language)
      renderHistory(historyContent, snapshot, historyView, language)
    },
    setRefreshing(value) {
      if (value) {
        refreshRequested = true
        refreshFeedback.dataset.state = 'loading'
        refreshFeedback.textContent = t('refreshing', language)
      } else if (refreshRequested) {
        refreshFeedback.dataset.state = latestStatus === 'fresh' ? 'success' : 'error'
        refreshFeedback.textContent = latestStatus === 'fresh' ? t('refreshSuccess', language) : t('refreshFailure', language)
        refreshRequested = false
      }
      refreshButton.disabled = value
      refreshButton.classList.toggle('is-refreshing', value)
      refreshButton.textContent = value ? t('refreshing', language) : t('refreshData', language)
    },
    setPairingSettingsOpen(value) {
      currentPairingOpen = value
      updatePairing()
    },
    setPairingInfo(value) {
      currentPairingInfo = value
      updatePairing()
    },
    setResetForecast(value) {
      currentResetForecast = value
      renderResetForecast(resetForecastCard, value, language)
    },
    setLanguage(value) {
      language = value
      lastTaskSignature = ''
      updateLanguageChrome()
      if (historySnapshot) this.update(historySnapshot)
      else renderResetForecast(resetForecastCard, currentResetForecast, language)
    },
    destroy() { root.replaceChildren() },
  }
}

export function renderDetailsPanel(root: HTMLElement, snapshot: Snapshot, onRefresh: () => void, onAcknowledge: (taskId: string) => void, onClose: () => void, pairingInfo?: { address: string; code: string }, isRefreshing = false, onAdvance?: () => void, pairingSettingsOpen = false, onTogglePairing?: () => void, onResetPairing?: () => void, onApproval?: (taskId: string, decision: ApprovalDecision) => void, onTestPairing?: () => Promise<boolean> | boolean, resetForecast?: ResetForecast, onOpenSettings?: () => void): void {
  const mounted = mountDetailsPanel(root, onRefresh, onAcknowledge, onClose, pairingInfo, onAdvance, pairingSettingsOpen, onTogglePairing, onResetPairing, onApproval, onTestPairing, resetForecast, onOpenSettings)
  mounted.update(snapshot)
  mounted.setRefreshing(isRefreshing)
}
