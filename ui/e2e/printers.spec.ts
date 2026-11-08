/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  browserRow,
  download,
  dropFile,
  notification,
  openApp,
  runCommand,
  test,
} from './helpers'
import { chooseContext, printerButton } from './support/print'

// Custom printer profiles with the real core (ui.md 3.17, 6.5 E-168 to E-170, ADR 0062): a printer
// made in the app is kept in the browser's library and stored in the document, a document that
// stores its printer opens the same in a fresh browser, and a version 2 document that names a
// printer this browser lacks offers to add it (newcomer error corpus NC13).

/** The `unknown_printer_profile` warning of engine.md 3.4 for `my-printer`. */
const UNKNOWN_MY_PRINTER =
  'Printer profile "my-printer" is not among your printers. Printer values and checks use the default FDM profile until you add that printer or pick another one.'

/** The printer dialog (ui.md 3.17). */
function printerDialog(page: Page): Locator {
  return page.getByTestId('printer-dialog')
}

/** Replaces the text of a printer dialog field, by its label. */
async function fillField(page: Page, label: string, text: string): Promise<void> {
  await printerDialog(page).getByRole('textbox', { name: label, exact: true }).fill(text)
}

/** Opens the printer menu with the status bar's printer button and returns it. */
async function openPrinterMenu(page: Page): Promise<Locator> {
  await page.getByTestId('status-printer').click()
  const menu = page.getByTestId('printer-menu')
  await expect(menu).toBeVisible()
  return menu
}

/** Opens the Problems panel with the palette command, which is there with or without problems (3.9). */
async function openProblemsPanel(page: Page): Promise<Locator> {
  await runCommand(page, 'Problems')
  const panel = page.getByTestId('problems-panel')
  await expect(panel).toBeVisible()
  return panel
}

test.describe('custom printer profiles (3.17)', () => {
  test('E-168 a printer made in the app is kept and stored in the document', async ({ page }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    await chooseContext(page, 'Print')
    await printerButton(page).click()
    await page.getByTestId('printer-menu').getByRole('menuitem', { name: 'Add printer…' }).click()
    await expect(printerDialog(page)).toHaveAccessibleName('New printer')
    await fillField(page, 'Name', 'My MK4')
    await fillField(page, 'Layer height', '0.15')
    await fillField(page, 'Slip fit', '0.15')
    await fillField(page, 'Loose fit', '0.3')
    await printerDialog(page).getByRole('button', { name: 'Save', exact: true }).click()
    await expect(printerDialog(page)).toHaveCount(0)
    await expect(page.getByTestId('status-printer')).toHaveText('My MK4')

    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    const lines = saved.bytes.toString('utf8').split('\n')
    const printerLine = lines.indexOf('  "printer": "my-mk4",')
    expect(printerLine).toBeGreaterThan(-1)
    expect(lines[printerLine + 1]).toBe('  "printer_profile": {')
    expect(lines).toContain('    "layer_height": 0.15,')

    await page.reload()
    await openApp(page, { fileSystemAccess: false })
    const menu = await openPrinterMenu(page)
    await expect(menu.getByRole('menuitemradio', { name: 'My MK4', exact: true })).toHaveCount(1)
  })

  test('E-169 a document that stores its printer opens the same in a fresh browser', async ({
    page,
  }) => {
    // Each test runs in a new browser context, whose IndexedDB starts empty.
    await openApp(page)
    await dropFile(page, 'custom-printer.ptsm')
    const panel = await openProblemsPanel(page)
    await expect(panel).toContainText('No problems.')
    await expect(page.getByTestId('status-printer')).toHaveText('My MK4')
    const clearance = browserRow(page, 'parameter', 'clearance = printer.fit.slip')
    await expect(clearance).toContainText('0.15 mm')
    const offer = notification(
      page,
      'info',
      "custom-printer.ptsm uses the printer 'My MK4', which is not among your printers.",
    )
    await expect(offer).toBeVisible()
    await offer.getByRole('button', { name: "Add 'My MK4' to my printers", exact: true }).click()
    await expect(notification(page, 'success', "Added 'My MK4' to your printers.")).toBeVisible()
    await bodyRow(page, 'Cube').click()
    // 20.4 with generic-fdm's slip of 0.2.
    await expect(page.getByTestId('body-size')).toHaveAttribute('data-value', '20 × 20 × 20.3 mm')

    await page.reload()
    await openApp(page)
    const menu = await openPrinterMenu(page)
    await expect(menu.getByRole('menuitemradio', { name: 'My MK4', exact: true })).toHaveCount(1)
    await expect(menu.getByRole('menuitemradio', { name: /\(this document\)$/ })).toHaveCount(0)
  })

  test('E-170 a version 2 document that names a printer this browser lacks offers to add it', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'printer-v2.ptsm')
    await expect(
      notification(
        page,
        'info',
        'printer-v2.ptsm was updated from format version 2. Saving writes the current format.',
      ),
    ).toBeVisible()
    const panel = await openProblemsPanel(page)
    await expect(panel.getByTestId('problem-row')).toHaveCount(0)
    await expect(panel.getByText(UNKNOWN_MY_PRINTER, { exact: true })).toHaveCount(1)
    await expect(panel.getByRole('button', { name: 'Choose printer', exact: true })).toBeVisible()
    await panel.getByRole('button', { name: "Add printer 'my-printer'", exact: true }).click()
    await printerDialog(page).getByRole('button', { name: 'Save', exact: true }).click()
    await expect(panel).toContainText('No problems.')
    await expect(page.getByTestId('status-printer')).toHaveText('my-printer')
  })
})
