/// <reference lib="dom" />
import { expect, test, type Locator, type Page } from '@playwright/test'
import { dropFile, openApp } from './helpers'

// Layout (ui.md 6.5 E-105 to E-107): boxes measured in the real browser, because jsdom has no
// layout. Each test names the section whose layout it holds.

type Box = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** Sub-pixel rounding of rendered boxes. */
const TOLERANCE_PIXELS = 0.5

/** The UI scales of the settings (2.16.9) that these tests check. */
const SCALES = [1, 2] as const

/** Opens the app at a UI scale, stored in the settings before the first load (2.15). */
async function openAtScale(page: Page, uiScale: number): Promise<void> {
  await page.addInitScript((scale) => {
    localStorage.setItem('partsmith.settings.v1', JSON.stringify({ uiScale: scale }))
  }, uiScale)
  await openApp(page)
  // The UI scale sets the root font size as a percentage of the browser's 16 pixels (2.16.9).
  await expect
    .poll(() => page.evaluate(() => getComputedStyle(document.documentElement).fontSize))
    .toBe(`${String(16 * uiScale)}px`)
  await page.evaluate(() => document.fonts.ready)
}

async function boxOf(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox()
  if (box === null) {
    throw new Error(`${locator.toString()} is not visible.`)
  }
  return box
}

/** Expects `inner` to lie within `outer`, up to sub-pixel rounding. */
function expectInside(inner: Box, outer: Box, what: string): void {
  expect(inner.x, `${what}: left`).toBeGreaterThanOrEqual(outer.x - TOLERANCE_PIXELS)
  expect(inner.y, `${what}: top`).toBeGreaterThanOrEqual(outer.y - TOLERANCE_PIXELS)
  expect(inner.x + inner.width, `${what}: right`).toBeLessThanOrEqual(
    outer.x + outer.width + TOLERANCE_PIXELS,
  )
  expect(inner.y + inner.height, `${what}: bottom`).toBeLessThanOrEqual(
    outer.y + outer.height + TOLERANCE_PIXELS,
  )
}

/**
 * The page boxes of the view cube's 8 corner buttons as the camera turns it, its 3D transform
 * applied (3.14). Each holds one corner of the cube, and the cube's outline is the hull of its
 * corners, so the cube is inside a rectangle exactly when these boxes are.
 */
async function viewCubeCornerBoxes(page: Page): Promise<Box[]> {
  const corners = page
    .getByRole('group', { name: 'View cube' })
    .getByRole('button', { name: /corner view$/ })
  await expect(corners).toHaveCount(8)
  return Promise.all((await corners.all()).map((corner) => boxOf(corner)))
}

/** The segmented choices and selects of the inspector, each with its field's path. */
function inspectorChoices(page: Page): Locator {
  return page
    .getByTestId('inspector')
    .getByTestId('inspector-field')
    .filter({ has: page.locator('[role=radiogroup], select') })
}

/** Expects every choice of the inspector to lie within its field (3.7). */
async function expectChoicesInsideFields(page: Page): Promise<void> {
  const fields = inspectorChoices(page)
  await expect(fields).not.toHaveCount(0)
  await Promise.all(
    (await fields.all()).map(async (field) => {
      const path = (await field.getAttribute('data-path')) ?? ''
      const control = field.locator('[role=radiogroup], select').first()
      expectInside(await boxOf(control), await boxOf(field), `the ${path} choice`)
    }),
  )
}

test.describe('layout', () => {
  for (const scale of SCALES) {
    test(`E-105 the view cube keeps every corner inside the viewport in the iso view at ${String(scale * 100)}% UI scale (3.14)`, async ({
      page,
    }) => {
      await openAtScale(page, scale)
      const viewport = page.getByTestId('viewport')
      await expect(viewport).toHaveAttribute('data-view', 'iso')
      const bounds = await boxOf(viewport)
      for (const corner of await viewCubeCornerBoxes(page)) {
        expectInside(corner, bounds, 'a view cube corner')
      }
    })
  }

  test('E-106 the Extrude inspector clips no choice, and choosing an option keeps the control (3.7)', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await page.evaluate(() => document.fonts.ready)
    await page.keyboard.press('e')
    const inspector = page.getByTestId('inspector')
    await expect(inspector).toHaveAccessibleName('Extrude')
    await expectChoicesInsideFields(page)
    const operation = inspector.getByRole('radiogroup', { name: 'Operation', exact: true })
    // Each choice draws its label bold, so each is checked and measured in turn.
    for (const name of ['Intersect', 'Cut', 'Join', 'New body']) {
      const radio = operation.getByRole('radio', { name, exact: true })
      // oxlint-disable-next-line no-await-in-loop
      await radio.check()
      // oxlint-disable-next-line no-await-in-loop
      await expect(radio).toBeChecked()
      // oxlint-disable-next-line no-await-in-loop
      await expectChoicesInsideFields(page)
    }
  })

  test('E-107 at 200% UI scale the plane-picking header keeps its prompt and plane buttons inside the viewport (3.16.1)', async ({
    page,
  }) => {
    await openAtScale(page, 2)
    await page
      .getByTestId('empty-state')
      .getByRole('button', { name: 'Sketch', exact: true })
      .click()
    const header = page.getByTestId('sketch-header')
    await expect(header).toHaveAttribute('data-state', 'picking')
    const bounds = await boxOf(page.getByTestId('viewport'))
    expectInside(await boxOf(header), bounds, 'the sketch header')
    expectInside(
      await boxOf(header.getByText('Pick a plane or a flat face to draw on')),
      bounds,
      'the prompt',
    )
    await Promise.all(
      ['Top plane (XY)', 'Front plane (XZ)', 'Right plane (YZ)'].map(async (plane) => {
        const button = header.getByRole('button', { name: `Sketch on ${plane}`, exact: true })
        expectInside(await boxOf(button), bounds, `the ${plane} button`)
      }),
    )
  })
})
