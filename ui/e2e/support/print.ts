/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { expect, type Locator, type Page } from '@playwright/test'

// Helpers for the viewport navigation, picking, Print context and export end-to-end tests
// (ui.md 6.3, 6.5 E-41 to E-55, E-65, E-95, E-97 and E-100): opening the app and fixtures, the
// canvas geometry the spec's positions are relative to, camera gestures, downloads, and locators
// for the Print toolbar, the print check badge and panel, and the export popover.

const fixtures = new URL('../fixtures/', import.meta.url)

/** How long opening a fixture may take: the core rebuilds the whole document. */
const OPEN_TIMEOUT_MILLISECONDS = 30_000

/** How long a print analysis may take: the core meshes and checks every body. */
const ANALYSIS_TIMEOUT_MILLISECONDS = 20_000

/** The canvas box in page pixels, its center and its height `H` (ui.md 6.3). */
export type CanvasBox = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  readonly cx: number
  readonly cy: number
  readonly H: number
}

/** A position on the page in CSS pixels. */
export type Point = { readonly x: number; readonly y: number }

/** The standard views of the view cube that `fitView` uses. */
export type StandardView = 'Top' | 'Bottom' | 'Front' | 'Back' | 'Left' | 'Right'

/** The suggested name and bytes of a download. */
export type Download = { readonly name: string; readonly bytes: Buffer }

/**
 * Opens the app and waits for the core to be ready (ui.md 6.3). Without File System Access an
 * init script removes both pickers before every load of the page. Camera moves are not animated
 * (reduced motion, 2.11), so a check right after a view change sees the final camera.
 */
export async function openApp(
  page: Page,
  { fileSystemAccess = true }: { readonly fileSystemAccess?: boolean } = {},
): Promise<void> {
  if (!fileSystemAccess) {
    await page.addInitScript(() => {
      Reflect.deleteProperty(window, 'showOpenFilePicker')
      Reflect.deleteProperty(window, 'showSaveFilePicker')
    })
  }
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('/')
  await expectReady(page)
}

/** Waits until the core runs and the startup document is open (ui.md 3.1). */
export async function expectReady(page: Page): Promise<void> {
  // Compiling the core's WebAssembly can take longer than the default expect timeout.
  await expect(page.getByTestId('app')).toHaveAttribute('data-core-status', 'ready', {
    timeout: 30_000,
  })
}

/** The number of `ps:open` measures, which grows when an opened document's first frame is drawn. */
function openCount(page: Page): Promise<number> {
  return page.evaluate(() => performance.getEntriesByName('ps:open').length)
}

/**
 * Drops a fixture on the app root (ui.md 6.3): `dragenter`, `dragover` and `drop` with a
 * `DataTransfer` holding the file. Waits for the document name and the first frame of the opened
 * document (`ps:open`, 2.18).
 */
export async function dropFile(page: Page, name: string): Promise<void> {
  const content = [...(await readFile(new URL(name, fixtures)))]
  const opensBefore = await openCount(page)
  await page.evaluate(
    ({ fileName, bytes }) => {
      const transfer = new DataTransfer()
      transfer.items.add(new File([new Uint8Array(bytes)], fileName))
      const root = document.querySelector('[data-testid=app]')
      if (root === null) {
        throw new Error('The app root is missing.')
      }
      for (const type of ['dragenter', 'dragover', 'drop']) {
        root.dispatchEvent(
          new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: transfer }),
        )
      }
    },
    { fileName: name, bytes: content },
  )
  await expect(page.getByTestId('document-name')).toHaveText(name.replace(/\.ptsm$/i, ''), {
    timeout: OPEN_TIMEOUT_MILLISECONDS,
  })
  await expect
    .poll(() => openCount(page), { timeout: OPEN_TIMEOUT_MILLISECONDS })
    .toBeGreaterThan(opensBefore)
}

/** The canvas box, its center and height (ui.md 6.3). */
export async function canvas(page: Page): Promise<CanvasBox> {
  const box = await page.getByTestId('viewport').locator('canvas').boundingBox()
  if (box === null) {
    throw new Error('The viewport canvas is not visible.')
  }
  return { ...box, cx: box.x + box.width / 2, cy: box.y + box.height / 2, H: box.height }
}

/** The viewport element, which mirrors the camera, hover and selection (ui.md 6.1). */
export function viewport(page: Page): Locator {
  return page.getByTestId('viewport')
}

/** A button of the view cube by its accessible name ("Front view", "Front right top corner view"). */
export function viewCubeButton(page: Page, name: string): Locator {
  return page.getByRole('group', { name: 'View cube' }).getByRole('button', { name, exact: true })
}

/** Looks at the model from a standard view, orthographic, fitted to the content (ui.md 6.3). */
export async function fitView(page: Page, view: StandardView): Promise<void> {
  await viewCubeButton(page, `${view} view`).click()
  const orthographic = page.getByRole('button', { name: 'Orthographic', exact: true })
  if ((await orthographic.getAttribute('aria-pressed')) !== 'true') {
    await orthographic.click()
  }
  await expect(orthographic).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: 'Fit to view', exact: true }).click()
  await expect(viewport(page)).toHaveAttribute('data-view', view.toLowerCase())
  await expect(viewport(page)).toHaveAttribute('data-projection', 'orthographic')
}

/**
 * Moves the mouse to a page position and polls the viewport's hover kind (ui.md 6.3). Each poll
 * moves away by one pixel and back, so the controller picks again for the current frame.
 */
export async function expectHover(page: Page, x: number, y: number, kind: string): Promise<void> {
  await expect
    .poll(async () => {
      await page.mouse.move(x + 1, y)
      await page.mouse.move(x, y)
      return viewport(page).getAttribute('data-hover-kind')
    })
    .toBe(kind)
}

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
  from: Point,
  to: Point,
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
export async function wheelAt(page: Page, at: Point, deltaY: number, count: number): Promise<void> {
  await page.mouse.move(at.x, at.y)
  for (let i = 0; i < count; i++) {
    // The wheel events are separate events, sent one after another.
    // oxlint-disable-next-line no-await-in-loop
    await page.mouse.wheel(0, deltaY)
  }
}

/** Runs a command from the palette: the option whose title is exactly `title`. */
export async function runCommand(page: Page, title: string): Promise<void> {
  await page.keyboard.press('ControlOrMeta+K')
  const input = page.getByTestId('palette-input')
  await expect(input).toBeFocused()
  await input.fill(title)
  await page
    .getByTestId('palette-option')
    .filter({ has: page.getByText(title, { exact: true }) })
    .first()
    .click()
  await expect(page.getByTestId('palette')).toBeHidden()
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

/** The notification of a severity whose message is exactly `message` (ui.md 3.22). */
export function notification(page: Page, severity: string, message: string): Locator {
  return page
    .locator(`[data-testid=notification][data-severity=${severity}]`)
    .filter({ has: page.getByText(message, { exact: true }) })
}

/** The suggested name and bytes of the download that `action` starts (ui.md 6.3). */
export async function download(page: Page, action: () => Promise<void>): Promise<Download> {
  const [started] = await Promise.all([page.waitForEvent('download'), action()])
  return { name: started.suggestedFilename(), bytes: await readFile(await started.path()) }
}

/** Opens the export popover with the top bar's Export button (ui.md 3.18). */
export async function openExportPopover(page: Page): Promise<Locator> {
  const topBar = page.getByTestId('top-bar')
  // In Print the context toolbar, also in the top bar, has an "Export" group button of its own.
  await topBar
    .getByRole('button', { name: 'Export', exact: true })
    .and(topBar.locator('xpath=.//button[not(ancestor::*[@role="toolbar"])]'))
    .click()
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

/** The browser row of a body whose name is exactly `name`. */
export function bodyRow(page: Page, name: string): Locator {
  return page
    .getByTestId('browser')
    .locator('[data-testid=browser-row][data-kind=body]')
    .filter({ has: page.getByText(name, { exact: true }) })
}

/** Renames a body in the browser: click its row, F2, type the new name, Enter (ui.md 3.10). */
export async function renameBody(page: Page, from: string, to: string): Promise<void> {
  await bodyRow(page, from).click()
  await page.keyboard.press('F2')
  const field = page.getByTestId('browser').getByRole('textbox', { name: 'Name', exact: true })
  await expect(field).toBeFocused()
  await page.keyboard.type(to)
  await page.keyboard.press('Enter')
  await expect(bodyRow(page, to)).toBeVisible()
}
