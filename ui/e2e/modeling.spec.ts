/// <reference lib="dom" />
import { expect, test, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  browserRow,
  canvas,
  clickTool,
  download,
  dropFile,
  expandOrigin,
  expectHover,
  fitView,
  inspector,
  inspectorField,
  inspectorInput,
  notification,
  openApp,
  originRow,
  runCommand,
  timelineItem,
} from './helpers'
import {
  chooseOption,
  expectPreviewShown,
  fillInspector,
  runFromGroup,
  selectionField,
  timelineLabels,
} from './support/modeling'

// Feature commands, the inspector, previews, the timeline, the browser, the palette and the
// parameters panel against the production build and the real core (ui.md 3.3 to 3.11, 6.5).

/** Clicks the canvas at an offset from its center given in canvas heights (`H`). */
async function clickCanvas(page: Page, dxInHeights: number, dyInHeights: number): Promise<void> {
  const box = await canvas(page)
  await page.mouse.click(box.cx + dxInHeights * box.H, box.cy + dyInHeights * box.H)
}

/** Selects a body by its browser row and expects its size. */
async function expectBodySize(page: Page, name: string, size: string): Promise<void> {
  await bodyRow(page, name).click()
  await expect(page.getByTestId('selection-title')).toHaveText(name)
  await expect(page.getByTestId('body-size')).toHaveText(size)
}

/** Expects the timeline to hold items with these labels, in order. */
async function expectTimelineLabels(page: Page, labels: readonly string[]): Promise<void> {
  await expect.poll(() => timelineLabels(page)).toEqual(labels)
}

/** Picks a browser row into an inspector selection field unless the field already shows it. */
async function pickRowInto(page: Page, path: string, row: Locator, name: string): Promise<void> {
  const field = selectionField(page, path)
  if ((await field.innerText()).trim() === name) {
    return
  }
  await field.click()
  await expect(field).toHaveAttribute('data-active', 'true')
  await row.click()
  await expect(field).toHaveText(name)
}

/**
 * Moves focus out of a Parameters panel text field by clicking the panel title, so Mod+Z reaches
 * the document instead of the field's own text undo (ui.md 2.10 dispatch rule 2).
 */
async function leaveParametersField(page: Page): Promise<void> {
  await page
    .getByTestId('parameters-panel')
    .getByRole('heading', { name: 'Parameters', exact: true })
    .click()
  await expect
    .poll(() => page.evaluate(() => document.activeElement instanceof HTMLInputElement))
    .toBe(false)
}

/** A menu item whose accessible name starts with `name` (the shortcut follows it). */
function menuItem(menu: Locator, name: string): Locator {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return menu.getByRole('menuitem', { name: new RegExp(`^${escaped}(\\s|$)`) })
}

/** Gives the viewport keyboard focus, where Enter confirms a complete command (ui.md 3.5). */
async function focusViewport(page: Page): Promise<void> {
  await page.getByRole('application', { name: '3D view' }).focus()
}

/** Confirms the active command with Enter and expects the new feature to be ok. */
async function confirmAndExpectOk(page: Page, label: string): Promise<void> {
  await page.keyboard.press('Enter')
  // The preview's item is in the timeline before OK, so the command must have ended first.
  await expect(page.locator('[data-testid=inspector][data-mode=command]')).toHaveCount(0)
  await expect(timelineItem(page, label)).toHaveAttribute('data-status', 'ok', { timeout: 15_000 })
}

/**
 * Moves focus to the viewport unless it is already where Enter confirms a command: the viewport,
 * or an inspector control that is not a button (ui.md 3.5 Enter ladder).
 */
async function focusWhereEnterConfirms(page: Page): Promise<void> {
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

/**
 * Clicks the canvas at an offset in canvas heights while a command is active and waits until the
 * selection field holds `count` items and the preview has settled for the new params (`shown`
 * when they are complete, `none` otherwise), so the next click picks the current meshes.
 */
async function pickOnCanvas(
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

/** Unchecks one entry of the viewport's Selection filter menu and closes the menu. */
async function uncheckFilter(page: Page, name: string): Promise<void> {
  const button = page.getByRole('button', { name: 'Selection filter', exact: true })
  if ((await button.getAttribute('aria-expanded')) !== 'true') {
    await button.click()
  }
  const entry = page.getByRole('menuitemcheckbox', { name, exact: true })
  await expect(entry).toHaveAttribute('aria-checked', 'true')
  await entry.click()
  if ((await button.getAttribute('aria-expanded')) === 'true') {
    await page.keyboard.press('Escape')
  }
  await expect(button).toHaveAttribute('aria-expanded', 'false')
}

/** Expects `count` timeline items, every one with `attribute` equal to `value`. */
async function expectEveryItem(
  page: Page,
  count: number,
  attribute: string,
  value: string,
): Promise<void> {
  await expect(page.getByTestId('timeline-item')).toHaveCount(count)
  await expect(
    page.locator(`[data-testid=timeline-item]:not([${attribute}="${value}"])`),
  ).toHaveCount(0)
}

/** The position of the gap before timeline item `index` (the end when it equals the count). */
async function timelineGap(page: Page, index: number): Promise<{ x: number; y: number }> {
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
async function dragTo(page: Page, from: Locator, to: { x: number; y: number }): Promise<void> {
  const bounds = await from.boundingBox()
  if (bounds === null) {
    throw new Error('The dragged element is not visible.')
  }
  await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
  await page.mouse.down()
  await page.mouse.move(to.x, to.y, { steps: 12 })
  await page.mouse.up()
}

test.describe('palette and shortcuts', () => {
  test('E-10 the palette runs commands, finds other apps’ names and selects bodies', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await page.keyboard.press('ControlOrMeta+K')
    await expect(page.getByTestId('palette')).toBeVisible()
    await expect(page.getByTestId('palette-input')).toBeFocused()
    await page.keyboard.type('ext')
    const first = page.getByTestId('palette-option').first()
    await expect(first).toContainText('Extrude')
    await expect(first.getByTestId('palette-shortcut')).toHaveText('E')
    await page.keyboard.press('Enter')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    await page.keyboard.press('Escape')
    await expect(inspector(page)).toBeHidden()

    await page.keyboard.press('ControlOrMeta+K')
    await page.keyboard.type('pad')
    await expect(
      page
        .getByTestId('palette-option')
        .filter({ hasText: 'Extrude' })
        .filter({ hasText: 'FreeCAD: Pad' }),
    ).toHaveCount(1)
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('palette')).toBeHidden()

    await dropFile(page, 'cube.ptsm')
    await page.keyboard.press('ControlOrMeta+K')
    await page.keyboard.type('Cube')
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-selection-count', '1')
    await expect(page.getByTestId('selection-title')).toHaveText('Cube')
  })

  test('E-11 letters typed into a parameter draft stay text; on the canvas they are shortcuts', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await runCommand(page, 'Parameters')
    const panel = page.getByTestId('parameters-panel')
    await panel.getByRole('button', { name: 'Add parameter', exact: true }).click()
    await page.keyboard.type('e')
    await expect
      .poll(() =>
        page.evaluate(() => {
          const focused = document.activeElement
          return focused instanceof HTMLInputElement &&
            focused.closest('[data-testid=parameters-panel]') !== null
            ? focused.value
            : null
        }),
      )
      .toBe('e')
    await expect(inspector(page)).toHaveCount(0)
    await page.keyboard.press('Escape')
    const { cx, cy } = await canvas(page)
    await page.mouse.click(cx, cy)
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
  })
})

test.describe('inspector, timeline and browser', () => {
  test('E-23 a parameter change rebuilds the model and undo restores it', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'parametric.ptsm')
    await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')
    await runCommand(page, 'Parameters')
    const expression = page.getByRole('combobox', { name: 'Expression of size', exact: true })
    await expression.fill('30 mm')
    await expression.press('Enter')
    await expect(page.getByTestId('parameter-value')).toHaveText('30 mm')
    await expect(page.getByTestId('body-size')).toHaveText('30 × 30 × 30 mm')
    await leaveParametersField(page)
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(page.getByTestId('body-size')).toHaveText('20 × 20 × 20 mm')
  })

  test('E-24 dragging the rollback marker rolls features back and undo restores them', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube-features.ptsm')
    const marker = page.getByTestId('timeline-marker')
    await expect(marker).toHaveAttribute('aria-valuenow', '4')
    await dragTo(page, marker, await timelineGap(page, 1))
    const items = page.getByTestId('timeline-item')
    await expect(items.nth(0)).toHaveAttribute('data-rolled-back', 'false')
    await expect(items.nth(1)).toHaveAttribute('data-rolled-back', 'true')
    await expect(items.nth(2)).toHaveAttribute('data-rolled-back', 'true')
    await expect(items.nth(3)).toHaveAttribute('data-rolled-back', 'true')
    await expect(marker).toHaveAttribute('aria-valuenow', '1')
    await expect(page.locator('[data-testid=browser-row][data-kind=body]')).toHaveText(
      'Cube (empty)',
      { useInnerText: true },
    )
    const { cx, cy } = await canvas(page)
    await expect
      .poll(async () => {
        await page.mouse.move(cx + 1, cy)
        await page.mouse.move(cx, cy)
        return page.getByTestId('viewport').getAttribute('data-hover-kind')
      })
      .not.toBe('face')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(marker).toHaveAttribute('aria-valuenow', '4')
    await expect(page.locator('[data-testid=timeline-item][data-rolled-back=true]')).toHaveCount(0)
  })

  test('E-25 the rollback marker moves with the arrow, Home and End keys', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube-features.ptsm')
    const marker = page.getByTestId('timeline-marker')
    await marker.focus()
    await page.keyboard.press('ArrowLeft')
    await expect(marker).toHaveAttribute('aria-valuenow', '3')
    await page.keyboard.press('End')
    await expect(marker).toHaveAttribute('aria-valuenow', '4')
    await expect(marker).toHaveAttribute('aria-valuetext', 'At the end')
    await page.keyboard.press('Home')
    await expect(marker).toHaveAttribute('aria-valuenow', '0')
    await expect(marker).toHaveAttribute('aria-valuetext', 'At the start')
    await expectEveryItem(page, 4, 'data-rolled-back', 'true')
  })

  test('E-26 reordering the timeline, a failure after a reorder and its Undo', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'two-bodies.ptsm')
    await expectTimelineLabels(page, ['Sketch 1', 'Extrude 1', 'Cylinder 1'])
    await dragTo(page, timelineItem(page, 'Cylinder 1'), await timelineGap(page, 0))
    await expectTimelineLabels(page, ['Cylinder 1', 'Sketch 1', 'Extrude 1'])
    await expectEveryItem(page, 3, 'data-status', 'ok')
    await dragTo(page, timelineItem(page, 'Extrude 1'), await timelineGap(page, 1))
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'error')
    const failed = page
      .locator('[data-testid=notification][data-severity=error]')
      .filter({ hasText: /^Extrude 1 failed\./ })
    await expect(failed).toBeVisible()
    await failed.getByRole('button', { name: 'Undo', exact: true }).click()
    await expectTimelineLabels(page, ['Cylinder 1', 'Sketch 1', 'Extrude 1'])
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok')
  })

  test('E-27 suppressing a feature from its context menu, and undo', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube-features.ptsm')
    const fillet = timelineItem(page, 'Fillet 1')
    await fillet.getByRole('button', { name: 'Fillet 1', exact: true }).click({ button: 'right' })
    await page
      .getByTestId('context-menu')
      .getByRole('menuitem', { name: 'Suppress', exact: true })
      .click()
    await expect(fillet).toHaveAttribute('data-suppressed', 'true')
    await expect(fillet).toHaveAttribute('data-status', 'suppressed')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(fillet).toHaveAttribute('data-suppressed', 'false')
  })

  test('E-28 a failing feature is listed in Problems and opens with its error banner', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'fillet-error.ptsm')
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'error')
    const problems = page.getByTestId('timeline-problems')
    await expect(problems).toHaveText('1 problem')
    await problems.click()
    const panel = page.getByTestId('problems-panel')
    await expect(panel).toBeVisible()
    await expect(panel).toContainText('Fillet 1')
    await expect(panel).toContainText('Fillet radius')
    await panel.getByRole('button', { name: 'Edit', exact: true }).click()
    await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
    await expect(inspector(page).getByTestId('inspector-error')).toHaveAttribute('role', 'alert')
  })

  test('E-29 editing a feature previews, Esc discards, Enter commits and undo reverts', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    const extrude = timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' })
    await extrude.dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await expect(inspectorInput(page, 'Distance')).toHaveValue('20 mm')
    await fillInspector(page, 'Distance', '30')
    await expectPreviewShown(page)
    await page.keyboard.press('Escape')
    // The double-click's clicks selected Extrude 1, so the selection inspector may take the place
    // of the command's (ui.md 3.8, 3.9).
    await expect(inspector(page)).not.toHaveAttribute('data-mode', 'command')
    await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')

    await extrude.dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await fillInspector(page, 'Distance', '30')
    await page.keyboard.press('Enter')
    await expect(inspector(page)).not.toHaveAttribute('data-mode', 'command')
    await expectBodySize(page, 'Cube', '20 × 20 × 30 mm')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(page.getByTestId('body-size')).toHaveText('20 × 20 × 20 mm')
  })

  test('E-30 essential fields, More options and its remembered state', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    const paths = () =>
      inspector(page)
        .getByTestId('inspector-field')
        .evaluateAll((fields) =>
          fields
            .filter((field) => field instanceof HTMLElement && field.offsetParent !== null)
            .map((field) => field.getAttribute('data-path')),
        )
    await expect.poll(paths).toEqual(['profile', 'extent.kind', 'extent.expr', 'reverse', 'op'])
    const more = inspector(page).getByTestId('inspector-more')
    await expect(more).toHaveAttribute('aria-expanded', 'false')
    await more.click()
    await expect(inspectorField(page, 'body')).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(inspector(page)).toBeHidden()
    await page.keyboard.press('e')
    await expect(inspector(page).getByTestId('inspector-more')).toHaveAttribute(
      'aria-expanded',
      'true',
    )
  })

  test('E-31 a Fillet command picks an edge, toggles it off and on, and commits', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await page.keyboard.press('f')
    const edges = selectionField(page, 'edges')
    await expect(edges).toHaveAttribute('data-active', 'true')
    await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
    await pickOnCanvas(page, 0, -0.4, 'edges', 0, 'none')
    await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
    await fillInspector(page, 'Radius', '2')
    await confirmAndExpectOk(page, 'Fillet 1')
  })

  test('E-32 a selected edge is preselected by the Fillet command', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await clickCanvas(page, 0, -0.4)
    await expect(page.getByTestId('selection-title')).toHaveText('1 edge')
    await page.keyboard.press('f')
    await expect(selectionField(page, 'edges')).toHaveAttribute('data-count', '1')
  })

  test('E-33 hover picks faces, edges and vertices, and the selection filter limits them', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy, H } = await canvas(page)
    await expectHover(page, cx, cy, 'face')
    await expectHover(page, cx, cy - 0.4 * H, 'edge')
    await expectHover(page, cx + 0.4 * H, cy - 0.4 * H, 'vertex')
    await uncheckFilter(page, 'Edges')
    await uncheckFilter(page, 'Vertices')
    await expectHover(page, cx, cy - 0.4 * H + 2, 'face')
    await expectHover(page, cx + 0.4 * H - 2, cy - 0.4 * H + 2, 'face')
  })

  test('E-34 click, Shift+click and a click on empty space set the selection', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await clickCanvas(page, 0, 0)
    await expect(page.getByTestId('selection-title')).toHaveText('1 face')
    const box = await canvas(page)
    await page.keyboard.down('Shift')
    await page.mouse.click(box.cx, box.cy - 0.4 * box.H)
    await page.keyboard.up('Shift')
    await expect(page.getByTestId('selection-title')).toHaveText('2 items')
    await clickCanvas(page, 0, 0.45)
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-selection-count', '0')
    await expect(inspector(page)).toHaveCount(0)
  })

  test('E-35 double-clicking a face edits the feature that made it', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    await page.mouse.dblclick(cx, cy)
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
  })

  test('E-36 hidden and locked bodies are not picked', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy } = await canvas(page)
    const browser = page.getByTestId('browser')
    await browser.getByRole('button', { name: 'Hide Cube', exact: true }).click()
    await expectHover(page, cx, cy, 'none')
    await browser.getByRole('button', { name: 'Show Cube', exact: true }).click()
    await expectHover(page, cx, cy, 'face')
    await browser.getByRole('button', { name: 'Lock Cube', exact: true }).click()
    await expectHover(page, cx, cy, 'none')
    await browser.getByRole('button', { name: 'Unlock Cube', exact: true }).click()
    await expectHover(page, cx, cy, 'face')
  })

  test('E-37 a body color is saved in the document', async ({ page }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'cube.ptsm')
    await page.getByRole('button', { name: 'Color of Cube', exact: true }).first().click()
    await page
      .getByTestId('color-popover')
      .getByRole('button', { name: 'Red', exact: true })
      .click()
    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    expect(saved.bytes.toString('utf8')).toContain('"color": "#e7000b"')
  })

  test('E-38 the timeline is operated with the keyboard', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    const focusedItem = () =>
      page.evaluate(() => {
        const item = document.activeElement?.closest('[data-testid=timeline-item]')
        return item?.querySelector('button')?.getAttribute('aria-label') ?? null
      })
    let presses = 0
    // Each Tab starts from where the previous one left focus, so the presses are sequential.
    // oxlint-disable-next-line no-await-in-loop
    while ((await focusedItem()) === null && presses < 30) {
      // oxlint-disable-next-line no-await-in-loop
      await page.keyboard.press('Tab')
      presses += 1
    }
    expect(await focusedItem()).toBe('Sketch 1')
    await page.keyboard.press('ArrowRight')
    await expect.poll(focusedItem).toBe('Extrude 1')
    await page.keyboard.press('Enter')
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await expect
      .poll(() =>
        page.evaluate(() => document.activeElement?.closest('[data-testid=inspector]') !== null),
      )
      .toBe(true)
    await page.keyboard.press('Escape')
    await expect(inspector(page)).toBeHidden()
    await expect.poll(focusedItem).toBe('Extrude 1')
  })

  test('E-40 every visible button has an accessible name', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    // A name is an aria-label, the text of the aria-labelledby elements, or the text content.
    const unnamed = await page.evaluate(() =>
      [...document.querySelectorAll('button, [role=button]')]
        .filter((element) => element instanceof HTMLElement && element.getClientRects().length > 0)
        .filter((element) => {
          const label = element.getAttribute('aria-label') ?? ''
          const labelledBy = (element.getAttribute('aria-labelledby') ?? '')
            .split(/\s+/)
            .filter((id) => id !== '')
            .map((id) => document.getElementById(id)?.textContent ?? '')
            .join(' ')
          return `${label}${labelledBy}${element.textContent ?? ''}`.trim() === ''
        })
        .map((element) => element.outerHTML.slice(0, 160)),
    )
    expect(unnamed).toEqual([])
  })
})

/** One row of E-57: a fixture (or a new document), the steps, and the result. */
type FeatureRow = {
  readonly type: string
  readonly label: string
  readonly steps: (page: Page) => Promise<void>
  readonly result: (page: Page) => Promise<void>
  /** Set when a core defect blocks the row: the test is marked fixme. */
  readonly blocked?: true
}

const FEATURE_ROWS: readonly FeatureRow[] = [
  {
    type: 'revolve',
    label: 'Revolve 1',
    steps: async (page) => {
      await dropFile(page, 'revolve-sketch.ptsm')
      await fitView(page, 'Front')
      await runCommand(page, 'Revolve')
      // The axis is still empty, so nothing is previewed yet.
      await pickOnCanvas(page, 0, 0, 'profile', 1, 'none')
      await expect(inspector(page).getByRole('radio', { name: 'Full', exact: true })).toBeChecked()
      await pickRowInto(page, 'axis', await originRow(page, 'Z axis'), 'Z axis')
    },
    result: async (page) => {
      await expectBodySize(page, 'Body 1', '40 × 40 × 10 mm')
    },
  },
  {
    type: 'fillet',
    label: 'Fillet 1',
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      await fitView(page, 'Front')
      await page.keyboard.press('f')
      const edges = selectionField(page, 'edges')
      await expect(edges).toHaveAttribute('data-active', 'true')
      await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
      await pickOnCanvas(page, 0, -0.4, 'edges', 0, 'none')
      await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
      await fillInspector(page, 'Radius', '2')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')
    },
  },
  {
    type: 'chamfer',
    label: 'Chamfer 1',
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      await fitView(page, 'Front')
      await runCommand(page, 'Chamfer')
      await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
      await fillInspector(page, 'Distance', '2')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')
    },
  },
  {
    type: 'shell',
    label: 'Shell 1',
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      await fitView(page, 'Top')
      await runCommand(page, 'Shell')
      await pickOnCanvas(page, 0, 0, 'remove_faces', 1, 'shown')
      await fillInspector(page, 'Thickness', '2')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')
    },
  },
  {
    type: 'hole',
    label: 'Hole 1',
    steps: async (page) => {
      await dropFile(page, 'hole-cube.ptsm')
      await fitView(page, 'Top')
      await runCommand(page, 'Hole')
      await pickOnCanvas(page, 0, 0, 'points', 1, 'shown')
      await fillInspector(page, 'Diameter', '5')
      await chooseOption(page, 'Extent', 'Through all')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')
      const { cx, cy } = await canvas(page)
      await expectHover(page, cx, cy, 'none')
    },
  },
  {
    type: 'box',
    label: 'Box 1',
    steps: async (page) => {
      await runFromGroup(page, 'Shapes', 'Box')
      await fillInspector(page, 'Length', '10')
      await fillInspector(page, 'Width', '10')
      await fillInspector(page, 'Height', '10')
      await chooseOption(page, 'Operation', 'New body')
    },
    result: async (page) => {
      await expectBodySize(page, 'Body 1', '10 × 10 × 10 mm')
    },
  },
  {
    type: 'cylinder',
    label: 'Cylinder 1',
    steps: async (page) => {
      await runFromGroup(page, 'Shapes', 'Cylinder')
      await fillInspector(page, 'Radius', '5')
      await fillInspector(page, 'Height', '10')
    },
    result: async (page) => {
      await expectBodySize(page, 'Body 1', '10 × 10 × 10 mm')
    },
  },
  {
    type: 'sphere',
    label: 'Sphere 1',
    steps: async (page) => {
      await runFromGroup(page, 'Shapes', 'Sphere')
      await fillInspector(page, 'Radius', '5')
    },
    result: async (page) => {
      await expectBodySize(page, 'Body 1', '10 × 10 × 10 mm')
    },
  },
  {
    type: 'cone',
    label: 'Cone 1',
    steps: async (page) => {
      await runFromGroup(page, 'Shapes', 'Cone')
      await fillInspector(page, 'Bottom radius', '5')
      await fillInspector(page, 'Top radius', '0')
      await fillInspector(page, 'Height', '10')
    },
    result: async (page) => {
      await expectBodySize(page, 'Body 1', '10 × 10 × 10 mm')
    },
  },
  {
    type: 'torus',
    label: 'Torus 1',
    steps: async (page) => {
      await runFromGroup(page, 'Shapes', 'Torus')
      await fillInspector(page, 'Ring radius', '10')
      await fillInspector(page, 'Tube radius', '2')
    },
    result: async (page) => {
      await expectBodySize(page, 'Body 1', '24 × 24 × 4 mm')
    },
  },
  {
    type: 'boolean',
    label: 'Boolean 1',
    steps: async (page) => {
      await dropFile(page, 'two-bodies.ptsm')
      await runCommand(page, 'Boolean')
      await pickRowInto(page, 'target', bodyRow(page, 'Cube'), 'Cube')
      await pickRowInto(page, 'tools', bodyRow(page, 'Peg'), 'Peg')
      await chooseOption(page, 'Operation', 'Cut')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '20 × 20 × 20 mm')
      await expect(bodyRow(page, 'Peg')).toHaveText('Peg (empty)', { useInnerText: true })
    },
  },
  {
    // Core defect, ui.md open issue 17: the catalog's move_body params schema has no `default`
    // for `translation`, so Move X, Y and Z start empty and Move X alone sends a translation of
    // length 1, which the core refuses as invalid params ("invalid length 1, expected an array of
    // length 3").
    type: 'move_body',
    label: 'Move body 1',
    blocked: true,
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      await runCommand(page, 'Move body')
      await pickRowInto(page, 'body', bodyRow(page, 'Cube'), 'Cube')
      await fillInspector(page, 'Move X', '10')
    },
    result: async (page) => {
      await bodyRow(page, 'Cube').click()
      await expect(page.getByTestId('body-center')).toHaveText('10, 0, 10 mm')
    },
  },
  {
    type: 'split_body',
    label: 'Split body 1',
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      await runCommand(page, 'Split body')
      await pickRowInto(page, 'body', bodyRow(page, 'Cube'), 'Cube')
      await pickRowInto(
        page,
        'plane',
        await originRow(page, 'Front plane (XZ)'),
        'Front plane (XZ)',
      )
    },
    result: async (page) => {
      // Target body starts as a new body (2.12), named by nextBodyName.
      await expect(page.locator('[data-testid=browser-row][data-kind=body]')).toHaveCount(2)
      await expectBodySize(page, 'Cube', '20 × 10 × 20 mm')
      await expectBodySize(page, 'Body 1', '20 × 10 × 20 mm')
    },
  },
  {
    type: 'mirror',
    label: 'Mirror 1',
    steps: async (page) => {
      await dropFile(page, 'offset-cube.ptsm')
      await runCommand(page, 'Mirror')
      await pickRowInto(page, 'body', bodyRow(page, 'Cube'), 'Cube')
      await pickRowInto(
        page,
        'plane',
        await originRow(page, 'Right plane (YZ)'),
        'Right plane (YZ)',
      )
      await chooseOption(page, 'Operation', 'Join')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '60 × 20 × 20 mm')
      await expect(page.getByTestId('body-center')).toHaveText('0, 0, 10 mm')
    },
  },
  {
    type: 'linear_pattern',
    label: 'Linear pattern 1',
    steps: async (page) => {
      await dropFile(page, 'offset-cube.ptsm')
      await runFromGroup(page, 'Pattern', 'Linear pattern')
      await pickRowInto(page, 'body', bodyRow(page, 'Cube'), 'Cube')
      await pickRowInto(page, 'direction.axis', await originRow(page, 'X axis'), 'X axis')
      await fillInspector(page, 'Count', '3')
      await fillInspector(page, 'Spacing', '30')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '80 × 20 × 20 mm')
    },
  },
  {
    type: 'circular_pattern',
    label: 'Circular pattern 1',
    steps: async (page) => {
      await dropFile(page, 'offset-cube.ptsm')
      await runFromGroup(page, 'Pattern', 'Circular pattern')
      await pickRowInto(page, 'body', bodyRow(page, 'Cube'), 'Cube')
      await pickRowInto(page, 'axis', await originRow(page, 'Z axis'), 'Z axis')
      await fillInspector(page, 'Count', '4')
      await fillInspector(page, 'Total angle', '360 deg')
    },
    result: async (page) => {
      await expectBodySize(page, 'Cube', '60 × 60 × 20 mm')
    },
  },
  {
    type: 'datum_plane',
    label: 'Datum plane 1',
    steps: async (page) => {
      await dropFile(page, 'cube.ptsm')
      await runCommand(page, 'Datum plane')
      await pickRowInto(page, 'base', await originRow(page, 'Top plane (XY)'), 'Top plane (XY)')
      await fillInspector(page, 'Offset', '10')
    },
    result: async (page) => {
      await expect(browserRow(page, 'datum', 'Datum plane 1')).toHaveCount(1)
    },
  },
]

test.describe('E-57 feature coverage', () => {
  for (const row of FEATURE_ROWS) {
    const define = row.blocked === true ? test.fixme : test
    define(`E-57 ${row.type}`, async ({ page }) => {
      await openApp(page)
      await row.steps(page)
      await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
      // Each row ends with Enter where it confirms the command (ui.md 3.5).
      await focusWhereEnterConfirms(page)
      await confirmAndExpectOk(page, row.label)
      await row.result(page)
    })
  }
})

test.describe('fixes and renames', () => {
  test('E-58 the suggestion of a failed fillet fixes it', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'fillet-error.ptsm')
    await page.getByTestId('timeline-problems').click()
    const panel = page.getByTestId('problems-panel')
    await panel.getByRole('button', { name: 'Edit', exact: true }).click()
    await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
    const use = inspector(page)
      .getByTestId('inspector-error')
      .getByRole('button', { name: /^Use / })
    await use.click()
    await expectPreviewShown(page)
    // The live preview already shows Fillet 1 ok, so the command must have closed as well.
    await confirmAndExpectOk(page, 'Fillet 1')
  })

  test('E-59 renaming a parameter keeps every feature ok, and undo restores the name', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'parametric.ptsm')
    await runCommand(page, 'Parameters')
    const name = page.getByRole('textbox', { name: 'Name of size', exact: true })
    await name.fill('side')
    await name.press('Enter')
    await expect(page.getByRole('textbox', { name: 'Name of side', exact: true })).toHaveValue(
      'side',
    )
    await expectEveryItem(page, 2, 'data-status', 'ok')
    await leaveParametersField(page)
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(page.getByRole('textbox', { name: 'Name of size', exact: true })).toHaveValue(
      'size',
    )
  })
})

test.describe('one interface for everyone', () => {
  test('E-63 the empty state, the empty timeline and the origin axes follow the document', async ({
    page,
  }) => {
    await openApp(page)
    const empty = page.getByTestId('empty-state')
    await expect(empty).toBeVisible()
    await expect(empty).toContainText('Start with a sketch on a plane, or add a box.')
    await expect(empty).toContainText('right-drag or drag the view cube')
    await expect(empty.getByRole('button', { name: 'Sketch', exact: true })).toBeVisible()
    await expect(empty.getByRole('button', { name: 'Box', exact: true })).toBeVisible()
    await expect(page.getByTestId('timeline-empty')).toHaveText(
      'Each step you add appears here. Double-click a step to change it later.',
    )
    await expandOrigin(page)
    const browser = page.getByTestId('browser')
    await expect(browser.getByRole('button', { name: 'Hide X axis', exact: true })).toBeVisible()
    await empty.getByRole('button', { name: 'Box', exact: true }).click()
    await expect(empty).toBeHidden()
    await expect(inspector(page)).toHaveAccessibleName('Box')
    await page.keyboard.press('Enter')
    // The preview puts "Box 1" in the timeline and hides both empty states before Enter, so the
    // command must have closed first.
    await expect(page.locator('[data-testid=inspector][data-mode=command]')).toHaveCount(0)
    await expect(timelineItem(page, 'Box 1')).toHaveCount(1)
    await expect(empty).toBeHidden()
    await expect(page.getByTestId('timeline-empty')).toBeHidden()
    await expect(browser.getByRole('button', { name: 'Show X axis', exact: true })).toBeVisible()
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(empty).toBeVisible()
  })

  test('E-66 disabled feature commands say what is missing', async ({ page }) => {
    await openApp(page)
    const extrude = page
      .getByRole('toolbar', { name: 'Model tools' })
      .getByRole('button', { name: 'Extrude', exact: true })
    await expect(extrude).toHaveAttribute('aria-disabled', 'true')
    // Playwright waits for aria-disabled controls to become enabled; this click is the point.
    await extrude.click({ force: true })
    await expect(notification(page, 'info', 'Draw a closed shape in a sketch first.')).toHaveText(
      'Draw a closed shape in a sketch first.',
    )
    await expect(inspector(page)).toHaveCount(0)
    await clickCanvas(page, 0, 0)
    await page.keyboard.press('f')
    await expect(
      notification(page, 'info', 'Make a solid first, for example with Extrude or Box.'),
    ).toBeVisible()
  })

  test('E-67 finishing a sketch selects its region and offers Extrude', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await browserRow(page, 'sketch', 'Sketch 1').dblclick()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
    await page
      .getByRole('toolbar', { name: 'Sketch tools' })
      .getByRole('button', { name: 'Finish sketch' })
      .click()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'model')
    await expect(page.getByTestId('selection-title')).toHaveText('1 region')
    const suggestion = inspector(page).getByRole('button', { name: 'Extrude' })
    await expect(suggestion).toBeVisible()
    await suggestion.click()
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    await expect(selectionField(page, 'profile')).toHaveAttribute('data-count', '1')
  })

  test('E-68 a Hole run after a sketch with points takes them', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'hole-cube.ptsm')
    await browserRow(page, 'sketch', 'Sketch 2').dblclick()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(page.getByTestId('selection-title')).toHaveText('1 point')
    await expect(inspector(page).getByRole('button', { name: 'Hole' })).toBeVisible()
    await page.keyboard.press('h')
    await expect(inspector(page)).toHaveAccessibleName('Hole')
    await expect(selectionField(page, 'points')).toHaveAttribute('data-count', '1')
    await confirmAndExpectOk(page, 'Hole 1')
  })

  test('E-69 a hole placed by clicking a face', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Top')
    await clickCanvas(page, 0, 0)
    await expect(page.getByTestId('selection-title')).toHaveText('1 face')
    await inspector(page).getByRole('button', { name: 'Hole' }).click()
    await expect(inspector(page)).toHaveAccessibleName('Hole')
    const points = selectionField(page, 'points')
    await expect(points).toHaveAttribute('data-active', 'true')
    await expect(points).toHaveText('Click a flat face or pick sketch points')
    await pickOnCanvas(page, 0, 0, 'points', 1, 'shown')
    await page.keyboard.press('Enter')
    await expectTimelineLabels(page, ['Sketch 1', 'Extrude 1', 'Sketch 2', 'Hole 1'])
    await expectEveryItem(page, 4, 'data-status', 'ok')
    const { cx, cy } = await canvas(page)
    await expectHover(page, cx, cy, 'none')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(page.getByTestId('timeline-item')).toHaveCount(2)
  })

  test('E-72 Fillet starts at 1 mm and then reuses the document’s last radius', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await clickCanvas(page, 0, -0.4)
    await page.keyboard.press('f')
    await expect(inspectorInput(page, 'Radius')).toHaveValue('1 mm')
    await expectPreviewShown(page)
    await expect(inspector(page).getByTestId('inspector-error')).toHaveCount(0)
    await fillInspector(page, 'Radius', '2')
    await confirmAndExpectOk(page, 'Fillet 1')
    await clickCanvas(page, 0, 0.4)
    await page.keyboard.press('f')
    await expect(inspector(page)).toHaveAccessibleName('Fillet')
    await expect(inspectorInput(page, 'Radius')).toHaveValue('2')
  })

  test('E-73 digits typed on the view go to the command’s value', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await clickCanvas(page, 0, -0.4)
    await page.keyboard.press('f')
    const edges = selectionField(page, 'edges')
    await expect(edges).toHaveAttribute('data-count', '1')
    await expect(edges).toHaveAttribute('data-active', 'true')
    await expectPreviewShown(page)
    await pickOnCanvas(page, 0.4, 0, 'edges', 2, 'shown')
    await page.keyboard.press('2')
    await confirmAndExpectOk(page, 'Fillet 1')
    await timelineItem(page, 'Fillet 1').getByRole('button', { name: 'Fillet 1' }).dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
    await expect(inspectorInput(page, 'Radius')).toHaveValue('2')
  })

  test('E-74 Shell starts with its faces field active', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Top')
    await clickTool(page, 'Shell')
    const faces = selectionField(page, 'remove_faces')
    await expect(faces).toHaveAttribute('data-active', 'true')
    await pickOnCanvas(page, 0, 0, 'remove_faces', 1, 'shown')
    await confirmAndExpectOk(page, 'Shell 1')
  })

  test('E-75 Mirror as a new body names the body it creates', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'offset-cube.ptsm')
    await runCommand(page, 'Mirror')
    await expect(selectionField(page, 'body')).toHaveText('Cube')
    const plane = selectionField(page, 'plane')
    await expect(plane).toHaveAttribute('data-active', 'true')
    await (await originRow(page, 'Right plane (YZ)')).click()
    await expect(plane).toHaveText('Right plane (YZ)')
    await expect(inspectorField(page, 'target')).toHaveCount(0)
    await chooseOption(page, 'Operation', 'New body')
    await expect(selectionField(page, 'target')).toHaveText('Body 1 (new)')
    await confirmAndExpectOk(page, 'Mirror 1')
    await expect(bodyRow(page, 'Cube')).toHaveCount(1)
    await expectBodySize(page, 'Body 1', '20 × 20 × 20 mm')
    await expect(page.getByTestId('body-center')).toHaveText('-20, 0, 10 mm')
  })

  test('E-76 a cut sketched on a face goes into the body', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Top')
    await clickCanvas(page, 0, 0)
    await inspector(page).getByRole('button', { name: 'Sketch' }).click()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
    const box = await canvas(page)
    await page.keyboard.press('c')
    await page.mouse.click(box.cx, box.cy)
    await page.mouse.click(box.cx + 40, box.cy)
    await page
      .getByRole('toolbar', { name: 'Sketch tools' })
      .getByRole('button', { name: 'Finish sketch' })
      .click()
    await expect(page.getByTestId('selection-title')).toHaveText('1 region')
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    await chooseOption(page, 'Operation', 'Cut')
    await expect(
      inspector(page).getByRole('button', { name: 'Reverse', exact: true }),
    ).toHaveAttribute('aria-pressed', 'true')
    await confirmAndExpectOk(page, 'Extrude 2')
  })

  test('E-77 a negative distance offers Reverse and use', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await fitView(page, 'Top')
    await page.keyboard.press('e')
    await pickOnCanvas(page, 0, 0, 'profile', 1, 'shown')
    await fillInspector(page, 'Distance', '-5')
    const fix = inspector(page)
      .getByTestId('inspector-error')
      .getByRole('button', { name: 'Reverse and use 5', exact: true })
    await fix.click()
    await expect(inspectorInput(page, 'Distance')).toHaveValue('5')
    await expect(
      inspector(page).getByRole('button', { name: 'Reverse', exact: true }),
    ).toHaveAttribute('aria-pressed', 'true')
    await expectPreviewShown(page)
  })

  test('E-78 parameter changes join the open command and follow its OK or Cancel', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'parametric.ptsm')
    const extrude = timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' })
    await extrude.dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('parameters-transaction')).toHaveText(
      'Changes are part of Extrude 1 until you press OK.',
    )
    const expression = page.getByRole('combobox', { name: 'Expression of size', exact: true })
    await expression.fill('30 mm')
    await expression.press('Enter')
    await expectPreviewShown(page)
    await expect(page.locator('[data-testid=notification][data-severity=error]')).toHaveCount(0)
    await inspector(page).getByRole('button', { name: 'Cancel', exact: true }).last().click()
    await expect(page.getByTestId('parameter-value')).toHaveText('20 mm')

    await extrude.dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await expression.fill('30 mm')
    await expression.press('Enter')
    await expectPreviewShown(page)
    await inspector(page).getByTestId('inspector-ok').click()
    await expect(page.getByTestId('parameter-value')).toHaveText('30 mm')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(page.getByTestId('parameter-value')).toHaveText('20 mm')
  })

  test('E-79 undo inside a command takes back picks before it cancels', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await page.keyboard.press('f')
    const edges = selectionField(page, 'edges')
    await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
    await pickOnCanvas(page, 0.4, 0, 'edges', 2, 'shown')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(edges).toHaveAttribute('data-count', '1')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(edges).toHaveAttribute('data-count', '0')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(inspector(page)).toHaveCount(0)
    await expect(page.getByTestId('timeline-item')).toHaveCount(2)
  })

  test('E-80 an implicitly canceled command can be restored', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await page.keyboard.press('f')
    await pickOnCanvas(page, 0, -0.4, 'edges', 1, 'shown')
    await clickTool(page, 'Chamfer')
    const canceled = notification(page, 'info', 'Fillet was canceled.')
    await expect(canceled).toBeVisible()
    await canceled.getByRole('button', { name: 'Restore', exact: true }).click()
    await expect(inspector(page)).toHaveAccessibleName('Fillet')
    await expect(selectionField(page, 'edges')).toHaveAttribute('data-count', '1')
  })

  test('E-84 descriptions and field help on hover, and in the palette footer', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await page
      .getByRole('toolbar', { name: 'Model tools' })
      .getByRole('button', { name: 'Shell', exact: true })
      .hover()
    await expect(page.getByRole('tooltip').filter({ hasText: 'Hollows out a body' })).toBeVisible()
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    await inspector(page).getByText('Operation', { exact: true }).hover()
    await expect(
      page.getByRole('tooltip').filter({ hasText: 'Intersect keeps only the overlap' }),
    ).toBeVisible()
    await page.keyboard.press('Escape')
    await page.keyboard.press('ControlOrMeta+K')
    await page.keyboard.type('shell')
    await expect(page.getByTestId('palette-footer')).toContainText('Hollows out a body')
  })

  test('E-85 the empty palette is browsable by category and says when nothing matches', async ({
    page,
  }) => {
    await openApp(page)
    await page.keyboard.press('ControlOrMeta+K')
    await expect(page.getByTestId('palette-heading').first()).toHaveText('Create')
    await expect(
      page.getByTestId('palette-option').filter({ has: page.getByText('Fillet', { exact: true }) }),
    ).toHaveCount(1)
    await page.keyboard.type('zzz')
    await expect(page.getByTestId('palette-empty')).toHaveText('Nothing matches zzz.')
  })

  test('E-87 the Sketch radio edits the selected sketch', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await browserRow(page, 'sketch', 'Sketch 1').click()
    await page.getByRole('radio', { name: 'Sketch', exact: true }).check()
    await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
    await expect(page.getByTestId('sketch-header')).toHaveAttribute('data-state', 'full')
    await expect(page.getByTestId('timeline-item')).toHaveCount(1)
  })

  test('E-88 the selection inspector suggests commands for a body', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await bodyRow(page, 'Cube').click()
    const move = inspector(page).getByRole('button', { name: 'Move body' })
    await expect(move).toHaveText(/Move body\s*M$/, { useInnerText: true })
    await expect(inspector(page).getByRole('button', { name: 'Mirror' })).toBeVisible()
    await expect(inspector(page).getByRole('button', { name: 'Split body' })).toBeVisible()
    await move.click()
    await expect(inspector(page)).toHaveAccessibleName('Move body')
    await expect(selectionField(page, 'body')).toHaveAttribute('data-count', '1')
  })

  test('E-89 a face shows the feature that made it and its body', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    await clickCanvas(page, 0, 0)
    await expect(inspector(page)).toContainText('Made by Extrude 1')
    await expect(inspector(page).getByRole('button', { name: 'Select Cube' })).toBeVisible()
    await inspector(page).getByRole('button', { name: 'Edit Extrude 1' }).click()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await page.keyboard.press('Escape')
    await expect(inspector(page)).not.toHaveAttribute('data-mode', 'command')
    await clickCanvas(page, 0, 0)
    await expect(inspector(page)).toContainText('Made by Extrude 1')
    await inspector(page).getByRole('button', { name: 'Select Cube' }).click()
    await expect(page.getByTestId('selection-title')).toHaveText('Cube')
  })

  test('E-90 two selected bodies offer Boolean with both preselected', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'two-bodies.ptsm')
    await bodyRow(page, 'Cube').click()
    await bodyRow(page, 'Peg').click({ modifiers: ['Shift'] })
    await expect(page.getByTestId('selection-title')).toHaveText('2 bodies')
    await inspector(page).getByRole('button', { name: 'Boolean' }).click()
    await expect(inspector(page)).toHaveAccessibleName('Boolean')
    await expect(selectionField(page, 'target')).toHaveAttribute('data-count', '1')
    await expect(selectionField(page, 'tools')).toHaveAttribute('data-count', '1')
  })

  test('E-92 selecting a body measures its volume', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await bodyRow(page, 'Cube').click()
    const volume = page.locator('[data-testid=measure-row][data-measure=volume]')
    await expect(volume).toContainText('Volume')
    await expect(volume).toContainText('8000 mm³')
  })

  // Core defect, ui.md open issue 17: the catalog's box params schema has no `default` for
  // `position`, so Position X alone sends a position of length 1, which the core refuses as
  // invalid params ("invalid length 1, expected an array of length 3").
  test.fixme('E-93 More options names the values changed out of sight', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await runFromGroup(page, 'Shapes', 'Box')
    await expect(inspector(page)).toHaveAccessibleName('Box')
    const more = inspector(page).getByTestId('inspector-more')
    await more.click()
    await fillInspector(page, 'Position X', '40')
    await chooseOption(page, 'Operation', 'New body')
    await expect(more).toHaveText('Fewer options')
    await more.click()
    await inspector(page).getByTestId('inspector-ok').click()
    await expect(timelineItem(page, 'Box 1')).toHaveAttribute('data-status', 'ok')
    await timelineItem(page, 'Box 1').getByRole('button', { name: 'Box 1' }).dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Box 1')
    await expect(inspector(page).getByTestId('inspector-more')).toHaveText(
      'More options · Position X',
    )
  })

  test('E-94 parameter names complete in fields, and a missing one is created from the field', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'parametric.ptsm')
    await timelineItem(page, 'Extrude 1').getByRole('button', { name: 'Extrude 1' }).dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Extrude 1')
    const distance = inspectorInput(page, 'Distance')
    const result = inspectorField(page, 'extent.expr').getByTestId('expression-result')
    await distance.fill('si')
    await expect(
      inspector(page)
        .getByTestId('expression-suggestions')
        .getByRole('option', { name: 'size = 20 mm' }),
    ).toBeVisible()
    await distance.press('Tab')
    await expect(distance).toHaveValue('size')
    await expect(result).toHaveText('= 20 mm')
    await distance.fill('width')
    await expect(inspectorField(page, 'extent.expr').getByTestId('field-error')).toContainText(
      'width',
    )
    const add = inspector(page).getByRole('button', { name: 'Add parameter width', exact: true })
    await add.click()
    const panel = page.getByTestId('parameters-panel')
    await expect(panel).toBeVisible()
    const draftState = () =>
      page.evaluate(() => {
        const panelElement = document.querySelector('[data-testid=parameters-panel]')
        const inputs = [...(panelElement?.querySelectorAll('input') ?? [])]
        const focused = document.activeElement
        return {
          values: inputs.map((input) => input.value),
          focused: focused instanceof HTMLInputElement ? focused.value : null,
        }
      })
    await expect.poll(draftState).toMatchObject({ focused: '20 mm' })
    expect((await draftState()).values).toContain('width')
    await page.keyboard.press('Enter')
    await expect(
      panel.getByTestId('parameter-row').filter({
        has: page.getByRole('textbox', { name: 'Name of width', exact: true }),
      }),
    ).toHaveCount(1)
    await expect(distance).toHaveValue('width')
    await expect(result).toHaveText('= 20 mm')
  })

  test('E-98 the view menu runs Chamfer and Enter repeats it on the next edge', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Front')
    const { cx, cy, H } = await canvas(page)
    await page.mouse.click(cx, cy - 0.4 * H, { button: 'right' })
    const menu = page.getByTestId('viewport-menu')
    await expect(menu).toBeVisible()
    await expect(menuItem(menu, 'Fillet')).toBeVisible()
    await expect(menuItem(menu, 'Chamfer')).toBeVisible()
    await expect(menuItem(menu, 'Edit Extrude 1')).toBeVisible()
    await menuItem(menu, 'Chamfer').click()
    await expect(inspector(page)).toHaveAccessibleName('Chamfer')
    await confirmAndExpectOk(page, 'Chamfer 1')
    await clickCanvas(page, 0.4, 0)
    await expect(page.getByTestId('selection-title')).toHaveText('1 edge')
    await page.keyboard.press('Enter')
    await expect(inspector(page)).toHaveAccessibleName('Chamfer')
    await expect(selectionField(page, 'edges')).toHaveAttribute('data-count', '1')
    await expect(inspectorInput(page, 'Distance')).toHaveValue('1 mm')
    await confirmAndExpectOk(page, 'Chamfer 2')
  })

  test('E-99 clicking a face adds all of its edges to Fillet', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'cube.ptsm')
    await fitView(page, 'Top')
    await page.keyboard.press('f')
    const edges = selectionField(page, 'edges')
    await expect(edges).toHaveText('Select edges or a face')
    await pickOnCanvas(page, 0, 0, 'edges', 4, 'shown')
    await pickOnCanvas(page, 0, 0, 'edges', 0, 'none')
    await pickOnCanvas(page, 0, 0, 'edges', 4, 'shown')
    await confirmAndExpectOk(page, 'Fillet 1')
  })

  test('E-101 the keyboard and mouse panel follows the navigation preset', async ({ page }) => {
    await openApp(page)
    await page.keyboard.press('F1')
    const panel = page.getByTestId('keys-panel')
    await expect(panel).toBeVisible()
    const orbit = panel.getByText('Orbit', { exact: true }).locator('..')
    await expect(orbit).toContainText('Shift + middle drag')
    await panel.getByRole('radio', { name: 'FreeCAD', exact: true }).check()
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-nav-preset', 'freecad')
    await expect(orbit).toContainText('Middle + left drag')
  })
})
