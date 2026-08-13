import { test, expect } from '@playwright/test'

test('unpaired page shows four digit pairing guidance', async ({ page }) => {
  await page.goto('/web/index.html')
  await expect(page.locator('#pairing')).toBeVisible()
  await expect(page.locator('#pair-input')).toHaveAttribute('maxlength', '4')
  expect(await page.content()).toContain('JavaScript')
})

test('pairing page exchanges and remembers a four digit code', async ({ page }) => {
  await page.goto('/web/index.html')
  await page.locator('#pair-input').fill('abcd')
  await page.locator('#pair-form').getByRole('button').click()
  await expect(page.locator('#pair-error')).toContainText('四位')
  await page.route('**/api/pair', async (route) => {
    const body = JSON.parse(route.request().postData() || '{}')
    if (body.code !== '1234') return route.fulfill({ status: 401, body: 'invalid' })
    return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ token: 'session-token' }) })
  })
  await page.locator('#pair-input').fill('1234')
  await page.locator('#pair-form').getByRole('button').click()
  await expect(page.locator('#dashboard')).toBeVisible()
  expect(await page.evaluate(() => localStorage.getItem('codex-pair-token:http://127.0.0.1:4173'))).toBe('session-token')
})

test('paired page keeps the last snapshot when the connection drops', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('codex-pair-token:http://127.0.0.1:4173', 'session-token')
    class FakeWebSocket {
      static OPEN = 1
      readyState = 1
      onopen?: () => void
      onmessage?: (event: { data: string }) => void
      onclose?: () => void
      onerror?: () => void
      constructor() {
        setTimeout(() => {
          this.onopen?.()
          this.onmessage?.({ data: JSON.stringify({ status: 'fresh', changed_at: 100, fetched_at: 100, quota_remaining_percent: 72, today_tokens: 1234, tasks: [{ id: 'done', title: '完成任务', status: 'completed', acknowledged: false }], schema_version: '1.0' }) })
          this.onmessage?.({ data: JSON.stringify({ status: 'fresh', changed_at: 90, fetched_at: 90, quota_remaining_percent: 1, today_tokens: 1, tasks: [], schema_version: '1.0' }) })
          setTimeout(() => this.onclose?.(), 20)
        }, 0)
      }
      close() { this.readyState = 3 }
    }
    Object.defineProperty(window, 'WebSocket', { value: FakeWebSocket })
  })
  await page.route('**/api/snapshot**', (route) => route.abort())
  await page.goto('/web/index.html')
  await expect(page.locator('#island')).toContainText('72%')
  await expect(page.locator('#island')).toContainText('1 个任务已完成')
  await expect(page.locator('.fresh')).toContainText('正在重连')
  await expect(page.locator('#island')).toContainText('72%')
})
