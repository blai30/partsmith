/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  cancelWordStores,
  dropFile,
  inspector,
  inspectorInput,
  measures,
  openApp,
  pageTime,
  renameBody,
  runCommand,
  test,
  timelineItem,
  viewSurface,
  watchCancelWord,
} from './helpers'

// Stopping a request on the web (ui.md 6.5 E-171 and E-172, ADR 0063): Stop keeps the undo
// history, and a newer value drops the preview that is running.

/** How long these tests may take: each rebuilds a document of 50 features or a 5 by 5 pattern. */
const STOP_TEST_TIMEOUT_MILLISECONDS = 60_000

/** The text of the rebuild_stopped and rebuild_stopped_one notifications (ui.md 4.3). */
const REBUILD_STOPPED = /^The rebuild stopped before '.+' was built\./

/** The text of changes_not_applied (ui.md 4.3). */
const CHANGES_NOT_APPLIED =
  'Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them.'

/** Opens the Parameters panel and returns the expression input of the parameter `name` (3.11). */
async function parameterExpression(page: Page, name: string): Promise<Locator> {
  await runCommand(page, 'Parameters')
  const expression = page
    .getByTestId('parameters-panel')
    .getByRole('combobox', { name: `Expression of ${name}`, exact: true })
  await expect(expression).toBeVisible()
  return expression
}

/** Opens the command palette with `text` typed into it. */
async function openPaletteWith(page: Page, text: string): Promise<void> {
  await page.keyboard.press('ControlOrMeta+K')
  const input = page.getByTestId('palette-input')
  await expect(input).toBeFocused()
  await input.fill(text)
}

/** The palette's Undo option, whose detail names the undo entry or says there is none (3.20). */
function undoOption(page: Page): Locator {
  return page.locator('[data-testid="palette-option"][data-item-id="edit.undo"]')
}

/** The rebuild_stopped notifications on the page. */
function rebuildStopped(page: Page): Locator {
  return page.getByTestId('notification').filter({ hasText: REBUILD_STOPPED })
}

/** The timeline items whose last status is not_run. */
function notBuilt(page: Page): Locator {
  return page.locator('[data-testid="timeline-item"][data-status="not_run"]')
}

/** Records every value `app`'s `data-core-status` takes from now on, in the page. */
async function watchCoreStatus(page: Page): Promise<void> {
  await page.evaluate(() => {
    const app = document.querySelector('[data-testid="app"]')
    const seen: string[] = [app?.getAttribute('data-core-status') ?? '']
    Reflect.set(window, 'partsmithCoreStatuses', seen)
    if (app !== null) {
      new MutationObserver(() => {
        seen.push(app.getAttribute('data-core-status') ?? '')
      }).observe(app, { attributes: true, attributeFilter: ['data-core-status'] })
    }
  })
}

/** The values `watchCoreStatus` recorded. */
function coreStatuses(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const seen: unknown = Reflect.get(window, 'partsmithCoreStatuses')
    return Array.isArray(seen) ? seen.map(String) : []
  })
}

test('E-171 Stop keeps the undo history on the web', async ({ page }) => {
  test.setTimeout(STOP_TEST_TIMEOUT_MILLISECONDS)
  await openApp(page)
  await dropFile(page, 'part-50.ptsm')
  const renamed = await pageTime(page)
  await renameBody(page, 'Base', 'Plate')
  await expect
    .poll(async () => (await measures(page, 'ps:rebuild', renamed)).length)
    .toBeGreaterThan(0)

  const base = await parameterExpression(page, 'base')
  await watchCoreStatus(page)
  await base.fill('250 mm')
  await base.press('Enter')
  const stop = page.getByTestId('core-stop')
  await stop.click()

  await expect(page.getByTestId('status-core')).toBeHidden({ timeout: 5_000 })
  await expect(notBuilt(page).first()).toBeVisible({ timeout: 5_000 })
  await expect(rebuildStopped(page)).toHaveAttribute('data-severity', 'info', { timeout: 5_000 })
  expect(new Set(await coreStatuses(page))).toEqual(new Set(['ready']))
  await expect(
    page.getByTestId('notification').filter({ hasText: CHANGES_NOT_APPLIED }),
  ).toHaveCount(0)

  await viewSurface(page).focus()
  await page.keyboard.press('ControlOrMeta+z')
  await expect(base).toHaveValue('240 mm')
  await expect(notBuilt(page)).toHaveCount(0)
  await page.keyboard.press('ControlOrMeta+z')
  await expect(bodyRow(page, 'Base')).toBeVisible()

  await page.keyboard.press('ControlOrMeta+Shift+z')
  await page.keyboard.press('ControlOrMeta+Shift+z')
  await expect(base).toHaveValue('250 mm')
  await expect(rebuildStopped(page)).toHaveCount(0)
  await expect(notBuilt(page)).toHaveCount(0)
  await expect(page.getByTestId('timeline-item').first()).not.toHaveAttribute(
    'data-status',
    'pending',
  )
})

test('E-172 a newer value drops the running preview', async ({ page }) => {
  test.setTimeout(STOP_TEST_TIMEOUT_MILLISECONDS)
  await watchCancelWord(page)
  await openApp(page)
  await dropFile(page, 'pattern-25.ptsm')
  await timelineItem(page, 'Linear pattern 1')
    .getByRole('button', { name: 'Linear pattern 1', exact: true })
    .dblclick()
  await expect(inspector(page)).toHaveAccessibleName('Linear pattern 1')
  const spacing = inspectorInput(page, 'Spacing')
  await expect(spacing).toHaveValue('15 mm')

  await spacing.fill('20 mm')
  // The 20 mm preview takes well over 300 ms in the web core. It is still running.
  await page.waitForTimeout(300)
  await spacing.fill('25 mm')
  const second = Date.now()
  await expect.poll(() => cancelWordStores(page)).toHaveLength(1)

  // The preview shows within 10 s of the second value (ui.md E-172).
  await expect(inspector(page)).toHaveAttribute('data-preview', 'shown', {
    // Playwright reads a timeout of 0 as no timeout. The window keeps at least 1 ms.
    timeout: Math.max(1, second + 10_000 - Date.now()),
  })
  await expect(spacing).toHaveValue('25 mm')
  expect(await cancelWordStores(page)).toHaveLength(1)
  await expect(rebuildStopped(page)).toHaveCount(0)

  await spacing.press('Enter')
  // The double-click's clicks selected Linear pattern 1. The selection inspector may take the
  // place of the command's (ui.md 3.8, 3.9).
  await expect(inspector(page)).not.toHaveAttribute('data-mode', 'command')
  // One undo entry: the palette's Undo names it, and after one undo nothing is left to undo.
  await openPaletteWith(page, 'Undo')
  await expect(undoOption(page)).toContainText('Undo Edit Linear pattern 1')
  await page.keyboard.press('Escape')
  await viewSurface(page).focus()
  await page.keyboard.press('ControlOrMeta+z')
  await expect(spacing).toBeHidden()
  // The undo waits in the core behind the commit's rebuild, and the palette reads the history
  // when it opens. It opens again until the undo has landed.
  await expect
    .poll(
      async () => {
        await openPaletteWith(page, 'Undo')
        const detail = await undoOption(page).textContent()
        await page.keyboard.press('Escape')
        return detail
      },
      { timeout: 10_000 },
    )
    .toContain('Nothing to undo.')
})
