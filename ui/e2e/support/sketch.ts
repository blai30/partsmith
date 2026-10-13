/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { expect, type Locator, type Page } from '@playwright/test'

// Helpers for the sketch mode end-to-end tests (ui.md 6.3, 6.5 E-12 to E-22 and the sketch tests
// of 3.16): opening the app and fixtures, the canvas geometry the spec's positions are relative
// to, the mapping from sketch millimeters to page pixels, and locators for the sketch chrome.

const fixtures = new URL('../fixtures/', import.meta.url)

/** How long opening a fixture may take: the core rebuilds the whole document. */
const OPEN_TIMEOUT_MILLISECONDS = 20_000

/** The fraction of the view the sketch framing leaves on each side (ui.md 2.11 `fitBounds`). */
const FIT_MARGIN = 0.1

/** A page position in CSS pixels. */
export type PagePoint = { readonly x: number; readonly y: number }

/** A point in sketch millimeters. */
export type SketchPoint = readonly [number, number]

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

/** Maps sketch millimeters to page pixels; `scale` is pixels per millimeter (`s` in ui.md 6.5). */
export type SketchMapping = {
  readonly scale: number
  toPage(point: SketchPoint): PagePoint
}

/**
 * Opens the app and waits for the core to be ready (ui.md 6.3). Camera moves are not animated
 * (reduced motion), so a click right after a view change lands on the final view.
 */
export async function openApp(page: Page): Promise<void> {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('/')
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
 * Drops a `.ptsm` fixture on the app root (ui.md 6.3) and waits for the document name and the
 * first frame of the opened document (`ps:open`).
 */
export async function dropFile(page: Page, name: string): Promise<void> {
  const bytes = [...(await readFile(new URL(name, fixtures)))]
  const opensBefore = await openCount(page)
  await page.evaluate(
    ({ fileName, content }) => {
      const transfer = new DataTransfer()
      transfer.items.add(new File([new Uint8Array(content)], fileName))
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
    { fileName: name, content: bytes },
  )
  await expect(page.getByTestId('document-name')).toHaveText(name.replace(/\.ptsm$/, ''), {
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

/** The page position at an offset in CSS pixels from the canvas center. */
export function offset(box: CanvasBox, dx: number, dy: number): PagePoint {
  return { x: box.cx + dx, y: box.cy + dy }
}

/**
 * The mapping for a sketch on XY just entered (ui.md 6.3 `sketchToScreen`): the top view with
 * the content bounds framed with margin 0.1, so they fill 80 percent of the limiting dimension and
 * their center is at the canvas center; sketch +X points right and +Y up.
 */
export function sketchToScreen(
  box: CanvasBox,
  contentMin: SketchPoint,
  contentMax: SketchPoint,
): SketchMapping {
  const width = contentMax[0] - contentMin[0]
  const height = contentMax[1] - contentMin[1]
  const visibleHeight = Math.max(height, width / (box.width / box.height)) / (1 - 2 * FIT_MARGIN)
  const scale = box.H / visibleHeight
  const centerX = (contentMin[0] + contentMax[0]) / 2
  const centerY = (contentMin[1] + contentMax[1]) / 2
  return {
    scale,
    toPage: ([x, y]) => ({ x: box.cx + (x - centerX) * scale, y: box.cy - (y - centerY) * scale }),
  }
}

/** The viewport (`main`, "3D view"). */
export function viewport(page: Page): Locator {
  return page.getByTestId('viewport')
}

/** The viewport's hover kind after moving the mouse to a page position. */
async function hoverKindAt(page: Page, point: PagePoint): Promise<string | null> {
  // Moving away and back makes the controller pick again for the current frame.
  await page.mouse.move(point.x + 1, point.y)
  await page.mouse.move(point.x, point.y)
  return viewport(page).getAttribute('data-hover-kind')
}

/** Moves the mouse to a page position and polls the viewport's hover kind (ui.md 6.3). */
export async function expectHover(page: Page, point: PagePoint, kind: string): Promise<void> {
  await expect.poll(() => hoverKindAt(page, point)).toBe(kind)
}

/** Moves the mouse to a page position and polls until the hover kind is not `kind`. */
export async function expectNoHover(page: Page, point: PagePoint, kind: string): Promise<void> {
  await expect.poll(() => hoverKindAt(page, point)).not.toBe(kind)
}

/** Clicks a page position. */
export async function clickAt(page: Page, point: PagePoint): Promise<void> {
  await page.mouse.click(point.x, point.y)
}

/** Presses the primary button at `from`, moves to `to` in `steps` pointer moves and releases. */
export async function dragBetween(
  page: Page,
  from: PagePoint,
  to: PagePoint,
  steps = 10,
): Promise<void> {
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  await page.mouse.move(to.x, to.y, { steps })
  await page.mouse.up()
}

/** The app root, which carries `data-context`. */
export function app(page: Page): Locator {
  return page.getByTestId('app')
}

/** The sketch header chip (ui.md 3.16.8). */
export function sketchHeader(page: Page): Locator {
  return page.getByTestId('sketch-header')
}

/** The sketch hint under the header (ui.md 3.16.3). */
export function sketchHint(page: Page): Locator {
  return page.getByTestId('sketch-hint')
}

/** Expects the sketch header and the status bar to show `dof` degrees of freedom. */
export async function expectDof(page: Page, dof: number): Promise<void> {
  await expect(sketchHeader(page)).toHaveAttribute('data-dof', String(dof))
  await expect(page.getByTestId('status-dof')).toHaveAttribute('data-dof', String(dof))
}

/** The timeline item with a feature label. */
export function timelineItem(page: Page, label: string): Locator {
  return page
    .getByTestId('timeline-item')
    .filter({ has: page.getByRole('button', { name: label, exact: true }) })
}

/** Expands the Origin group of the browser when it is collapsed. */
export async function expandOrigin(page: Page): Promise<void> {
  const origin = page.getByRole('treeitem', { name: 'Origin', exact: true })
  if ((await origin.getAttribute('aria-expanded')) !== 'true') {
    await origin.click()
  }
  await expect(origin).toHaveAttribute('aria-expanded', 'true')
}

/** The browser row of an origin item ("Top plane (XY)"), expanding Origin first. */
export async function originRow(page: Page, name: string): Promise<Locator> {
  await expandOrigin(page)
  return page
    .getByTestId('browser')
    .locator('[data-testid=browser-row]')
    .filter({ hasText: name })
    .first()
}

/** Clicks a button of the Model tools toolbar. */
export async function clickModelTool(page: Page, title: string): Promise<void> {
  await page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: title, exact: true })
    .click()
}

/**
 * Starts a new sketch on the top plane as E-12 does: Sketch in the toolbar, then the browser row
 * "Top plane (XY)". Waits for the empty sketch, seen from the top with the origin at the canvas
 * center (an empty sketch keeps the visible height and targets its origin, 3.16.1).
 */
export async function newSketchOnTopPlane(page: Page): Promise<void> {
  await clickModelTool(page, 'Sketch')
  await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')
  await (await originRow(page, 'Top plane (XY)')).click()
  await expect(timelineItem(page, 'Sketch 1')).toBeVisible()
  await expect(viewport(page)).toHaveAttribute('data-view', 'top')
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'empty')
}

/** Waits until a sketch is being edited, seen from the top of its plane. */
async function expectEditing(page: Page): Promise<void> {
  await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  await expect(viewport(page)).toHaveAttribute('data-view', 'top')
  await expect(sketchHeader(page)).toHaveAttribute('data-dof', /^\d+$/)
}

/** Edits a sketch by double-clicking its timeline item (ui.md 3.9). */
export async function editSketchFromTimeline(page: Page, label: string): Promise<void> {
  await timelineItem(page, label).getByRole('button', { name: label, exact: true }).dblclick()
  await expectEditing(page)
}

/** The browser row of a sketch by its label. */
export function sketchRow(page: Page, label: string): Locator {
  return page
    .getByTestId('browser')
    .locator('[data-testid=browser-row][data-kind=sketch]')
    .filter({ has: page.getByText(label, { exact: true }) })
}

/**
 * Edits a sketch by double-clicking its browser row (ui.md 3.10), for the tests that say
 * "double-click 'Sketch 1'" or "enter the sketch" without naming the timeline.
 */
export async function editSketchFromBrowser(page: Page, label: string): Promise<void> {
  await sketchRow(page, label).getByText(label, { exact: true }).dblclick()
  await expectEditing(page)
}

/** The inference glyphs shown while drawing (ui.md 3.16.5). */
export function inferenceGlyphs(page: Page): Locator {
  return page.getByTestId('inference-glyph')
}

/** The constraint glyphs of one constraint type. */
export function constraintGlyphs(page: Page, type: string): Locator {
  return page.locator(`[data-testid=constraint-glyph][data-type=${type}]`)
}

/** The dimension labels on the canvas. */
export function sketchDimensions(page: Page): Locator {
  return page.getByTestId('sketch-dimension')
}

/**
 * The dimension labels that show exactly `text` ("40", "fx 20", "(40)", "⌀12"). The label's
 * accessible name puts the dimension's kind before that text, in a visually hidden span.
 */
export function dimensionLabel(page: Page, text: string): Locator {
  return sketchDimensions(page).filter({ has: page.getByText(text, { exact: true }) })
}

/** The notification of a severity whose message contains `text`. */
export function notification(page: Page, severity: string, text: string): Locator {
  return page
    .locator(`[data-testid=notification][data-severity=${severity}]`)
    .filter({ hasText: text })
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

/** The inspector panel. */
export function inspector(page: Page): Locator {
  return page.getByTestId('inspector')
}

/** The browser row of a body by its visible name. */
export function bodyRow(page: Page, name: string): Locator {
  return page
    .getByTestId('browser')
    .locator('[data-testid=browser-row][data-kind=body]')
    .filter({ has: page.getByText(name, { exact: true }) })
}

/** The inspector field container with a param path. */
export function inspectorField(page: Page, path: string): Locator {
  return inspector(page).locator(`[data-testid=inspector-field][data-path="${path}"]`)
}

/** The expression or text input of the inspector labeled `label` (the manifest label). */
export function inspectorInput(page: Page, label: string): Locator {
  const panel = inspector(page)
  return panel
    .getByRole('combobox', { name: label, exact: true })
    .or(panel.getByRole('textbox', { name: label, exact: true }))
}
