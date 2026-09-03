import { Snapshot, STATUS_COLOR, TaskStatus, taskStatusCounts } from '../domain'
import { Language, getLanguage, t } from '../i18n'

export interface MountedView {
  update(snapshot: Snapshot): void
  setLanguage(value: Language): void
  setRefreshing(value: boolean): void
  destroy(): void
}

function statusLabel(status: TaskStatus, language: Language) {
  return { none: t('noActiveTasks', language), needs_action: t('needsAction', language), running: t('running', language), completed: t('readyForReview', language) }[status]
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

function tokenLabel(snapshot: Snapshot) {
  const now = new Date()
  const date = snapshot.usageDate?.slice(5) ?? `${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`
  return `Token · ${date}`
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

function renderTaskStatusCounts(container: HTMLElement, snapshot: Snapshot, language: Language) {
  const counts = taskStatusCounts(snapshot.tasks)
  const entries: Array<{ status: TaskStatus; label: string }> = [
    { status: 'needs_action', label: t('needsAction', language) },
    { status: 'running', label: t('running', language) },
    { status: 'completed', label: t('readyForReview', language) },
  ].filter(({ status }) => counts[status] > 0)
  if (!entries.length) {
    container.innerHTML = `<span class="task-count task-count-empty" data-status="none"><i class="task-count-dot" style="--status-color:${STATUS_COLOR.none}"></i><b>0</b></span>`
    container.setAttribute('aria-label', `${t('noActiveTasks', language)} 0`)
    return
  }
  container.innerHTML = entries.map(({ status, label }) => `<span class="task-count" data-status="${status}" aria-label="${label}${counts[status]}"><i class="task-count-dot" style="--status-color:${STATUS_COLOR[status]}"></i><b>${counts[status]}</b></span>`).join('')
  container.setAttribute('aria-label', entries.map(({ status, label }) => `${label} ${counts[status]}`).join(', '))
}

export function mountFloatingBall(root: HTMLElement, onOpen: () => void): MountedView {
  root.innerHTML = `<button class="floating-ball" type="button" data-tauri-drag-region>
    <span class="ball-gauge"><span class="quota-ring" aria-hidden="true"></span><b></b></span>
    <span class="ball-status" aria-label=""></span>
  </button>`
  const button = root.querySelector<HTMLButtonElement>('.floating-ball')!
  const ring = root.querySelector<HTMLElement>('.quota-ring')!
  const percentLabel = root.querySelector<HTMLElement>('.ball-gauge b')!
  const statusDot = root.querySelector<HTMLElement>('.ball-status')!
  let language = getLanguage()
  let currentSnapshot: Snapshot | undefined
  let previousPercent: number | undefined
  let changeTimer: number | undefined
  button.querySelectorAll<HTMLElement>('*').forEach((element) => element.setAttribute('data-tauri-drag-region', ''))
  wireDoubleClickOrDrag(button, onOpen)
  return {
    update(snapshot) {
      currentSnapshot = snapshot
      const percent = quotaPercent(snapshot)
      const changed = previousPercent !== undefined && percent !== undefined && percent !== previousPercent
      previousPercent = percent
      if (changed) {
        button.classList.add('quota-changed')
        if (changeTimer !== undefined) window.clearTimeout(changeTimer)
        changeTimer = window.setTimeout(() => button.classList.remove('quota-changed'), 5600)
      }
      percentLabel.textContent = percent === undefined ? '--' : `${percent}%`
      button.classList.toggle('island-stale', snapshot.status !== 'fresh')
      const status = statusFor(snapshot)
      statusDot.style.setProperty('--status-color', STATUS_COLOR[status])
      button.setAttribute('aria-label', t('expandStatus', language))
      statusDot.setAttribute('aria-label', statusLabel(status, language))
      drawQuotaRing(ring, percent)
    },
    setLanguage(value) { language = value; if (currentSnapshot) this.update(currentSnapshot) },
    setRefreshing() {},
    destroy() { if (changeTimer !== undefined) window.clearTimeout(changeTimer); root.replaceChildren() },
  }
}

export function renderFloatingBall(root: HTMLElement, snapshot: Snapshot, onOpen: () => void): void {
  const mounted = mountFloatingBall(root, onOpen)
  mounted.update(snapshot)
}

export function mountFloatingIsland(root: HTMLElement, onOpen: () => void): MountedView {
  root.innerHTML = `<button class="island-shell" type="button" data-tauri-drag-region>
    <span class="island-segment quota-segment"><span class="quota-gauge"><span class="quota-ring" aria-hidden="true"></span><b></b></span><span><small class="quota-label"></small><strong class="quota-copy"></strong></span></span>
    <span class="island-segment task-segment"><span class="status-dot"></span><span class="task-status-copy"><small class="task-label"></small><strong class="task-copy"></strong><span class="task-counts"></span></span></span>
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
  const taskLabel = root.querySelector<HTMLElement>('.task-label')!
  const tokenLabelNode = root.querySelector<HTMLElement>('.token-label')!
  const tokenCopy = root.querySelector<HTMLElement>('.token-copy')!
  let language = getLanguage()
  let currentSnapshot: Snapshot | undefined
  let previousPercent: number | undefined
  let changeTimer: number | undefined
  button.querySelectorAll<HTMLElement>('*').forEach((element) => element.setAttribute('data-tauri-drag-region', ''))
  wireDoubleClickOrDrag(button, onOpen)
  return {
    update(snapshot) {
      currentSnapshot = snapshot
      const percent = quotaPercent(snapshot)
      const changed = previousPercent !== undefined && percent !== undefined && percent !== previousPercent
      previousPercent = percent
      if (changed) {
        button.classList.add('quota-changed')
        if (changeTimer !== undefined) window.clearTimeout(changeTimer)
        changeTimer = window.setTimeout(() => button.classList.remove('quota-changed'), 5600)
      }
      percentLabel.textContent = percent === undefined ? '--' : `${percent}%`
      button.classList.toggle('island-stale', snapshot.status !== 'fresh')
      const plus = isPlusPlan(snapshot)
      const weeklyPercent = snapshot.quotaRemainingPercent
      button.setAttribute('aria-label', t('openDetails', language))
      quotaLabel.textContent = plus ? `${t('weeklyShort', language)} ${weeklyPercent === undefined ? '--' : `${weeklyPercent}%`}` : t('weeklyRemaining', language)
      quotaCopy.textContent = snapshot.status !== 'fresh' ? t('dataPending', language) : plus ? t('fiveHourQuota', language) : t('quotaAvailable', language)
      const summary = islandSummary(snapshot)
      taskDot.style.setProperty('--status-color', STATUS_COLOR[summary.status])
      taskLabel.textContent = t('taskStatus', language)
      taskCopy.textContent = summary.taskCount ? t('taskStatus', language) : t('noActiveTasks', language)
      renderTaskStatusCounts(taskCounts, snapshot, language)
      tokenLabelNode.textContent = tokenLabel(snapshot)
      tokenCopy.textContent = formatTokens(snapshot.todayTokens)
      drawQuotaRing(ring, percent)
    },
    setLanguage(value) { language = value; if (currentSnapshot) this.update(currentSnapshot) },
    setRefreshing() {},
    destroy() { if (changeTimer !== undefined) window.clearTimeout(changeTimer); root.replaceChildren() },
  }
}

export function renderFloatingIsland(root: HTMLElement, snapshot: Snapshot, onOpen: () => void): void {
  const mounted = mountFloatingIsland(root, onOpen)
  mounted.update(snapshot)
}
