import { Snapshot, STATUS_COLOR, TaskStatus } from '../domain'

const statusLabel: Record<TaskStatus, string> = { none: '无活跃任务', needs_action: '需要处理', running: '执行中', completed: '可验收' }

function formatTokens(tokens?: number) {
  if (tokens === undefined) return '--'
  return tokens >= 1000 ? `${(tokens / 1000).toFixed(tokens >= 100000 ? 0 : 1)}K` : String(tokens)
}

export function renderFloatingIsland(root: HTMLElement, snapshot: Snapshot, onOpen: () => void): void {
  const percent = snapshot.quotaRemainingPercent
  const percentText = percent === undefined ? '--' : `${percent}%`
  const status = snapshot.tasks.find((task) => task.status === 'needs_action')?.status
    ?? snapshot.tasks.find((task) => task.status === 'running')?.status
    ?? snapshot.tasks.find((task) => task.status === 'completed' && !task.acknowledged)?.status
    ?? 'none'
  const taskCount = snapshot.tasks.filter((task) => !task.acknowledged && (task.status === 'running' || task.status === 'needs_action')).length
  const stale = snapshot.status !== 'fresh'
  root.innerHTML = `
    <button class="island-shell ${stale ? 'island-stale' : ''}" aria-label="打开额度详情" type="button">
      <span class="island-segment quota-segment">
        <span class="quota-ring" style="--quota:${percent ?? 0}%;--ring-color:${percent === undefined ? '#a8ada3' : '#ddb480'}"><b>${percentText}</b></span>
        <span><small>本周剩余</small><strong>${stale ? '数据待确认' : '额度可用'}</strong></span>
      </span>
      <span class="island-segment task-segment">
        <span class="status-dot" style="--status-color:${STATUS_COLOR[status]}"></span>
        <span><small>任务状态</small><strong>${taskCount ? `${taskCount} 个任务执行中` : statusLabel[status]}</strong></span>
      </span>
      <span class="island-segment token-segment"><span><small>本日 Token</small><strong>${formatTokens(snapshot.todayTokens)}</strong></span></span>
    </button>`
  root.querySelector('button')?.addEventListener('click', onOpen)
}
