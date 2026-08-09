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
  fetchedAt?: number
  quotaRemainingPercent?: number
  quotaResetsAt?: number
  plan?: string
  resetCredits?: number
  todayTokens?: number
  activeTaskCount: number
  tasks: TaskSummary[]
  error?: string
  schemaVersion: string
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
