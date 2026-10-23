/// <reference lib="dom" />
import { expect, test } from '@playwright/test'
import {
  canvas,
  download,
  dropFile,
  inspector,
  inspectorField,
  inspectorInput,
  modelTool,
  notification,
  openApp,
  runCommand,
  sketchToScreen,
  timelineItem,
} from './helpers'
import {
  buttonNames,
  expectInOrder,
  expectStatuses,
  nestedProblemRow,
  notificationActions,
  openProblems,
  problemRow,
} from './support/documents'
import {
  editFeatureFromTimeline,
  expectBodySize,
  expectPreviewShown,
  fillInspector,
  openTimelineMenu,
} from './support/modeling'
import {
  app,
  clickAt,
  dimensionLabel,
  editSketchFromBrowser,
  sketchHeader,
  sketchHint,
} from './support/sketch'

// The newcomer error corpus with the real core and UI (ui.md 6.5, E-110 to E-121; the newcomer
// error corpus NC01 to NC12, whose texts these tests check exactly), cause lines and uses
// included. NC08's cause line needs the edit stored in the session, which the dimension editor
// refuses, so engine.md test 150 checks it (ui.md open issue 24) and E-117 checks that a document
// opened fresh has none.

const CUT_AWAY =
  "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it."
const NOT_SOLVED =
  "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape."

test.describe('newcomer errors', () => {
  test('E-110 NC01, open outline: Extrude names the sketch, and Connect the ends closes it', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc01.ptsm')
    await expect(modelTool(page, 'Extrude')).toHaveAttribute('aria-disabled', 'true')
    // Playwright waits for aria-disabled controls to become enabled; this click is the point.
    await modelTool(page, 'Extrude').click({ force: true })
    const reason = notification(
      page,
      'info',
      "'Sketch 1' has no closed shape: 2 ends are not connected.",
    )
    await expect(reason).toBeVisible()
    await reason.getByRole('button', { name: "Show 'Sketch 1'", exact: true }).click()
    await expect(app(page)).toHaveAttribute('data-context', 'sketch')
    await expect(sketchHeader(page)).toHaveAttribute('data-outline', '2')
    await expect(sketchHeader(page)).toContainText('2 open ends')

    await sketchHeader(page).click()
    const problems = page.getByTestId('sketch-problems')
    await expect(problems).toContainText(
      'The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.',
    )
    await problems
      .getByRole('button', { name: 'Connect the ends (0.3 mm apart)', exact: true })
      .click()
    await expect(sketchHeader(page)).toHaveAttribute('data-outline', '0')
    // The popover stays open, without the finding (3.16.8). An open popover takes only Mod+K (2.10
    // dispatch rule 3), so Esc closes it before the sketch is finished.
    await expect(problems).toBeVisible()
    await expect(problems).not.toContainText('is not connected to other geometry')
    await page.keyboard.press('Escape')
    await expect(problems).toBeHidden()
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    await expect(page.getByTestId('selection-title')).toHaveText('1 region')
    await expect(modelTool(page, 'Extrude')).not.toHaveAttribute('aria-disabled', 'true')
  })

  test('E-111 NC02, join buried in the body: Attach puts the sketch on the top', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc02.ptsm')
    await expect(timelineItem(page, 'Extrude 2')).toHaveAttribute('data-status', 'warning')
    await openProblems(page)
    const row = problemRow(page, 'f-ext2')
    await expect(row).toContainText(
      "This extrude adds nothing to body 'Body 1': it lies entirely inside the body.",
    )
    await row
      .getByRole('button', { name: "Attach 'Sketch 2' to the top of 'Extrude 1'", exact: true })
      .click()
    await expect(timelineItem(page, 'Extrude 2')).toHaveAttribute('data-status', 'ok')
    await expectBodySize(page, 'Body 1', '80 × 50 × 40 mm')
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(timelineItem(page, 'Extrude 2')).toHaveAttribute('data-status', 'warning')
  })

  test('E-112 NC03, cut that points away: Turn on Reverse only where reversing reaches the body', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc03.ptsm')
    await editFeatureFromTimeline(page, 'Extrude 2')
    await fillInspector(page, 'Distance', '5')
    await expectPreviewShown(page)
    const banner = inspector(page).getByTestId('inspector-error')
    await expect(banner).toContainText(CUT_AWAY)
    // Reversed, a 5 mm cut ends at z = 35 and misses too.
    await expect(banner.getByRole('button', { name: 'Turn on Reverse', exact: true })).toHaveCount(
      0,
    )
    await page.keyboard.press('Escape')
    await expect(page.locator('[data-testid=inspector][data-mode=command]')).toHaveCount(0)

    await openProblems(page)
    const row = problemRow(page, 'f-ext2')
    await expect(row).toContainText(CUT_AWAY)
    await row.getByRole('button', { name: 'Turn on Reverse', exact: true }).click()
    await expect(timelineItem(page, 'Extrude 2')).toHaveAttribute('data-status', 'ok')
    await expectBodySize(page, 'Body 1', '80 × 50 × 30 mm')
  })

  test('E-113 NC04, floating join: the second fix attaches the sketch to the top', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc04.ptsm')
    await openProblems(page)
    const row = problemRow(page, 'f-ext2')
    await expect(row).toContainText(
      "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.",
    )
    const attach = "Attach 'Sketch 2' to the top of 'Extrude 1'"
    expectInOrder(await buttonNames(row), ['Put it in a new body', attach])
    await row.getByRole('button', { name: attach, exact: true }).click()
    await expect(timelineItem(page, 'Extrude 2')).toHaveAttribute('data-status', 'ok')
    await expectBodySize(page, 'Body 1', '80 × 50 × 45 mm')
  })

  test('E-114 NC05, fillet too large: the limit under Radius, and Use 2.3 mm builds it', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc05.ptsm')
    await editFeatureFromTimeline(page, 'Fillet 1')
    const radius = inspectorField(page, 'radius')
    await expect(radius.getByTestId('field-error')).toHaveText(
      "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.",
    )
    await expect(radius.getByTestId('field-limit')).toHaveText('Largest that works: 2.3 mm')
    const use = inspector(page)
      .getByTestId('inspector-error')
      .getByRole('button', { name: 'Use 2.3 mm', exact: true })
    await expect(use).toBeVisible()
    await use.click()
    await expectPreviewShown(page)
    await page.keyboard.press('Enter')
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'ok')
    await editFeatureFromTimeline(page, 'Fillet 1')
    await expect(inspectorInput(page, 'Radius')).toHaveValue('2.3 mm')
  })

  test('E-115 NC06, 2 mm walls after a 1 mm fillet build with sharp inner corners', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc06.ptsm')
    await expectStatuses(page, [
      ['Box 1', 'ok'],
      ['Fillet 1', 'ok'],
      ['Shell 1', 'ok'],
    ])
    await runCommand(page, 'Problems')
    await expect(page.getByTestId('problems-panel')).toBeVisible()
    await expect(page.locator('[data-testid=problem-row][data-feature-id="f-sh1"]')).toHaveCount(0)
    await expectBodySize(page, 'Body 1', '40 × 40 × 20 mm')
  })

  test('E-116 NC07, constraint conflict: one problem, its fixes, and the extrude nested under it', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc07.ptsm')
    await expect(timelineItem(page, 'Sketch 1')).toHaveAttribute('data-status', 'error')
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'error')
    await expect(page.getByTestId('timeline-problems')).toHaveText('1 problem')

    const panel = await openProblems(page)
    const topLevel = panel.locator('[data-testid=problem-row]:not([data-nested="true"])')
    await expect(topLevel).toHaveCount(1)
    await expect(topLevel).toHaveAttribute('data-feature-id', 'f-sk1')
    const row = problemRow(page, 'f-sk1')
    await expect(row).toContainText(
      "In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.",
    )
    const fixes = [
      'Delete Distance 30 mm',
      'Make Distance 30 mm a reference',
      'Delete Distance 40 mm',
      'Make Distance 40 mm a reference',
    ]
    expectInOrder(await buttonNames(row), fixes)
    const nested = nestedProblemRow(page, 'f-ext1')
    await expect(nested).toHaveCount(1)
    await expect(nested.getByRole('button', { name: "Show 'Sketch 1'", exact: true })).toBeVisible()

    await row.getByRole('button', { name: 'Delete Distance 30 mm', exact: true }).click()
    await expect(timelineItem(page, 'Sketch 1')).toHaveAttribute('data-status', 'ok')
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok')
    await expectBodySize(page, 'Body 1', '40 × 20 × 10 mm')
  })

  test('E-117 NC08, a value the sketch cannot take: the dimension editor refuses it, a stored one fails', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc08.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    await dimensionLabel(page, '50').dblclick()
    const editor = page.getByTestId('dimension-editor')
    await expect(editor).toBeVisible()
    const value = editor.getByRole('textbox', { name: 'Dimension value', exact: true })
    await value.fill('80')
    await value.press('Enter')
    const alert = editor.getByRole('alert')
    await expect(alert).toContainText(NOT_SOLVED)
    await expect(alert.getByRole('button', { name: 'Add reference', exact: true })).toBeVisible()
    const cancel = alert.getByRole('button', { name: 'Cancel', exact: true })
    await expect(cancel).toBeVisible()
    await expect(editor).toBeVisible()
    await expect(dimensionLabel(page, '50')).toHaveCount(1)
    await cancel.click()
    await expect(editor).toBeHidden()

    await dropFile(page, 'nc08-broken.ptsm')
    await expect(timelineItem(page, 'Sketch 1')).toHaveAttribute('data-status', 'error')
    await openProblems(page)
    const row = problemRow(page, 'f-sk1')
    await expect(row).toContainText(NOT_SOLVED)
    // A document opened fresh has no cause (4.1).
    await expect(row.getByTestId('cause-line')).toHaveCount(0)
  })

  test('E-118 NC09, units switch with bare numbers: Keep sizes adds mm to every number', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'nc09.ptsm')
    const units = page.getByTestId('status-units')
    await units.click()
    const keep = page.getByTestId('keep-sizes')
    await expect(keep).toHaveAttribute('aria-checked', 'true')
    await expect(keep).toHaveText('Keep sizes: add mm to the 4 numbers without a unit')
    await page.getByRole('menuitemradio', { name: 'in', exact: true }).click()
    await expect(units).toHaveText('in')
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'ok')

    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    const text = saved.bytes.toString('utf8')
    for (const kept of ['"40 mm"', '"20 mm"', '"10 mm"', '"2 mm"']) {
      expect(text).toContain(kept)
    }
    await page.keyboard.press('ControlOrMeta+Z')
    await expect(units).toHaveText('mm')
  })

  test('E-119 NC10, misspelled parameter: Replace hieght with height', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'nc10.ptsm')
    await openProblems(page)
    const row = problemRow(page, 'f-ext1')
    await expect(row).toContainText("Distance: 'hieght' is not a parameter. Did you mean 'height'?")
    await row.getByRole('button', { name: 'Replace hieght with height', exact: true }).click()
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok')
  })

  test('E-120 NC11, a deleted sketch line: one notification for the root, undone in one click', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc11.ptsm')
    await expectStatuses(page, [
      ['Sketch 1', 'ok'],
      ['Extrude 1', 'ok'],
      ['Fillet 1', 'ok'],
    ])
    const menu = await openTimelineMenu(page, 'Extrude 1')
    await expect(
      menu.getByRole('menuitem', { name: "Used by 'Fillet 1'", exact: true }),
    ).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.getByTestId('context-menu')).toBeHidden()

    await editSketchFromBrowser(page, 'Sketch 1')
    const marks = page.getByTestId('entity-used')
    await expect(marks).toHaveCount(1)
    await expect(marks).toHaveAttribute('data-entity', 'e-3')
    const mapping = sketchToScreen(await canvas(page), [0, 0], [80, 50])
    // The back line, at its middle.
    await clickAt(page, mapping.toPage([40, 50]))
    await expect(page.getByTestId('selection-title')).toHaveText('1 line')
    await page.keyboard.press('Delete')
    await expect(sketchHint(page)).toHaveText(
      "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it.",
    )
    await expect(page.locator('[data-testid=notification][data-severity=error]')).toHaveCount(0)

    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    // The sketch's own warning about its open outline (3.16.11) may stand beside it.
    const errors = page.locator('[data-testid=notification][data-severity=error]')
    await expect(errors).toHaveCount(1)
    const lost = errors.first()
    await expect(
      lost.getByText(
        "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 1 feature after it failed too.",
        { exact: true },
      ),
    ).toBeVisible()
    await expect(lost.getByTestId('cause-line')).toHaveText(
      "This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.",
    )
    expect(await notificationActions(lost)).toEqual(["Show what 'Extrude 1' lost", 'Undo'])
    await expect(page.locator('[data-testid=notification][data-code=feature_failed]')).toHaveCount(
      0,
    )

    await openProblems(page)
    const row = problemRow(page, 'f-ext1')
    await expect(row).toContainText(
      "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted.",
    )
    expectInOrder(await buttonNames(row), ["Show 'Sketch 1'", 'Pick a region again'])
    const nested = nestedProblemRow(page, 'f-fil1')
    await expect(
      nested.getByRole('button', { name: "Show 'Extrude 1'", exact: true }),
    ).toBeVisible()

    await lost.getByRole('button', { name: 'Undo', exact: true }).click()
    await expectStatuses(page, [
      ['Sketch 1', 'ok'],
      ['Extrude 1', 'ok'],
      ['Fillet 1', 'ok'],
    ])
    await expect(errors).toHaveCount(0)
  })

  test('E-121 NC12, hole beside the body: the point is named by its place, and no Reverse', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'nc12.ptsm')
    await openProblems(page)
    const row = problemRow(page, 'f-hole1')
    await expect(row).toContainText(
      "The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body.",
    )
    await expect(row.getByRole('button', { name: 'Turn on Reverse', exact: true })).toHaveCount(0)
  })
})
