import { TaskSummary, STATUS_COLOR } from '../domain'
import { Language, getLanguage, t } from '../i18n'

export type ApprovalDecision = 'accept' | 'decline'

export function renderTaskList(root: HTMLElement, tasks: TaskSummary[], onAcknowledge: (taskId: string) => void, onApproval: (taskId: string, decision: ApprovalDecision) => void = () => {}, language: Language = getLanguage()): void {
  const visibleTasks = tasks.filter((task) => !(task.status === 'completed' && task.acknowledged))
  root.innerHTML = visibleTasks.length ? visibleTasks.map((task) => `
    <li class="task-row ${task.acknowledged ? 'task-acknowledged' : ''}">
      <span class="status-dot" style="--status-color:${STATUS_COLOR[task.status]}"></span>
      <span class="task-copy"><strong>${escapeHtml(task.title)}</strong>${task.activity ? `<small class="task-activity">${t('currentActivity', language) ?? '当前内容'}：${escapeHtml(task.activity)}</small>` : ''}<small>${label(task.status, language)}</small>${task.status === 'needs_action' ? `<small class="task-reason">${escapeHtml(task.waitingReason ?? t('waitingApproval', language))}</small>` : ''}</span>
      ${task.status === 'needs_action' && task.approvalRequestId ? `<span class="approval-actions"><button data-approval-task="${escapeHtml(task.id)}" data-decision="accept" class="approval-button approval-accept" type="button">${t('approve', language)}</button><button data-approval-task="${escapeHtml(task.id)}" data-decision="decline" class="approval-button approval-decline" type="button">${t('decline', language)}</button></span>` : ''}
      ${task.status === 'completed' && !task.acknowledged ? `<button data-task-id="${escapeHtml(task.id)}" class="ack-button" type="button">${t('readyForReview', language)}</button>` : ''}
    </li>`).join('') : `<li class="empty-task">${t('noTaskRecords', language)}</li>`
  root.querySelectorAll<HTMLButtonElement>('[data-task-id]').forEach((button) => button.addEventListener('click', () => onAcknowledge(button.dataset.taskId!)))
  root.querySelectorAll<HTMLButtonElement>('[data-approval-task]').forEach((button) => button.addEventListener('click', () => onApproval(button.dataset.approvalTask!, button.dataset.decision as ApprovalDecision)))
}

function label(status: TaskSummary['status'], language: Language) { return { none: t('noActiveTasks', language), needs_action: t('needsApproval', language), running: t('runningFull', language), completed: t('completedFull', language) }[status] }
function escapeHtml(value: string) { return value.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]!)) }
