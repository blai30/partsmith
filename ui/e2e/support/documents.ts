/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { expect, type Locator, type Page } from '@playwright/test'

// Helpers for the document, file, recovery, undo, notification and status bar tests (ui.md 6.3,
// 6.5 E-1 to E-9 and the tests of 3.19 to 3.22): opening the app and fixtures, downloads, the
// IndexedDB records of recent files and recovery (2.15), and locators for the regions involved.

const fixtures = new URL('../fixtures/', import.meta.url)

/** How long opening a fixture may take: the core rebuilds the whole document. */
const OPEN_TIMEOUT_MILLISECONDS = 20_000

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

/** The standard views of the view cube that `fitView` uses. */
export type StandardView = 'Top' | 'Bottom' | 'Front' | 'Back' | 'Left' | 'Right'

/** The recovery copy in IndexedDB (ui.md 2.15), without its handle. */
export type StoredRecovery = {
  readonly name: string
  readonly text: string
  readonly reason: string
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
 * init script removes both pickers, before every load of the page. Camera moves are not animated
 * (reduced motion), so a click right after a view change lands on the final view.
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
 * `DataTransfer` holding the file. For a `.ptsm` file it waits for the document name and the first
 * frame of the opened document (`ps:open`); for any other file, for a notification.
 */
export async function dropFile(page: Page, name: string): Promise<void> {
  const content = [...(await fixtureBytes(name))]
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
  if (!/\.ptsm$/i.test(name)) {
    await expect(page.getByTestId('notification').first()).toBeVisible()
    return
  }
  // Opening rebuilds every feature in the core, which on a busy runner takes longer than the
  // default expect timeout.
  await expect(page.getByTestId('document-name')).toHaveText(name.replace(/\.ptsm$/i, ''), {
    timeout: OPEN_TIMEOUT_MILLISECONDS,
  })
  await expect
    .poll(() => openCount(page), { timeout: OPEN_TIMEOUT_MILLISECONDS })
    .toBeGreaterThan(opensBefore)
}

/** The suggested name and bytes of the download that `action` starts (ui.md 6.3). */
export async function download(
  page: Page,
  action: () => Promise<void>,
): Promise<{ readonly name: string; readonly bytes: Buffer }> {
  const [started] = await Promise.all([page.waitForEvent('download'), action()])
  return { name: started.suggestedFilename(), bytes: await readFile(await started.path()) }
}

/** The canvas box, its center and height (ui.md 6.3). */
export async function canvas(page: Page): Promise<CanvasBox> {
  const box = await page.getByTestId('viewport').locator('canvas').boundingBox()
  if (box === null) {
    throw new Error('The viewport canvas is not visible.')
  }
  return { ...box, cx: box.x + box.width / 2, cy: box.y + box.height / 2, H: box.height }
}

/** Looks at the model from a standard view, orthographic, fitted to the content (ui.md 6.3). */
export async function fitView(page: Page, view: StandardView): Promise<void> {
  const viewport = page.getByTestId('viewport')
  await page
    .getByRole('group', { name: 'View cube' })
    .getByRole('button', { name: `${view} view`, exact: true })
    .click()
  const orthographic = page.getByRole('button', { name: 'Orthographic', exact: true })
  if ((await orthographic.getAttribute('aria-pressed')) !== 'true') {
    await orthographic.click()
  }
  await expect(orthographic).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: 'Fit to view', exact: true }).click()
  await expect(viewport).toHaveAttribute('data-view', view.toLowerCase())
  await expect(viewport).toHaveAttribute('data-projection', 'orthographic')
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

/** The timeline items. */
export function timelineItems(page: Page): Locator {
  return page.getByTestId('timeline-item')
}

/** The timeline item of a feature, by its label (the accessible name of its button). */
export function timelineItem(page: Page, label: string): Locator {
  return timelineItems(page).filter({
    has: page.getByRole('button', { name: label, exact: true }),
  })
}

/** Expects the timeline to hold exactly these features, in order, each with `data-status="ok"`. */
export async function expectTimeline(page: Page, labels: readonly string[]): Promise<void> {
  const items = timelineItems(page)
  await expect(items).toHaveCount(labels.length)
  await Promise.all(
    labels.flatMap((label, index) => [
      expect(items.nth(index).getByRole('button')).toHaveAccessibleName(label),
      expect(items.nth(index)).toHaveAttribute('data-status', 'ok'),
    ]),
  )
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

/** The notification whose message is exactly `message` (ui.md 3.22). */
export function notification(page: Page, message: string): Locator {
  return page.getByTestId('notification').filter({ has: page.getByText(message, { exact: true }) })
}

/**
 * Runs a read-only IndexedDB request on one object store of the app's database (ui.md 2.15) and
 * returns its result, or null while the database or the record does not exist. The database is
 * never created or upgraded here, so the app's own first open still creates its stores.
 */
function readStore(
  page: Page,
  storeName: 'recent' | 'recovery',
  key: string | null,
): Promise<unknown> {
  return page.evaluate(
    async ({ store, recordKey }) => {
      const databases = await indexedDB.databases()
      if (!databases.some((database) => database.name === 'partsmith')) {
        return null
      }
      const database = await new Promise<IDBDatabase | null>((resolve, reject) => {
        const request = indexedDB.open('partsmith')
        request.addEventListener('upgradeneeded', () => {
          request.transaction?.abort()
        })
        request.addEventListener('success', () => {
          resolve(request.result)
        })
        request.addEventListener('error', () => {
          // An aborted upgrade means the database was deleted meanwhile.
          resolve(null)
        })
        request.addEventListener('blocked', () => {
          reject(new Error('The partsmith database is blocked.'))
        })
      })
      if (database === null) {
        return null
      }
      try {
        if (!database.objectStoreNames.contains(store)) {
          return null
        }
        const objectStore = database.transaction(store, 'readonly').objectStore(store)
        const request = recordKey === null ? objectStore.getAll() : objectStore.get(recordKey)
        return await new Promise<unknown>((resolve, reject) => {
          request.addEventListener('success', () => {
            resolve(request.result ?? null)
          })
          request.addEventListener('error', () => {
            reject(request.error ?? new Error(`Reading ${store} failed.`))
          })
        })
      } finally {
        database.close()
      }
    },
    { store: storeName, recordKey: key },
  )
}

/** The file names in the recent files store (ui.md 2.15), in no particular order. */
export async function recentFileNames(page: Page): Promise<string[]> {
  const records = await readStore(page, 'recent', null)
  if (!Array.isArray(records)) {
    return []
  }
  return records.map((record: unknown) =>
    typeof record === 'object' && record !== null ? String(Reflect.get(record, 'name')) : '',
  )
}

/** The recovery copy in IndexedDB (ui.md 2.15), or null when there is none. */
export async function storedRecovery(page: Page): Promise<StoredRecovery | null> {
  const record = await readStore(page, 'recovery', 'current')
  if (typeof record !== 'object' || record === null) {
    return null
  }
  return {
    name: String(Reflect.get(record, 'name')),
    text: String(Reflect.get(record, 'text')),
    reason: String(Reflect.get(record, 'reason')),
  }
}
