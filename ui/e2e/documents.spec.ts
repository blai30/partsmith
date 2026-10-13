/// <reference lib="dom" />
import { expect, test, type Page } from '@playwright/test'
import {
  bodyRow,
  bodyRows,
  canvas,
  download,
  dropFile,
  expectReady,
  fixtureBytes,
  fixturePath,
  inspector,
  inspectorInput,
  openApp,
  renameBody,
  runCommand,
  timelineItem,
} from './helpers'
import {
  expectTimeline,
  notification,
  recentFileNames,
  storedRecovery,
  timelineItems,
} from './support/documents'

// Documents, files, recovery, undo and redo, notifications and the status bar (ui.md 3.1,
// 3.19 to 3.22): the E-tests of ui.md 6.5 that exercise them, against the real core.

const MODEL_TOOLS = [
  'Sketch',
  'Extrude',
  'Revolve',
  'Hole',
  'Fillet',
  'Chamfer',
  'Shell',
  'Shapes',
  'Pattern',
  'Mirror',
  'Boolean',
]

/** Expects the state E-3 and E-4 describe after opening `cube.ptsm`. */
async function expectCubeOpened(page: Page): Promise<void> {
  await expect(page.getByTestId('document-name')).toHaveText('cube')
  await expectTimeline(page, ['Sketch 1', 'Extrude 1'])
  await expect(bodyRow(page, 'Cube')).toBeVisible()
  await expect(page.getByTestId('status-recompute')).toHaveText(/^Rebuilt in \d+(\.\d)? (ms|s)$/)
  await expect(page.getByTestId('document-name')).toHaveAttribute('data-dirty', 'false')
}

/** The data-grid-spacing of the viewport as a number, or NaN when it is not a number. */
async function gridSpacing(page: Page): Promise<number> {
  const value = await page.getByTestId('viewport').getAttribute('data-grid-spacing')
  return value === null || value === '' ? Number.NaN : Number(value)
}

test.describe('Startup and files', () => {
  test('E-1 cold start shows an untitled, clean document in the Model context', async ({
    page,
  }) => {
    await openApp(page)
    const name = page.getByTestId('document-name')
    await expect(name).toHaveText('Untitled')
    await expect(name).toHaveAttribute('data-dirty', 'false')
    await expect(page.getByRole('radio', { name: 'Model', exact: true })).toBeChecked()
    const tools = page.getByRole('toolbar', { name: 'Model tools' }).getByRole('button')
    await expect(tools).toHaveCount(MODEL_TOOLS.length)
    await Promise.all(
      MODEL_TOOLS.map((title, index) => expect(tools.nth(index)).toHaveAccessibleName(title)),
    )
    await expect(page.getByTestId('status-units')).toHaveText('mm')
    await expect(page.getByTestId('health-badge')).toHaveAttribute('data-state', 'none')
  })

  test('E-2 without WebGL2 only the unsupported page renders and no worker starts', async ({
    page,
  }) => {
    await page.addInitScript(() => {
      const getContext = HTMLCanvasElement.prototype.getContext
      // Every other context type still works; only WebGL2 is missing.
      HTMLCanvasElement.prototype.getContext = function (
        this: HTMLCanvasElement,
        type: string,
        ...rest: unknown[]
      ) {
        return type === 'webgl2' ? null : Reflect.apply(getContext, this, [type, ...rest])
      } as typeof getContext
    })
    await page.goto('/', { waitUntil: 'load' })
    const unsupported = page.getByTestId('unsupported')
    await expect(unsupported).toBeVisible()
    await expect(unsupported).toHaveText(
      'Partsmith needs WebGL2 to draw the 3D view, and this browser does not provide it. Turn on hardware acceleration in your browser settings, or use a current version of Chrome, Edge, Firefox or Safari.',
    )
    await expect(page.getByTestId('app')).toHaveCount(0)
    expect(page.workers()).toHaveLength(0)
  })

  test('E-3 dropping cube.ptsm opens it clean with its features and body', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await expectCubeOpened(page)
  })

  test('E-4 Open without File System Access reads the file from the file chooser', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    const chooser = page.waitForEvent('filechooser')
    await page.keyboard.press('ControlOrMeta+O')
    await (await chooser).setFiles(fixturePath('cube.ptsm'))
    await expectCubeOpened(page)
  })

  test('E-5 dropping a file that is not a model explains it and keeps the document', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'notes.txt')
    const error = notification(
      page,
      'notes.txt is not a Partsmith model. Partsmith opens .ptsm files.',
    )
    await expect(error).toBeVisible()
    await expect(error).toHaveAttribute('data-severity', 'error')
    await expect(page.getByTestId('document-name')).toHaveText('Untitled')
  })

  test('E-6 Save without File System Access downloads the unchanged file byte for byte', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    expect(saved.name).toBe('cube.ptsm')
    const fixture = await fixtureBytes('cube.ptsm')
    expect(saved.bytes.toString('utf8')).toBe(fixture.toString('utf8'))
    expect(saved.bytes.equals(fixture)).toBe(true)
    const hint = notification(
      page,
      'This browser saves by downloading. cube.ptsm is in your downloads folder.',
    )
    await expect(hint).toBeVisible()
    await expect(hint).toHaveAttribute('data-severity', 'info')
  })

  test('E-7 an edit marks the document edited and saving it clears the mark', async ({ page }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    await renameBody(page, 'Cube', 'Box')
    const name = page.getByTestId('document-name')
    const saveState = page.getByTestId('save-state')
    await expect(saveState).toHaveText('Edited')
    await expect(name).toHaveAttribute('data-dirty', 'true')
    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    expect(saved.bytes.toString('utf8')).toContain('"name": "Box"')
    await expect(name).toHaveAttribute('data-dirty', 'false')
    await expect(saveState).toHaveText('')
  })

  test('E-8 Open recent reopens a dropped file after a reload', async ({ page }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    // Remembering the file is the last step of opening it (ui.md 3.19); reload once it is stored.
    await expect.poll(() => recentFileNames(page)).toContain('cube.ptsm')
    await page.reload()
    await expectReady(page)
    await page.getByTestId('document-name').click()
    const menu = page.getByTestId('document-menu')
    await expect(menu).toBeVisible()
    const openRecent = menu.getByRole('menuitem', { name: 'Open recent' })
    await openRecent.click()
    await expect(openRecent).toHaveAttribute('aria-expanded', 'true')
    const recent = page.getByRole('menuitem', { name: 'cube' })
    await expect(recent).toBeVisible()
    await recent.click()
    await expect(page.getByTestId('document-name')).toHaveText('cube')
    await expect(timelineItems(page)).toHaveCount(2)
  })

  test('E-9 unsaved changes ask before leaving and are offered again after a reload', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await renameBody(page, 'Cube', 'Box')
    // The spec waits 3 s: the recovery copy is written 2 s after the last change (ui.md 3.19).
    // Waiting for that copy itself keeps the test independent of timing.
    await expect
      .poll(async () => {
        const recovery = await storedRecovery(page)
        return recovery !== null && recovery.text.includes('"name": "Box"')
      })
      .toBe(true)
    const dialogTypes: string[] = []
    page.on('dialog', (dialog) => {
      dialogTypes.push(dialog.type())
      void dialog.accept()
    })
    await page.reload()
    expect(dialogTypes).toEqual(['beforeunload'])
    await expectReady(page)
    const offer = page
      .getByTestId('notification')
      .filter({ hasText: /^Partsmith kept unsaved changes to cube from/ })
    await expect(offer).toBeVisible()
    await offer.getByRole('button', { name: 'Open', exact: true }).click()
    await expect(bodyRow(page, 'Box')).toBeVisible()
    await expect(page.getByTestId('document-name')).toHaveAttribute('data-dirty', 'true')
  })
})

test.describe('Undo and redo', () => {
  test('E-39 undo and redo a rename; Mod+Z in a text field is not a document undo', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await renameBody(page, 'Cube', 'Box')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(bodyRows(page)).toHaveCount(1)
    await expect(bodyRow(page, 'Cube')).toBeVisible()
    await page.keyboard.press('ControlOrMeta+Shift+Z')
    await expect(bodyRow(page, 'Box')).toBeVisible()
    await runCommand(page, 'Parameters')
    await page
      .getByTestId('parameters-panel')
      .getByRole('button', { name: 'Add parameter', exact: true })
      .click()
    await page.keyboard.type('abc')
    // The keys went to the new parameter's name field, so Mod+Z is the field's own undo.
    await expect(page.getByTestId('parameters-panel').locator('input:focus')).toHaveValue('abc')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(bodyRows(page)).toHaveCount(1)
    await expect(bodyRow(page, 'Box')).toBeVisible()
  })
})

test.describe('Notifications', () => {
  test('E-83 a failure notification goes away when an undo fixes the failure', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube-features.ptsm')
    await timelineItem(page, 'Fillet 1').getByRole('button').dblclick()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    const radius = inspectorInput(page, 'Radius')
    await radius.fill('15')
    await expect(radius).toHaveValue('15')
    await page.keyboard.press('Enter')
    const errors = page.locator('[data-testid=notification][data-severity=error]')
    await expect(errors.first()).toBeVisible()
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(errors).toHaveCount(0, { timeout: 2_000 })
  })
})

test.describe('Status bar', () => {
  test('E-96 the grid spacing follows the zoom and hides in Print', async ({ page }) => {
    await openApp(page)
    const viewport = page.getByTestId('viewport')
    const status = page.getByTestId('status-grid')
    await expect.poll(() => gridSpacing(page)).not.toBeNaN()
    const before = await gridSpacing(page)
    await expect(status).toHaveText(`Grid ${String(before)} mm`)
    const { cx, cy } = await canvas(page)
    await page.mouse.move(cx, cy)
    for (let i = 0; i < 20; i++) {
      // The wheel events are separate events, sent one after another.
      // oxlint-disable-next-line no-await-in-loop
      await page.mouse.wheel(0, 100)
    }
    await expect.poll(() => gridSpacing(page)).toBeGreaterThan(before)
    await page.getByRole('radio', { name: 'Print', exact: true }).click()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'print')
    await expect.poll(async () => (await viewport.getAttribute('data-grid-spacing')) ?? '').toBe('')
    await expect(status).toBeHidden()
  })
})
