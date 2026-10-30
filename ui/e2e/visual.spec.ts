/// <reference lib="dom" />
import { fileURLToPath } from 'node:url'
import { expect, type Page } from '@playwright/test'
import {
  bodyRow,
  canvas,
  dropFile,
  expectHover,
  fitView,
  inspector,
  openApp,
  runCommand,
  test,
  timelineItem,
  toScreen,
} from './helpers'
import { openProblems } from './support/documents'
import { selectionField } from './support/modeling'
import { chooseContext, expectCurrentReport } from './support/print'
import { editSketchFromTimeline } from './support/sketch'

// Visual tests (ui.md 6.6): screenshots of the whole window in both themes, compared with the
// baselines in visual.spec.ts-snapshots/. The configuration sets `animations: 'disabled'`,
// `maxDiffPixelRatio: 0.002` and `threshold: 0.05` for every `toHaveScreenshot`.

type ColorScheme = 'light' | 'dark'

/**
 * The self-hosted fonts every screenshot must show (2.16.7). `document.fonts.check` alone also
 * passes when a stylesheet never loaded (a family without faces counts as available), and a family
 * no element has used yet is not loaded (JetBrains Mono on a screen without code), so each family
 * is loaded explicitly and must resolve with at least one of its faces.
 */
const FONTS = ['14px Geist', '14px "JetBrains Mono"'] as const

/** Hides what changes from run to run while a screenshot is taken. */
const SCREENSHOT_STYLE = fileURLToPath(new URL('visual.css', import.meta.url))

/** Opens the app in one theme, chosen through the OS preference so settings stay `system`. */
async function openInTheme(page: Page, colorScheme: ColorScheme): Promise<void> {
  await page.emulateMedia({ colorScheme })
  await openApp(page)
  // The root always carries `dark:scheme-dark` (2.16.2), so the theme is the `dark` token itself.
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(colorScheme === 'dark')
}

/** Waits until both web fonts are loaded, and fails when either falls back to a system font. */
async function expectWebFonts(page: Page): Promise<void> {
  const fonts = await page.evaluate(async (specifiers) => {
    await document.fonts.ready
    const loaded = await Promise.all(specifiers.map((specifier) => document.fonts.load(specifier)))
    return specifiers.map((specifier, index) => ({
      specifier,
      faces: loaded[index]?.length ?? 0,
      check: document.fonts.check(specifier),
    }))
  }, FONTS)
  for (const font of fonts) {
    expect(font.faces, `${font.specifier} has no web font face`).toBeGreaterThan(0)
    expect(font.check, `${font.specifier} is not loaded`).toBe(true)
  }
}

/**
 * Moves the pointer onto the status bar's left padding, where nothing hovers or shows a tooltip at
 * any UI scale, and waits until no tooltip is open.
 */
async function parkPointer(page: Page): Promise<void> {
  const bar = await page.getByTestId('status-bar').boundingBox()
  if (bar === null) {
    throw new Error('The status bar is not visible.')
  }
  await page.mouse.move(bar.x + 4, bar.y + bar.height / 2)
  await expect(page.getByRole('tooltip')).toHaveCount(0)
}

/** Compares the window with a baseline once the fonts are in. */
async function expectWindow(page: Page, name: string): Promise<void> {
  await expectWebFonts(page)
  await expect(page).toHaveScreenshot(name, { stylePath: SCREENSHOT_STYLE })
}

/** Edits "Sketch 1" by double-clicking its timeline item and waits for its solved state. */
async function editSketch(page: Page, state: string, dof: string): Promise<void> {
  await editSketchFromTimeline(page, 'Sketch 1')
  const header = page.getByTestId('sketch-header')
  await expect(header).toHaveAttribute('data-state', state)
  await expect(header).toHaveAttribute('data-dof', dof)
}

/** Opens the Problems panel of `fillet-error.ptsm` and edits "Fillet 1" from it (E-28). */
async function editFailedFillet(page: Page): Promise<void> {
  await dropFile(page, 'fillet-error.ptsm')
  await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'error')
  const panel = await openProblems(page)
  await panel.getByRole('button', { name: 'Edit', exact: true }).click()
  await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
  await expect(inspector(page)).toHaveAttribute('data-preview', 'failed')
  await expect(inspector(page).getByTestId('inspector-error')).toHaveAttribute('role', 'alert')
}

test.describe('visual @visual', () => {
  // Playwright applies `stylePath` as a `<style>` element, which the preview's `style-src 'self'`
  // refuses, so the rebuild time would show; the other E-tests check the policy (ui.md 6.6,
  // delivery.md 2.7).
  test.use({ bypassCSP: true })

  for (const [id, colorScheme] of [
    ['V-1', 'light'],
    ['V-2', 'dark'],
  ] as const) {
    test(`${id} empty app, ${colorScheme}`, async ({ page }) => {
      await openInTheme(page, colorScheme)
      await expect(page.getByTestId('empty-state')).toBeVisible()
      await expect(page.getByTestId('timeline-empty')).toBeVisible()
      await parkPointer(page)
      await expectWindow(page, `${id}-empty-${colorScheme}.png`)
    })
  }

  for (const [id, colorScheme] of [
    ['V-3', 'light'],
    ['V-4', 'dark'],
  ] as const) {
    test(`${id} cube in the home view, ${colorScheme}`, async ({ page }) => {
      await openInTheme(page, colorScheme)
      await dropFile(page, 'cube.ptsm')
      await page.getByRole('button', { name: 'Home view', exact: true }).click()
      await expect(page.getByTestId('viewport')).toHaveAttribute('data-view', 'iso')
      await parkPointer(page)
      await expectWindow(page, `${id}-cube-home-${colorScheme}.png`)
    })
  }

  test('V-5 fully constrained sketch with dimensions, light', async ({ page }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketch(page, 'full', '0')
    await expect(page.getByTestId('sketch-dimension')).toHaveCount(2)
    await parkPointer(page)
    await expectWindow(page, 'V-5-sketch-full-light.png')
  })

  test('V-6 under-constrained sketch, dark', async ({ page }) => {
    await openInTheme(page, 'dark')
    await dropFile(page, 'underconstrained.ptsm')
    await editSketch(page, 'under', '7')
    await parkPointer(page)
    await expectWindow(page, 'V-6-sketch-under-dark.png')
  })

  test('V-7 overhang map and legend in Print, light', async ({ page }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'overhang.ptsm')
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    await expect(page.getByTestId('overhang-legend')).toBeVisible()
    // The overhang is the underside of the cap, so look up at it from below (3.14).
    await page
      .getByRole('group', { name: 'View cube' })
      .getByRole('button', { name: 'Front right bottom corner view', exact: true })
      .click()
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-view', 'custom')
    await parkPointer(page)
    await expectWindow(page, 'V-7-print-overhangs-light.png')
  })

  test('V-8 Extrude inspector in edit mode, light', async ({ page }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'cube.ptsm')
    await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await parkPointer(page)
    await expectWindow(page, 'V-8-extrude-edit-light.png')
  })

  test('V-9 front face selected and top edge hovered, light', async ({ page }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const cube = toScreen(await canvas(page), { min: [-10, -10, 0], max: [10, 10, 20] }, 'Front')
    const center = cube.toPage([0, -10, 10])
    await page.mouse.click(center.x, center.y)
    await expect(page.getByTestId('selection-title')).toHaveText('1 face')
    const topEdge = cube.toPage([0, -10, 20])
    await expectHover(page, topEdge.x, topEdge.y, 'edge')
    await expectWindow(page, 'V-9-face-selected-edge-hovered-light.png')
  })

  for (const [id, colorScheme] of [
    ['V-10', 'light'],
    ['V-11', 'dark'],
  ] as const) {
    test(`${id} Problems panel and the error banner of Fillet 1, ${colorScheme}`, async ({
      page,
    }) => {
      await openInTheme(page, colorScheme)
      await editFailedFillet(page)
      await parkPointer(page)
      await expectWindow(page, `${id}-fillet-error-${colorScheme}.png`)
    })
  }

  test('V-12 Extrude with its empty Profile field active shows the regions, light', async ({
    page,
  }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'rect-sketch.ptsm')
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    const profile = selectionField(page, 'profile')
    await expect(profile).toHaveAttribute('data-active', 'true')
    await expect(profile).toHaveAttribute('data-count', '0')
    await parkPointer(page)
    await expectWindow(page, 'V-12-extrude-profile-regions-light.png')
  })

  for (const [id, colorScheme] of [
    ['V-13', 'light'],
    ['V-14', 'dark'],
  ] as const) {
    test(`${id} both side islands docked and the Extrude inspector card in the free area, ${colorScheme}`, async ({
      page,
    }) => {
      await openInTheme(page, colorScheme)
      await dropFile(page, 'cube.ptsm')
      await runCommand(page, 'Parameters')
      await expect(page.getByTestId('side-panel')).toBeVisible()
      await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
      await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
      await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
      await parkPointer(page)
      await expectWindow(page, `${id}-islands-edit-${colorScheme}.png`)
    })
  }

  for (const [id, colorScheme] of [
    ['V-15', 'light'],
    ['V-16', 'dark'],
  ] as const) {
    test(`${id} origin axes and planes in the X, Y and Z colors while a plane is picked, ${colorScheme}`, async ({
      page,
    }) => {
      await openInTheme(page, colorScheme)
      await page.getByTestId('empty-state').getByRole('button', { name: 'Sketch' }).click()
      await expect(page.getByTestId('sketch-header')).toHaveAttribute('data-state', 'picking')
      await expect(page.getByTestId('plane-label')).toHaveCount(3)
      await expect(page.locator('[data-testid=axis-label]:not([hidden])')).toHaveCount(3)
      await parkPointer(page)
      await expectWindow(page, `${id}-origin-axes-planes-${colorScheme}.png`)
    })
  }

  test('V-17 a sketch on the Front plane with its axes in the X and Z colors, dark', async ({
    page,
  }) => {
    await openInTheme(page, 'dark')
    await dropFile(page, 'revolve-sketch.ptsm')
    // A sketch on the Front plane is edited looking at it from the front (3.16.1).
    await timelineItem(page, 'Sketch 1').getByRole('button', { name: 'Sketch 1' }).dblclick()
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-view', 'front')
    const header = page.getByTestId('sketch-header')
    await expect(header).toHaveAttribute('data-state', 'full')
    await expect(header).toHaveAttribute('data-dof', '0')
    await expect(page.getByTestId('sketch-overlay').getByTestId('axis-label')).toHaveCount(2)
    await parkPointer(page)
    await expectWindow(page, 'V-17-sketch-front-axes-dark.png')
  })

  test('V-18 Size and Center readouts with their axis letters, light', async ({ page }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'cube.ptsm')
    await bodyRow(page, 'Cube').click()
    await expect(page.getByTestId('body-center')).toHaveAttribute('data-value', '0, 0, 10 mm')
    await parkPointer(page)
    await expectWindow(page, 'V-18-readouts-light.png')
  })

  test('V-19 the X, Y and Z letters inside the Move body fields, dark', async ({ page }) => {
    await openInTheme(page, 'dark')
    await dropFile(page, 'cube.ptsm')
    await runCommand(page, 'Move body')
    await expect(inspector(page)).toHaveAccessibleName('Move body')
    await expect(inspector(page).getByTestId('axis-letter')).toHaveCount(3)
    await parkPointer(page)
    await expectWindow(page, 'V-19-move-fields-dark.png')
  })

  test('V-20 the islands, view cube, controls and slot at 200% UI scale, light', async ({
    page,
  }) => {
    await page.addInitScript(() => {
      localStorage.setItem('partsmith.settings.v1', JSON.stringify({ uiScale: 2 }))
    })
    await openInTheme(page, 'light')
    await expect(page.getByTestId('browser')).toBeHidden()
    await dropFile(page, 'cube.ptsm')
    await page.getByRole('button', { name: 'Home view', exact: true }).click()
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-view', 'iso')
    await parkPointer(page)
    await expectWindow(page, 'V-20-cube-home-200-light.png')
  })

  test('V-21 parameter names and expressions as code beside the Distance field, light', async ({
    page,
  }) => {
    await openInTheme(page, 'light')
    await dropFile(page, 'parametric.ptsm')
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('parameters-panel')).toBeVisible()
    await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await expect(inspector(page).getByRole('combobox', { name: 'Distance' })).toHaveValue('size')
    await parkPointer(page)
    await expectWindow(page, 'V-21-parameters-code-light.png')
  })

  test('V-22 the islands and the inspector card edged by the system in forced colors, light', async ({
    page,
  }) => {
    await page.emulateMedia({ forcedColors: 'active' })
    await openInTheme(page, 'light')
    await dropFile(page, 'cube.ptsm')
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('side-panel')).toBeVisible()
    await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await parkPointer(page)
    await expectWindow(page, 'V-22-islands-forced-colors-light.png')
  })

  for (const [id, colorScheme] of [
    ['V-23', 'light'],
    ['V-24', 'dark'],
  ] as const) {
    test(`${id} the Document menu over the browser, ${colorScheme}`, async ({ page }) => {
      await openInTheme(page, colorScheme)
      await dropFile(page, 'cube.ptsm')
      await page.getByRole('button', { name: 'Home view', exact: true }).click()
      await expect(page.getByTestId('viewport')).toHaveAttribute('data-view', 'iso')
      await page.getByTestId('document-name').click()
      await expect(page.getByTestId('document-menu')).toBeVisible()
      // The pointer rests on the status bar, so no menu item is hovered or shows a tooltip.
      await parkPointer(page)
      await expectWindow(page, `${id}-document-menu-${colorScheme}.png`)
    })
  }

  test('V-25 the browser opened over the inspector at 200% UI scale, dark', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('partsmith.settings.v1', JSON.stringify({ uiScale: 2 }))
    })
    await openInTheme(page, 'dark')
    await expect(page.getByTestId('browser')).toBeHidden()
    await dropFile(page, 'cube.ptsm')
    await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await page.getByRole('button', { name: 'Toggle browser', exact: true }).click()
    await expect(page.getByTestId('browser')).toBeVisible()
    await parkPointer(page)
    await expectWindow(page, 'V-25-browser-over-inspector-200-dark.png')
  })
})
