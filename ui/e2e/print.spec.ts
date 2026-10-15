/// <reference lib="dom" />
import { expect, test, type Page } from '@playwright/test'
import {
  canvas,
  download,
  dropFile,
  expectHover,
  expectReady,
  fitView,
  notification,
  openApp,
  renameBody,
  runCommand,
  viewCubeButton,
  viewport,
} from './helpers'
import {
  cameraTarget,
  chooseContext,
  dragOnCanvas,
  expectCurrentReport,
  expectTargetChanged,
  exportButton,
  exportFromPopover,
  exportStatusText,
  healthBadge,
  healthItems,
  openExportPopover,
  openPrintCheck,
  printTool,
  printToolbar,
  printerButton,
  watchReportLeft,
  wheelAt,
} from './support/print'

// Viewport navigation and picking (ui.md 3.12 to 3.15), the Print context (3.17), export (3.18)
// and the tooltips of their controls after Esc (3.24) against the production build and the real
// core (6.5).

const REGION_CRASHED = 'This part of the window hit a problem. Reload to restore it.'

/** Expects the viewport's classified view (ui.md 3.13). */
async function expectView(page: Page, view: string): Promise<void> {
  await expect(viewport(page)).toHaveAttribute('data-view', view)
}

/** `text` with the characters that are special in a regular expression escaped. */
function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/** Starts a page without File System Access, opens `cube.ptsm` and goes to Print. */
async function openCubeInPrint(page: Page): Promise<void> {
  await openApp(page, { fileSystemAccess: false })
  await dropFile(page, 'cube.ptsm')
  await chooseContext(page, 'Print')
}

test.describe('Viewport', () => {
  test('E-41 the view cube sets standard views and Shift+middle drag orbits', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    const home = page.getByRole('button', { name: 'Home view', exact: true })
    await viewCubeButton(page, 'Top view').click()
    await expectView(page, 'top')
    // Seen from the top, the cube's Front face is turned away and cannot be clicked, so the Home
    // view comes first (ui.md 6.5 E-41).
    await home.click()
    await expectView(page, 'iso')
    await viewCubeButton(page, 'Front view').click()
    await expectView(page, 'front')
    await viewCubeButton(page, 'Front right top corner view').click()
    await expectView(page, 'iso')
    const { cx, cy } = await canvas(page)
    await dragOnCanvas(page, { x: cx, y: cy }, { x: cx + 100, y: cy }, ['middle'], { shift: true })
    await expectView(page, 'custom')
  })

  test('E-42 the Orthographic control and Shift+P switch the projection', async ({ page }) => {
    await openApp(page)
    const orthographic = page.getByRole('button', { name: 'Orthographic', exact: true })
    await orthographic.click()
    await expect(orthographic).toHaveAttribute('aria-pressed', 'true')
    await expect(viewport(page)).toHaveAttribute('data-projection', 'orthographic')
    await page.keyboard.press('Shift+P')
    await expect(viewport(page)).toHaveAttribute('data-projection', 'perspective')
    await expect(orthographic).toHaveAttribute('aria-pressed', 'false')
  })

  test('E-43 middle drag pans, F6 fits, and the wheel zooms at the cursor', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy, H } = await canvas(page)
    const before = await cameraTarget(page)
    await dragOnCanvas(page, { x: cx, y: cy }, { x: cx + 0.6 * H, y: cy }, ['middle'])
    await expectTargetChanged(page, before)
    await expectHover(page, cx, cy, 'none')
    await expectHover(page, cx + 0.6 * H, cy, 'face')
    const fitted = await cameraTarget(page)
    await page.keyboard.press('F6')
    await expectTargetChanged(page, fitted)
    await wheelAt(page, { x: cx, y: cy }, -100, 5)
    await expectHover(page, cx, cy + 0.45 * H, 'face')
  })

  test('E-44 the FreeCAD preset orbits with middle and left, and pans with the middle button', async ({
    page,
  }) => {
    await openApp(page)
    await runCommand(page, 'Navigation: FreeCAD')
    await expect(viewport(page)).toHaveAttribute('data-nav-preset', 'freecad')
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    await dragOnCanvas(page, { x: cx, y: cy }, { x: cx + 100, y: cy }, ['middle', 'left'])
    await expectView(page, 'custom')
    await viewCubeButton(page, 'Front view').click()
    await expectView(page, 'front')
    const before = await cameraTarget(page)
    await dragOnCanvas(page, { x: cx, y: cy }, { x: cx + 100, y: cy }, ['middle'])
    await expectTargetChanged(page, before)
    await expectView(page, 'front')
  })

  test('E-45 the section view removes the half above its offset from view and picking', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy, H } = await canvas(page)
    await page.keyboard.press('Shift+X')
    const popover = page.getByTestId('section-popover')
    await expect(popover).toBeVisible()
    const offset = popover.getByRole('combobox', { name: 'Offset', exact: true })
    await offset.fill('10')
    await offset.press('Enter')
    await expect(viewport(page)).toHaveAttribute('data-section', 'on')
    await expectHover(page, cx, cy - 0.2 * H, 'none')
    await expectHover(page, cx, cy + 0.2 * H, 'face')
    // While a popover is open only Mod+K is a shortcut (ui.md 2.10), so Esc closes it first.
    await page.keyboard.press('Escape')
    await expect(popover).toBeHidden()
    await page.keyboard.press('Shift+X')
    await expect(viewport(page)).toHaveAttribute('data-section', 'off')
    await expectHover(page, cx, cy - 0.2 * H, 'face')
  })

  test('E-46 zooming out shrinks the cube, and F6 frames it again', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy, H } = await canvas(page)
    await wheelAt(page, { x: cx, y: cy }, 100, 10)
    await expectHover(page, cx, cy - 0.39 * H, 'none')
    await page.keyboard.press('F6')
    await expectHover(page, cx, cy - 0.38 * H, 'face')
    await expectHover(page, cx, cy - 0.42 * H, 'none')
  })

  test('E-65 right drag orbits, Shift+right drag pans, and a right click opens the menu', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    await dragOnCanvas(page, { x: cx, y: cy }, { x: cx + 100, y: cy }, ['right'])
    await expectView(page, 'custom')
    await viewCubeButton(page, 'Front view').click()
    await expectView(page, 'front')
    const before = await cameraTarget(page)
    await dragOnCanvas(page, { x: cx, y: cy }, { x: cx + 100, y: cy }, ['right'], { shift: true })
    await expectView(page, 'front')
    await expectTargetChanged(page, before)
    await page.mouse.click(cx, cy, { button: 'right' })
    const menu = page.getByTestId('viewport-menu')
    await expect(menu).toBeVisible()
    await expect(viewport(page)).toHaveAttribute('data-selection-count', '1')
    await expectView(page, 'front')
    await page.keyboard.press('Escape')
    await expect(menu).toBeHidden()
  })
})

test.describe('Print and export', () => {
  test('E-47 Print shows its tools, the overhang map and findings that frame their place', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'overhang.ptsm')
    await chooseContext(page, 'Print')
    const buttons = printToolbar(page).getByRole('button')
    const expected = [
      'Printer:',
      'Overhang map',
      'Checks',
      'Print check',
      'Lay on face',
      'Export 3MF',
      'Export',
    ]
    await expect(buttons).toHaveCount(expected.length)
    await Promise.all(
      expected.map((prefix, index) =>
        expect(buttons.nth(index)).toHaveAccessibleName(new RegExp(`^${escapeRegExp(prefix)}`)),
      ),
    )
    await expect(printTool(page, 'Overhang map')).toHaveAttribute('aria-pressed', 'true')
    const badge = healthBadge(page)
    await expect(badge).toHaveAttribute('data-state', /^(warning|error)$/, { timeout: 20_000 })
    await expect(badge).toHaveAttribute('data-stale', 'false')
    const legend = page.getByTestId('overhang-legend')
    await expect(legend).toBeVisible()
    await expect(legend).toContainText('°')
    await badge.click()
    const overhangs = healthItems(page, 'overhangs')
    await expect(overhangs.first()).toBeVisible()
    const before = await cameraTarget(page)
    await overhangs.first().click()
    await expectTargetChanged(page, before)
    await expect(page.getByTestId('finding-marker')).toBeVisible()
  })

  test('E-48 choosing a printer shows it in the status bar, and undo restores it', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await chooseContext(page, 'Print')
    const status = page.getByTestId('status-printer')
    const previous = (await status.innerText()).trim()
    await printerButton(page).click()
    const menu = page.getByTestId('printer-menu')
    const items = menu.getByRole('menuitemradio')
    await expect(menu).toBeVisible()
    await expect.poll(() => items.count()).toBeGreaterThanOrEqual(2)
    // A profile that is not checked; the last item, "Default (...)", only clears the printer.
    const unchecked = items.and(menu.locator('[aria-checked=false]')).filter({
      hasNotText: /^Default \(/,
    })
    const choice = unchecked.first()
    // The item's first line is the profile name; its key numbers follow on the next line.
    const name = (await choice.innerText()).split('\n')[0]?.trim() ?? ''
    await choice.click()
    await expect(status).toHaveText(name)
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(status).toHaveText(previous)
  })

  test('E-49 a part larger than the bed fails the bed fit check', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'oversize.ptsm')
    await chooseContext(page, 'Print')
    await expect(healthBadge(page)).toHaveAttribute('data-state', 'error', { timeout: 20_000 })
    await openPrintCheck(page)
    await expect(
      healthItems(page, 'bed_fit').and(page.locator('[data-severity=error]')),
    ).not.toHaveCount(0)
  })

  test('E-50 a hidden check hides its findings only, the map has its own toggle, and the choice is kept', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'overhang.ptsm')
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    await openPrintCheck(page)
    await expect(healthItems(page, 'overhangs')).not.toHaveCount(0)
    await printTool(page, 'Checks').click()
    const checks = page.getByTestId('checks-popover')
    await expect(checks).toBeVisible()
    const overhangsSwitch = checks.getByRole('switch', { name: 'Overhangs', exact: true })
    await overhangsSwitch.click()
    await expect(overhangsSwitch).toHaveAttribute('aria-checked', 'false')
    await expect(healthItems(page, 'overhangs')).toHaveCount(0)
    const legend = page.getByTestId('overhang-legend')
    await expect(legend).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(checks).toBeHidden()
    await printTool(page, 'Overhang map').click()
    await expect(legend).toHaveCount(0)
    await page.reload()
    await expectReady(page)
    await dropFile(page, 'overhang.ptsm')
    await chooseContext(page, 'Print')
    await printTool(page, 'Checks').click()
    await expect(
      page.getByTestId('checks-popover').getByRole('switch', { name: 'Overhangs', exact: true }),
    ).toHaveAttribute('aria-checked', 'false')
  })

  test('E-51 a change in Model makes the report out of date, and the badge checks again', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'overhang.ptsm')
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    await chooseContext(page, 'Model')
    await renameBody(page, 'Mushroom', 'Toadstool')
    const badge = healthBadge(page)
    await expect(badge).toHaveAttribute('data-stale', 'true')
    await badge.click()
    await expect(badge).toHaveAttribute('data-stale', 'false', { timeout: 20_000 })
    await expect(page.getByTestId('health-list')).toBeVisible()
  })

  test('E-52 Export 3MF downloads a 3MF package', async ({ page }) => {
    await openCubeInPrint(page)
    const file = await download(page, () => printTool(page, 'Export 3MF').click())
    expect(file.name).toBe('cube.3mf')
    expect([...file.bytes.subarray(0, 4)]).toEqual([0x50, 0x4b, 0x03, 0x04])
    await expect(notification(page, 'success', 'Exported cube.3mf.')).toBeVisible()
  })

  test('E-53 the export popover exports binary STL', async ({ page }) => {
    await openCubeInPrint(page)
    const file = await exportFromPopover(page, 'STL')
    expect(file.name).toBe('cube.stl')
    const triangles = file.bytes.readUInt32LE(80)
    expect(file.bytes.length).toBe(84 + 50 * triangles)
    expect(triangles).toBeGreaterThanOrEqual(12)
  })

  test('E-54 the export popover exports STEP and OBJ', async ({ page }) => {
    await openCubeInPrint(page)
    const step = await exportFromPopover(page, 'STEP')
    expect(step.name).toBe('cube.step')
    expect(step.bytes.toString('utf8').startsWith('ISO-10303-21;')).toBe(true)
    const obj = await exportFromPopover(page, 'OBJ')
    expect(obj.name).toBe('cube.obj')
    const lines = obj.bytes.toString('utf8').split(/\r?\n/)
    expect(lines.some((line) => line.startsWith('v '))).toBe(true)
    expect(lines.some((line) => line.startsWith('f '))).toBe(true)
  })

  test('E-55 exporting with no visible body says so and downloads nothing', async ({ page }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    await page
      .getByTestId('browser')
      .getByRole('button', { name: 'Hide Cube', exact: true })
      .click()
    await expect(
      page.getByTestId('browser').getByRole('button', { name: 'Show Cube', exact: true }),
    ).toBeVisible()
    await chooseContext(page, 'Print')
    let downloads = 0
    page.on('download', () => {
      downloads += 1
    })
    await printTool(page, 'Export 3MF').click()
    await expect(
      notification(page, 'error', 'There is no visible body with geometry to export.'),
    ).toBeVisible()
    // The spec's window for a download that must not start (ui.md 6.5 E-55).
    const started = await page.waitForEvent('download', { timeout: 2_000 }).then(
      () => true,
      () => false,
    )
    expect(started).toBe(false)
    expect(downloads).toBe(0)
  })

  test('E-95 the export popover and the export name the failed feature', async ({ page }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'fillet-error.ptsm')
    const popover = await openExportPopover(page)
    await expect
      .poll(() => exportStatusText(page))
      .toContain('Fillet 1 failed, so it is not in the export.')
    await expect.poll(() => exportStatusText(page)).toContain('Print check: Not checked')
    await expect(popover.getByRole('button', { name: 'Show problems', exact: true })).toBeVisible()
    await expect(popover.getByRole('button', { name: 'Check print', exact: true })).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(popover).toBeHidden()
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    await download(page, () => printTool(page, 'Export 3MF').click())
    const warning = notification(
      page,
      'warning',
      'Exported fillet-error.3mf. Fillet 1 failed, so it is not in the export.',
    )
    await expect(warning).toBeVisible()
    await expect(warning.getByRole('button', { name: 'Show problems', exact: true })).toBeVisible()
  })

  test('E-97 the printer button names the default printer and the menu lists its key numbers', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await chooseContext(page, 'Print')
    const button = printerButton(page)
    // The toolbar shows the button's label, or only its icon when the labels do not fit (3.2);
    // either way the label is its accessible name.
    await expect(button).toHaveAccessibleName('Printer: Generic FDM 0.4 mm (default)')
    await expect(page.getByTestId('status-printer')).toHaveText('Generic FDM 0.4 mm (default)')
    await button.click()
    const item = page
      .getByTestId('printer-menu')
      .getByRole('menuitemradio', { name: 'Generic FDM 0.4 mm', exact: true })
    await expect(item).toContainText('256 × 256 × 256 mm bed · 0.4 mm nozzle')
  })

  test('E-100 Lay on face rests a picked face on the bed until it is clicked again', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    const layOnFace = printTool(page, 'Lay on face')
    await layOnFace.click()
    await expect(page.getByTestId('print-hint')).toHaveText(
      'Pick the face that should touch the bed.',
    )
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    // The new orientation makes the report out of date before it is checked again (3.17), so the
    // current report after that is the one for the picked face, not the one from before the pick.
    const reportLeft = await watchReportLeft(page)
    await page.mouse.click(cx, cy)
    await expect(layOnFace).toHaveAttribute('aria-pressed', 'true')
    await reportLeft()
    await expectCurrentReport(page)
    await openExportPopover(page)
    await expect
      .poll(() => exportStatusText(page))
      .toContain('Printing with the picked face of Cube on the bed.')
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('export-popover')).toBeHidden()
    await layOnFace.click()
    await expect(layOnFace).toHaveAttribute('aria-pressed', 'false')
  })
})

test.describe('Tooltips after Esc', () => {
  test('E-132 Esc gives focus back without a tooltip', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    const exporting = exportButton(page)
    const popover = await openExportPopover(page)
    await page.keyboard.press('Escape')
    await expect(popover).toBeHidden()
    await expect(exporting).toBeFocused()
    const tooltips = page.getByRole('tooltip')
    await expect(tooltips).toHaveCount(0)
    await expect(page.getByTestId('top-bar')).toBeVisible()
    await expect(page.getByText(REGION_CRASHED, { exact: true })).toHaveCount(0)
    // Keyboard focus that follows another key shows the tooltip again.
    await page.keyboard.press('Shift+Tab')
    await page.keyboard.press('Tab')
    await expect(exporting).toBeFocused()
    await expect(tooltips).toHaveCount(1)
    await expect(tooltips).toContainText('Export')

    await fitView(page, 'Front')
    await page.keyboard.press('Shift+X')
    const section = page.getByTestId('section-popover')
    await expect(section).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(section).toBeHidden()
    await expect(page.getByRole('button', { name: 'Fit to view', exact: true })).toBeFocused()
    await expect(tooltips).toHaveCount(0)
    await expect(viewport(page)).toBeVisible()
  })
})
