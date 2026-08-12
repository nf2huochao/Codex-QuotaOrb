// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'
import { mountFloatingBall, mountFloatingIsland, renderFloatingBall, renderFloatingIsland } from './FloatingIsland'
import { mountDetailsPanel, renderDetailsPanel } from './DetailsPanel'
import { Snapshot } from '../domain'

const base = (): Snapshot => ({ status: 'fresh', fetchedAt: 100, quotaRemainingPercent: 72, quotaResetsAt: 200, plan: 'Plus', resetCredits: 1, todayTokens: 128400, activeTaskCount: 3, schemaVersion: '1.0', tasks: [1, 2, 3].map((id) => ({ id: String(id), title: `任务 ${id}`, status: 'running', updatedAt: 100, acknowledged: false })) })

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
  it('shows quota ring, task count and compact tokens', () => { const root = document.createElement('div'); renderFloatingIsland(root, base(), vi.fn()); expect(root.textContent).toContain('72%'); expect(root.textContent).toContain('3 个任务执行中'); expect(root.textContent).toContain('128.4K'); expect(root.querySelector('.quota-ring')).toBeTruthy(); expect(root.querySelector('canvas,img')).toBeNull(); expect(root.querySelector<HTMLElement>('.quota-ring')?.style.getPropertyValue('--quota')).toBe('259.2deg') })
  it('shows green completion as soon as one task is complete', () => {
    const root = document.createElement('div')
    const snapshot = { ...base(), tasks: [
      { id: 'done', title: '已完成', status: 'completed' as const, updatedAt: 100, acknowledged: false },
      { id: 'running', title: '执行中', status: 'running' as const, updatedAt: 100, acknowledged: false },
    ] }
    renderFloatingIsland(root, snapshot, vi.fn())
    expect(root.querySelector('.status-dot')?.getAttribute('style')).toContain('#87a56e')
    expect(root.textContent).toContain('1 个任务已完成')
  })
  it('marks stale data explicitly', () => { const root = document.createElement('div'); const snapshot = base(); snapshot.status = 'stale'; snapshot.error = '数据已过期'; renderDetailsPanel(root, snapshot, vi.fn(), vi.fn(), vi.fn()); expect(root.textContent).toContain('数据已过期'); })
  it('shows placeholders for a first error', () => { const root = document.createElement('div'); renderFloatingIsland(root, { ...base(), status: 'error', quotaRemainingPercent: undefined, todayTokens: undefined, tasks: [], error: '无法连接' }, vi.fn()); expect(root.textContent).toContain('--'); expect(root.textContent).toContain('数据待确认'); })
  it('keeps completed task until acknowledgement callback', () => { const root = document.createElement('div'); const acknowledge = vi.fn(); const snapshot = { ...base(), tasks: [{ id: 'done', title: '已完成任务', status: 'completed' as const, updatedAt: 100, acknowledged: false }] }; renderDetailsPanel(root, snapshot, vi.fn(), acknowledge, vi.fn()); expect(root.textContent).toContain('已验收'); root.querySelector<HTMLButtonElement>('[data-task-id="done"]')!.click(); expect(acknowledge).toHaveBeenCalledWith('done'); })
  it('reveals pairing settings only when requested', () => {
    const root = document.createElement('div')
    const toggle = vi.fn()
    const pairing = { address: 'http://127.0.0.1:18765/?pair=test', token: 'test' }
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
