import { Snapshot, STATUS_COLOR, taskStatusCounts, TaskStatus } from '../domain'
import { ApprovalDecision, renderTaskList } from './TaskList'
import { MountedView } from './FloatingIsland'

function formatRecentTime(epoch?: number) { return epoch ? new Date(epoch * 1000).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false }) : '--' }
function formatResetTime(epoch?: number) { return epoch ? new Date(epoch * 1000).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }) : '--' }
function formatResetDate(epoch?: number) { return epoch ? new Date(epoch * 1000).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric' }) : '--' }
function value(value: unknown) { return value === undefined || value === null ? '--' : String(value) }
function isPlusPlan(snapshot: Snapshot) { return snapshot.plan?.trim().toLowerCase() === 'plus' }
function quotaPercent(snapshot: Snapshot) { return isPlusPlan(snapshot) ? snapshot.fiveHourRemainingPercent : snapshot.quotaRemainingPercent }
function escapeHtml(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }
function taskSignature(snapshot: Snapshot) { return snapshot.tasks.map((task) => `${task.id}:${task.status}:${task.acknowledged}:${task.title}:${task.activity ?? ''}:${task.waitingReason ?? ''}:${task.approvalRequestId ?? ''}:${task.updatedAt}:${task.tokenCount ?? ''}`).join('|') }
function sourceLabel(source?: Snapshot['source']) { return source === 'app-server-event' ? '实时事件' : source === 'task-watch' ? '任务监听' : source === 'permission-hook' ? '批准监听' : source === 'metrics-poll' ? '指标轮询' : source === 'full-poll' ? '完整校准' : source === 'manual-refresh' ? '手动刷新' : source === 'local-cache' ? '本地缓存' : '' }
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
function historyWindowLabel(hours: number) { return hours <= 24 ? '当天采样' : `最近 ${Math.ceil(hours / 24)} 天` }
function renderHistory(root: HTMLElement, snapshot: Snapshot, visibleHours = 24) {
  const sorted = [...snapshot.history].sort((left, right) => left.at - right.at)
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  const slots = visibleHours <= 24 ? sorted.filter((point) => point.at >= Math.floor(today.getTime() / 1000)) : sorted.slice(-visibleHours)
  const values = slots.map((point) => point?.quotaRemainingPercent ?? 0)
  const max = Math.max(1, ...values)
  const first = slots.find((point) => point?.quotaRemainingPercent !== undefined)?.quotaRemainingPercent
  const last = [...slots].reverse().find((point) => point?.quotaRemainingPercent !== undefined)?.quotaRemainingPercent
  const bars = slots.length ? slots.map((point, index) => {
    const date = new Date(point.at * 1000)
    const label = `${date.getMonth() + 1}/${date.getDate()} ${String(date.getHours()).padStart(2, '0')}:00`
    return `<i class="history-slot has-value" style="height:${Math.max(8, (values[index] / max) * 100)}%" title="${label} · ${point.quotaRemainingPercent ?? '--'}%"></i>`
  }).join('') : Array.from({ length: 24 }, (_, index) => `<i class="history-slot" style="height:4%" title="${index}:00 · 暂无数据"></i>`).join('')
  root.innerHTML = `<div class="history-summary"><span>周额度 ${first ?? '--'}% → ${last ?? '--'}%</span><span>${snapshot.history.length ? visibleHours <= 24 ? '今日已采样' : '已保存最近 7 天' : '暂无成功采样'}</span></div><div class="history-scroll"><div class="history-track" aria-label="${historyWindowLabel(visibleHours)}每小时周额度趋势">${bars}</div></div>`
}

function renderTaskCounts(root: HTMLElement, snapshot: Snapshot) {
  const counts = taskStatusCounts(snapshot.tasks)
  const entries: Array<{ status: TaskStatus; label: string; count: number }> = [
    { status: 'needs_action', label: '红', count: counts.needs_action },
    { status: 'running', label: '黄', count: counts.running },
    { status: 'completed', label: '绿', count: counts.completed },
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
): MountedDetailsView {
  root.innerHTML = `<section class="details-panel" aria-label="额度详情">
    <header class="details-drag-region" data-tauri-drag-region><div data-tauri-drag-region><small data-tauri-drag-region>CODEX 额度状态</small><h1 class="details-title" data-tauri-drag-region></h1><small class="weekly-quota-note" data-tauri-drag-region></small></div><button class="close-button pairing-settings-button" type="button" aria-label="配对设置"></button></header>
    <div class="detail-grid"><div><small class="reset-label">下次重置</small><strong class="reset-value"></strong></div><div><small>当前套餐</small><strong class="plan-value"></strong></div><div><small>可用重置机会</small><strong class="credits-value"></strong></div><div><small class="token-detail-label"></small><strong class="tokens-value"></strong></div></div>
    <div class="freshness"></div>
    <div class="history-card" aria-label="近期趋势"><div class="history-head"><strong>近期趋势</strong><small class="history-window"></small></div><div class="history-content"></div></div>
    <div class="pairing-settings" aria-label="配对设置" hidden></div>
    <div class="task-header"><strong>任务状态</strong><span class="task-count"></span></div><ul class="task-list"></ul>
    <button class="refresh-button" type="button"></button>
  </section>`

  const panel = root.querySelector<HTMLElement>('.details-panel')!
  const title = root.querySelector<HTMLElement>('.details-title')!
  const weeklyQuotaNote = root.querySelector<HTMLElement>('.weekly-quota-note')!
  const resetLabel = root.querySelector<HTMLElement>('.reset-label')!
  const resetValue = root.querySelector<HTMLElement>('.reset-value')!
  const planValue = root.querySelector<HTMLElement>('.plan-value')!
  const creditsValue = root.querySelector<HTMLElement>('.credits-value')!
  const tokenDetailLabel = root.querySelector<HTMLElement>('.token-detail-label')!
  const tokensValue = root.querySelector<HTMLElement>('.tokens-value')!
  const freshness = root.querySelector<HTMLElement>('.freshness')!
  const historyWindow = root.querySelector<HTMLElement>('.history-window')!
  const historyContent = root.querySelector<HTMLElement>('.history-content')!
  const historyCard = root.querySelector<HTMLElement>('.history-card')!
  const pairingSettings = root.querySelector<HTMLElement>('.pairing-settings')!
  const pairingButton = root.querySelector<HTMLButtonElement>('.pairing-settings-button')!
  const taskCount = root.querySelector<HTMLElement>('.task-count')!
  const taskList = root.querySelector<HTMLElement>('.task-list')!
  const refreshButton = root.querySelector<HTMLButtonElement>('.refresh-button')!
  let lastTaskSignature = ''
  let currentPairingOpen = pairingSettingsOpen
  let currentPairingInfo = pairingInfo
  let visibleHistoryHours = 24
  let historySnapshot: Snapshot | undefined

  pairingButton.addEventListener('click', () => onTogglePairing?.())
  refreshButton.addEventListener('click', onRefresh)
  historyCard.addEventListener('wheel', (event) => {
    const direction = event.deltaY < 0 ? 1 : -1
    const next = Math.max(24, Math.min(168, visibleHistoryHours + direction * 24))
    if (next === visibleHistoryHours) return
    event.preventDefault()
    visibleHistoryHours = next
    if (historySnapshot) {
      historyWindow.textContent = historyWindowLabel(visibleHistoryHours)
      renderHistory(historyContent, historySnapshot, visibleHistoryHours)
    }
  }, { passive: false })
  if (onAdvance) panel.addEventListener('dblclick', (event) => {
    if ((event.target as HTMLElement).closest('button')) return
    onAdvance()
  })

  const updatePairing = () => {
    pairingButton.textContent = '配对设置'
    pairingButton.setAttribute('aria-expanded', String(currentPairingOpen))
    pairingSettings.hidden = !currentPairingOpen
    pairingSettings.innerHTML = currentPairingOpen
      ? currentPairingInfo
        ? `<div class="pairing-card"><small>同一 Wi‑Fi 网页地址</small><code>${escapeHtml(currentPairingInfo.address)}</code><small>首次打开网页输入四位配对码 <b>${escapeHtml(currentPairingInfo.code)}</b>，以后会自动记住。</small><button class="pairing-reset-button" type="button">重置配对</button></div>`
        : '<div class="pairing-card"><small>配对设置</small><strong>桌面端启动后生成本机配对地址</strong></div>'
      : ''
  }
  updatePairing()
  pairingSettings.addEventListener('click', (event) => {
    if ((event.target as HTMLElement).closest('.pairing-reset-button')) onResetPairing?.()
  })

  return {
    update(snapshot) {
      const plus = isPlusPlan(snapshot)
      const percent = quotaPercent(snapshot)
      title.textContent = `${plus ? '5小时额度剩余' : '本周剩余'} ${value(percent)}${percent === undefined ? '' : '%'}`
      resetLabel.textContent = plus ? '5小时额度重置时间' : '下次重置'
      resetValue.textContent = formatResetTime(plus ? snapshot.fiveHourResetsAt : snapshot.quotaResetsAt)
      weeklyQuotaNote.textContent = plus ? `本周剩余 ${value(snapshot.quotaRemainingPercent)}${snapshot.quotaRemainingPercent === undefined ? '' : '%'}，重置时间为 ${formatResetDate(snapshot.quotaResetsAt)}` : ''
      planValue.textContent = value(snapshot.plan)
      creditsValue.textContent = value(snapshot.resetCredits)
      tokenDetailLabel.textContent = snapshot.usageDate ? `Token · ${snapshot.usageDate.slice(5)}` : '本日 Token'
      tokensValue.textContent = formatTokens(snapshot.todayTokens)
      freshness.className = `freshness ${snapshot.status}`
      const source = sourceLabel(snapshot.source)
      freshness.textContent = snapshot.status === 'fresh'
        ? `已连接 · 最近更新 ${formatRecentTime(snapshot.fetchedAt)}${source ? ` · ${source}` : ''}`
        : snapshot.status === 'stale'
          ? `${snapshot.error ?? '数据已过期'} · 正在重连 · 保留最后成功数据 · 最后成功于 ${formatRecentTime(snapshot.fetchedAt)}${source ? ` · ${source}` : ''}`
          : `${snapshot.error ?? '暂时无法读取数据'} · 保留最后成功数据 · 最后成功于 ${formatRecentTime(snapshot.fetchedAt)}`
      const signature = taskSignature(snapshot)
      if (signature !== lastTaskSignature) {
        renderTaskList(taskList, snapshot.tasks, onAcknowledge, onApproval)
        lastTaskSignature = signature
      }
      renderTaskCounts(taskCount, snapshot)
      historySnapshot = snapshot
      historyWindow.textContent = historyWindowLabel(visibleHistoryHours)
      renderHistory(historyContent, snapshot, visibleHistoryHours)
    },
    setRefreshing(value) {
      refreshButton.disabled = value
      refreshButton.classList.toggle('is-refreshing', value)
      refreshButton.textContent = value ? '正在更新…' : '立即更新'
    },
    setPairingSettingsOpen(value) {
      currentPairingOpen = value
      updatePairing()
    },
    setPairingInfo(value) {
      currentPairingInfo = value
      updatePairing()
    },
    destroy() { root.replaceChildren() },
  }
}

export function renderDetailsPanel(root: HTMLElement, snapshot: Snapshot, onRefresh: () => void, onAcknowledge: (taskId: string) => void, onClose: () => void, pairingInfo?: { address: string; code: string }, isRefreshing = false, onAdvance?: () => void, pairingSettingsOpen = false, onTogglePairing?: () => void, onResetPairing?: () => void, onApproval?: (taskId: string, decision: ApprovalDecision) => void): void {
  const mounted = mountDetailsPanel(root, onRefresh, onAcknowledge, onClose, pairingInfo, onAdvance, pairingSettingsOpen, onTogglePairing, onResetPairing, onApproval)
  mounted.update(snapshot)
  mounted.setRefreshing(isRefreshing)
}
