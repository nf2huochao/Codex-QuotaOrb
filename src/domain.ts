export type TaskStatus = 'none' | 'needs_action' | 'running' | 'completed'
export type DataStatus = 'fresh' | 'stale' | 'error' | 'unauthenticated'

export interface TaskEvent {
  id: string
  title: string
  waitingForUser: boolean
  running: boolean
  completed: boolean
  tokenCount?: number
  updatedAt: number
}

export interface TaskSummary {
  id: string
  title: string
  status: Exclude<TaskStatus, 'none'> | 'none'
  tokenCount?: number
  updatedAt: number
  acknowledged: boolean
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
  tasks: TaskSummary[]
  error?: string
  schemaVersion: string
}

export type SnapshotChanges = Partial<Pick<Snapshot, 'status' | 'changedAt' | 'source' | 'fetchedAt' | 'quotaRemainingPercent' | 'quotaResetsAt' | 'plan' | 'resetCredits' | 'todayTokens' | 'usageDate' | 'activeTaskCount' | 'tasks' | 'error' | 'schemaVersion'>>

function tasksEqual(left: Snapshot['tasks'], right: Snapshot['tasks']) {
  if (left.length !== right.length) return false
  return left.every((task, index) => {
    const other = right[index]
    return task.id === other.id && task.title === other.title && task.status === other.status && task.tokenCount === other.tokenCount && task.updatedAt === other.updatedAt && task.acknowledged === other.acknowledged
  })
}

/** Return only fields that changed so mounted views can update stable DOM nodes. */
export function diffSnapshot(previous: Snapshot, next: Snapshot): SnapshotChanges {
  const changes: SnapshotChanges = {}
  const scalarKeys: Array<keyof Snapshot> = ['status', 'changedAt', 'source', 'fetchedAt', 'quotaRemainingPercent', 'quotaResetsAt', 'plan', 'resetCredits', 'todayTokens', 'usageDate', 'activeTaskCount', 'error', 'schemaVersion']
  scalarKeys.forEach((key) => { if (previous[key] !== next[key]) (changes as Record<string, unknown>)[key] = next[key] })
  if (!tasksEqual(previous.tasks, next.tasks)) changes.tasks = next.tasks
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
    tasks: rawTasks.map((item) => {
      const task = (item && typeof item === 'object' ? item : {}) as Record<string, unknown>
      return {
        id: String(task.id ?? ''),
        title: String(task.title ?? 'Codex task'),
        status: (task.status as TaskSummary['status']) ?? 'none',
        tokenCount: asNumber(task.tokenCount ?? task.token_count),
        updatedAt: Number(task.updatedAt ?? task.updated_at ?? 0),
        acknowledged: Boolean(task.acknowledged),
      }
    }),
    error: raw.error as string | undefined,
    schemaVersion: String(raw.schemaVersion ?? raw.schema_version ?? '1.0'),
  }
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
