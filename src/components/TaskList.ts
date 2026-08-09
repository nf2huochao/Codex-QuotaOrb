import { TaskSummary, STATUS_COLOR } from '../domain'

export function renderTaskList(root: HTMLElement, tasks: TaskSummary[], onAcknowledge: (taskId: string) => void): void {
  root.innerHTML = tasks.length ? tasks.map((task) => `
    <li class="task-row ${task.acknowledged ? 'task-acknowledged' : ''}">
      <span class="status-dot" style="--status-color:${STATUS_COLOR[task.status]}"></span>
      <span class="task-copy"><strong>${escapeHtml(task.title)}</strong><small>${label(task.status)}</small></span>
      ${task.status === 'completed' && !task.acknowledged ? `<button data-task-id="${escapeHtml(task.id)}" class="ack-button" type="button">已验收</button>` : ''}
    </li>`).join('') : '<li class="empty-task">暂无任务记录</li>'
  root.querySelectorAll<HTMLButtonElement>('[data-task-id]').forEach((button) => button.addEventListener('click', () => onAcknowledge(button.dataset.taskId!)))
}

function label(status: TaskSummary['status']) { return { none: '无活跃任务', needs_action: '需要回复 / 确认 / 授权', running: '正在执行', completed: '已完成，可验收' }[status] }
function escapeHtml(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }
