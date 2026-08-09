import { Snapshot } from '../domain'
import { renderTaskList } from './TaskList'

function formatTime(epoch?: number) { return epoch ? new Date(epoch * 1000).toLocaleString('zh-CN', { hour12: false }) : '--' }
function value(value: unknown) { return value === undefined || value === null ? '--' : String(value) }

export function renderDetailsPanel(root: HTMLElement, snapshot: Snapshot, onRefresh: () => void, onAcknowledge: (taskId: string) => void, onClose: () => void): void {
  root.innerHTML = `<section class="details-panel" aria-label="额度详情">
    <header><div><small>CODEX 额度状态</small><h1>本周剩余 ${value(snapshot.quotaRemainingPercent)}${snapshot.quotaRemainingPercent === undefined ? '' : '%'}</h1></div><button class="close-button" type="button" aria-label="收起">收起</button></header>
    <div class="detail-grid"><div><small>下次重置</small><strong>${formatTime(snapshot.quotaResetsAt)}</strong></div><div><small>当前套餐</small><strong>${value(snapshot.plan)}</strong></div><div><small>可用重置机会</small><strong>${value(snapshot.resetCredits)}</strong></div><div><small>本日 Token</small><strong>${value(snapshot.todayTokens)}</strong></div></div>
    <div class="freshness ${snapshot.status}">${snapshot.status === 'fresh' ? `最近更新 ${formatTime(snapshot.fetchedAt)}` : snapshot.status === 'stale' ? `数据已过期 · 最后更新于 ${formatTime(snapshot.fetchedAt)}` : snapshot.error ?? '暂时无法读取数据'}</div>
    <div class="task-header"><strong>任务状态</strong><span>${snapshot.activeTaskCount} 个活跃任务</span></div><ul class="task-list"></ul>
    <button class="refresh-button" type="button">立即更新</button>
  </section>`
  root.querySelector('.close-button')?.addEventListener('click', onClose)
  root.querySelector('.refresh-button')?.addEventListener('click', onRefresh)
  const list = root.querySelector<HTMLElement>('.task-list')!
  renderTaskList(list, snapshot.tasks, onAcknowledge)
}
