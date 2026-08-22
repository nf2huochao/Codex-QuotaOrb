import { TaskSummary, STATUS_COLOR } from '../domain'

export type ApprovalDecision = 'accept' | 'decline'

export function renderTaskList(root: HTMLElement, tasks: TaskSummary[], onAcknowledge: (taskId: string) => void, onApproval: (taskId: string, decision: ApprovalDecision) => void = () => {}): void {
  root.innerHTML = tasks.length ? tasks.map((task) => `
    <li class="task-row ${task.acknowledged ? 'task-acknowledged' : ''}">
      <span class="status-dot" style="--status-color:${STATUS_COLOR[task.status]}"></span>
      <span class="task-copy"><strong>${escapeHtml(task.title)}</strong>${task.activity ? `<small class="task-activity">当前内容：${escapeHtml(task.activity)}</small>` : ''}<small>${label(task.status)}</small>${task.status === 'needs_action' ? `<small class="task-reason">${escapeHtml(task.waitingReason ?? '等待你的批准')}</small>` : ''}</span>
      ${task.status === 'needs_action' && task.approvalRequestId ? `<span class="approval-actions"><button data-approval-task="${escapeHtml(task.id)}" data-decision="accept" class="approval-button approval-accept" type="button">批准</button><button data-approval-task="${escapeHtml(task.id)}" data-decision="decline" class="approval-button approval-decline" type="button">拒绝</button></span>` : ''}
      ${task.status === 'completed' && !task.acknowledged ? `<button data-task-id="${escapeHtml(task.id)}" class="ack-button" type="button">可验收</button>` : ''}
    </li>`).join('') : '<li class="empty-task">暂无任务记录</li>'
  root.querySelectorAll<HTMLButtonElement>('[data-task-id]').forEach((button) => button.addEventListener('click', () => onAcknowledge(button.dataset.taskId!)))
  root.querySelectorAll<HTMLButtonElement>('[data-approval-task]').forEach((button) => button.addEventListener('click', () => onApproval(button.dataset.approvalTask!, button.dataset.decision as ApprovalDecision)))
}

function label(status: TaskSummary['status']) { return { none: '无活跃任务', needs_action: '需要批准 / 回复 / 确认', running: '正在执行', completed: '已完成，可验收' }[status] }
function escapeHtml(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }
