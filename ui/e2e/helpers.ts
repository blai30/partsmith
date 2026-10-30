/// <reference lib="dom" />
/// <reference lib="dom.asynciterable" />
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { test as base, expect, type Locator, type Page } from '@playwright/test'

// The shared end-to-end helpers of ui.md 6.3: opening the app and fixtures, the canvas geometry
// the spec's positions are relative to, the mapping from model and sketch millimeters to page
// pixels, hover polling, downloads, performance measures, percentiles and long tasks, and the
// locators every area uses. The files in support/ hold the helpers of one area each.

const fixtures = new URL('fixtures/', import.meta.url)

/**
 * How long opening the app or a fixture may take: the core compiles, then rebuilds the document.
 * Opening `dense.ptsm` alone takes about 25 seconds, and longer on a busy runner.
 */
const OPEN_TIMEOUT_MILLISECONDS = 60_000

/** The fraction of the view that fitting leaves on each side (ui.md 2.11 `fitBounds`). */
const FIT_MARGIN = 0.1

/** The `window` property where `watchLongTasks` keeps what it observes. */
const LONG_TASKS_KEY = 'partsmithLongTasks'

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

/** The fields of a `csp-violation` report's body that a failure shows. */
type ViolationReportBody = { readonly effectiveDirective: string; readonly blockedURL: string }

/**
 * Every content security policy violation reported in the scope that runs it (a page or a worker)
 * since that scope started, one line each as `{directive} {URL}` (ui.md 6.3, delivery.md 2.7): a
 * `ReportingObserver` made with `buffered` is also given the reports made before it existed.
 */
export function reportedViolations(): string[] {
  const observer = new ReportingObserver(() => undefined, {
    types: ['csp-violation'],
    buffered: true,
  })
  observer.observe()
  const reports = observer.takeRecords()
  observer.disconnect()
  return reports.map((report) => {
    const body = report.body as ViolationReportBody
    return `${body.effectiveDirective} ${body.blockedURL}`
  })
}

/**
 * The `test` of every E-test file but `security.spec.ts` (ui.md 6.3): when a test that used `page`
 * ends, the page must have reported no content security policy violation, so every state a test
 * visits is checked against the policy the preview sends (delivery.md 2.7).
 */
export const test = base.extend({
  // Playwright calls the second argument `use`; this name keeps the lint's React hook rule away.
  page: async ({ page }, runTest) => {
    await runTest(page)
    if (!page.isClosed()) {
      expect(
        await page.evaluate(reportedViolations),
        'the content security policy violations of the page',
      ).toEqual([])
    }
  },
})

/**
 * Opens the app and waits for the core to be ready (ui.md 6.3). Without File System Access an
 * init script removes the three pickers before every load of the page. Camera moves are not animated
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
      Reflect.deleteProperty(window, 'showDirectoryPicker')
    })
  }
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto('/')
  await expectReady(page)
}

/**
 * What the page shows of its startup (ui.md 3.1, 4.4), read in one go: the unsupported page's text
 * (null when that page is not shown) and the core status of `app` (`not rendered` before `app`
 * exists), where `failed` carries the fatal panel's text.
 */
function startupState(
  page: Page,
): Promise<{ readonly unsupported: string | null; readonly status: string }> {
  return page.evaluate(() => {
    const unsupported = document.querySelector('[data-testid="unsupported"]')?.textContent ?? null
    const status =
      document.querySelector('[data-testid="app"]')?.getAttribute('data-core-status') ??
      'not rendered'
    if (status !== 'failed') {
      return { unsupported, status }
    }
    const fatal = document.querySelector('[data-testid="core-fatal"] p')?.textContent
    return { unsupported, status: `failed: ${fatal ?? 'no fatal panel'}` }
  })
}

/**
 * Waits until the core runs and the startup document is open (ui.md 3.1, 6.3). The unsupported page
 * fails it at once with its text; a wait that ends without `ready`, at its timeout or the test's,
 * fails with the last status it read, the core's fatal text included.
 */
export async function expectReady(page: Page): Promise<void> {
  // Compiling the core's WebAssembly can take longer than the default expect timeout.
  await expect
    .poll(
      async () => {
        const { unsupported, status } = await startupState(page)
        // Nothing starts after the unsupported page, so waiting longer cannot help.
        if (unsupported !== null) {
          throw new Error(`The app shows its unsupported page instead of starting: ${unsupported}`)
        }
        return status
      },
      { timeout: OPEN_TIMEOUT_MILLISECONDS },
    )
    .toBe('ready')
}

/**
 * The durations of the performance entries named `name` (ui.md 2.18) that started at or after
 * `since` (a `pageTime`), oldest first.
 */
export function measures(page: Page, name: string, since = 0): Promise<number[]> {
  return page.evaluate(
    ({ entryName, start }) =>
      performance
        .getEntriesByName(entryName)
        .filter((entry) => entry.startTime >= start)
        .map((entry) => entry.duration),
    { entryName: name, start: since },
  )
}

/** The page's `performance.now()`: the `since` of `measures` and `longTasks` for what follows. */
export function pageTime(page: Page): Promise<number> {
  return page.evaluate(() => performance.now())
}

/**
 * The nearest-rank percentile of `values` (ui.md 6.3): the values sorted ascending, the one at
 * index `ceil(fraction × n) - 1`. The median is `percentile(values, 0.5)` (the lower middle value
 * of an even count) and the 95th percentile `percentile(values, 0.95)`.
 */
export function percentile(values: readonly number[], fraction: number): number {
  if (values.length === 0) {
    throw new Error('A percentile needs at least one value.')
  }
  const sorted = values.toSorted((first, second) => first - second)
  return sorted[Math.min(Math.max(Math.ceil(fraction * sorted.length) - 1, 0), sorted.length - 1)]
}

/** A long task as `watchLongTasks` keeps it. */
type LongTask = { readonly startTime: number; readonly duration: number }

/**
 * Observes the page's long tasks (ui.md 6.3): an init script that, on every load and before the
 * page's own scripts run, keeps the start time and duration of each `longtask` entry on `window`.
 * Called before `openApp`.
 */
export async function watchLongTasks(page: Page): Promise<void> {
  await page.addInitScript((key) => {
    const tasks: LongTask[] = []
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        tasks.push({ startTime: entry.startTime, duration: entry.duration })
      }
    })
    observer.observe({ type: 'longtask', buffered: true })
    Reflect.set(window, key, { tasks, observer })
  }, LONG_TASKS_KEY)
}

/**
 * The durations of the long tasks `watchLongTasks` saw that started at or after `since` (a
 * `pageTime`), oldest first.
 */
export async function longTasks(page: Page, since = 0): Promise<number[]> {
  const durations = await page.evaluate(
    ({ key, start }) => {
      const watched: unknown = Reflect.get(window, key)
      if (typeof watched !== 'object' || watched === null) {
        return null
      }
      const { tasks, observer } = watched as {
        readonly tasks: LongTask[]
        readonly observer: PerformanceObserver
      }
      // Entries the observer has queued but not delivered yet.
      for (const entry of observer.takeRecords()) {
        tasks.push({ startTime: entry.startTime, duration: entry.duration })
      }
      return tasks
        .filter((task) => task.startTime >= start)
        .toSorted((first, second) => first.startTime - second.startTime)
        .map((task) => task.duration)
    },
    { key: LONG_TASKS_KEY, start: since },
  )
  if (durations === null) {
    throw new Error('Long tasks are not watched: call watchLongTasks before openApp.')
  }
  return durations
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

/** A rectangle on the page, in CSS pixels. */
export type Box = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** The page box of a visible element; throws when it is not visible. */
export async function boxOf(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox()
  if (box === null) {
    throw new Error(`${locator.toString()} is not visible.`)
  }
  return box
}

/**
 * The view the tests aim at, its center and height (ui.md 6.3): the free area, the `viewport`
 * element's box. The canvas fills the window under the islands, but fit, the standard views and
 * zoom center the model in the free area and scale it to the free area's height (2.11).
 */
export async function canvas(page: Page): Promise<CanvasBox> {
  const box = await page.getByTestId('viewport').boundingBox()
  if (box === null) {
    throw new Error('The viewport is not visible.')
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

/** One picker call of the pickers `useFolders` installs, a handle option given as its name. */
export type PickerCall = {
  readonly picker: 'showOpenFilePicker' | 'showSaveFilePicker' | 'showDirectoryPicker'
  readonly options: Readonly<Record<string, unknown>>
}

/** The `window` property where the pickers of `useFolders` keep their queue and calls. */
const FOLDERS_KEY = 'partsmithFolders'

/**
 * Replaces the three File System Access pickers with pickers over the page's origin private file
 * system, on every load of the page (ui.md 6.3). Called before `openApp`. Each picker call takes
 * the next path that `queuePick` queued, slash-separated from the root: the open picker resolves
 * the file there, the save picker the file there (created empty when missing, as Chromium's save
 * dialog creates the file it names), the directory picker the folder there. With the queue empty
 * a call rejects with an `AbortError`, as a canceled dialog does. Every call is recorded with its
 * options, a handle given as its name (`pickerCalls`).
 */
export async function useFolders(page: Page): Promise<void> {
  await page.addInitScript((key) => {
    type Picker = 'showOpenFilePicker' | 'showSaveFilePicker' | 'showDirectoryPicker'
    const state: {
      queue: string[]
      calls: { picker: Picker; options: Record<string, unknown> }[]
    } = { queue: [], calls: [] }
    Object.defineProperty(window, key, { value: state })

    function record(picker: Picker, options: unknown): void {
      const described: Record<string, unknown> = {}
      if (typeof options === 'object' && options !== null) {
        for (const [name, value] of Object.entries(options)) {
          described[name] = value instanceof FileSystemHandle ? value.name : value
        }
      }
      state.calls.push({ picker, options: described })
    }

    function nextPath(): string[] {
      const path = state.queue.shift()
      if (path === undefined) {
        throw new DOMException('The user aborted a request.', 'AbortError')
      }
      return path.split('/').filter((part) => part !== '')
    }

    // The init script runs in the page, so its helpers live inside it.
    // oxlint-disable-next-line unicorn/consistent-function-scoping
    function folderAt(parts: readonly string[]): Promise<FileSystemDirectoryHandle> {
      return parts.reduce<Promise<FileSystemDirectoryHandle>>(
        async (folder, part) => (await folder).getDirectoryHandle(part),
        navigator.storage.getDirectory(),
      )
    }

    async function fileAt(
      parts: readonly string[],
      create: boolean,
    ): Promise<FileSystemFileHandle> {
      const name = parts.at(-1)
      if (name === undefined) {
        throw new DOMException('A file picker needs a file path.', 'TypeMismatchError')
      }
      const folder = await folderAt(parts.slice(0, -1))
      return folder.getFileHandle(name, { create })
    }

    Object.assign(window, {
      async showOpenFilePicker(options: unknown) {
        record('showOpenFilePicker', options)
        return [await fileAt(nextPath(), false)]
      },
      async showSaveFilePicker(options: unknown) {
        record('showSaveFilePicker', options)
        return fileAt(nextPath(), true)
      },
      async showDirectoryPicker(options: unknown) {
        record('showDirectoryPicker', options)
        return folderAt(nextPath())
      },
    })
  }, FOLDERS_KEY)
}

/** Queues the path the next picker call of `useFolders` answers with (ui.md 6.3). */
export async function queuePick(page: Page, path: string): Promise<void> {
  await page.evaluate(
    ({ key, picked }) => {
      const state: unknown = Reflect.get(window, key)
      if (typeof state !== 'object' || state === null) {
        throw new Error('useFolders was not called before the page loaded.')
      }
      const queue: unknown = Reflect.get(state, 'queue')
      if (!Array.isArray(queue)) {
        throw new Error('The picker queue is missing.')
      }
      queue.push(picked)
    },
    { key: FOLDERS_KEY, picked: path },
  )
}

/** The picker calls of the current load of the page, oldest first (ui.md 6.3). */
export function pickerCalls(page: Page): Promise<PickerCall[]> {
  return page.evaluate((key) => {
    const state: unknown = Reflect.get(window, key)
    const calls: unknown =
      typeof state === 'object' && state !== null ? Reflect.get(state, 'calls') : []
    return JSON.parse(JSON.stringify(calls)) as PickerCall[]
  }, FOLDERS_KEY)
}

/** Writes `bytes` to the file at a slash-separated path of the origin private file system, creating its folders. */
export async function writeFolderFile(
  page: Page,
  path: string,
  bytes: Uint8Array | Buffer,
): Promise<void> {
  await page.evaluate(
    async ({ parts, content }) => {
      const name = parts.at(-1)
      if (name === undefined) {
        throw new Error('writeFolderFile needs a file path.')
      }
      const folder = await parts
        .slice(0, -1)
        .reduce<Promise<FileSystemDirectoryHandle>>(
          async (parent, part) => (await parent).getDirectoryHandle(part, { create: true }),
          navigator.storage.getDirectory(),
        )
      const file = await folder.getFileHandle(name, { create: true })
      const writable = await file.createWritable()
      await writable.write(new Uint8Array(content))
      await writable.close()
    },
    { parts: path.split('/').filter((part) => part !== ''), content: [...bytes] },
  )
}

/** The bytes of the file at a slash-separated path of the origin private file system, or null when there is none. */
export async function readFolderFile(page: Page, path: string): Promise<Buffer | null> {
  const content = await page.evaluate(
    async (parts) => {
      const name = parts.at(-1)
      if (name === undefined) {
        return null
      }
      try {
        const folder = await parts
          .slice(0, -1)
          .reduce<Promise<FileSystemDirectoryHandle>>(
            async (parent, part) => (await parent).getDirectoryHandle(part),
            navigator.storage.getDirectory(),
          )
        const file = await (await folder.getFileHandle(name)).getFile()
        return [...new Uint8Array(await file.arrayBuffer())]
      } catch (error) {
        if (error instanceof DOMException && error.name === 'NotFoundError') {
          return null
        }
        throw error
      }
    },
    path.split('/').filter((part) => part !== ''),
  )
  return content === null ? null : Buffer.from(content)
}

/** The sorted names of the entries of the folder at a slash-separated path of the origin private file system. */
export function folderNames(page: Page, path: string): Promise<string[]> {
  return page.evaluate(
    async (parts) => {
      const folder = await parts.reduce<Promise<FileSystemDirectoryHandle>>(
        async (parent, part) => (await parent).getDirectoryHandle(part),
        navigator.storage.getDirectory(),
      )
      const names: string[] = []
      for await (const name of folder.keys()) {
        names.push(name)
      }
      return names.toSorted()
    },
    path.split('/').filter((part) => part !== ''),
  )
}

/**
 * Makes the core hang on one request, as a call stuck in the kernel would (ui.md 6.3). Called
 * before `openApp`: the core module URL without a query is routed to a module that loads the real
 * one (`partsmith_core.mjs?real`) and replaces `_ps_call` of the instance it made, so a request
 * frame whose bytes contain `marker` in UTF-8 loops forever and every other request reaches the
 * real core. The route serves every load of the page, its reloads and the core worker's restarts.
 */
export async function stallCore(page: Page, marker: string): Promise<void> {
  const module = [
    "import createRealCore from './partsmith_core.mjs?real'",
    `const marker = ${JSON.stringify(marker)}`,
    'export default async function createPartsmithCore(...options) {',
    '  const core = await createRealCore(...options)',
    '  const call = core._ps_call',
    '  core._ps_call = (request, requestLength, responseLengthOut) => {',
    '    const frame = core.HEAPU8.slice(request, request + requestLength)',
    '    if (new TextDecoder().decode(frame).includes(marker)) {',
    '      for (;;) {',
    '        // Stuck, as a call that never leaves the kernel.',
    '      }',
    '    }',
    '    return call(request, requestLength, responseLengthOut)',
    '  }',
    '  return core',
    '}',
  ].join('\n')
  await page.route(
    (url) => url.pathname.endsWith('/core/partsmith_core.mjs') && url.search === '',
    (route) => route.fulfill({ contentType: 'text/javascript', body: module }),
  )
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

/** A button of the Model tools toolbar, by its title. */
export function modelTool(page: Page, title: string): Locator {
  return page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: title, exact: true })
}

/** Clicks a button of the Model tools toolbar. */
export async function clickTool(page: Page, title: string): Promise<void> {
  await modelTool(page, title).click()
}

/** The timeline item of a feature, by its label (the accessible name of its button). */
export function timelineItem(page: Page, label: string): Locator {
  return page
    .getByTestId('timeline-item')
    .filter({ has: page.getByRole('button', { name: label, exact: true }) })
}

/** The browser rows of one kind ("body", "sketch"). */
function browserRows(page: Page, kind: string): Locator {
  return page.getByTestId('browser').locator(`[data-testid=browser-row][data-kind=${kind}]`)
}

/** The browser row of a body, sketch or other item, by its kind and the exact text of its name. */
export function browserRow(page: Page, kind: string, name: string): Locator {
  return browserRows(page, kind).filter({ has: page.getByText(name, { exact: true }) })
}

/** The browser rows of bodies. */
export function bodyRows(page: Page): Locator {
  return browserRows(page, 'body')
}

/** The browser row of a body whose name is exactly `name`. */
export function bodyRow(page: Page, name: string): Locator {
  return browserRow(page, 'body', name)
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
