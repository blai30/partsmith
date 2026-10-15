/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import { download, viewport, type Download, type PagePoint } from '../helpers'

// Helpers for the viewport navigation, picking, Print context and export end-to-end tests
// (ui.md 6.5 E-41 to E-55, E-65, E-95, E-97, E-100 and E-130 to E-132): camera gestures, and
// locators for the Print toolbar, the print check badge and panel, and the export popover. The
// shared helpers of 6.3 are in ../helpers.ts.

/** How long a print analysis may take: the core meshes and checks every body. */
const ANALYSIS_TIMEOUT_MILLISECONDS = 20_000

/** The camera target the viewport mirrors in `data-camera-target` (ui.md 3.13). */
export async function cameraTarget(page: Page): Promise<string> {
  return (await viewport(page).getAttribute('data-camera-target')) ?? ''
}

/** Expects the camera target to change from `before`. */
export async function expectTargetChanged(page: Page, before: string): Promise<void> {
  await expect(viewport(page)).not.toHaveAttribute('data-camera-target', before)
}

/**
 * Drags on the canvas with mouse buttons pressed in the order given and released in reverse, in
 * small steps so the gesture passes its 3-pixel threshold (ui.md 3.13). With `shift`, Shift is
 * held for the whole drag.
 */
export async function dragOnCanvas(
  page: Page,
  from: PagePoint,
  to: PagePoint,
  buttons: readonly ('left' | 'middle' | 'right')[],
  { shift = false }: { readonly shift?: boolean } = {},
): Promise<void> {
  if (shift) {
    await page.keyboard.down('Shift')
  }
  await page.mouse.move(from.x, from.y)
  for (const button of buttons) {
    // The buttons go down one after another, in the order the gesture needs.
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.down({ button })
  }
  await page.mouse.move(to.x, to.y, { steps: 10 })
  for (const button of buttons.toReversed()) {
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.up({ button })
  }
  if (shift) {
    await page.keyboard.up('Shift')
  }
}

/** Sends `count` wheel events of `deltaY` at a page position, one after another. */
export async function wheelAt(
  page: Page,
  at: PagePoint,
  deltaY: number,
  count: number,
): Promise<void> {
  await page.mouse.move(at.x, at.y)
  for (let i = 0; i < count; i++) {
    // The wheel events are separate events, sent one after another.
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.wheel(0, deltaY)
  }
}

/** Chooses a context with the top bar's context switcher and waits for it (ui.md 3.3). */
export async function chooseContext(page: Page, context: 'Model' | 'Print'): Promise<void> {
  await page
    .getByRole('radiogroup', { name: 'Context' })
    .getByRole('radio', { name: context, exact: true })
    .click()
  await expect(page.getByTestId('app')).toHaveAttribute('data-context', context.toLowerCase())
}

/** The Print context's toolbar (ui.md 3.3). */
export function printToolbar(page: Page): Locator {
  return page.getByRole('toolbar', { name: 'Print tools' })
}

/** A button of the Print toolbar whose accessible name is exactly `name`. */
export function printTool(page: Page, name: string): Locator {
  return printToolbar(page).getByRole('button', { name, exact: true })
}

/** The Print toolbar's printer menu button, whose name starts with "Printer:" (ui.md 3.17). */
export function printerButton(page: Page): Locator {
  return printToolbar(page).getByRole('button', { name: /^Printer:/ })
}

/** The print check badge in the status bar (ui.md 3.17). */
export function healthBadge(page: Page): Locator {
  return page.getByTestId('health-badge')
}

/**
 * Waits for a print report of the current document revision and orientation: the badge is
 * neither running, out of date nor without a report (ui.md 3.17).
 */
export async function expectCurrentReport(page: Page): Promise<void> {
  const badge = healthBadge(page)
  await expect(badge).toHaveAttribute('data-state', /^(ok|warning|error)$/, {
    timeout: ANALYSIS_TIMEOUT_MILLISECONDS,
  })
  await expect(badge).toHaveAttribute('data-stale', 'false', {
    timeout: ANALYSIS_TIMEOUT_MILLISECONDS,
  })
}

/**
 * Starts recording whether the print check badge leaves its report: it goes out of date or starts
 * a check (ui.md 3.17). The returned function waits until it has, so a test can tell the next
 * report from the one shown before an action, however fast the check runs.
 */
export async function watchReportLeft(page: Page): Promise<() => Promise<void>> {
  const flag = 'partsmithReportLeft'
  await page.evaluate((name) => {
    Reflect.set(window, name, false)
    const observer = new MutationObserver(() => {
      const left = document.querySelector(
        '[data-testid=health-badge][data-stale=true], [data-testid=health-badge][data-state=running]',
      )
      if (left !== null) {
        Reflect.set(window, name, true)
        observer.disconnect()
      }
    })
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-stale', 'data-state'],
    })
  }, flag)
  return async () => {
    await expect
      .poll(() => page.evaluate((name) => Reflect.get(window, name) === true, flag))
      .toBe(true)
  }
}

/** The rows of the print check panel, optionally only those of one check (ui.md 3.17). */
export function healthItems(page: Page, check?: string): Locator {
  const items = page.getByTestId('health-list').getByTestId('health-item')
  return check === undefined ? items : items.and(page.locator(`[data-check=${check}]`))
}

/** Opens the print check panel with the Print toolbar's "Print check" button (ui.md 3.17). */
export async function openPrintCheck(page: Page): Promise<void> {
  await printToolbar(page)
    .getByRole('button', { name: /^Print check/ })
    .click()
  await expect(page.getByTestId('health-list')).toBeVisible()
}

/** The top bar's Export button, which opens the export popover (ui.md 3.18). */
export function exportButton(page: Page): Locator {
  const topBar = page.getByTestId('top-bar')
  // In Print the context toolbar, also in the top bar, has an "Export" group button of its own.
  return topBar
    .getByRole('button', { name: 'Export', exact: true })
    .and(topBar.locator('xpath=.//button[not(ancestor::*[@role="toolbar"])]'))
}

/** Opens the export popover with the top bar's Export button (ui.md 3.18). */
export async function openExportPopover(page: Page): Promise<Locator> {
  await exportButton(page).click()
  const popover = page.getByTestId('export-popover')
  await expect(popover).toBeVisible()
  return popover
}

/** The text of every status line of the export popover, joined by line breaks (ui.md 3.18). */
export async function exportStatusText(page: Page): Promise<string> {
  const lines = await page
    .getByTestId('export-popover')
    .getByTestId('export-status')
    .allInnerTexts()
  return lines.join('\n')
}

/**
 * Exports through the export popover: opens it, chooses the format and clicks its Export button.
 * Returns the download.
 */
export async function exportFromPopover(page: Page, format: string): Promise<Download> {
  const popover = await openExportPopover(page)
  await popover
    .getByRole('radiogroup', { name: 'Format' })
    .getByRole('radio', { name: format, exact: true })
    .check()
  const button = popover.getByRole('button', { name: 'Export', exact: true })
  return download(page, () => button.click())
}
