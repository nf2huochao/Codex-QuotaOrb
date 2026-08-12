import { test, expect } from '@playwright/test'

test('unpaired page shows pairing and Kindle fallback guidance', async ({ page }) => {
  await page.goto('/web/index.html')
  await expect(page.locator('#pairing')).toBeVisible()
  await expect(page.locator('#pair-input')).toBeVisible()
  expect(await page.content()).toContain('JavaScript')
})

test('pairing page accepts a pasted URL or pairing code', async ({ page }) => {
  await page.goto('/web/index.html')
  await page.locator('#pair-input').fill('not-a-pair-code')
  await page.locator('#pair-form').getByRole('button').click()
  await expect(page.locator('#pair-error')).toContainText('格式不正确')
  await page.locator('#pair-input').fill('0123456789abcdef0123456789abcdef')
  await page.locator('#pair-form').getByRole('button').click()
  await expect(page).toHaveURL(/\?pair=0123456789abcdef0123456789abcdef/)
})

test('paired page keeps the last snapshot when the connection drops', async ({ page }) => {
  await page.addInitScript(() => {
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
          this.onmessage?.({ data: JSON.stringify({ status: 'fresh', fetched_at: 100, quota_remaining_percent: 72, today_tokens: 1234, tasks: [{ id: 'done', title: '完成任务', status: 'completed', acknowledged: false }], schema_version: '1.0' }) })
          setTimeout(() => this.onclose?.(), 20)
        }, 0)
      }
      close() { this.readyState = 3 }
    }
    Object.defineProperty(window, 'WebSocket', { value: FakeWebSocket })
  })
  await page.route('**/api/snapshot**', (route) => route.abort())
  await page.goto('/web/index.html?pair=0123456789abcdef0123456789abcdef')
  await expect(page.locator('#island')).toContainText('72%')
  await expect(page.locator('#island')).toContainText('1 个任务已完成')
  await expect(page.locator('.fresh')).toContainText('正在重连')
  await expect(page.locator('#island')).toContainText('72%')
})
