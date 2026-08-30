import { Snapshot, STATUS_COLOR, TaskStatus, taskStatusCounts } from '../domain'

export interface MountedView {
  update(snapshot: Snapshot): void
  setRefreshing(value: boolean): void
  destroy(): void
}

const statusLabel: Record<TaskStatus, string> = { none: '无活跃任务', needs_action: '需要处理', running: '执行中', completed: '可验收' }

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

function tokenLabel(snapshot: Snapshot) {
  return snapshot.usageDate ? `Token · ${snapshot.usageDate.slice(5)}` : '本日 Token'
}

function isPlusPlan(snapshot: Snapshot) {
  return snapshot.plan?.trim().toLowerCase() === 'plus'
}

function quotaPercent(snapshot: Snapshot) {
  return isPlusPlan(snapshot) ? snapshot.fiveHourRemainingPercent : snapshot.quotaRemainingPercent
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
  const counts = taskStatusCounts(snapshot.tasks)
  if (counts.needs_action > 0) return 'needs_action'
  if (counts.completed > 0) return 'completed'
  if (counts.running > 0) return 'running'
  return 'none'
}

function islandSummary(snapshot: Snapshot) {
  const status = statusFor(snapshot)
  const counts = taskStatusCounts(snapshot.tasks)
  const taskCount = counts.needs_action + counts.running + counts.completed
  return { status, taskCount }
}

function renderTaskStatusCounts(container: HTMLElement, snapshot: Snapshot) {
  const counts = taskStatusCounts(snapshot.tasks)
  const entries: Array<{ status: TaskStatus; label: string }> = [
    { status: 'needs_action', label: '红' },
    { status: 'running', label: '黄' },
    { status: 'completed', label: '绿' },
  ].filter(({ status }) => counts[status] > 0)
  if (!entries.length) {
    container.innerHTML = `<span class="task-count task-count-empty" data-status="none"><i class="task-count-dot" style="--status-color:${STATUS_COLOR.none}"></i><b>0</b></span>`
    container.setAttribute('aria-label', '无任务 0')
    return
  }
  container.innerHTML = entries.map(({ status, label }) => `<span class="task-count" data-status="${status}" aria-label="${label}${counts[status]}"><i class="task-count-dot" style="--status-color:${STATUS_COLOR[status]}"></i><b>${counts[status]}</b></span>`).join('')
  container.setAttribute('aria-label', entries.map(({ status, label }) => `${label} ${counts[status]}`).join('，'))
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
      const percent = quotaPercent(snapshot)
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
    <span class="island-segment quota-segment"><span class="quota-gauge"><span class="quota-ring" aria-hidden="true"></span><b></b></span><span><small class="quota-label">本周剩余</small><strong class="quota-copy"></strong></span></span>
    <span class="island-segment task-segment"><span class="status-dot"></span><span class="task-status-copy"><small>任务状态</small><strong class="task-copy"></strong><span class="task-counts" aria-label="任务状态统计"></span></span></span>
    <span class="island-segment token-segment"><span><small class="token-label"></small><strong class="token-copy"></strong></span></span>
  </button>`
  const button = root.querySelector<HTMLButtonElement>('.island-shell')!
  const ring = root.querySelector<HTMLElement>('.quota-ring')!
  const percentLabel = root.querySelector<HTMLElement>('.quota-gauge b')!
  const quotaLabel = root.querySelector<HTMLElement>('.quota-label')!
  const quotaCopy = root.querySelector<HTMLElement>('.quota-copy')!
  const taskDot = root.querySelector<HTMLElement>('.status-dot')!
  const taskCopy = root.querySelector<HTMLElement>('.task-copy')!
  const taskCounts = root.querySelector<HTMLElement>('.task-counts')!
  const tokenLabelNode = root.querySelector<HTMLElement>('.token-label')!
  const tokenCopy = root.querySelector<HTMLElement>('.token-copy')!
  button.querySelectorAll<HTMLElement>('*').forEach((element) => element.setAttribute('data-tauri-drag-region', ''))
  wireDoubleClickOrDrag(button, onOpen)
  return {
    update(snapshot) {
      const percent = quotaPercent(snapshot)
      percentLabel.textContent = percent === undefined ? '--' : `${percent}%`
      button.classList.toggle('island-stale', snapshot.status !== 'fresh')
      const plus = isPlusPlan(snapshot)
      const weeklyPercent = snapshot.quotaRemainingPercent
      quotaLabel.textContent = plus ? `周额度剩余 ${weeklyPercent === undefined ? '--' : `${weeklyPercent}%`}` : '本周剩余'
      quotaCopy.textContent = snapshot.status !== 'fresh' ? '数据待确认' : plus ? '5小时额度可用' : '额度可用'
      const summary = islandSummary(snapshot)
      taskDot.style.setProperty('--status-color', STATUS_COLOR[summary.status])
      taskCopy.textContent = summary.taskCount ? '任务状态' : statusLabel.none
      renderTaskStatusCounts(taskCounts, snapshot)
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
