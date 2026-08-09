import './styles.css'
import { invoke } from '@tauri-apps/api/core'
import { DataStatus, Snapshot } from './domain'
import { renderDetailsPanel } from './components/DetailsPanel'
import { renderFloatingIsland } from './components/FloatingIsland'

const app = document.querySelector<HTMLMainElement>('#app')
if (!app) throw new Error('应用根节点不存在')

const fallback: Snapshot = { status: 'stale', tasks: [], activeTaskCount: 0, schemaVersion: '1.0', error: '等待连接 Codex app-server' }
let snapshot = fallback
let expanded = false

app.innerHTML = '<div class="app-frame"><div id="island-root"></div><div id="details-root" hidden></div></div>'
const islandRoot = document.querySelector<HTMLElement>('#island-root')!
const detailsRoot = document.querySelector<HTMLElement>('#details-root')!

function render() {
  if (expanded) {
    islandRoot.hidden = true
    detailsRoot.hidden = false
    renderDetailsPanel(detailsRoot, snapshot, refresh, acknowledge, () => { expanded = false; render() })
  } else {
    islandRoot.hidden = false
    detailsRoot.hidden = true
    renderFloatingIsland(islandRoot, snapshot, () => { expanded = true; render() })
  }
}

async function refresh() {
  try { snapshot = await invoke<Snapshot>('refresh_now') } catch { try { snapshot = await invoke<Snapshot>('get_snapshot') } catch { snapshot = { ...snapshot, status: snapshot.status === 'fresh' ? 'error' : snapshot.status, error: '桌面端尚未连接 Codex app-server' } } }
  render()
}
async function acknowledge(taskId: string) { try { await invoke('acknowledge_task', { taskId }) } catch { const task = snapshot.tasks.find((item) => item.id === taskId); if (task) task.acknowledged = true } render() }

render()
void refresh()
window.setInterval(refresh, 120_000)
