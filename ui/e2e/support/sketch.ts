/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  hoverKindAt,
  originRow,
  timelineItem,
  viewport,
  type CanvasBox,
  type PagePoint,
} from '../helpers'

// Helpers for the sketch mode end-to-end tests (ui.md 6.5 E-12 to E-22 and the sketch tests of
// 3.16): pointer gestures on the canvas, starting and editing sketches, and locators for the
// sketch chrome. The shared helpers of 6.3, `sketchToScreen` among them, are in ../helpers.ts.

/** The page position at an offset in CSS pixels from the canvas center. */
export function offset(box: CanvasBox, dx: number, dy: number): PagePoint {
  return { x: box.cx + dx, y: box.cy + dy }
}

/** Moves the mouse to a page position and polls until the hover kind is not `kind`. */
export async function expectNoHover(page: Page, point: PagePoint, kind: string): Promise<void> {
  await expect.poll(() => hoverKindAt(page, point.x, point.y)).not.toBe(kind)
}

/** Clicks a page position. */
export async function clickAt(page: Page, point: PagePoint): Promise<void> {
  await page.mouse.click(point.x, point.y)
}

/** Presses the primary button at `from`, moves to `to` in `steps` pointer moves and releases. */
export async function dragBetween(
  page: Page,
  from: PagePoint,
  to: PagePoint,
  steps = 10,
): Promise<void> {
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  await page.mouse.move(to.x, to.y, { steps })
  await page.mouse.up()
}

/** The app root, which carries `data-context`. */
export function app(page: Page): Locator {
  return page.getByTestId('app')
}

/** The sketch header chip (ui.md 3.16.8). */
export function sketchHeader(page: Page): Locator {
  return page.getByTestId('sketch-header')
}

/** The sketch hint under the header (ui.md 3.16.3). */
export function sketchHint(page: Page): Locator {
  return page.getByTestId('sketch-hint')
}

/** Expects the sketch header and the status bar to show `dof` degrees of freedom. */
export async function expectDof(page: Page, dof: number): Promise<void> {
  await expect(sketchHeader(page)).toHaveAttribute('data-dof', String(dof))
  await expect(page.getByTestId('status-dof')).toHaveAttribute('data-dof', String(dof))
}

/** Clicks a button of the Model tools toolbar. */
export async function clickModelTool(page: Page, title: string): Promise<void> {
  await page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: title, exact: true })
    .click()
}

/**
 * Starts a new sketch on the top plane as E-12 does: Sketch in the toolbar, then the browser row
 * "Top plane (XY)". Waits for the empty sketch, seen from the top with the origin at the canvas
 * center (an empty sketch keeps the visible height and targets its origin, 3.16.1).
 */
export async function newSketchOnTopPlane(page: Page): Promise<void> {
  await clickModelTool(page, 'Sketch')
  await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')
  await (await originRow(page, 'Top plane (XY)')).click()
  await expect(timelineItem(page, 'Sketch 1')).toBeVisible()
  await expect(viewport(page)).toHaveAttribute('data-view', 'top')
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'empty')
}

/** Waits until a sketch is being edited, seen from the top of its plane. */
async function expectEditing(page: Page): Promise<void> {
  await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  await expect(viewport(page)).toHaveAttribute('data-view', 'top')
  await expect(sketchHeader(page)).toHaveAttribute('data-dof', /^\d+$/)
}

/** Edits a sketch by double-clicking its timeline item (ui.md 3.9). */
export async function editSketchFromTimeline(page: Page, label: string): Promise<void> {
  await timelineItem(page, label).getByRole('button', { name: label, exact: true }).dblclick()
  await expectEditing(page)
}

/** The browser row of a sketch by its label. */
export function sketchRow(page: Page, label: string): Locator {
  return page
    .getByTestId('browser')
    .locator('[data-testid=browser-row][data-kind=sketch]')
    .filter({ has: page.getByText(label, { exact: true }) })
}

/**
 * Edits a sketch by double-clicking its browser row (ui.md 3.10), for the tests that say
 * "double-click 'Sketch 1'" or "enter the sketch" without naming the timeline.
 */
export async function editSketchFromBrowser(page: Page, label: string): Promise<void> {
  await sketchRow(page, label).getByText(label, { exact: true }).dblclick()
  await expectEditing(page)
}

/** The inference glyphs shown while drawing (ui.md 3.16.5). */
export function inferenceGlyphs(page: Page): Locator {
  return page.getByTestId('inference-glyph')
}

/** The constraint glyphs of one constraint type. */
export function constraintGlyphs(page: Page, type: string): Locator {
  return page.locator(`[data-testid=constraint-glyph][data-type=${type}]`)
}

/** The dimension labels on the canvas. */
export function sketchDimensions(page: Page): Locator {
  return page.getByTestId('sketch-dimension')
}

/**
 * The dimension labels that show exactly `text` ("40", "fx 20", "(40)", "⌀12"). The label's
 * accessible name puts the dimension's kind before that text, in a visually hidden span.
 */
export function dimensionLabel(page: Page, text: string): Locator {
  return sketchDimensions(page).filter({ has: page.getByText(text, { exact: true }) })
}
