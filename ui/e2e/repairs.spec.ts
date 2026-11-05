/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  browserRow,
  canvas,
  download,
  dropFile,
  expectHover,
  fitView,
  fixtureBytes,
  inspector,
  inspectorField,
  inspectorInput,
  notificationCoded,
  openApp,
  originRow,
  runCommand,
  sketchToScreen,
  test,
  timelineItem,
  toScreen,
} from './helpers'
import {
  buttonNames,
  expectInOrder,
  expectOnlyNotification,
  expectStatuses,
  expectTimeline,
  notificationActions,
  openProblems,
  problemRow,
} from './support/documents'
import {
  editFeatureFromTimeline,
  expectPreviewShown,
  fillInspector,
  holdOver,
  openTimelineMenu,
  timelineGap,
} from './support/modeling'
import {
  app,
  clickAt,
  editSketchFromBrowser,
  expectNoHover,
  sketchHeader,
  sketchHint,
} from './support/sketch'

// Lost references with the real core (ui.md 6.5, E-109 and E-122 to E-126; the reference
// stability study section 6.3): a notification for every new loss with its cause line, repair mode
// with the lost element's ghost, the core's fixes and the cause's reverts, the drop hint, used
// marks and notes.

/** The text of the stale_fix error (protocol.md 4, engine.md 3.22). */
const STALE_FIX =
  'This fix no longer applies because the document changed after it was offered. Look at the problem again.'

/** The loss notification of 'Fillet 1' (ui.md 4.2), whatever else is shown. */
function filletLoss(page: Page): Locator {
  return page.getByTestId('notification').filter({ hasText: "'Fillet 1' lost" })
}

/** The text of `attach_moved` for 'Sketch 1' on `target` (ui.md 4.3). */
function attachMoved(target: string): string {
  return `'Sketch 1' cannot be placed on ${target}: the two do not move together, and what later features place on the faces made from 'Sketch 1' could shift with no message. Pick a plane or face that moves the same way as 'Sketch 1'.`
}

/** The text of `datum_base_moved` (ui.md 4.3) for 'Datum plane 2' on `target`. */
function datumBaseMoved(target: string): string {
  return `'Datum plane 2' cannot be placed on ${target}: the two do not move together, and what later features place on the faces made from the sketches that build on 'Datum plane 2' could shift with no message. Pick a plane or face that moves the same way as 'Datum plane 2'.`
}

/** Expects every timeline item of datum-base-lifted.ptsm to build without a problem. */
async function expectLiftedOk(page: Page): Promise<void> {
  await expectStatuses(page, [
    ['Box 1', 'ok'],
    ['Move body 1', 'ok'],
    ['Datum plane 1', 'ok'],
    ['Datum plane 2', 'ok'],
    ['Sketch 1', 'ok'],
    ['Extrude 1', 'ok'],
  ])
}

/** Edits 'Linear pattern 1' of pattern-fillet.ptsm to Count 2 and confirms, as E-122 and E-124 do. */
async function lowerPatternCount(page: Page): Promise<void> {
  await editFeatureFromTimeline(page, 'Linear pattern 1')
  await fillInspector(page, 'Count', '2')
  await page.keyboard.press('Enter')
  await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'warning')
}

test.describe('lost references and their fixes', () => {
  test('E-109 corpus R03: a line deleted in a sketch session notifies once when the session ends', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'bevel-corner.ptsm')
    await expectTimeline(page, ['Sketch 1', 'Extrude 1', 'Fillet 1'])
    await editSketchFromBrowser(page, 'Sketch 1')
    const mapping = sketchToScreen(await canvas(page), [0, 0], [80, 50])
    await clickAt(page, mapping.toPage([75, 45]))
    await expect(page.getByTestId('selection-title')).toHaveText('1 line')
    await expect(page.locator('[data-testid=entity-used][data-entity="e-5"]')).toHaveAccessibleName(
      "Used by 'Fillet 1'",
    )
    await page.keyboard.press('Delete')
    await expect(sketchHint(page)).toHaveText(
      "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it.",
    )
    await expect(page.getByTestId('notification')).toHaveCount(0)

    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    const lost = await expectOnlyNotification(page)
    await expect(lost).toHaveAttribute('data-severity', 'warning')
    await expect(
      lost.getByText("'Fillet 1' lost the top edge of 'Extrude 1' along a line of 'Sketch 1'.", {
        exact: true,
      }),
    ).toBeVisible()
    await expect(lost.getByTestId('cause-line')).toHaveText(
      "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
    )
    expect(await notificationActions(lost)).toEqual(["Show what 'Fillet 1' lost", 'Undo'])
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok')
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'warning')

    await openProblems(page)
    const row = problemRow(page, 'f-fil1')
    await expect(row).toContainText(
      "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.",
    )
    const names = await buttonNames(row)
    const uses = names.filter((name) => name.startsWith('Use '))
    expect(uses.length).toBeGreaterThan(0)
    expectInOrder(names, [
      uses.at(-1) ?? '',
      'Remove from Edges',
      'Pick an edge again',
      'Undo',
      'Show the line',
    ])
    await expect(row.getByTestId('cause-line')).toHaveText(
      "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
    )

    await row.getByRole('button', { name: 'Show the line', exact: true }).click()
    await expect(page.getByTestId('cause-ghost')).toHaveCount(1)
    await expect(page.getByTestId('cause-ghost')).toBeVisible()

    await row.getByRole('button', { name: 'Remove from Edges', exact: true }).click()
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'ok')
  })

  test('E-123 corpus R16: unsuppressing a cut that removes the filleted edge, repaired from repair mode', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'rabbet.ptsm')
    await expectStatuses(page, [
      ['Box 1', 'ok'],
      ['Sketch 1', 'ok'],
      ['Extrude 1', 'suppressed'],
      ['Fillet 1', 'ok'],
    ])
    const menu = await openTimelineMenu(page, 'Extrude 1')
    await menu.getByRole('menuitem', { name: 'Unsuppress', exact: true }).click()
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'error')
    const lost = await expectOnlyNotification(page)
    await expect(lost).toHaveAttribute('data-severity', 'error')
    await expect(
      lost.getByText("'Fillet 1' lost the front top edge of 'Box 1'.", { exact: true }),
    ).toBeVisible()
    await expect(lost.getByTestId('cause-line')).toHaveText(
      "This started after you unsuppressed 'Extrude 1'.",
    )
    expect(await notificationActions(lost)).toEqual([
      "Show what 'Fillet 1' lost",
      "Suppress 'Extrude 1'",
      'Undo',
    ])

    await lost.getByRole('button', { name: "Show what 'Fillet 1' lost", exact: true }).click()
    await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(page.getByTestId('repair-ghost')).toHaveCount(1)
    expect(await page.getByTestId('repair-candidate').count()).toBeGreaterThanOrEqual(2)
    const candidates = inspector(page).getByTestId('repair-candidate-button')
    await expect(candidates.nth(0)).toContainText(
      "1. The front edge of the cut made by 'Extrude 1'",
    )
    await expect(candidates.nth(1)).toContainText("2. The top edge of the cut made by 'Extrude 1'")

    await candidates.nth(0).click()
    await expectPreviewShown(page)
    await page.keyboard.press('Enter')
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'ok')
  })

  test('E-124 corpus R41: lowering a pattern count notifies about the copy it no longer makes', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'pattern-fillet.ptsm')
    await expectTimeline(page, ['Box 1', 'Linear pattern 1', 'Fillet 1'])
    await lowerPatternCount(page)

    const lost = await expectOnlyNotification(page)
    await expect(lost).toHaveAttribute('data-severity', 'warning')
    await expect(
      lost.getByText(
        "'Fillet 1' lost the right top edge of 'Box 1' of the third copy of 'Linear pattern 1'.",
        { exact: true },
      ),
    ).toBeVisible()
    await expect(lost.getByTestId('cause-line')).toHaveText(
      "This started after you changed Count of 'Linear pattern 1' from 3 to 2.",
    )
    expect(await notificationActions(lost)).toEqual([
      "Show what 'Fillet 1' lost",
      "Set Count of 'Linear pattern 1' back to 3",
      'Undo',
    ])

    await lost
      .getByRole('button', { name: "Set Count of 'Linear pattern 1' back to 3", exact: true })
      .click()
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'ok')
    await expect(filletLoss(page)).toHaveCount(0)

    await page.keyboard.press('ControlOrMeta+Z')
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'warning')
  })

  test('E-122 a fix offered before an unrelated edit reports that it no longer applies', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'pattern-fillet.ptsm')
    await expectTimeline(page, ['Box 1', 'Linear pattern 1', 'Fillet 1'])
    await lowerPatternCount(page)
    const lost = filletLoss(page)
    await expect(lost).toHaveCount(1)

    // With the pointer over the notification, so its timer waits, an unrelated edit from the palette.
    await lost.hover()
    await page.keyboard.press('ControlOrMeta+K')
    const input = page.getByTestId('palette-input')
    await expect(input).toBeFocused()
    await input.fill('Units: cm')
    await expect(page.getByTestId('palette-option').first()).toContainText('Units: cm')
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('status-units')).toHaveText('cm')

    await lost
      .getByRole('button', { name: "Set Count of 'Linear pattern 1' back to 3", exact: true })
      .click()
    const stale = notificationCoded(page, 'stale_fix')
    await expect(stale).toHaveCount(1)
    await expect(stale).toHaveAttribute('data-severity', 'error')
    await expect(stale.getByText(STALE_FIX, { exact: true })).toBeVisible()
    expect(await notificationActions(stale)).toEqual(['Show problems'])

    await editFeatureFromTimeline(page, 'Linear pattern 1')
    await expect(inspectorInput(page, 'Count')).toHaveValue('2')
  })

  test('E-125 corpus R28: a fillet dropped above what makes its edge, moved back by its fix', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'boss-fillet.ptsm')
    const labels = ['Sketch 1', 'Extrude 1', 'Datum plane 1', 'Sketch 2', 'Extrude 2', 'Fillet 1']
    await expectTimeline(page, labels)

    // Drag 'Fillet 1' over the gap between 'Extrude 1' and 'Datum plane 1' and hold it there.
    await holdOver(page, timelineItem(page, 'Fillet 1'), await timelineGap(page, 2))
    const hint = page.getByTestId('drop-hint')
    // The uses the drag asked for may arrive after the pointer stopped over the gap; they warn
    // there without another move (3.9).
    await expect(hint).toHaveText(
      "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?",
    )
    await page.mouse.up()
    await expect(hint.filter({ hasText: 'Drop below' })).toHaveCount(0)
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'error')
    await openProblems(page)
    const move = problemRow(page, 'f-fil1').getByRole('button', {
      name: "Move 'Fillet 1' below 'Extrude 2'",
      exact: true,
    })
    await expect(move).toBeVisible()
    await move.click()
    await expectTimeline(page, labels)

    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    expect(saved.bytes.equals(await fixtureBytes('boss-fillet.ptsm'))).toBe(true)
  })

  test('E-126 corpus R06 (a): a side line redrawn leaves a note, and Keep this match removes it', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'side-line.ptsm')
    await expectTimeline(page, ['Sketch 1', 'Extrude 1', 'Fillet 1'])
    await editSketchFromBrowser(page, 'Sketch 1')
    const mapping = sketchToScreen(await canvas(page), [0, 0], [80, 50])
    // Each step waits for the state it needs, so a busy machine cannot delete nothing and draw a second side.
    const side = mapping.toPage([80, 25])
    await expectHover(page, side.x, side.y, 'sketch_curve')
    await clickAt(page, side)
    await expect(page.getByTestId('selection-title')).toHaveText('1 line')
    await page.keyboard.press('Delete')
    await expectNoHover(page, side, 'sketch_curve')
    await page.keyboard.press('L')
    await expect(sketchHint(page)).toHaveText('Click to start a line.')
    await clickAt(page, mapping.toPage([80, 0]))
    await clickAt(page, mapping.toPage([80, 50]))
    await page.keyboard.press('Escape')
    await expectHover(page, side.x, side.y, 'sketch_curve')
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(app(page)).toHaveAttribute('data-context', 'model')

    const fillet = timelineItem(page, 'Fillet 1')
    await expect(fillet).toHaveAttribute('data-status', 'ok')
    await expect(fillet).toHaveAttribute('data-notes', '1')
    await expect(page.getByTestId('notification')).toHaveCount(0)
    await expect(page.getByTestId('timeline-problems')).toBeHidden()

    await runCommand(page, 'Problems')
    const panel = page.getByTestId('problems-panel')
    const notes = panel.getByTestId('problems-notes')
    await notes.click()
    await expect(notes).toHaveAttribute('aria-expanded', 'true')
    const keep = panel.getByRole('button', { name: 'Keep this match', exact: true })
    await expect(keep).toHaveCount(1)
    await keep.click()
    await expect(fillet).toHaveAttribute('data-notes', '0')
    await expect(panel.getByRole('button', { name: 'Keep this match', exact: true })).toHaveCount(0)
  })

  test('E-165 a re-attach that would move later features is refused', async ({ page }) => {
    // In a 720-pixel window the sketch header and its five candidates cover both tops the test clicks.
    await page.setViewportSize({ width: 1280, height: 1600 })
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'reattach-moved.ptsm')
    const sketch = timelineItem(page, 'Sketch 1')
    await expect(sketch).toHaveAttribute('data-status', 'warning')

    await runCommand(page, 'Problems')
    await problemRow(page, 'f-sk1')
      .getByRole('button', { name: 'Pick a face again', exact: true })
      .click()
    await expect(app(page)).toHaveAttribute('data-context', 'sketch')
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')
    await expect(sketchHeader(page)).toContainText("Pick a plane or a flat face for 'Sketch 1'")

    // The sketch moves with the cube; the Top plane did not move.
    await sketchHeader(page)
      .getByRole('button', { name: /Top plane \(XY\)/ })
      .click()
    await expect(sketchHint(page)).toHaveText(attachMoved('the Top plane (XY)'))
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')

    // The clicks stay away from the numbered markers at the candidates' centroids (3.7).
    await fitView(page, 'Top')
    const mapping = toScreen(await canvas(page), { min: [95, -5, 0], max: [105, 13, 20] }, 'Top')
    await clickAt(page, mapping.toPage([97, 4, 20]))
    await expect(sketchHint(page)).toHaveText(attachMoved("this face of 'Box 2'"))
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')

    await clickAt(page, mapping.toPage([97, -4, 10]))
    await expect(sketchHeader(page)).not.toHaveAttribute('data-state', 'picking')
    await expect(app(page)).toHaveAttribute('data-context', 'sketch')
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(sketch).toHaveAttribute('data-status', 'ok')

    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    const savedDocument = JSON.parse(saved.bytes.toString('utf8')) as {
      readonly features: readonly { readonly id: string; readonly plane?: unknown }[]
    }
    const plane = savedDocument.features.find((feature) => feature.id === 'f-sk1')?.plane as {
      readonly face: { readonly q: string }
      readonly fallback: { readonly origin: readonly number[] }
      readonly fallback_frame?: string
    }
    expect(plane.face.q.startsWith('f-box1/top')).toBe(true)
    const [x, y, z] = plane.fallback.origin
    expect(x).toBeCloseTo(-100, 9)
    expect(y).toBeCloseTo(0, 9)
    expect(z).toBeCloseTo(10, 9)
    expect(plane.fallback_frame).toBe('body')

    await page.keyboard.press('ControlOrMeta+Z')
    await expect(sketch).toHaveAttribute('data-status', 'warning')
  })
  test('E-166 a new base of a datum plane that would move later features is refused', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'datum-base-lifted.ptsm')
    await expectLiftedOk(page)

    await editFeatureFromTimeline(page, 'Datum plane 2')
    const base = inspectorField(page, 'base')
    const field = base.getByTestId('selection-field')
    await field.click()
    await expect(field).toHaveAttribute('data-active', 'true')

    // The datum moves with the lifted cube; the Top plane did not move.
    await (await originRow(page, 'Top plane (XY)')).click()
    await expect(base).toContainText(datumBaseMoved('the Top plane (XY)'))
    await expect(field).toHaveAttribute('data-active', 'true')
    await expect(field).toHaveAttribute('data-count', '1')

    // The lift slides along the front side, so the front side did not move the datum's way.
    await fitView(page, 'Front')
    const mapping = toScreen(await canvas(page), { min: [-5, -5, 20], max: [5, 5, 30] }, 'Front')
    await clickAt(page, mapping.toPage([3, -5, 25]))
    await expect(base).toContainText(datumBaseMoved("this face of 'Box 1'"))
    await expect(field).toHaveAttribute('data-active', 'true')

    // 'Datum plane 1' sits on the same top and moves the same way.
    await browserRow(page, 'datum', 'Datum plane 1').click()
    await expect(field).toHaveText('Datum plane 1')
    await page.getByTestId('inspector-ok').click()
    await expect(inspector(page)).toHaveCount(0)
    await expectLiftedOk(page)

    const saved = await download(page, () => page.keyboard.press('ControlOrMeta+S'))
    const savedDocument = JSON.parse(saved.bytes.toString('utf8')) as {
      readonly features: readonly { readonly id: string; readonly base?: unknown }[]
    }
    expect(savedDocument.features.find((feature) => feature.id === 'f-dp2')?.base).toEqual({
      kind: 'datum',
      feature: 'f-dp1',
    })
  })
})
