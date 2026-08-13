import { Snapshot } from '../domain'
import { renderTaskList } from './TaskList'
import { MountedView } from './FloatingIsland'

function formatTime(epoch?: number) { return epoch ? new Date(epoch * 1000).toLocaleString('zh-CN', { hour12: false }) : '--' }
function value(value: unknown) { return value === undefined || value === null ? '--' : String(value) }
function escapeHtml(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }
function taskSignature(snapshot: Snapshot) { return snapshot.tasks.map((task) => `${task.id}:${task.status}:${task.acknowledged}:${task.title}:${task.updatedAt}:${task.tokenCount ?? ''}`).join('|') }
function sourceLabel(source?: Snapshot['source']) { return source === 'app-server-event' ? '实时事件' : source === 'task-watch' ? '任务监听' : source === 'metrics-poll' ? '指标轮询' : source === 'full-poll' ? '完整校准' : source === 'manual-refresh' ? '手动刷新' : source === 'local-cache' ? '本地缓存' : '' }

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
): MountedDetailsView {
  root.innerHTML = `<section class="details-panel" aria-label="额度详情">
    <header class="details-drag-region" data-tauri-drag-region><div data-tauri-drag-region><small data-tauri-drag-region>CODEX 额度状态</small><h1 class="details-title" data-tauri-drag-region></h1></div><button class="close-button pairing-settings-button" type="button" aria-label="配对设置"></button></header>
    <div class="detail-grid"><div><small>下次重置</small><strong class="reset-value"></strong></div><div><small>当前套餐</small><strong class="plan-value"></strong></div><div><small>可用重置机会</small><strong class="credits-value"></strong></div><div><small class="token-detail-label"></small><strong class="tokens-value"></strong></div></div>
    <div class="freshness"></div>
    <div class="pairing-settings" aria-label="配对设置" hidden></div>
    <div class="task-header"><strong>任务状态</strong><span class="task-count"></span></div><ul class="task-list"></ul>
    <button class="refresh-button" type="button"></button>
  </section>`

  const panel = root.querySelector<HTMLElement>('.details-panel')!
  const title = root.querySelector<HTMLElement>('.details-title')!
  const resetValue = root.querySelector<HTMLElement>('.reset-value')!
  const planValue = root.querySelector<HTMLElement>('.plan-value')!
  const creditsValue = root.querySelector<HTMLElement>('.credits-value')!
  const tokenDetailLabel = root.querySelector<HTMLElement>('.token-detail-label')!
  const tokensValue = root.querySelector<HTMLElement>('.tokens-value')!
  const freshness = root.querySelector<HTMLElement>('.freshness')!
  const pairingSettings = root.querySelector<HTMLElement>('.pairing-settings')!
  const pairingButton = root.querySelector<HTMLButtonElement>('.pairing-settings-button')!
  const taskCount = root.querySelector<HTMLElement>('.task-count')!
  const taskList = root.querySelector<HTMLElement>('.task-list')!
  const refreshButton = root.querySelector<HTMLButtonElement>('.refresh-button')!
  let lastTaskSignature = ''
  let currentPairingOpen = pairingSettingsOpen
  let currentPairingInfo = pairingInfo

  pairingButton.addEventListener('click', () => onTogglePairing?.())
  refreshButton.addEventListener('click', onRefresh)
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
      title.textContent = `本周剩余 ${value(snapshot.quotaRemainingPercent)}${snapshot.quotaRemainingPercent === undefined ? '' : '%'}`
      resetValue.textContent = formatTime(snapshot.quotaResetsAt)
      planValue.textContent = value(snapshot.plan)
      creditsValue.textContent = value(snapshot.resetCredits)
      tokenDetailLabel.textContent = snapshot.usageDate ? `Token · ${snapshot.usageDate.slice(5)}` : '本日 Token'
      tokensValue.textContent = value(snapshot.todayTokens)
      freshness.className = `freshness ${snapshot.status}`
      const source = sourceLabel(snapshot.source)
      freshness.textContent = snapshot.status === 'fresh'
        ? `已连接 · 最近更新 ${formatTime(snapshot.fetchedAt)}${source ? ` · ${source}` : ''}`
        : snapshot.status === 'stale'
          ? `${snapshot.error ?? '数据已过期'} · 最后成功于 ${formatTime(snapshot.fetchedAt)}${source ? ` · ${source}` : ''}`
          : `${snapshot.error ?? '暂时无法读取数据'} · 最后成功于 ${formatTime(snapshot.fetchedAt)}`
      taskCount.textContent = `${snapshot.activeTaskCount} 个活跃任务`
      const signature = taskSignature(snapshot)
      if (signature !== lastTaskSignature) {
        renderTaskList(taskList, snapshot.tasks, onAcknowledge)
        lastTaskSignature = signature
      }
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

export function renderDetailsPanel(root: HTMLElement, snapshot: Snapshot, onRefresh: () => void, onAcknowledge: (taskId: string) => void, onClose: () => void, pairingInfo?: { address: string; code: string }, isRefreshing = false, onAdvance?: () => void, pairingSettingsOpen = false, onTogglePairing?: () => void, onResetPairing?: () => void): void {
  const mounted = mountDetailsPanel(root, onRefresh, onAcknowledge, onClose, pairingInfo, onAdvance, pairingSettingsOpen, onTogglePairing, onResetPairing)
  mounted.update(snapshot)
  mounted.setRefreshing(isRefreshing)
}
