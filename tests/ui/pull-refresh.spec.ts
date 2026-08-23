import { test, expect, type Page, type Route } from '@playwright/test'

function dispatchTouch(page: Page, type: string, y: number) {
  return page.evaluate(({ type, y }) => {
    const event = new Event(type, { bubbles: true, cancelable: true })
    Object.defineProperty(event, 'touches', { value: type === 'touchend' ? [] : [{ clientY: y }] })
    Object.defineProperty(event, 'changedTouches', { value: [{ clientY: y }] })
    window.dispatchEvent(event)
  }, { type, y })
}

test('top pull shows the hint and refreshes once after the threshold', async ({ page }) => {
  test.setTimeout(30000)
  let snapshotCalls = 0
  await page.addInitScript(() => localStorage.setItem('codex-pair-token:http://127.0.0.1:4173', 'session-token'))
  const fulfillSnapshot = async (route: Route) => {
    snapshotCalls += 1
    if (snapshotCalls > 1) await new Promise((resolve) => setTimeout(resolve, 120))
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ status: 'fresh', changed_at: snapshotCalls, fetched_at: snapshotCalls, source: 'metrics-poll', quota_remaining_percent: 72, today_tokens: 100, tasks: [], schema_version: '1.0' }),
    })
  }
  await page.route('**/api/snapshot**', fulfillSnapshot)
  await page.route('**/api/refresh**', fulfillSnapshot)
  await page.goto('/web/index.html')
  await expect(page.locator('#dashboard')).toBeVisible()
  await page.waitForTimeout(100)
  const callsBeforePull = snapshotCalls

  await dispatchTouch(page, 'touchstart', 100)
  await dispatchTouch(page, 'touchmove', 130)
  await expect(page.locator('#pull-refresh')).toBeVisible()
  await expect(page.locator('#pull-refresh')).toContainText('下滑刷新')
  await dispatchTouch(page, 'touchend', 130)
  expect(snapshotCalls).toBe(callsBeforePull)

  await dispatchTouch(page, 'touchstart', 100)
  await dispatchTouch(page, 'touchmove', 160)
  await expect(page.locator('#pull-refresh')).toBeVisible()
  await expect(page.locator('#pull-refresh')).toContainText('松开刷新')
  await dispatchTouch(page, 'touchend', 160)
  await expect(page.locator('#pull-refresh')).toContainText('正在刷新')
  await expect.poll(() => snapshotCalls).toBe(callsBeforePull + 1)
  await expect(page.locator('#pull-refresh')).toContainText('已更新 ·')
  await expect(page.locator('.fresh')).toContainText('指标轮询')
  await expect(page.locator('#pull-refresh')).toBeHidden()
})
