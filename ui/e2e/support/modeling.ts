/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  canvas,
  inspector,
  inspectorField,
  inspectorInput,
  modelTool,
  timelineItem,
} from '../helpers'

// Helpers for the modeling end-to-end tests: the Model toolbar's groups, inspector fields and
// choices, picks on the canvas, confirming a command, previews, body sizes, editing from the
// timeline, an item's context menu, timeline labels and drags. The shared helpers of ui.md 6.3 are
// in ../helpers.ts.

/** Opens a toolbar group ("Shapes", "Pattern") and runs one of its commands. */
export async function runFromGroup(page: Page, group: string, title: string): Promise<void> {
  await modelTool(page, group).click()
  await page.getByRole('menuitem', { name: title }).first().click()
}

/** The selection box of the inspector field with a param path. */
export function selectionField(page: Page, path: string): Locator {
  return inspectorField(page, path).getByTestId('selection-field')
}

/** Replaces the text of an inspector input. */
export async function fillInspector(page: Page, label: string, text: string): Promise<void> {
  const input = inspectorInput(page, label)
  await input.fill(text)
  await expect(input).toHaveValue(text)
}

/** Chooses an option of a choice field (segmented control or select) labeled `label`. */
export async function chooseOption(page: Page, label: string, option: string): Promise<void> {
  const panel = inspector(page)
  const group = panel.getByRole('radiogroup', { name: label, exact: true })
  if ((await group.count()) > 0) {
    await group.getByRole('radio', { name: option, exact: true }).check()
    return
  }
  await panel.getByRole('combobox', { name: label, exact: true }).selectOption({ label: option })
}

/** Clicks the canvas at an offset from its center given in canvas heights (`H`). */
export async function clickCanvas(
  page: Page,
  dxInHeights: number,
  dyInHeights: number,
): Promise<void> {
  const box = await canvas(page)
  await page.mouse.click(box.cx + dxInHeights * box.H, box.cy + dyInHeights * box.H)
}

/**
 * Clicks the canvas at an offset in canvas heights while a command is active and waits until the
 * selection field holds `count` items and the preview has settled for the new params (`shown`
 * when they are complete, `none` otherwise), so the next click picks the current meshes.
 */
export async function pickOnCanvas(
  page: Page,
  dxInHeights: number,
  dyInHeights: number,
  path: string,
  count: number,
  preview: 'shown' | 'none',
): Promise<void> {
  await clickCanvas(page, dxInHeights, dyInHeights)
  await expect(selectionField(page, path)).toHaveAttribute('data-count', String(count))
  await expect(inspector(page)).toHaveAttribute('data-preview', preview, { timeout: 15_000 })
}

/** Gives the viewport keyboard focus, where Enter confirms a complete command (ui.md 3.5). */
export async function focusViewport(page: Page): Promise<void> {
  await page.getByRole('application', { name: '3D view' }).focus()
}

/**
 * Moves focus to the viewport unless it is already where Enter confirms a command: the viewport,
 * or an inspector control that is not a button (ui.md 3.5 Enter ladder).
 */
export async function focusWhereEnterConfirms(page: Page): Promise<void> {
  const ready = await page.evaluate(() => {
    const focused = document.activeElement
    if (!(focused instanceof HTMLElement)) {
      return false
    }
    if (focused.getAttribute('role') === 'application') {
      return true
    }
    return (
      focused.closest('[data-testid=inspector]') !== null &&
      focused.closest('button, [role=button]') === null
    )
  })
  if (!ready) {
    await focusViewport(page)
  }
}

/** Confirms the active command with Enter and expects the new feature to be ok. */
export async function confirmAndExpectOk(page: Page, label: string): Promise<void> {
  await page.keyboard.press('Enter')
  // The preview's item is in the timeline before OK, so the command must have ended first.
  await expect(page.locator('[data-testid=inspector][data-mode=command]')).toHaveCount(0)
  await expect(timelineItem(page, label)).toHaveAttribute('data-status', 'ok', { timeout: 15_000 })
}

/** Waits until the command's preview is drawn for its latest params. */
export async function expectPreviewShown(page: Page): Promise<void> {
  await expect(inspector(page)).toHaveAttribute('data-preview', 'shown', { timeout: 15_000 })
}

/** Selects a body by its browser row and expects its size (ui.md 3.8). */
export async function expectBodySize(page: Page, name: string, size: string): Promise<void> {
  await bodyRow(page, name).click()
  await expect(page.getByTestId('selection-title')).toHaveText(name)
  await expect(page.getByTestId('body-size')).toHaveAttribute('data-value', size)
}

/** Double-clicks a feature's timeline item, which opens its feature command (ui.md 3.9). */
export async function editFeatureFromTimeline(page: Page, label: string): Promise<void> {
  await timelineItem(page, label).getByRole('button', { name: label, exact: true }).dblclick()
  await expect(inspector(page)).toHaveAccessibleName(label)
  await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
}

/** The labels of the timeline items, in order. */
export function timelineLabels(page: Page): Promise<string[]> {
  return page
    .getByTestId('timeline-item')
    .evaluateAll((items) =>
      items.map((item) => item.querySelector('button')?.getAttribute('aria-label') ?? ''),
    )
}

/** The position of the gap before timeline item `index` (the end when it equals the count). */
export async function timelineGap(page: Page, index: number): Promise<{ x: number; y: number }> {
  const items = page.getByTestId('timeline-item')
  const count = await items.count()
  const box = async (position: number) => {
    const bounds = await items.nth(position).boundingBox()
    if (bounds === null) {
      throw new Error(`Timeline item ${String(position)} is not visible.`)
    }
    return bounds
  }
  if (index === 0) {
    const first = await box(0)
    return { x: first.x - 2, y: first.y + first.height / 2 }
  }
  const before = await box(index - 1)
  if (index === count) {
    return { x: before.x + before.width + 2, y: before.y + before.height / 2 }
  }
  const after = await box(index)
  return { x: (before.x + before.width + after.x) / 2, y: before.y + before.height / 2 }
}

/** Opens the context menu of a feature's timeline item with a right click (ui.md 3.9) and returns it. */
export async function openTimelineMenu(page: Page, label: string): Promise<Locator> {
  await timelineItem(page, label)
    .getByRole('button', { name: label, exact: true })
    .click({ button: 'right' })
  return page.getByTestId('context-menu')
}

/** Presses on an element's center and moves to a page position in small steps, still pressed. */
export async function holdOver(
  page: Page,
  from: Locator,
  to: { x: number; y: number },
): Promise<void> {
  const bounds = await from.boundingBox()
  if (bounds === null) {
    throw new Error('The dragged element is not visible.')
  }
  await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
  await page.mouse.down()
  await page.mouse.move(to.x, to.y, { steps: 12 })
}

/** Drags from an element's center to a page position in small steps. */
export async function dragTo(
  page: Page,
  from: Locator,
  to: { x: number; y: number },
): Promise<void> {
  await holdOver(page, from, to)
  await page.mouse.up()
}
