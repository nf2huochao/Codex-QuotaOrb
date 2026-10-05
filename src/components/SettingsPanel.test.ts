// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { mountSettingsPanel, SettingsActions } from './SettingsPanel'

const flush = async () => { await new Promise(resolve => setTimeout(resolve, 0)) }
function actions(): SettingsActions {
  return {
    onClose: vi.fn(), onCheckUpdates: vi.fn(),
    onGenerateDiagnosticReport: vi.fn().mockResolvedValue({ text: 'report', saved_path: 'report.txt' }),
    getAutostart: vi.fn().mockResolvedValue(false), setAutostart: vi.fn().mockResolvedValue(undefined),
    getAlwaysOnTop: vi.fn().mockResolvedValue(false), setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
    getCodexBinaryPath: vi.fn().mockResolvedValue(null), setCodexBinaryPath: vi.fn().mockResolvedValue('codex'),
    clearCodexBinaryPath: vi.fn().mockResolvedValue(undefined),
  }
}

it('locks settings while loading and serializes path writes', async () => {
  const root = document.createElement('div')
  const a = actions()
  let finish!: (value: string) => void
  a.setCodexBinaryPath = vi.fn(() => new Promise(resolve => { finish = resolve }))
  const view = mountSettingsPanel(root, a)
  const input = root.querySelector<HTMLInputElement>('.codex-path-input')!
  const save = root.querySelector<HTMLButtonElement>('.settings-path-save')!
  const clear = root.querySelector<HTMLButtonElement>('.settings-path-clear')!
  expect(input.disabled).toBe(true)
  expect(root.querySelector<HTMLInputElement>('.autostart-input')!.disabled).toBe(true)
  await flush()
  input.value = 'codex'
  save.click()
  clear.click()
  expect(input.disabled).toBe(true)
  expect(a.clearCodexBinaryPath).not.toHaveBeenCalled()
  finish('validated')
  await flush()
  expect(input.value).toBe('validated')
  expect(clear.disabled).toBe(false)
  view.destroy()
})

it('restores a failed switch write and removes stale diagnostic reports on retry', async () => {
  const root = document.createElement('div')
  const a = actions()
  a.setAutostart = vi.fn().mockRejectedValue(new Error('failed'))
  const view = mountSettingsPanel(root, a)
  await flush()
  const input = root.querySelector<HTMLInputElement>('.autostart-input')!
  input.checked = true
  input.dispatchEvent(new Event('change'))
  expect(input.disabled).toBe(true)
  await flush()
  expect(input.checked).toBe(false)
  expect(input.disabled).toBe(false)
  const generate = root.querySelector<HTMLButtonElement>('.diagnostics-generate')!
  generate.click()
  await flush()
  expect(root.querySelector<HTMLTextAreaElement>('.diagnostics-preview')!.value).toBe('report')
  a.onGenerateDiagnosticReport = vi.fn().mockRejectedValue(new Error('failed'))
  generate.click()
  await flush()
  expect(root.querySelector<HTMLTextAreaElement>('.diagnostics-preview')!.hidden).toBe(true)
  expect(root.querySelector<HTMLButtonElement>('.diagnostics-copy')!.hidden).toBe(true)
  view.destroy()
})
