/// <reference lib="dom" />
import { expect, test } from '@playwright/test'

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
