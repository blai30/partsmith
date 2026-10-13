/// <reference lib="dom" />
import { expect, test } from '@playwright/test'
import { longTasks, measures, openApp, pageTime, percentile, watchLongTasks } from './helpers'

/** How long the busy task of the long task check runs, and the least duration it must report. */
const BUSY_MILLISECONDS = 120
const BUSY_REPORTED_MILLISECONDS = 100

test('the production build starts the WebAssembly core in a cross-origin isolated page', async ({
  page,
}) => {
  await page.goto('/')
  await expect(page).toHaveTitle('Partsmith')
  const app = page.getByTestId('app')
  // Compiling the core's WebAssembly can take longer than the default expect timeout on a cold runner.
  await expect(app).not.toHaveAttribute('data-core-status', 'loading', { timeout: 30_000 })
  await expect(app).toHaveAttribute('data-core-status', 'ready')
  expect(await page.evaluate(() => window.crossOriginIsolated)).toBe(true)
  // The first frame with the startup document and the core ready (ui.md 2.18).
  await expect
    .poll(() => page.evaluate(() => performance.getEntriesByName('ps:usable').length))
    .toBe(1)
})

// The helpers that only the @perf tests use (ui.md 6.3, 6.7), checked on every run.

test('percentile is the nearest-rank percentile of the values in any order', () => {
  const oneToTwenty = Array.from({ length: 20 }, (_, i) => i + 1)
  expect(percentile([5], 0.95)).toBe(5)
  expect(percentile(oneToTwenty.slice(0, 10).toReversed(), 0.95)).toBe(10)
  expect(percentile(oneToTwenty, 0.5)).toBe(10)
  expect(percentile(oneToTwenty, 0.95)).toBe(19)
  expect(() => percentile([], 0.5)).toThrow('A percentile needs at least one value.')
})

test('longTasks and measures report what started at or after a page time', async ({ page }) => {
  await watchLongTasks(page)
  await openApp(page)
  await page.evaluate(() => {
    performance.measure('helpers-check', { start: performance.now(), duration: 1 })
  })
  const since = await pageTime(page)
  // The busy loop runs in a task of the page's own, since script that Playwright evaluates
  // directly is not a task the page reports.
  await page.evaluate(
    (milliseconds) =>
      new Promise<void>((resolve) => {
        setTimeout(() => {
          const end = performance.now() + milliseconds
          while (performance.now() < end) {
            // Busy, so this task is a long task.
          }
          performance.measure('helpers-check', { start: performance.now(), duration: 2 })
          resolve()
        })
      }),
    BUSY_MILLISECONDS,
  )
  const after = await pageTime(page)
  const longest = async (start: number) => Math.max(0, ...(await longTasks(page, start)))
  await expect.poll(() => longest(since)).toBeGreaterThanOrEqual(BUSY_REPORTED_MILLISECONDS)
  expect(await longest(after)).toBeLessThan(BUSY_REPORTED_MILLISECONDS)
  expect(await measures(page, 'helpers-check', since)).toEqual([2])
  expect(await measures(page, 'helpers-check')).toEqual([1, 2])
})
