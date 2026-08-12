import { Snapshot, STATUS_COLOR, TaskStatus } from '../domain'

export interface MountedView {
  update(snapshot: Snapshot): void
  setRefreshing(value: boolean): void
  destroy(): void
}

const statusLabel: Record<TaskStatus, string> = { none: '无活跃任务', needs_action: '需要处理', running: '执行中', completed: '可验收' }

function formatTokens(tokens?: number) {
  if (tokens === undefined) return '--'
  return tokens >= 1000 ? `${(tokens / 1000).toFixed(tokens >= 1_000_000 ? 0 : 1)}K` : String(tokens)
}

function tokenLabel(snapshot: Snapshot) {
  return snapshot.usageDate ? `Token · ${snapshot.usageDate.slice(5)}` : '本日 Token'
}

function drawQuotaRing(ring: HTMLElement, percent?: number) {
  const value = percent === undefined ? 0 : Math.max(0, Math.min(100, percent))
  ring.style.setProperty('--quota', `${value * 3.6}deg`)
}

function wireDoubleClickOrDrag(button: HTMLButtonElement, onDoubleClick: () => void) {
  let startX = 0
  let startY = 0
  let moved = false
  button.addEventListener('pointerdown', (event) => { startX = event.clientX; startY = event.clientY; moved = false })
  button.addEventListener('pointermove', (event) => { moved = moved || Math.hypot(event.clientX - startX, event.clientY - startY) > 6 })
  button.addEventListener('dblclick', (event) => {
    if (moved) { event.preventDefault(); moved = false; return }
    event.preventDefault()
    onDoubleClick()
  })
}

function statusFor(snapshot: Snapshot): TaskStatus {
  return snapshot.tasks.find((task) => task.status === 'needs_action')?.status
    ?? snapshot.tasks.find((task) => task.status === 'completed' && !task.acknowledged)?.status
    ?? snapshot.tasks.find((task) => task.status === 'running')?.status
    ?? 'none'
}

function islandSummary(snapshot: Snapshot) {
  const status = statusFor(snapshot)
  const taskCount = snapshot.tasks.filter((task) => !task.acknowledged && (task.status === 'running' || task.status === 'needs_action')).length
  const runningCount = snapshot.tasks.filter((task) => !task.acknowledged && task.status === 'running').length
  const actionCount = snapshot.tasks.filter((task) => !task.acknowledged && task.status === 'needs_action').length
  const completedCount = snapshot.tasks.filter((task) => !task.acknowledged && task.status === 'completed').length
  const taskSummary = actionCount
    ? `${actionCount} 个任务待处理`
    : completedCount
      ? `${completedCount} 个任务已完成`
      : runningCount
        ? `${runningCount} 个任务执行中`
        : statusLabel[status]
  return { status, taskCount, taskSummary }
}

export function mountFloatingBall(root: HTMLElement, onOpen: () => void): MountedView {
  root.innerHTML = `<button class="floating-ball" aria-label="展开 Codex 额度状态" type="button" data-tauri-drag-region>
    <span class="ball-gauge"><span class="quota-ring" aria-hidden="true"></span><b></b></span>
    <span class="ball-status" aria-label=""></span>
  </button>`
  const button = root.querySelector<HTMLButtonElement>('.floating-ball')!
  const ring = root.querySelector<HTMLElement>('.quota-ring')!
  const percentLabel = root.querySelector<HTMLElement>('.ball-gauge b')!
  const statusDot = root.querySelector<HTMLElement>('.ball-status')!
  button.querySelectorAll<HTMLElement>('*').forEach((element) => element.setAttribute('data-tauri-drag-region', ''))
  wireDoubleClickOrDrag(button, onOpen)
  return {
    update(snapshot) {
      const percent = snapshot.quotaRemainingPercent
      percentLabel.textContent = percent === undefined ? '--' : `${percent}%`
      button.classList.toggle('island-stale', snapshot.status !== 'fresh')
      const status = statusFor(snapshot)
      statusDot.style.setProperty('--status-color', STATUS_COLOR[status])
      statusDot.setAttribute('aria-label', statusLabel[status])
      drawQuotaRing(ring, percent)
    },
    setRefreshing() {},
    destroy() { root.replaceChildren() },
  }
}

export function renderFloatingBall(root: HTMLElement, snapshot: Snapshot, onOpen: () => void): void {
  const mounted = mountFloatingBall(root, onOpen)
  mounted.update(snapshot)
}

export function mountFloatingIsland(root: HTMLElement, onOpen: () => void): MountedView {
  root.innerHTML = `<button class="island-shell" aria-label="打开额度详情" type="button" data-tauri-drag-region>
    <span class="island-segment quota-segment"><span class="quota-gauge"><span class="quota-ring" aria-hidden="true"></span><b></b></span><span><small>本周剩余</small><strong class="quota-copy"></strong></span></span>
    <span class="island-segment task-segment"><span class="status-dot"></span><span><small>任务状态</small><strong class="task-copy"></strong></span></span>
    <span class="island-segment token-segment"><span><small class="token-label"></small><strong class="token-copy"></strong></span></span>
  </button>`
  const button = root.querySelector<HTMLButtonElement>('.island-shell')!
  const ring = root.querySelector<HTMLElement>('.quota-ring')!
  const percentLabel = root.querySelector<HTMLElement>('.quota-gauge b')!
  const quotaCopy = root.querySelector<HTMLElement>('.quota-copy')!
  const taskDot = root.querySelector<HTMLElement>('.status-dot')!
  const taskCopy = root.querySelector<HTMLElement>('.task-copy')!
  const tokenLabelNode = root.querySelector<HTMLElement>('.token-label')!
  const tokenCopy = root.querySelector<HTMLElement>('.token-copy')!
  button.querySelectorAll<HTMLElement>('*').forEach((element) => element.setAttribute('data-tauri-drag-region', ''))
  wireDoubleClickOrDrag(button, onOpen)
  return {
    update(snapshot) {
      const percent = snapshot.quotaRemainingPercent
      percentLabel.textContent = percent === undefined ? '--' : `${percent}%`
      button.classList.toggle('island-stale', snapshot.status !== 'fresh')
      quotaCopy.textContent = snapshot.status !== 'fresh' ? '数据待确认' : '额度可用'
      const summary = islandSummary(snapshot)
      taskDot.style.setProperty('--status-color', STATUS_COLOR[summary.status])
      taskCopy.textContent = summary.taskCount ? summary.taskSummary : statusLabel[summary.status]
      tokenLabelNode.textContent = tokenLabel(snapshot)
      tokenCopy.textContent = formatTokens(snapshot.todayTokens)
      drawQuotaRing(ring, percent)
    },
    setRefreshing() {},
    destroy() { root.replaceChildren() },
  }
}

export function renderFloatingIsland(root: HTMLElement, snapshot: Snapshot, onOpen: () => void): void {
  const mounted = mountFloatingIsland(root, onOpen)
  mounted.update(snapshot)
}
