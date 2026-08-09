import { describe, expect, it } from 'vitest'
import { mapTaskStatus, snapshotStatus } from './domain'

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
