import { test, expect } from '@playwright/test'

test('design preview updates mounted views without replacing the surface', async ({ page }) => {
  await page.goto('/?design-preview=1')
  await expect(page.locator('.floating-ball')).toBeVisible()

  await page.evaluate(() => {
    const apply = (window as Window & { __codexTestApplySnapshot?: (value: unknown) => void }).__codexTestApplySnapshot!
    const snapshot = { status: 'fresh', changed_at: 10, source: 'metrics-poll', quota_remaining_percent: 22, today_tokens: 100, active_task_count: 0, tasks: [], schema_version: '1.0' }
    ;(window as Window & { __ballNode?: Element }).__ballNode = document.querySelector('.floating-ball')!
    for (let index = 0; index < 20; index += 1) apply(snapshot)
  })
  await page.waitForTimeout(220)
  expect(await page.evaluate(() => document.querySelector('.floating-ball') === (window as Window & { __ballNode?: Element }).__ballNode)).toBe(true)

  await page.evaluate(() => {
    const apply = (window as Window & { __codexTestApplySnapshot?: (value: unknown) => void }).__codexTestApplySnapshot!
    apply({ status: 'fresh', changed_at: 11, source: 'metrics-poll', quota_remaining_percent: 23, today_tokens: 101, active_task_count: 0, tasks: [], schema_version: '1.0' })
  })
  await page.waitForTimeout(220)
  await expect(page.locator('.floating-ball')).toContainText('23%')
})

test('single click does not advance; double click cycles the three views', async ({ page }) => {
  await page.goto('/?design-preview=1')
  await expect(page.locator('.floating-ball')).toBeVisible()
  await expect(page.locator('#island-root')).toBeHidden()
  await expect(page.locator('#details-root')).toBeHidden()
  await page.locator('.floating-ball').click()
  await expect(page.locator('#app')).toHaveAttribute('data-view', 'ball')
  await page.locator('.floating-ball').dispatchEvent('dblclick')
  await expect(page.locator('#app')).toHaveAttribute('data-view', 'summary')
  await expect(page.locator('#ball-root')).toBeHidden()
  await expect(page.locator('.island-shell')).toBeVisible()
  await expect(page.locator('#details-root')).toBeHidden()
  await page.locator('.island-shell').dispatchEvent('dblclick')
  await expect(page.locator('#app')).toHaveAttribute('data-view', 'details')
  await expect(page.locator('#ball-root')).toBeHidden()
  await expect(page.locator('#island-root')).toBeHidden()
  await expect(page.locator('.details-panel')).toBeVisible()
  await page.locator('.details-panel').dispatchEvent('dblclick')
  await expect(page.locator('#app')).toHaveAttribute('data-view', 'ball')
  await expect(page.locator('.floating-ball')).toBeVisible()
  await expect(page.locator('#island-root')).toBeHidden()
  await expect(page.locator('#details-root')).toBeHidden()
})
