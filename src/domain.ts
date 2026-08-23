export type TaskStatus = 'none' | 'needs_action' | 'running' | 'completed'
export type DataStatus = 'fresh' | 'stale' | 'error' | 'unauthenticated'

export interface TaskEvent {
  id: string
  title: string
  waitingReason?: string
  approvalRequestId?: string
  waitingForUser: boolean
  running: boolean
  completed: boolean
  tokenCount?: number
  updatedAt: number
}

export interface TaskSummary {
  id: string
  title: string
  activity?: string
  waitingReason?: string
  approvalRequestId?: string
  status: Exclude<TaskStatus, 'none'> | 'none'
  tokenCount?: number
  updatedAt: number
  acknowledged: boolean
  source?: string
  turnId?: string
  receivedAt?: number
}

export interface HookDiagnostic {
  event: string
  sessionId?: string
  turnId?: string
  receivedAt: number
  httpStatus?: number
  delivered: boolean
  error?: string
}

export interface HookDiagnostics {
  last?: HookDiagnostic
  receivedCount: number
}

export interface UsagePoint {
  at: number
  quotaRemainingPercent?: number
}

export interface TaskCounts {
  none: number
  needsAction: number
  running: number
  completed: number
}

export interface Snapshot {
  status: DataStatus
  changedAt?: number
  source?: 'task-watch' | 'metrics-poll' | 'full-poll' | 'manual-refresh' | string
  fetchedAt?: number
  quotaRemainingPercent?: number
  quotaResetsAt?: number
  plan?: string
  resetCredits?: number
  todayTokens?: number
  usageDate?: string
  activeTaskCount: number
  taskCounts: TaskCounts
  tasks: TaskSummary[]
  error?: string
  history: UsagePoint[]
  hookDiagnostics?: HookDiagnostics
  schemaVersion: string
}

export type SnapshotChanges = Partial<Pick<Snapshot, 'status' | 'changedAt' | 'source' | 'fetchedAt' | 'quotaRemainingPercent' | 'quotaResetsAt' | 'plan' | 'resetCredits' | 'todayTokens' | 'usageDate' | 'activeTaskCount' | 'taskCounts' | 'tasks' | 'error' | 'history' | 'hookDiagnostics' | 'schemaVersion'>>

function tasksEqual(left: Snapshot['tasks'], right: Snapshot['tasks']) {
  if (left.length !== right.length) return false
  return left.every((task, index) => {
    const other = right[index]
    return task.id === other.id && task.title === other.title && task.activity === other.activity && task.waitingReason === other.waitingReason && task.approvalRequestId === other.approvalRequestId && task.status === other.status && task.tokenCount === other.tokenCount && task.updatedAt === other.updatedAt && task.acknowledged === other.acknowledged && task.source === other.source && task.turnId === other.turnId && task.receivedAt === other.receivedAt
  })
}

function taskCountsEqual(left: Snapshot['taskCounts'], right: Snapshot['taskCounts']) {
  return left.none === right.none && left.needsAction === right.needsAction && left.running === right.running && left.completed === right.completed
}

function historyEqual(left: Snapshot['history'], right: Snapshot['history']) {
  return left.length === right.length && left.every((point, index) => {
    const other = right[index]
    return point.at === other.at && point.quotaRemainingPercent === other.quotaRemainingPercent
  })
}

function hookDiagnosticsEqual(left: Snapshot['hookDiagnostics'], right: Snapshot['hookDiagnostics']) {
  const a = left ?? { receivedCount: 0 }
  const b = right ?? { receivedCount: 0 }
  const lastA = a.last
  const lastB = b.last
  if (a.receivedCount !== b.receivedCount) return false
  if (!lastA && !lastB) return true
  if (!lastA || !lastB) return false
  return lastA.event === lastB.event && lastA.sessionId === lastB.sessionId && lastA.turnId === lastB.turnId && lastA.receivedAt === lastB.receivedAt && lastA.httpStatus === lastB.httpStatus && lastA.delivered === lastB.delivered && lastA.error === lastB.error
}

/** Return only fields that changed so mounted views can update stable DOM nodes. */
export function diffSnapshot(previous: Snapshot, next: Snapshot): SnapshotChanges {
  const changes: SnapshotChanges = {}
  const scalarKeys: Array<keyof Snapshot> = ['status', 'changedAt', 'source', 'fetchedAt', 'quotaRemainingPercent', 'quotaResetsAt', 'plan', 'resetCredits', 'todayTokens', 'usageDate', 'activeTaskCount', 'error', 'history', 'schemaVersion']
  scalarKeys.filter((key) => key !== 'history').forEach((key) => { if (previous[key] !== next[key]) (changes as Record<string, unknown>)[key] = next[key] })
  if (!taskCountsEqual(previous.taskCounts, next.taskCounts)) changes.taskCounts = next.taskCounts
  if (!historyEqual(previous.history, next.history)) changes.history = next.history
  if (!tasksEqual(previous.tasks, next.tasks)) changes.tasks = next.tasks
  if (!hookDiagnosticsEqual(previous.hookDiagnostics, next.hookDiagnostics)) changes.hookDiagnostics = next.hookDiagnostics
  return changes
}

/** Convert the Rust/Tauri snake_case payload to the browser camelCase model. */
export function normalizeSnapshot(input: unknown): Snapshot {
  const raw = (input && typeof input === 'object' ? input : {}) as Record<string, unknown>
  const rawTasks = Array.isArray(raw.tasks) ? raw.tasks : []
  const asNumber = (value: unknown): number | undefined => typeof value === 'number' && Number.isFinite(value) ? value : undefined
  return {
    status: (raw.status as DataStatus) ?? 'error',
    changedAt: asNumber(raw.changedAt ?? raw.changed_at),
    source: typeof raw.source === 'string' ? raw.source : undefined,
    fetchedAt: asNumber(raw.fetchedAt ?? raw.fetched_at),
    quotaRemainingPercent: asNumber(raw.quotaRemainingPercent ?? raw.quota_remaining_percent),
    quotaResetsAt: asNumber(raw.quotaResetsAt ?? raw.quota_resets_at),
    plan: typeof raw.plan === 'string' ? raw.plan : undefined,
    resetCredits: asNumber(raw.resetCredits ?? raw.reset_credits),
    todayTokens: asNumber(raw.todayTokens ?? raw.today_tokens),
    usageDate: typeof (raw.usageDate ?? raw.usage_date) === 'string' ? String(raw.usageDate ?? raw.usage_date) : undefined,
    activeTaskCount: Number(raw.activeTaskCount ?? raw.active_task_count ?? 0),
    taskCounts: (() => {
      const counts = (raw.taskCounts ?? raw.task_counts) as Record<string, unknown> | undefined
      return {
        none: Number(counts?.none ?? 0),
        needsAction: Number(counts?.needsAction ?? counts?.needs_action ?? 0),
        running: Number(counts?.running ?? 0),
        completed: Number(counts?.completed ?? 0),
      }
    })(),
    tasks: rawTasks.map((item) => {
      const task = (item && typeof item === 'object' ? item : {}) as Record<string, unknown>
      return {
        id: String(task.id ?? ''),
        title: String(task.title ?? 'Codex task'),
        activity: typeof task.activity === 'string' ? task.activity : undefined,
        waitingReason: typeof task.waitingReason === 'string' ? task.waitingReason : typeof task.waiting_reason === 'string' ? task.waiting_reason : undefined,
        approvalRequestId: typeof task.approvalRequestId === 'string' ? task.approvalRequestId : typeof task.approval_request_id === 'string' ? task.approval_request_id : undefined,
        status: (task.status as TaskSummary['status']) ?? 'none',
        tokenCount: asNumber(task.tokenCount ?? task.token_count),
        updatedAt: Number(task.updatedAt ?? task.updated_at ?? 0),
        acknowledged: Boolean(task.acknowledged),
        source: typeof task.source === 'string' ? task.source : undefined,
        turnId: typeof task.turnId === 'string' ? task.turnId : typeof task.turn_id === 'string' ? task.turn_id : undefined,
        receivedAt: Number(task.receivedAt ?? task.received_at ?? 0),
      }
    }),
    error: raw.error as string | undefined,
    history: Array.isArray(raw.history) ? raw.history.map((item) => {
      const point = (item && typeof item === 'object' ? item : {}) as Record<string, unknown>
      return { at: Number(point.at ?? 0), quotaRemainingPercent: asNumber(point.quotaRemainingPercent ?? point.quota_remaining_percent) }
    }).filter((point) => point.at > 0) : [],
    hookDiagnostics: (() => {
      const value = (raw.hookDiagnostics ?? raw.hook_diagnostics) as Record<string, unknown> | undefined
      const last = (value?.last && typeof value.last === 'object' ? value.last : undefined) as Record<string, unknown> | undefined
      return {
        receivedCount: Number(value?.receivedCount ?? value?.received_count ?? 0),
        last: last ? {
          event: String(last.event ?? 'unknown'),
          sessionId: typeof last.sessionId === 'string' ? last.sessionId : typeof last.session_id === 'string' ? last.session_id : undefined,
          turnId: typeof last.turnId === 'string' ? last.turnId : typeof last.turn_id === 'string' ? last.turn_id : undefined,
          receivedAt: Number(last.receivedAt ?? last.received_at ?? 0),
          httpStatus: asNumber(last.httpStatus ?? last.http_status),
          delivered: Boolean(last.delivered),
          error: typeof last.error === 'string' ? last.error : undefined,
        } : undefined,
      }
    })(),
    schemaVersion: String(raw.schemaVersion ?? raw.schema_version ?? '1.0'),
  }
}

export function taskStatusCounts(tasks: TaskSummary[]) {
  const counts: Record<TaskStatus, number> = { none: 0, needs_action: 0, running: 0, completed: 0 }
  tasks.forEach((task) => { if (!(task.acknowledged && task.status === 'completed')) counts[task.status] += 1 })
  return counts
}

export function mapTaskStatus(event: Pick<TaskEvent, 'waitingForUser' | 'running' | 'completed'>): TaskStatus {
  if (event.waitingForUser) return 'needs_action'
  if (event.running) return 'running'
  if (event.completed) return 'completed'
  return 'none'
}

export function snapshotStatus(lastSuccess: number | undefined, now: number, hasError: boolean, authenticated: boolean): DataStatus {
  if (!authenticated) return 'unauthenticated'
  if (lastSuccess === undefined) return hasError ? 'error' : 'stale'
  if (now - lastSuccess > 900) return 'stale'
  if (hasError) return 'error'
  return 'fresh'
}

export const STATUS_COLOR: Record<TaskStatus, string> = {
  none: '#a8ada3',
  needs_action: '#c96e63',
  running: '#d6a85e',
  completed: '#87a56e',
}
