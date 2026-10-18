/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  inspector,
  inspectorField,
  inspectorInput,
  modelTool,
  timelineItem,
} from '../helpers'

// Helpers for the modeling end-to-end tests: the Model toolbar's groups, inspector fields and
// choices, previews, body sizes, editing from the timeline and timeline labels. The shared helpers
// of ui.md 6.3 are in ../helpers.ts.

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

/** Drags from an element's center to a page position in small steps. */
export async function dragTo(
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
  await page.mouse.up()
}
