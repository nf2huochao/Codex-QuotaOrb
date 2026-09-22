import { test, expect, type Page } from '@playwright/test'

const snapshot = { status: 'fresh', changed_at: 100, fetched_at: 100, plan: 'plus', five_hour_remaining_percent: 85, quota_remaining_percent: 40, today_tokens: 260000000, tasks: [{ id: 'done', title: '验收任务', status: 'completed', acknowledged: false }], schema_version: '1.1' }

async function isolateConnection(page: Page, paired = true) {
  await page.addInitScript((paired) => {
    if (paired) localStorage.setItem('codex-pair-token:' + location.origin, 'expired-session')
    class QuietSocket { static OPEN = 1; static CONNECTING = 0; readyState = 1; close() { this.readyState = 3 } }
    Object.defineProperty(window, 'WebSocket', { value: QuietSocket })
  }, paired)
  await page.route('https://codex.lunarwerx.com/**', route => route.abort())
}

test('expired saved pairing can reconnect without reloading and resumes offline feedback', async ({ page }) => {
  await isolateConnection(page)
  let paired = false
  await page.route('**/api/snapshot', route => route.fulfill(paired ? { json: snapshot } : { status: 401, body: 'expired' }))
  await page.route('**/api/pair', route => {
    paired = true
    return route.fulfill({ json: { token: 'new-test-session' } })
  })
  await page.goto('/web/index.html')
  await expect(page.locator('#pair-error')).toContainText('配对')
  await page.locator('#pair-input').fill('1234')
  await page.locator('.pair-submit').click()
  await expect(page.locator('#dashboard')).toBeVisible()
  await expect(page.locator('#details h1')).toContainText('85%')
  await page.evaluate(() => window.dispatchEvent(new Event('offline')))
  await expect(page.locator('.fresh')).toContainText('网络已断开')
})

test('first pairing installs the reconnect listeners and failed review stays actionable', async ({ page }) => {
  await isolateConnection(page, false)
  await page.route('**/api/pair', route => route.fulfill({ json: { token: 'new-test-session' } }))
  await page.route('**/api/snapshot', route => route.fulfill({ json: snapshot }))
  await page.route('**/api/tasks/done/acknowledge', route => route.fulfill({ status: 409, body: 'changed' }))
  await page.goto('/web/index.html')
  await page.locator('#pair-input').fill('1234')
  await page.locator('.pair-submit').click()
  await expect(page.locator('.ack')).toBeVisible()
  await page.locator('.ack').click()
  await expect(page.locator('.fresh')).toContainText('未能标记已验收')
  await expect(page.locator('.ack')).toBeEnabled()
  await page.evaluate(() => window.dispatchEvent(new Event('offline')))
  await expect(page.locator('.fresh')).toContainText('网络已断开')
})

test('English phone layout and stale HTTP success never claim a successful refresh', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await isolateConnection(page)
  await page.route('**/api/snapshot', route => route.fulfill({ json: snapshot }))
  await page.route('**/api/refresh', route => route.fulfill({ json: { ...snapshot, changed_at: 101, status: 'stale', error: '读取 Codex 数据超时' } }))
  await page.goto('/web/index.html')
  await page.locator('#language-toggle').click()
  await expect(page.locator('#island')).toContainText('260M')
  await expect(page.locator('#details h1')).toHaveText('5-hour 85%')
  await page.evaluate(() => {
    for (const [type, y] of [['touchstart', 100], ['touchmove', 170], ['touchend', 170]] as const) {
      const event = new Event(type, { bubbles: true, cancelable: true })
      Object.defineProperty(event, 'touches', { value: type === 'touchend' ? [] : [{ clientY: y }] })
      window.dispatchEvent(event)
    }
  })
  await expect(page.locator('.refresh-feedback')).toHaveAttribute('data-state', 'error')
  await expect(page.locator('.fresh')).toContainText('last successful')
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
})
