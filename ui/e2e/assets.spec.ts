/// <reference lib="dom" />
import { readFile } from 'node:fs/promises'
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  dropFile,
  expectReady,
  fixtureBytes,
  folderNames,
  openApp,
  pickerCalls,
  queuePick,
  readFolderFile,
  renameBody,
  stallCore,
  test,
  timelineItem,
  useFolders,
  writeFolderFile,
} from './helpers'
import { buttonNames, notificationActions, sendRename } from './support/documents'

// Documents with imports and their assets folder, and saves that stop when the file was changed
// elsewhere (ui.md 3.19, 4.4, 6.5 E-150 to E-154, ADR 0037). The tests run on the origin private
// file system behind the pickers of `useFolders` (6.3), with the fixture `step-block.ptsm` and its
// asset `S`, read from `examples/import/assets/` (6.2).

/** `S`: the import track's STEP asset, the 10 by 20 by 30 "Block" of features/import.md 6. */
const S = 'b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step'

/** Opening a document rebuilds it in the real core, which on a busy runner takes a while. */
const OPEN_TIMEOUT = { timeout: 60_000 }

const FOLDER_NEEDED =
  'The imported geometry of step-block.ptsm is missing, because the browser has not given Partsmith the folder that holds it and its assets folder. Choose the folder step-block.ptsm is in.'

/** The bytes of `S`. */
function assetBytes(): Promise<Buffer> {
  return readFile(new URL(`../../examples/import/assets/${S}`, import.meta.url))
}

/** The notification with this `data-code`. */
function coded(page: Page, code: string): Locator {
  return page.locator(`[data-testid=notification][data-code=${code}]`)
}

/** The button of a notification, by its exact name. */
function button(notification: Locator, name: string): Locator {
  return notification.getByRole('button', { name, exact: true })
}

/** Expects "Import 1" to have this `data-status`. */
async function expectImport(page: Page, status: string): Promise<void> {
  await expect(timelineItem(page, 'Import 1')).toHaveAttribute('data-status', status, OPEN_TIMEOUT)
}

/** Expects the body row `name` to show the block's size once clicked. */
async function expectBlock(page: Page, name: string): Promise<void> {
  await bodyRow(page, name).click()
  await expect(page.getByTestId('body-size')).toHaveAttribute('data-value', '10 × 20 × 30 mm')
}

/** Writes `work/step-block.ptsm` with its asset and an `other` folder (E-150's files). */
async function writeWork(page: Page): Promise<void> {
  await writeFolderFile(page, 'work/step-block.ptsm', await fixtureBytes('step-block.ptsm'))
  await writeFolderFile(page, `work/assets/${S}`, await assetBytes())
  await writeFolderFile(page, 'other/notes.txt', Buffer.from('x'))
}

/** E-150's steps: opens `work/step-block.ptsm`, then chooses `other`, then `work`. */
async function openWithFolder(page: Page): Promise<void> {
  await writeWork(page)
  await queuePick(page, 'work/step-block.ptsm')
  await page.keyboard.press('ControlOrMeta+O')
  await expect(page.getByTestId('document-name')).toHaveText('step-block', OPEN_TIMEOUT)
  await expectImport(page, 'error')
  const needed = coded(page, 'assets_folder_needed')
  await expect(needed).toContainText(FOLDER_NEEDED)
  await queuePick(page, 'other')
  await button(needed, 'Choose folder').click()
  const wrong = coded(page, 'assets_wrong_folder')
  await expect(wrong).toContainText(
    'other is not the folder that holds step-block.ptsm. Choose the folder step-block.ptsm is in.',
  )
  await queuePick(page, 'work')
  await button(wrong, 'Choose folder').click()
  await expectImport(page, 'ok')
}

test.describe('Assets and files changed elsewhere', () => {
  test('E-150 opening a document with its folder', async ({ page }) => {
    await useFolders(page)
    await openApp(page)
    await writeWork(page)
    await queuePick(page, 'work/step-block.ptsm')
    await page.keyboard.press('ControlOrMeta+O')
    await expect(page.getByTestId('document-name')).toHaveText('step-block', OPEN_TIMEOUT)
    await expectImport(page, 'error')
    const needed = coded(page, 'assets_folder_needed')
    await expect(needed).toContainText(FOLDER_NEEDED)
    await expect(button(needed, 'Choose folder')).toBeVisible()

    await queuePick(page, 'other')
    await button(needed, 'Choose folder').click()
    const wrong = coded(page, 'assets_wrong_folder')
    await expect(wrong).toContainText(
      'other is not the folder that holds step-block.ptsm. Choose the folder step-block.ptsm is in.',
    )
    expect((await pickerCalls(page)).at(-1)).toEqual({
      picker: 'showDirectoryPicker',
      options: { mode: 'readwrite', startIn: 'step-block.ptsm' },
    })

    await queuePick(page, 'work')
    await button(wrong, 'Choose folder').click()
    await expectImport(page, 'ok')
    await expect(coded(page, 'assets_folder_needed')).toHaveCount(0)
    await expect(page.getByTestId('document-name')).toHaveAttribute('data-dirty', 'false')
    await expectBlock(page, 'Body 1')
  })

  test('E-151 Save as into another folder writes the assets first', async ({ page }) => {
    test.setTimeout(90_000)
    await useFolders(page)
    await openApp(page)
    await openWithFolder(page)
    await writeFolderFile(page, 'copy/notes.txt', Buffer.from('x'))
    const fixture = await fixtureBytes('step-block.ptsm')

    await queuePick(page, 'copy/step-block.ptsm')
    await page.keyboard.press('ControlOrMeta+Shift+S')
    const needed = coded(page, 'save_folder_needed')
    await expect(button(needed, 'Choose folder')).toBeVisible()
    expect(await folderNames(page, 'copy')).not.toContain('assets')

    await queuePick(page, 'copy')
    await button(needed, 'Choose folder').click()
    const name = page.getByTestId('document-name')
    await expect(name).toHaveAttribute('data-dirty', 'false')
    await expect.poll(() => readFolderFile(page, `copy/assets/${S}`)).toEqual(await assetBytes())
    expect(await folderNames(page, 'copy/assets')).toEqual([S])
    // The unchanged document saves as its canonical text.
    expect(await readFolderFile(page, 'copy/step-block.ptsm')).toEqual(fixture)
    expect(await folderNames(page, 'work/assets')).toEqual([S])

    await renameBody(page, 'Body 1', 'Block')
    const callsBefore = (await pickerCalls(page)).length
    await page.keyboard.press('ControlOrMeta+S')
    await expect(name).toHaveAttribute('data-dirty', 'false')
    expect(await pickerCalls(page)).toHaveLength(callsBefore)
    expect((await readFolderFile(page, 'copy/step-block.ptsm'))?.toString('utf8')).toContain(
      '"name": "Block"',
    )
    expect(await readFolderFile(page, 'work/step-block.ptsm')).toEqual(fixture)

    await page.reload()
    await expectReady(page)
    await name.click()
    const menu = page.getByTestId('document-menu')
    await expect(menu).toBeVisible()
    const openRecent = menu.getByRole('menuitem', { name: 'Open recent' })
    await openRecent.click()
    await expect(openRecent).toHaveAttribute('aria-expanded', 'true')
    await page.getByRole('menuitem', { name: 'step-block' }).first().click()
    await expect(bodyRow(page, 'Block')).toBeVisible(OPEN_TIMEOUT)
    await expectImport(page, 'ok')
    // The recent entry kept the folder `copy`, so nothing asked for it.
    expect(await pickerCalls(page)).toEqual([])
  })

  test('E-152 Save stops when the file changed elsewhere', async ({ page }) => {
    await useFolders(page)
    await openApp(page)
    const cube = await fixtureBytes('cube.ptsm')
    const lid = Buffer.from(cube.toString('utf8').replace('"Cube"', '"Lid"'))
    await writeFolderFile(page, 'work/cube.ptsm', cube)
    await queuePick(page, 'work/cube.ptsm')
    await page.keyboard.press('ControlOrMeta+O')
    const name = page.getByTestId('document-name')
    await expect(name).toHaveText('cube', OPEN_TIMEOUT)
    await renameBody(page, 'Cube', 'Box')

    // Another program writes the file.
    await writeFolderFile(page, 'work/cube.ptsm', lid)
    await page.keyboard.press('ControlOrMeta+S')
    const conflict = coded(page, 'save_conflict')
    await expect(conflict).toContainText(
      'cube.ptsm was changed elsewhere after you opened or saved it, so Partsmith did not save over it.',
    )
    expect(await notificationActions(conflict)).toEqual([
      'Overwrite cube.ptsm',
      'Save as',
      'Open the newer file',
    ])
    expect((await readFolderFile(page, 'work/cube.ptsm'))?.toString('utf8')).toContain(
      '"name": "Lid"',
    )
    await expect(name).toHaveAttribute('data-dirty', 'true')

    await button(conflict, 'Overwrite cube.ptsm').click()
    await expect(name).toHaveAttribute('data-dirty', 'false')
    expect((await readFolderFile(page, 'work/cube.ptsm'))?.toString('utf8')).toContain(
      '"name": "Box"',
    )

    // The same bytes written back count as unchanged.
    await renameBody(page, 'Box', 'Peg')
    const own = await readFolderFile(page, 'work/cube.ptsm')
    if (own === null) {
      throw new Error('work/cube.ptsm is gone.')
    }
    await writeFolderFile(page, 'work/cube.ptsm', own)
    await page.keyboard.press('ControlOrMeta+S')
    await expect(name).toHaveAttribute('data-dirty', 'false')
    await expect(coded(page, 'save_conflict')).toHaveCount(0)
    expect((await readFolderFile(page, 'work/cube.ptsm'))?.toString('utf8')).toContain(
      '"name": "Peg"',
    )

    await renameBody(page, 'Peg', 'Pin')
    await writeFolderFile(page, 'work/cube.ptsm', lid)
    await page.keyboard.press('ControlOrMeta+S')
    await button(coded(page, 'save_conflict'), 'Open the newer file').click()
    await expect(bodyRow(page, 'Lid')).toBeVisible(OPEN_TIMEOUT)
    await expect(name).toHaveAttribute('data-dirty', 'false')
    const stashed = page
      .getByTestId('notification')
      .filter({ hasText: 'cube had unsaved changes. They are kept for you.' })
    await expect(button(stashed, 'Reopen')).toBeVisible()
  })

  test('E-153 a core restart keeps the imported geometry', async ({ page }) => {
    test.setTimeout(60_000)
    await stallCore(page, 'Stall here')
    await useFolders(page)
    await openApp(page)
    await openWithFolder(page)
    const callsBefore = (await pickerCalls(page)).length

    // The core never answers this rename.
    await sendRename(page, 'Body 1', 'Stall here')
    const working = page.getByTestId('core-working')
    await expect(working).toBeVisible({ timeout: 20_000 })
    await button(working, 'Stop and restart core').click()
    await expectReady(page)
    await expect(page.getByTestId('core-fatal')).toHaveCount(0)
    await expectImport(page, 'ok')
    expect(await pickerCalls(page)).toHaveLength(callsBefore)
    // The rename was not applied.
    await expectBlock(page, 'Body 1')
  })

  test('E-154 without its folder, the warning says why', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'step-block.ptsm')
    await expectImport(page, 'error')
    const without = coded(page, 'assets_without_folder')
    await expect(without).toContainText(
      'The imported geometry of step-block.ptsm is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead.',
    )
    await expect(button(without, 'Open...')).toBeVisible()

    const second = await page.context().newPage()
    await openApp(second, { fileSystemAccess: false })
    await dropFile(second, 'step-block.ptsm')
    await expectImport(second, 'error')
    const unsupported = coded(second, 'assets_unsupported')
    await expect(unsupported).toContainText(
      'The imported geometry of step-block.ptsm is missing, because this browser does not let Partsmith reach files next to the one you chose. Open it in Chrome or Edge, or in the Partsmith desktop app.',
    )
    expect(await buttonNames(unsupported)).toEqual(['Dismiss'])

    await dropFile(second, 'cube.ptsm')
    await expect(coded(second, 'assets_unsupported')).toHaveCount(0)
    await expect(coded(second, 'assets_without_folder')).toHaveCount(0)
  })
})
