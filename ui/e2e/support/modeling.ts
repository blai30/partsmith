/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import { inspector, inspectorField, inspectorInput } from '../helpers'

// Helpers for the modeling end-to-end tests: the Model toolbar and its groups, inspector fields
// and choices, previews, timeline labels and browser rows by kind. The shared helpers of ui.md
// 6.3 are in ../helpers.ts.

/** Opens a toolbar group ("Shapes", "Pattern") and runs one of its commands. */
export async function runFromGroup(page: Page, group: string, title: string): Promise<void> {
  await page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: group, exact: true })
    .click()
  await page.getByRole('menuitem', { name: title }).first().click()
}

/** Clicks a toolbar button of the Model context. */
export async function clickTool(page: Page, title: string): Promise<void> {
  await page
    .getByRole('toolbar', { name: 'Model tools' })
    .getByRole('button', { name: title, exact: true })
    .click()
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

/** The labels of the timeline items, in order. */
export function timelineLabels(page: Page): Promise<string[]> {
  return page
    .getByTestId('timeline-item')
    .evaluateAll((items) =>
      items.map((item) => item.querySelector('button')?.getAttribute('aria-label') ?? ''),
    )
}

/** The browser row of a body, sketch or other item, by its kind and the exact text of its name. */
export function browserRow(page: Page, kind: string, name: string): Locator {
  return page
    .getByTestId('browser')
    .locator(`[data-testid=browser-row][data-kind=${kind}]`)
    .filter({ has: page.getByText(name, { exact: true }) })
}
