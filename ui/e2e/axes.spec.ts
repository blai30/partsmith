/// <reference lib="dom" />
import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  boxOf,
  canvas,
  dropFile,
  fitView,
  inspector,
  inspectorField,
  inspectorInput,
  openApp,
  runCommand,
  timelineItem,
  type Box,
} from './helpers'
import { fillInspector, runFromGroup } from './support/modeling'

// X, Y and Z in red, green and blue (ui.md 6.5 E-139 and E-140): the axis letters of coordinate
// fields and readouts, measured in the real browser.

const AXES = ['X', 'Y', 'Z'] as const

/** Sub-pixel rounding of rendered boxes. */
const TOLERANCE_PIXELS = 0.5

/** How far inside its input's left edge a field's axis letter may sit. */
const LETTER_REACH_PIXELS = 8

/** The axis letter in the inspector field with a param path. */
function fieldLetter(page: Page, path: string): Locator {
  return inspectorField(page, path).getByTestId('axis-letter')
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

/** The letter of an axis in a readout. */
function readoutLetter(readout: Locator, axis: string): Locator {
  return readout.locator(`[data-testid=axis-letter][data-axis=${axis}]`)
}

/** Expects the X, Y and Z letters of two readouts to share their left edges (3.8). */
async function expectLettersAligned(first: Locator, second: Locator): Promise<void> {
  await Promise.all(
    AXES.map(async (axis) => {
      const [above, below] = await Promise.all([
        boxOf(readoutLetter(first, axis)),
        boxOf(readoutLetter(second, axis)),
      ])
      expect(Math.abs(above.x - below.x), `the ${axis} letters' left edges`).toBeLessThanOrEqual(
        TOLERANCE_PIXELS,
      )
    }),
  )
}

/**
 * Expects the Move field of an axis to hold its letter, a world axis, inside the input's start,
 * and the input to leave room for it (3.7).
 */
async function expectMoveLetter(page: Page, axis: string, index: number): Promise<void> {
  const letter = fieldLetter(page, `translation[${String(index)}]`)
  await expect(letter).toHaveText(axis)
  await expect(letter).toHaveAttribute('data-world', 'true')
  const input = inspectorInput(page, `Move ${axis}`)
  const [letterBox, inputBox] = await Promise.all([boxOf(letter), boxOf(input)])
  expectInside(letterBox, inputBox, `the ${axis} letter`)
  expect(letterBox.x - inputBox.x, `the ${axis} letter's offset`).toBeLessThanOrEqual(
    LETTER_REACH_PIXELS,
  )
  expect(await input.evaluate((element) => getComputedStyle(element).paddingLeft)).toBe('24px')
}

test.describe('axis letters and readouts', () => {
  test('E-139 coordinate fields show their axis letter inside the input', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await runCommand(page, 'Move body')
    await expect(inspector(page)).toHaveAccessibleName('Move body')
    await Promise.all(AXES.map((axis, index) => expectMoveLetter(page, axis, index)))

    await inspector(page).getByRole('button', { name: 'Cancel', exact: true }).last().click()
    await expect(inspector(page)).toBeHidden()
    await runFromGroup(page, 'Shapes', 'Box')
    await expect(inspector(page)).toHaveAccessibleName('Box')
    await inspector(page).getByTestId('inspector-more').click()
    const positions = ['position[0]', 'position[1]', 'position[2]']
    await Promise.all(
      positions.map((path) =>
        expect(fieldLetter(page, path)).toHaveAttribute('data-world', 'true'),
      ),
    )
    await inspector(page).getByRole('button', { name: 'Choose Plane', exact: true }).click()
    await page.getByRole('menuitem', { name: 'Front plane (XZ)' }).click()
    await Promise.all(
      positions.map((path) =>
        expect(fieldLetter(page, path)).toHaveAttribute('data-world', 'false'),
      ),
    )
    await Promise.all(
      AXES.map((axis) => expect(inspectorInput(page, `Position ${axis}`)).toBeVisible()),
    )
  })

  test('E-140 coordinate readouts read each value after its axis letter', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await bodyRow(page, 'Cube').click()
    const size = page.getByTestId('body-size')
    const center = page.getByTestId('body-center')
    await expect(size).toHaveText('X 20 Y 20 Z 20 mm')
    await expect(size).toHaveAttribute('data-value', '20 × 20 × 20 mm')
    await expect(center).toHaveText('X 0 Y 0 Z 10 mm')
    await expect(center).toHaveAttribute('data-value', '0, 0, 10 mm')
    await expectLettersAligned(size, center)

    // The selection inspector closes, so the fit frames the whole free area (2.11).
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('inspector')).toBeHidden()
    await fitView(page, 'Front')
    const box = await canvas(page)
    await page.mouse.click(box.cx + 0.4 * box.H, box.cy - 0.4 * box.H)
    const position = page.getByTestId('vertex-position')
    await expect(position).toHaveText('X 10 Y -10 Z 20 mm')
    await expect(position).toHaveAttribute('data-value', '10, -10, 20 mm')

    await bodyRow(page, 'Cube').click()
    await runCommand(page, 'Move body')
    await expect(inspector(page)).toHaveAccessibleName('Move body')
    for (const axis of AXES) {
      // Typing goes to one field at a time.
      // oxlint-disable-next-line no-await-in-loop
      await fillInspector(page, `Move ${axis}`, '-1234.56')
    }
    await inspector(page).getByTestId('inspector-ok').click()
    await expect(timelineItem(page, 'Move body 1')).toHaveAttribute('data-status', 'ok')
    await bodyRow(page, 'Cube').click()
    await expect(center).toHaveText('X -1234.56 Y -1234.56 Z -1224.56 mm')
    expectInside(await boxOf(center), await boxOf(inspector(page)), 'the center readout')
  })
})
