import './styles.css'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { diffSnapshot, normalizeSnapshot, Snapshot } from './domain'
import { mountDetailsPanel, MountedDetailsView } from './components/DetailsPanel'
import { mountFloatingBall, mountFloatingIsland, MountedView } from './components/FloatingIsland'

const app = document.querySelector<HTMLMainElement>('#app')
if (!app) throw new Error('app root is missing')

const designPreview = new URLSearchParams(window.location.search).has('design-preview')
const fallback: Snapshot = designPreview
  ? { status: 'fresh', fetchedAt: Math.floor(Date.now() / 1000), quotaRemainingPercent: 22, todayTokens: 0, tasks: [], taskCounts: { none: 0, needsAction: 0, running: 0, completed: 0 }, activeTaskCount: 0, history: [], schemaVersion: '1.0' }
  : { status: 'stale', tasks: [], taskCounts: { none: 0, needsAction: 0, running: 0, completed: 0 }, activeTaskCount: 0, history: [], schemaVersion: '1.0', error: '等待连接 Codex app-server' }
let snapshot = fallback
type ViewState = 'ball' | 'summary' | 'details'
let viewState: ViewState = 'ball'
let refreshing = false
let pairingInfo: { address: string; code: string } | undefined
let retryTimer: number | undefined
let retryDelay = 1500
let renderedViewState: ViewState | undefined
let resizeTarget = ''
let pairingSettingsOpen = false
let ballView: MountedView | undefined
let islandView: MountedView | undefined
let detailsView: MountedDetailsView | undefined
let snapshotQueue: Snapshot | undefined
let snapshotQueueTimer: number | undefined
let lastChangedAt = snapshot.changedAt ?? 0
const REFRESH_TIMEOUT_MS = 35_000

app.innerHTML = '<div class="app-frame"><div id="ball-root"></div><div id="island-root" hidden></div><div id="details-root" hidden></div></div>'
const ballRoot = document.querySelector<HTMLElement>('#ball-root')!
const islandRoot = document.querySelector<HTMLElement>('#island-root')!
const detailsRoot = document.querySelector<HTMLElement>('#details-root')!

function resizeWindow(width: number, height: number, expanded: boolean) {
  if (designPreview) return
  const target = `${width}x${height}x${expanded}`
  if (target === resizeTarget) return
  resizeTarget = target
  void invoke('set_window_expanded', { expanded, width, height }).catch(() => undefined)
}

function activeView(): MountedView | undefined {
  return viewState === 'ball' ? ballView : viewState === 'summary' ? islandView : detailsView
}

function updateMountedSnapshot() {
  activeView()?.update(snapshot)
  if (viewState === 'details') scheduleDetailsResize()
}

function scheduleDetailsResize() {
  if (viewState !== 'details') return
  window.requestAnimationFrame(() => {
    const panel = detailsRoot.querySelector<HTMLElement>('.details-panel')
    if (panel) {
      // Measure after the task list has rendered so the footer remains below it.
      const desiredHeight = Math.ceil(panel.scrollHeight + 32)
      const screenHeight = window.screen?.availHeight ?? 0
      const screenLimit = screenHeight > 0 ? Math.max(360, screenHeight - 24) : desiredHeight
      resizeWindow(620, Math.min(desiredHeight, screenLimit), true)
    }
  })
}

function renderView() {
  const entering = renderedViewState !== viewState
  app.dataset.transition = entering ? 'enter' : 'update'
  app.dataset.view = viewState
  if (viewState === 'details') {
    ballRoot.hidden = true
    islandRoot.hidden = true
    detailsRoot.hidden = false
    if (!detailsView) {
        detailsView = mountDetailsPanel(detailsRoot, refresh, acknowledge, () => { viewState = 'summary'; renderView() }, pairingInfo, () => { viewState = 'ball'; renderView() }, pairingSettingsOpen, () => {
        pairingSettingsOpen = !pairingSettingsOpen
        detailsView?.setPairingSettingsOpen(pairingSettingsOpen)
        scheduleDetailsResize()
      }, () => {
        void invoke<typeof pairingInfo>('reset_pairing').then((next) => {
          pairingInfo = next
          detailsView?.setPairingInfo(next)
        }).catch(() => undefined)
      }, async (taskId, decision) => {
        try {
          await invoke('respond_to_approval', { taskId, decision })
          await refresh()
        } catch (error) {
          applySnapshot({ ...snapshot, status: 'error', error: error instanceof Error ? error.message : '批准请求处理失败' })
        }
      })
    }
    detailsView.update(snapshot)
    detailsView.setRefreshing(refreshing)
    if (entering) scheduleDetailsResize()
  } else if (viewState === 'summary') {
    ballRoot.hidden = true
    islandRoot.hidden = false
    detailsRoot.hidden = true
    if (!islandView) islandView = mountFloatingIsland(islandRoot, () => { viewState = 'details'; renderView() })
    islandView.update(snapshot)
    if (entering) resizeWindow(520, 120, false)
  } else {
    ballRoot.hidden = false
    islandRoot.hidden = true
    detailsRoot.hidden = true
    if (!ballView) ballView = mountFloatingBall(ballRoot, () => { viewState = 'summary'; renderView() })
    ballView.update(snapshot)
    if (entering) resizeWindow(116, 116, false)
  }
  renderedViewState = viewState
}

function applySnapshot(next: Snapshot) {
  if (next.changedAt !== undefined && next.changedAt < lastChangedAt) return
  const changes = diffSnapshot(snapshot, next)
  if (Object.keys(changes).length === 0) return
  snapshot = next
  if (next.changedAt !== undefined) lastChangedAt = Math.max(lastChangedAt, next.changedAt)
  updateMountedSnapshot()
}

function queueSnapshot(input: unknown) {
  const next = normalizeSnapshot(input)
  if (next.changedAt !== undefined && next.changedAt < lastChangedAt) return
  if (!snapshotQueue || (next.changedAt ?? 0) >= (snapshotQueue.changedAt ?? 0)) snapshotQueue = next
  if (snapshotQueueTimer !== undefined) return
  snapshotQueueTimer = window.setTimeout(() => {
    snapshotQueueTimer = undefined
    if (snapshotQueue) applySnapshot(snapshotQueue)
    snapshotQueue = undefined
  }, 150)
}

async function refresh() {
  if (designPreview || refreshing) return
  refreshing = true
  activeView()?.setRefreshing(true)
  if (!pairingInfo) {
    try { pairingInfo = await invoke<typeof pairingInfo>('get_pairing_info') } catch { /* web preview */ }
  }
  try {
    const refreshed = await Promise.race([
      invoke<unknown>('refresh_now'),
      new Promise<never>((_, reject) => window.setTimeout(() => reject(new Error('refresh-timeout')), REFRESH_TIMEOUT_MS)),
    ])
    applySnapshot(normalizeSnapshot(refreshed))
    retryDelay = 1500
    if (retryTimer !== undefined) { window.clearTimeout(retryTimer); retryTimer = undefined }
  } catch (error) {
    try { applySnapshot(normalizeSnapshot(await invoke<unknown>('get_snapshot'))) } catch {
      const timedOut = error instanceof Error && error.message === 'refresh-timeout'
      applySnapshot({ ...snapshot, status: snapshot.status === 'fresh' ? 'error' : snapshot.status, error: timedOut ? '同步数据超时，请稍后重试' : '桌面端尚未连接 Codex app-server' })
    }
    if (retryTimer === undefined && snapshot.status !== 'fresh') {
      retryTimer = window.setTimeout(() => { retryTimer = undefined; void refresh() }, retryDelay)
      retryDelay = Math.min(retryDelay * 2, 30_000)
    }
  }
  refreshing = false
  activeView()?.setRefreshing(false)
}

async function acknowledge(taskId: string) {
  try { await invoke('acknowledge_task', { taskId }) } catch {
    const task = snapshot.tasks.find((item) => item.id === taskId)
    if (task) applySnapshot({ ...snapshot, tasks: snapshot.tasks.map((item) => item.id === taskId ? { ...item, acknowledged: true } : item) })
  }
}

// A small test bridge uses the same coalesced path as Tauri events in design preview.
;(window as Window & { __codexTestApplySnapshot?: (value: unknown) => void }).__codexTestApplySnapshot = queueSnapshot

renderView()
if (!designPreview) {
  void refresh()
  void listen<Snapshot>('snapshot-updated', (event) => queueSnapshot(event.payload))
  void listen('refresh-requested', () => { void refresh() })
  void listen('update-check-requested', async () => {
    try {
      const result = await invoke<{ message: string; available: boolean }>('check_for_updates')
      window.alert(result.message)
      if (result.available) await invoke('relaunch_app')
    } catch {
      window.alert('Update check is unavailable in web preview')
    }
  })
  window.setInterval(refresh, 120_000)
}
