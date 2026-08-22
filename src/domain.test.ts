import { describe, expect, it } from 'vitest'
import { diffSnapshot, mapTaskStatus, normalizeSnapshot, snapshotStatus, taskStatusCounts } from './domain'

describe('task status mapping', () => {
  it('prioritises user action over running', () => {
    expect(mapTaskStatus({ waitingForUser: true, running: true, completed: false })).toBe('needs_action')
  })
  it('maps running and completed states', () => {
    expect(mapTaskStatus({ waitingForUser: false, running: true, completed: false })).toBe('running')
    expect(mapTaskStatus({ waitingForUser: false, running: false, completed: true })).toBe('completed')
    expect(mapTaskStatus({ waitingForUser: false, running: false, completed: false })).toBe('none')
  })
})

describe('snapshot freshness', () => {
  it('marks snapshots older than 15 minutes stale', () => {
    expect(snapshotStatus(0, 901, false, true)).toBe('stale')
  })
  it('does not present errors as fresh', () => {
    expect(snapshotStatus(100, 200, true, true)).toBe('error')
    expect(snapshotStatus(undefined, 200, true, true)).toBe('error')
  })
})

describe('snapshot transport', () => {
  it('normalizes Tauri snake_case fields for the desktop view', () => {
    const snapshot = normalizeSnapshot({ status: 'fresh', quota_remaining_percent: 29, today_tokens: 128400, active_task_count: 2, tasks: [{ id: 't1', title: 'Build', status: 'running', token_count: 400, updated_at: 12, acknowledged: false }], schema_version: '1.0' })
    expect(snapshot.quotaRemainingPercent).toBe(29)
    expect(snapshot.todayTokens).toBe(128400)
    expect(snapshot.activeTaskCount).toBe(2)
    expect(snapshot.tasks[0].tokenCount).toBe(400)
  })
  it('turns null numeric payloads into unknown values', () => {
    const snapshot = normalizeSnapshot({ status: 'error', quota_remaining_percent: null, today_tokens: null, active_task_count: null, tasks: [{ id: 't1', token_count: null }] })
    expect(snapshot.quotaRemainingPercent).toBeUndefined()
    expect(snapshot.todayTokens).toBeUndefined()
    expect(snapshot.tasks[0].tokenCount).toBeUndefined()
  })
  it('normalizes canonical task counts from the backend', () => {
    const snapshot = normalizeSnapshot({ task_counts: { none: 0, needs_action: 1, running: 2, completed: 3 }, tasks: [] })
    expect(taskStatusCounts(snapshot.tasks, snapshot.taskCounts)).toEqual({ none: 0, needs_action: 1, running: 2, completed: 3 })
  })
  it('normalizes quota-only hourly history points', () => {
    const snapshot = normalizeSnapshot({ history: [{ at: 100, quota_remaining_percent: 72, today_tokens: 999 }] })
    expect(snapshot.history).toEqual([{ at: 100, quotaRemainingPercent: 72 }])
  })
})

describe('snapshot diff', () => {
  const base = normalizeSnapshot({ status: 'fresh', quota_remaining_percent: 72, today_tokens: 1, active_task_count: 0, tasks: [], schema_version: '1.0' })
  it('returns only the changed metric', () => {
    expect(diffSnapshot(base, { ...base, todayTokens: 2 })).toEqual({ todayTokens: 2 })
  })
  it('returns an empty diff for identical snapshots', () => {
    expect(diffSnapshot(base, { ...base })).toEqual({})
  })
  it('includes task rows when task state changes', () => {
    const tasks = [{ id: 't', title: 'Task', status: 'running' as const, updatedAt: 1, acknowledged: false }]
    const next = { ...base, activeTaskCount: 1, tasks }
    expect(diffSnapshot(base, next)).toEqual({ activeTaskCount: 1, tasks })
  })
})
