/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { expect, type Locator, type Page } from '@playwright/test'

// Helpers for the modeling end-to-end tests (ui.md 6.3): opening the app and fixtures, the canvas
// geometry the spec's positions are relative to, and locators for the inspector, timeline,
// browser and palette.

const fixtures = new URL('../fixtures/', import.meta.url)

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

/**
 * Opens the app and waits for the core to be ready. Camera moves are not animated (reduced
 * motion), so a click right after a view change lands on the final view.
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
 * Drops a fixture on the app root (ui.md 6.3). For a model, waits for the document name and for
 * the first frame with the opened document (`ps:open`); otherwise for a notification.
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
  if (!name.endsWith('.ptsm')) {
    await expect(page.getByTestId('notification').first()).toBeVisible()
    return
  }
  // Opening rebuilds the whole model in the core, which takes a few seconds on a loaded runner.
  await expect(page.getByTestId('document-name')).toHaveText(name.replace(/\.ptsm$/, ''), {
    timeout: 30_000,
  })
  await expect.poll(() => openCount(page), { timeout: 30_000 }).toBeGreaterThan(opensBefore)
}

/** The canvas box, its center and height (ui.md 6.3). */
export async function canvas(page: Page): Promise<CanvasBox> {
  const box = await page.getByTestId('viewport').locator('canvas').boundingBox()
  if (box === null) {
    throw new Error('The viewport canvas is not visible.')
  }
  return {
    ...box,
    cx: box.x + box.width / 2,
    cy: box.y + box.height / 2,
    H: box.height,
  }
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

/** Moves the mouse to a page position and polls the viewport's hover kind (ui.md 6.3). */
export async function expectHover(page: Page, x: number, y: number, kind: string): Promise<void> {
  const viewport = page.getByTestId('viewport')
  await expect
    .poll(async () => {
      // Moving away and back makes the controller pick again for the current frame.
      await page.mouse.move(x + 1, y)
      await page.mouse.move(x, y)
      return viewport.getAttribute('data-hover-kind')
    })
    .toBe(kind)
}

/** The suggested name and bytes of the download that `action` starts (ui.md 6.3). */
export async function download(
  page: Page,
  action: () => Promise<void>,
): Promise<{ readonly name: string; readonly bytes: Buffer }> {
  const [started] = await Promise.all([page.waitForEvent('download'), action()])
  const path = await started.path()
  return { name: started.suggestedFilename(), bytes: await readFile(path) }
}

/** Runs a command from the palette: the option whose title is exactly `title`. */
export async function runCommand(page: Page, title: string): Promise<void> {
  await page.keyboard.press('ControlOrMeta+K')
  const input = page.getByTestId('palette-input')
  await expect(input).toBeFocused()
  await input.fill(title)
  const option = page
    .getByTestId('palette-option')
    .filter({ has: page.getByText(title, { exact: true }) })
    .first()
  await option.click()
  await expect(page.getByTestId('palette')).toBeHidden()
}

/** Opens a toolbar group ("Shapes", "Pattern") and runs one of its commands. */
export async function runFromGroup(page: Page, group: string, title: string): Promise<void> {
  await page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: group, exact: true })
    .click()
  await page.getByRole('menuitem', { name: title }).first().click()
}

/** Clicks a toolbar button of the Model context. */
export async function clickTool(page: Page, title: string): Promise<void> {
  await page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: title, exact: true })
    .click()
}

/** The inspector panel. */
export function inspector(page: Page): Locator {
  return page.getByTestId('inspector')
}

/** The inspector field container with a param path. */
export function inspectorField(page: Page, path: string): Locator {
  return inspector(page).locator(`[data-testid=inspector-field][data-path="${path}"]`)
}

/** The selection box of the inspector field with a param path. */
export function selectionField(page: Page, path: string): Locator {
  return inspectorField(page, path).getByTestId('selection-field')
}

/** The expression or text input of the inspector labeled `label` (the manifest label). */
export function inspectorInput(page: Page, label: string): Locator {
  const panel = inspector(page)
  return panel
    .getByRole('combobox', { name: label, exact: true })
    .or(panel.getByRole('textbox', { name: label, exact: true }))
}

/** Replaces the text of an inspector input. */
export async function fillInspector(page: Page, label: string, text: string): Promise<void> {
  const input = inspectorInput(page, label)
  await input.fill(text)
  await expect(input).toHaveValue(text)
}

/** Chooses an option of a choice field (segmented control or select) labeled `label`. */
export async function chooseOption(page: Page, label: string, option: string): Promise<void> {
  const panel = inspector(page)
  const group = panel.getByRole('radiogroup', { name: label, exact: true })
  if ((await group.count()) > 0) {
    await group.getByRole('radio', { name: option, exact: true }).check()
    return
  }
  await panel.getByRole('combobox', { name: label, exact: true }).selectOption({ label: option })
}

/** Waits until the command's preview is drawn for its latest params. */
export async function expectPreviewShown(page: Page): Promise<void> {
  await expect(inspector(page)).toHaveAttribute('data-preview', 'shown', { timeout: 15_000 })
}

/** The timeline item with a feature label. */
export function timelineItem(page: Page, label: string): Locator {
  return page
    .getByTestId('timeline-item')
    .filter({ has: page.getByRole('button', { name: label, exact: true }) })
}

/** The labels of the timeline items, in order. */
export function timelineLabels(page: Page): Promise<string[]> {
  return page
    .getByTestId('timeline-item')
    .evaluateAll((items) =>
      items.map((item) => item.querySelector('button')?.getAttribute('aria-label') ?? ''),
    )
}

/** The browser row of a body, sketch or other item, by its kind and the exact text of its name. */
export function browserRow(page: Page, kind: string, name: string): Locator {
  return page
    .getByTestId('browser')
    .locator(`[data-testid=browser-row][data-kind=${kind}]`)
    .filter({ has: page.getByText(name, { exact: true }) })
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
