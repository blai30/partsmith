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
  notification,
  openApp,
  renameBody,
  runCommand,
  stallCore,
  timelineItem,
} from './helpers'
import {
  expectStoredRecovery,
  expectTimeline,
  heldRecoveryLocks,
  recentFileNames,
  recoveryOffers,
  storedRecoveries,
  timelineItems,
} from './support/documents'
import {
  chooseContext,
  dragOnCanvas,
  expectCurrentReport,
  healthBadge,
  printTool,
  printToolbar,
} from './support/print'

// Documents, files, recovery, undo and redo, notifications, the status bar and a stuck core
// (ui.md 3.1, 3.19 to 3.22, 4.4): the E-tests of ui.md 6.5 that exercise them, against the real
// core.

const STILL_WORKING =
  'The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. Stopping it reopens your document as it was after the last change the core finished, and closes any open feature command without keeping it.'
const CHANGES_NOT_APPLIED =
  'Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them.'

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

/**
 * Starts renaming a body as `renameBody` does (click its row, F2, type, Enter), without waiting
 * for the core to answer. Returns the time Enter was pressed, which sends the rename.
 */
async function sendRename(page: Page, from: string, to: string): Promise<number> {
  await bodyRow(page, from).click()
  await page.keyboard.press('F2')
  const field = page.getByTestId('browser').getByRole('textbox', { name: 'Name', exact: true })
  await expect(field).toBeFocused()
  await page.keyboard.type(to)
  const sentAt = Date.now()
  await page.keyboard.press('Enter')
  return sentAt
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
    await expect(
      notification(
        page,
        'error',
        'notes.txt is not a Partsmith model. Partsmith opens .ptsm files.',
      ),
    ).toBeVisible()
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
    await expect(
      notification(
        page,
        'info',
        'This browser saves by downloading. cube.ptsm is in your downloads folder.',
      ),
    ).toBeVisible()
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
    await expectStoredRecovery(page, '"name": "Box"')
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

test.describe('Recovery sessions and a stuck core', () => {
  test('E-127 a replaced document keeps its copy through a later autosave and save', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    await renameBody(page, 'Cube', 'Box')
    await expectStoredRecovery(page, '"name": "Box"')
    await dropFile(page, 'two-bodies.ptsm')
    await expect(
      notification(page, 'info', 'cube had unsaved changes. They are kept for you.'),
    ).toBeVisible()
    await renameBody(page, 'Peg', 'Pin')
    await expectStoredRecovery(page, '"name": "Pin"')
    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    expect(saved.name).toBe('two-bodies.ptsm')
    expect(saved.bytes.toString('utf8')).toContain('"name": "Pin"')
    const name = page.getByTestId('document-name')
    await expect(name).toHaveAttribute('data-dirty', 'false')

    await page.reload()
    await expectReady(page)
    const offers = recoveryOffers(page)
    await expect(offers).toHaveCount(1)
    await expect(offers).toHaveText(/^Partsmith kept unsaved changes to cube from/)
    await expect(offers).not.toContainText('other document')
    await offers.getByRole('button', { name: 'Open', exact: true }).click()
    await expect(name).toHaveText('cube')
    await expect(bodyRow(page, 'Box')).toBeVisible()
    await expect(name).toHaveAttribute('data-dirty', 'true')
  })

  test("E-128 two tabs never offer or discard each other's live copy", async ({ context }) => {
    const tabA = await context.newPage()
    await openApp(tabA)
    await dropFile(tabA, 'cube.ptsm')
    await renameBody(tabA, 'Cube', 'Box')
    await expectStoredRecovery(tabA, '"name": "Box"')

    const tabB = await context.newPage()
    await openApp(tabB)
    // A's copy is live, so B offers nothing.
    await expect(recoveryOffers(tabB)).toHaveCount(0)
    await dropFile(tabB, 'two-bodies.ptsm')
    await renameBody(tabB, 'Peg', 'Pin')
    await expectStoredRecovery(tabB, '"name": "Pin"')

    // Closing a page runs no beforeunload; the browser releases its locks.
    await tabA.close()
    await expect.poll(() => heldRecoveryLocks(tabB)).toHaveLength(1)
    const tabC = await context.newPage()
    await openApp(tabC)
    const offersOfC = recoveryOffers(tabC)
    await expect(offersOfC).toHaveCount(1)
    await expect(offersOfC).toHaveText(/^Partsmith kept unsaved changes to cube from/)
    await expect(offersOfC).not.toContainText('other document')
    await offersOfC.getByRole('button', { name: 'Discard', exact: true }).click()
    await expect(offersOfC).toHaveCount(0)
    await expect
      .poll(async () => (await storedRecoveries(tabC)).map((entry) => entry.name))
      .toEqual(['two-bodies'])

    await tabB.close()
    await expect.poll(() => heldRecoveryLocks(tabC)).toHaveLength(0)
    const tabD = await context.newPage()
    await openApp(tabD)
    const offersOfD = recoveryOffers(tabD)
    await expect(offersOfD).toHaveCount(1)
    await expect(offersOfD).toHaveText(/^Partsmith kept unsaved changes to two-bodies from/)
    await offersOfD.getByRole('button', { name: 'Open', exact: true }).click()
    await expect(bodyRow(tabD, 'Cube')).toBeVisible()
    await expect(bodyRow(tabD, 'Pin')).toBeVisible()
    await expect(tabD.getByTestId('document-name')).toHaveAttribute('data-dirty', 'true')
  })

  test('E-129 a stuck core: Stop and restart core reopens the settled recipe and keeps its copy', async ({
    page,
  }) => {
    test.setTimeout(60_000)
    await stallCore(page, 'Stall here')
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await renameBody(page, 'Cube', 'Box')
    await expectStoredRecovery(page, '"name": "Box"')

    // The core never answers this rename, so the row keeps its name.
    const stalledAt = await sendRename(page, 'Box', 'Stall here')
    const status = page.getByTestId('status-core')
    await expect(status).toHaveText('Rebuilding', { timeout: 1_000 })
    // A change that waits behind the rename.
    await bodyRow(page, 'Box').getByRole('button', { name: 'Hide Box', exact: true }).click()

    const working = page.getByTestId('core-working')
    await page.waitForTimeout(Math.max(0, stalledAt + 8_000 - Date.now()))
    await expect(working).toBeHidden()
    await expect(working).toBeVisible({ timeout: Math.max(0, stalledAt + 14_000 - Date.now()) })
    await expect(working).toContainText(STILL_WORKING)
    await expect(status).toHaveText('Still working')

    await working.getByRole('button', { name: 'Stop and restart core', exact: true }).click()
    await expect(working).toBeHidden()
    // The warning comes once the document is open again in the new core.
    await expect(notification(page, 'warning', CHANGES_NOT_APPLIED)).toBeVisible()
    await expectReady(page)
    await expect(page.getByTestId('core-fatal')).toHaveCount(0)
    const name = page.getByTestId('document-name')
    await expect(name).toHaveText('cube')
    await expect(bodyRows(page)).toHaveCount(1)
    await expect(
      bodyRow(page, 'Box').getByRole('button', { name: 'Hide Box', exact: true }),
    ).toHaveCount(1)
    await expect(name).toHaveAttribute('data-dirty', 'true')

    // The new core answers.
    await renameBody(page, 'Box', 'Lid')
    await expectStoredRecovery(page, '"name": "Lid"')
    page.on('dialog', (dialog) => {
      void dialog.accept()
    })
    await page.reload()
    await expectReady(page)
    const offers = recoveryOffers(page)
    await expect(offers).toHaveCount(1)
    // The restart continued the session's entry, so no second entry waits.
    await expect(offers).toHaveText(/^Partsmith kept unsaved changes to cube from/)
    await expect(offers).not.toContainText('other document')
    await offers.getByRole('button', { name: 'Open', exact: true }).click()
    await expect(bodyRow(page, 'Lid')).toBeVisible()
  })
})

test.describe('A core that is still working, continued', () => {
  test('E-130 a slow print check is not a stuck core', async ({ page }) => {
    test.setTimeout(60_000)
    // Only the print analysis request contains this marker.
    await stallCore(page, 'analyze_print')
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await chooseContext(page, 'Print')
    const badge = healthBadge(page)
    await expect(badge).toHaveAttribute('data-state', 'running')
    await expect(badge).toHaveText('Checking')

    await page.waitForTimeout(14_000)
    const working = page.getByTestId('core-working')
    const status = page.getByTestId('status-core')
    await expect(working).toBeHidden()
    await expect(status).toBeHidden()
    await expect(badge).toHaveAttribute('data-state', 'running')
    await expect(page.getByTestId('viewport')).toBeVisible()
    await expect(printToolbar(page)).toBeVisible()
    await expect(page.getByRole('radiogroup', { name: 'Context' })).toBeVisible()

    // The core takes the rename only after the stuck analysis.
    const renamedAt = await sendRename(page, 'Cube', 'Box')
    await expect(status).toHaveText('Rebuilding', { timeout: 1_000 })
    await page.waitForTimeout(Math.max(0, renamedAt + 8_000 - Date.now()))
    await expect(working).toBeHidden()
    await expect(working).toBeVisible({ timeout: Math.max(0, renamedAt + 14_000 - Date.now()) })
    await working.getByRole('button', { name: 'Stop and restart core', exact: true }).click()
    // The warning comes once the document is open again in the new core.
    await expect(notification(page, 'warning', CHANGES_NOT_APPLIED)).toBeVisible()
    await expectReady(page)
    await expect(bodyRow(page, 'Cube')).toBeVisible()
  })

  test('E-131 the still-working panel leaves the model and the window usable', async ({ page }) => {
    test.setTimeout(60_000)
    await stallCore(page, 'Stall here')
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await chooseContext(page, 'Print')
    // The report is in, so no analysis is left for the core to run before the rename.
    await expectCurrentReport(page)

    const stalledAt = await sendRename(page, 'Cube', 'Stall here')
    const view = page.getByTestId('viewport')
    const working = view.getByTestId('core-working')
    await expect(working).toBeVisible({ timeout: Math.max(0, stalledAt + 14_000 - Date.now()) })
    await expect(view).toBeVisible()
    await expect(view.locator('canvas')).toBeVisible()
    const overhangMap = printTool(page, 'Overhang map')
    await expect(overhangMap).toBeVisible()
    await expect(printTool(page, 'Lay on face')).toBeVisible()
    await expect(
      page
        .getByRole('radiogroup', { name: 'Context' })
        .getByRole('radio', { name: 'Print', exact: true }),
    ).toBeChecked()

    await overhangMap.click()
    await expect(overhangMap).toHaveAttribute('aria-pressed', 'false')
    // Below the panel, where the overhang legend was.
    const { cx, cy, H } = await canvas(page)
    const from = { x: cx, y: cy + 0.4 * H }
    await dragOnCanvas(page, from, { x: from.x + 100, y: from.y }, ['right'])
    await expect(view).toHaveAttribute('data-view', 'custom')
    await chooseContext(page, 'Model')
    await expect(page.getByRole('toolbar', { name: 'Model tools' })).toBeVisible()
    await expect(working).toBeVisible()

    await working.getByRole('button', { name: 'Stop and restart core', exact: true }).click()
    await expect(working).toBeHidden()
    await expectReady(page)
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
