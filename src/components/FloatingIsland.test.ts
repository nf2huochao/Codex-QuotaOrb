// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'
import { mountFloatingBall, mountFloatingIsland, renderFloatingBall, renderFloatingIsland } from './FloatingIsland'
import { mountDetailsPanel, renderDetailsPanel } from './DetailsPanel'
import { Snapshot } from '../domain'

const base = (): Snapshot => ({ status: 'fresh', fetchedAt: 100, quotaRemainingPercent: 72, quotaResetsAt: 200, plan: 'Plus', resetCredits: 1, todayTokens: 128400, activeTaskCount: 3, taskCounts: { none: 0, needsAction: 0, running: 3, completed: 0 }, history: [], schemaVersion: '1.0', tasks: [1, 2, 3].map((id) => ({ id: String(id), title: `任务 ${id}`, status: 'running', updatedAt: 100, acknowledged: false })) })

describe('compact floating island', () => {
  it('renders the compact ball and opens on double click only', () => {
    const root = document.createElement('div')
    const open = vi.fn()
    renderFloatingBall(root, base(), open)
    expect(root.querySelector('.floating-ball')).toBeTruthy()
    expect(root.textContent).toContain('72%')
    const button = root.querySelector<HTMLButtonElement>('.floating-ball')!
    button.click()
    expect(open).not.toHaveBeenCalled()
    button.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))
    expect(open).toHaveBeenCalledTimes(1)
  })
  it('shows quota ring, canonical task count and compact tokens', () => { const root = document.createElement('div'); renderFloatingIsland(root, base(), vi.fn()); expect(root.textContent).toContain('72%'); expect(root.textContent).toContain('3'); expect(root.querySelector('[data-status="running"]')).toBeTruthy(); expect(root.textContent).toContain('12.8万'); expect(root.querySelector('.quota-ring')).toBeTruthy(); expect(root.querySelector('canvas,img')).toBeNull(); expect(root.querySelector<HTMLElement>('.quota-ring')?.style.getPropertyValue('--quota')).toBe('259.2deg') })
  it('shows green completion as soon as one task is complete', () => {
    const root = document.createElement('div')
    const snapshot = { ...base(), taskCounts: { none: 0, needsAction: 0, running: 1, completed: 1 }, tasks: [
      { id: 'done', title: '已完成', status: 'completed' as const, updatedAt: 100, acknowledged: false },
      { id: 'running', title: '执行中', status: 'running' as const, updatedAt: 100, acknowledged: false },
    ] }
    renderFloatingIsland(root, snapshot, vi.fn())
    expect(root.querySelector('.status-dot')?.getAttribute('style')).toContain('#87a56e')
    expect(root.textContent).toContain('1')
  })
  it('shows counts for every non-empty task status', () => {
    const root = document.createElement('div')
    const snapshot = { ...base(), taskCounts: { none: 0, needsAction: 1, running: 2, completed: 1 }, tasks: [
      { id: 'action', title: '需确认', status: 'needs_action' as const, updatedAt: 100, acknowledged: false },
      { id: 'run-1', title: '执行中 1', status: 'running' as const, updatedAt: 100, acknowledged: false },
      { id: 'run-2', title: '执行中 2', status: 'running' as const, updatedAt: 100, acknowledged: false },
      { id: 'done', title: '已完成', status: 'completed' as const, updatedAt: 100, acknowledged: false },
    ] }
    renderFloatingIsland(root, snapshot, vi.fn())
    expect(root.querySelector('[data-status="needs_action"]')?.textContent).toContain('1')
    expect(root.querySelector('[data-status="running"]')?.textContent).toContain('2')
    expect(root.querySelector('[data-status="completed"]')?.textContent).toContain('1')
    expect(root.querySelector('[data-status="none"]')).toBeNull()
  })
  it('keeps the compact count equal to details when an acknowledged task runs again', () => {
    const root = document.createElement('div')
    const snapshot = { ...base(), taskCounts: { none: 0, needsAction: 0, running: 1, completed: 0 }, tasks: [
      { id: 'run-again', title: '重新运行', status: 'running' as const, updatedAt: 100, acknowledged: true },
      { id: 'run-now', title: '当前运行', status: 'running' as const, updatedAt: 101, acknowledged: false },
    ] }
    renderFloatingIsland(root, snapshot, vi.fn())
    expect(root.querySelector('[data-status="running"] b')?.textContent).toBe('2')
    const details = document.createElement('div')
    renderDetailsPanel(details, snapshot, vi.fn(), vi.fn(), vi.fn())
    expect(details.querySelector('.task-count')?.textContent).toContain('2')
  })
  it('shows gray zero only when there are no unacknowledged tasks', () => {
    const root = document.createElement('div')
    renderFloatingIsland(root, { ...base(), taskCounts: { none: 0, needsAction: 0, running: 0, completed: 0 }, tasks: [{ id: 'done', title: '已完成', status: 'completed' as const, updatedAt: 100, acknowledged: true }] }, vi.fn())
    expect(root.querySelector('[data-status="none"]')?.textContent).toContain('0')
    expect(root.querySelector('[data-status="running"]')).toBeNull()
  })
  it('marks stale data explicitly', () => { const root = document.createElement('div'); const snapshot = base(); snapshot.status = 'stale'; snapshot.error = '数据已过期'; renderDetailsPanel(root, snapshot, vi.fn(), vi.fn(), vi.fn()); expect(root.textContent).toContain('数据已过期'); })
  it('shows placeholders for a first error', () => { const root = document.createElement('div'); renderFloatingIsland(root, { ...base(), status: 'error', quotaRemainingPercent: undefined, todayTokens: undefined, tasks: [], error: '无法连接' }, vi.fn()); expect(root.textContent).toContain('--'); expect(root.textContent).toContain('数据待确认'); })
  it('keeps completed task until acknowledgement callback', () => { const root = document.createElement('div'); const acknowledge = vi.fn(); const snapshot = { ...base(), tasks: [{ id: 'done', title: '已完成任务', status: 'completed' as const, updatedAt: 100, acknowledged: false }] }; renderDetailsPanel(root, snapshot, vi.fn(), acknowledge, vi.fn()); expect(root.textContent).toContain('可验收'); root.querySelector<HTMLButtonElement>('[data-task-id="done"]')!.click(); expect(acknowledge).toHaveBeenCalledWith('done'); })
  it('hides a completed task after it has been acknowledged', () => { const root = document.createElement('div'); const snapshot = { ...base(), tasks: [{ id: 'done', title: '已验收任务', status: 'completed' as const, updatedAt: 100, acknowledged: true }] }; renderDetailsPanel(root, snapshot, vi.fn(), vi.fn(), vi.fn()); expect(root.querySelector('.task-row')).toBeNull(); expect(root.textContent).toContain('暂无任务记录'); })
  it('shows approval reason and keeps approval manual', () => {
    const root = document.createElement('div')
    const approval = vi.fn()
    const snapshot = { ...base(), tasks: [{ id: 'wait', title: '需要授权', status: 'needs_action' as const, waitingReason: '需要确认命令', approvalRequestId: 'req-1', updatedAt: 100, acknowledged: false }] }
    renderDetailsPanel(root, snapshot, vi.fn(), vi.fn(), vi.fn(), undefined, false, undefined, false, undefined, undefined, approval)
    expect(root.textContent).toContain('需要确认命令')
    expect(approval).not.toHaveBeenCalled()
    root.querySelector<HTMLButtonElement>('[data-decision="accept"]')!.click()
    expect(approval).toHaveBeenCalledWith('wait', 'accept')
  })
  it('keeps the conversation title above the current activity and status', () => {
    const root = document.createElement('div')
    const snapshot = { ...base(), tasks: [{ id: 'task', title: '对话标题', activity: '正在修复任务同步', status: 'running' as const, updatedAt: 100, acknowledged: false }] }
    renderDetailsPanel(root, snapshot, vi.fn(), vi.fn(), vi.fn())
    const task = root.querySelector('.task-row')!
    expect(task.querySelector('strong')?.textContent).toBe('对话标题')
    expect(task.querySelector('.task-activity')?.textContent).toContain('正在修复任务同步')
    expect(task.querySelector('.task-copy')?.textContent).toMatch(/对话标题.*正在修复任务同步.*正在执行/)
  })
  it('reveals pairing settings only when requested', () => {
    const root = document.createElement('div')
    const toggle = vi.fn()
    const pairing = { address: 'http://127.0.0.1:18765/', code: '1234' }
    renderDetailsPanel(root, base(), vi.fn(), vi.fn(), vi.fn(), pairing, false, undefined, false, toggle)
    expect(root.querySelector('.pairing-card')).toBeNull()
    root.querySelector<HTMLButtonElement>('.pairing-settings-button')!.click()
    expect(toggle).toHaveBeenCalledTimes(1)
    renderDetailsPanel(root, base(), vi.fn(), vi.fn(), vi.fn(), pairing, false, undefined, true, toggle)
    expect(root.querySelector('.pairing-card')).toBeTruthy()
  })
  it('keeps the ball DOM node stable when data changes', () => {
    const root = document.createElement('div')
    const mounted = mountFloatingBall(root, vi.fn())
    mounted.update(base())
    const button = root.querySelector('.floating-ball')
    mounted.update({ ...base(), quotaRemainingPercent: 71 })
    expect(root.querySelector('.floating-ball')).toBe(button)
  })
  it('keeps the island and task list mounted across updates', () => {
    const root = document.createElement('div')
    const mounted = mountFloatingIsland(root, vi.fn())
    mounted.update(base())
    const button = root.querySelector('.island-shell')
    mounted.update({ ...base(), todayTokens: 128401 })
    expect(root.querySelector('.island-shell')).toBe(button)
  })
  it('keeps the details task list node stable for metric-only updates', () => {
    const root = document.createElement('div')
    const mounted = mountDetailsPanel(root, vi.fn(), vi.fn(), vi.fn())
    mounted.update(base())
    const list = root.querySelector('.task-list')
    mounted.update({ ...base(), todayTokens: 128401 })
    expect(root.querySelector('.task-list')).toBe(list)
  })
  it('provides a dedicated native drag region in the details header', () => {
    const root = document.createElement('div')
    mountDetailsPanel(root, vi.fn(), vi.fn(), vi.fn())
    expect(root.querySelector('.details-drag-region')?.hasAttribute('data-tauri-drag-region')).toBe(true)
  })
})
