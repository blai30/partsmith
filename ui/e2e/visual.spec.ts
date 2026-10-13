/// <reference lib="dom" />
import { fileURLToPath } from 'node:url'
import { expect, test, type Page } from '@playwright/test'
import {
  canvas,
  dropFile,
  expectHover,
  fitView,
  inspector,
  openApp,
  timelineItem,
  toScreen,
} from './helpers'
import { selectionField } from './support/modeling'
import { chooseContext, expectCurrentReport } from './support/print'

// Visual tests (ui.md 6.6): screenshots of the whole window in both themes, compared with the
// baselines in visual.spec.ts-snapshots/. The configuration sets `animations: 'disabled'`,
// `maxDiffPixelRatio: 0.002` and `threshold: 0.05` for every `toHaveScreenshot`.

type ColorScheme = 'light' | 'dark'

/**
 * The web fonts every screenshot must show (2.16.7). `document.fonts.check` alone also passes when
 * the Google Fonts stylesheet never loaded (a family without faces counts as available), and a
 * family no element has used yet is not loaded, so each family is loaded explicitly and must
 * resolve with at least one of its web font faces.
 */
const FONTS = ['14px Inter', '14px "JetBrains Mono"'] as const

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
 * Moves the pointer onto the status bar's empty middle, where nothing hovers or shows a tooltip,
 * and waits until no tooltip is open.
 */
async function parkPointer(page: Page): Promise<void> {
  const bar = await page.getByTestId('status-bar').boundingBox()
  if (bar === null) {
    throw new Error('The status bar is not visible.')
  }
  await page.mouse.move(bar.x + bar.width / 2, bar.y + bar.height / 2)
  await expect(page.getByRole('tooltip')).toHaveCount(0)
}

/** Compares the window with a baseline once the fonts are in. */
async function expectWindow(page: Page, name: string): Promise<void> {
  await expectWebFonts(page)
  await expect(page).toHaveScreenshot(name, { stylePath: SCREENSHOT_STYLE })
}

/** Edits a sketch by double-clicking its timeline item and waits for its solved state. */
async function editSketch(page: Page, state: string, dof: string): Promise<void> {
  await timelineItem(page, 'Sketch 1').getByRole('button', { name: 'Sketch 1' }).dblclick()
  await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
  const header = page.getByTestId('sketch-header')
  await expect(header).toHaveAttribute('data-state', state)
  await expect(header).toHaveAttribute('data-dof', dof)
  await expect(page.getByTestId('viewport')).toHaveAttribute('data-view', 'top')
}

/** Opens the Problems panel of `fillet-error.ptsm` and edits "Fillet 1" from it (E-28). */
async function editFailedFillet(page: Page): Promise<void> {
  await dropFile(page, 'fillet-error.ptsm')
  await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'error')
  await page.getByTestId('timeline-problems').click()
  const panel = page.getByTestId('problems-panel')
  await expect(panel).toBeVisible()
  await panel.getByRole('button', { name: 'Edit', exact: true }).click()
  await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
  await expect(inspector(page)).toHaveAttribute('data-preview', 'failed')
  await expect(inspector(page).getByTestId('inspector-error')).toHaveAttribute('role', 'alert')
}

test.describe('visual @visual', () => {
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
})
