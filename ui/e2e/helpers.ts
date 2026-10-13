/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { expect, type Locator, type Page } from '@playwright/test'

// The shared end-to-end helpers of ui.md 6.3: opening the app and fixtures, the canvas geometry
// the spec's positions are relative to, the mapping from model and sketch millimeters to page
// pixels, hover polling, downloads, performance measures, and the locators every area uses. The
// files in support/ hold the helpers of one area each.

const fixtures = new URL('fixtures/', import.meta.url)

/** How long opening the app or a fixture may take: the core compiles, then rebuilds the document. */
const OPEN_TIMEOUT_MILLISECONDS = 30_000

/** The fraction of the view that fitting leaves on each side (ui.md 2.11 `fitBounds`). */
const FIT_MARGIN = 0.1

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

/** A page position in CSS pixels. */
export type PagePoint = { readonly x: number; readonly y: number }

/** A point in model millimeters. */
export type ModelPoint = readonly [number, number, number]

/** A point in sketch millimeters. */
export type SketchPoint = readonly [number, number]

/** An axis-aligned box in model millimeters. */
export type ModelBounds = { readonly min: ModelPoint; readonly max: ModelPoint }

/** The standard views of the view cube that `fitView` and `toScreen` use. */
export type StandardView = 'Top' | 'Bottom' | 'Front' | 'Back' | 'Left' | 'Right'

/** Maps model millimeters to page pixels; `scale` is pixels per millimeter (`s` in ui.md 6.3). */
export type ScreenMapping = {
  readonly scale: number
  toPage(point: ModelPoint): PagePoint
}

/** Maps sketch millimeters to page pixels; `scale` is pixels per millimeter (`s` in ui.md 6.5). */
export type SketchMapping = {
  readonly scale: number
  toPage(point: SketchPoint): PagePoint
}

/** The suggested name and bytes of a download. */
export type Download = { readonly name: string; readonly bytes: Buffer }

/**
 * The screen's right and up directions in model space for each standard view (ui.md 2.11): front
 * from -Y, back from +Y, right from +X and left from -X with +Z up; top from +Z with +Y up and
 * bottom from -Z with -Y up.
 */
const VIEW_AXES: Readonly<Record<StandardView, { right: ModelPoint; up: ModelPoint }>> = {
  Front: { right: [1, 0, 0], up: [0, 0, 1] },
  Back: { right: [-1, 0, 0], up: [0, 0, 1] },
  Right: { right: [0, 1, 0], up: [0, 0, 1] },
  Left: { right: [0, -1, 0], up: [0, 0, 1] },
  Top: { right: [1, 0, 0], up: [0, 1, 0] },
  Bottom: { right: [1, 0, 0], up: [0, -1, 0] },
}

/** The path of a fixture file on disk. */
export function fixturePath(name: string): string {
  return fileURLToPath(new URL(name, fixtures))
}

/** The bytes of a fixture file. */
export function fixtureBytes(name: string): Promise<Buffer> {
  return readFile(new URL(name, fixtures))
}

/**
 * Opens the app and waits for the core to be ready (ui.md 6.3). Without File System Access an
 * init script removes both pickers before every load of the page. Camera moves are not animated
 * (reduced motion, 2.11), so a click right after a view change lands on the final view.
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
    timeout: OPEN_TIMEOUT_MILLISECONDS,
  })
}

/** The durations of the performance entries named `name` (ui.md 2.18), oldest first. */
export function measures(page: Page, name: string): Promise<number[]> {
  return page.evaluate(
    (entryName) => performance.getEntriesByName(entryName).map((entry) => entry.duration),
    name,
  )
}

/**
 * Drops a fixture on the app root (ui.md 6.3): `dragenter`, `dragover` and `drop` with a
 * `DataTransfer` holding the file. For a `.ptsm` file it waits for the document name and for the
 * first frame of the opened document (a new `ps:open` measure, 2.18); for any other file, for a
 * notification.
 */
export async function dropFile(page: Page, name: string): Promise<void> {
  const content = [...(await fixtureBytes(name))]
  const opensBefore = (await measures(page, 'ps:open')).length
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
  const model = /\.ptsm$/i
  if (!model.test(name)) {
    await expect(page.getByTestId('notification').first()).toBeVisible()
    return
  }
  // Opening rebuilds every feature in the core, which on a busy runner takes longer than the
  // default expect timeout.
  await expect(page.getByTestId('document-name')).toHaveText(name.replace(model, ''), {
    timeout: OPEN_TIMEOUT_MILLISECONDS,
  })
  await expect
    .poll(async () => (await measures(page, 'ps:open')).length, {
      timeout: OPEN_TIMEOUT_MILLISECONDS,
    })
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

function dot(first: ModelPoint, second: ModelPoint): number {
  return first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

/**
 * The mapping from model millimeters to page pixels after `fitView(page, view)` framed `bounds`
 * (ui.md 6.3): the fit rule of 2.11 scales the content's extents to 80 percent of the limiting
 * dimension and puts the bounds center at the canvas center. For the cube in the front view the
 * scale is 0.04 H pixels per millimeter and the top edge of the front face is at `cy - 0.4 H`.
 */
export function toScreen(box: CanvasBox, bounds: ModelBounds, view: StandardView): ScreenMapping {
  const { right, up } = VIEW_AXES[view]
  const corners: ModelPoint[] = []
  for (const x of [bounds.min[0], bounds.max[0]]) {
    for (const y of [bounds.min[1], bounds.max[1]]) {
      for (const z of [bounds.min[2], bounds.max[2]]) {
        corners.push([x, y, z])
      }
    }
  }
  const extent = (axis: ModelPoint) => {
    const along = corners.map((corner) => dot(corner, axis))
    return Math.max(...along) - Math.min(...along)
  }
  const visibleHeight =
    Math.max(extent(up), extent(right) / (box.width / box.height)) / (1 - 2 * FIT_MARGIN)
  const scale = box.H / visibleHeight
  const center: ModelPoint = [
    (bounds.min[0] + bounds.max[0]) / 2,
    (bounds.min[1] + bounds.max[1]) / 2,
    (bounds.min[2] + bounds.max[2]) / 2,
  ]
  return {
    scale,
    toPage: (point) => {
      const relative: ModelPoint = [
        point[0] - center[0],
        point[1] - center[1],
        point[2] - center[2],
      ]
      return { x: box.cx + dot(relative, right) * scale, y: box.cy - dot(relative, up) * scale }
    },
  }
}

/**
 * The mapping for a sketch on XY just entered (ui.md 6.3): the top view with the sketch content
 * from `contentMin` to `contentMax` framed with margin 0.1; sketch +X points right and +Y up.
 */
export function sketchToScreen(
  box: CanvasBox,
  contentMin: SketchPoint,
  contentMax: SketchPoint,
): SketchMapping {
  const mapping = toScreen(
    box,
    { min: [contentMin[0], contentMin[1], 0], max: [contentMax[0], contentMax[1], 0] },
    'Top',
  )
  return { scale: mapping.scale, toPage: ([x, y]) => mapping.toPage([x, y, 0]) }
}

/**
 * The viewport's hover kind after moving the mouse to a page position. Moving away by one pixel
 * and back makes the controller pick again for the current frame.
 */
export async function hoverKindAt(page: Page, x: number, y: number): Promise<string | null> {
  await page.mouse.move(x + 1, y)
  await page.mouse.move(x, y)
  return viewport(page).getAttribute('data-hover-kind')
}

/** Moves the mouse to a page position and polls the viewport's hover kind (ui.md 6.3). */
export async function expectHover(page: Page, x: number, y: number, kind: string): Promise<void> {
  await expect.poll(() => hoverKindAt(page, x, y)).toBe(kind)
}

/** The suggested name and bytes of the download that `action` starts (ui.md 6.3). */
export async function download(page: Page, action: () => Promise<void>): Promise<Download> {
  const [started] = await Promise.all([page.waitForEvent('download'), action()])
  return { name: started.suggestedFilename(), bytes: await readFile(await started.path()) }
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

/** The notification of a severity whose message is exactly `message` (ui.md 3.22). */
export function notification(page: Page, severity: string, message: string): Locator {
  return page
    .locator(`[data-testid=notification][data-severity=${severity}]`)
    .filter({ has: page.getByText(message, { exact: true }) })
}

/** The inspector panel. */
export function inspector(page: Page): Locator {
  return page.getByTestId('inspector')
}

/** The inspector field container with a param path. */
export function inspectorField(page: Page, path: string): Locator {
  return inspector(page).locator(`[data-testid=inspector-field][data-path="${path}"]`)
}

/**
 * The expression or text input of the inspector labeled `label` (the manifest label). An
 * expression input is a combobox while it offers parameter names (ui.md 2.13).
 */
export function inspectorInput(page: Page, label: string): Locator {
  const panel = inspector(page)
  return panel
    .getByRole('combobox', { name: label, exact: true })
    .or(panel.getByRole('textbox', { name: label, exact: true }))
}

/** The timeline item of a feature, by its label (the accessible name of its button). */
export function timelineItem(page: Page, label: string): Locator {
  return page
    .getByTestId('timeline-item')
    .filter({ has: page.getByRole('button', { name: label, exact: true }) })
}

/** The browser rows of bodies. */
export function bodyRows(page: Page): Locator {
  return page.getByTestId('browser').locator('[data-testid=browser-row][data-kind=body]')
}

/** The browser row of a body whose name is exactly `name`. */
export function bodyRow(page: Page, name: string): Locator {
  return bodyRows(page).filter({ has: page.getByText(name, { exact: true }) })
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

/** Expands the Origin group of the browser when it is collapsed. */
export async function expandOrigin(page: Page): Promise<void> {
  const origin = page.getByRole('treeitem', { name: 'Origin', exact: true })
  if ((await origin.getAttribute('aria-expanded')) !== 'true') {
    await origin.click()
  }
  await expect(origin).toHaveAttribute('aria-expanded', 'true')
}

/** The browser row of an origin item ("Top plane (XY)", "Z axis"), expanding Origin first. */
export async function originRow(page: Page, name: string): Promise<Locator> {
  await expandOrigin(page)
  return page
    .getByTestId('browser')
    .locator('[data-testid=browser-row]')
    .filter({ has: page.getByText(name, { exact: true }) })
}
