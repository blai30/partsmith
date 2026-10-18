/// <reference lib="dom" />
import { expect, test } from '@playwright/test'
import {
  canvas,
  download,
  dropFile,
  expectHover,
  fixtureBytes,
  inspector,
  openApp,
  runCommand,
  sketchToScreen,
  timelineItem,
} from './helpers'
import {
  buttonNames,
  expectOnlyNotification,
  expectStatuses,
  expectTimeline,
  notificationActions,
  openProblems,
  problemRow,
} from './support/documents'
import { dragTo, expectPreviewShown, fillInspector, timelineGap } from './support/modeling'
import { app, clickAt, editSketchFromBrowser, expectNoHover, sketchHint } from './support/sketch'

// Lost references with the real core (ui.md 6.5, E-109 and E-123 to E-126; the reference
// stability study section 6.3): a notification for every new loss, repair mode, the core's fixes
// and notes. Phase 1 of the plan (reference-stability-and-errors.md 10.3) has no cause lines,
// ghosts or uses yet: phase 4 adds the cause lines and their reverts, the repair ghost, the drop
// hint and the used marks of these tests (10.6).

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
    await page.keyboard.press('Delete')
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
    const remove = names.indexOf('Remove from Edges')
    const pick = names.indexOf('Pick an edge again')
    expect(names.indexOf(uses.at(-1) ?? '')).toBeLessThan(remove)
    expect(remove).toBeLessThan(pick)

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
    await timelineItem(page, 'Extrude 1')
      .getByRole('button', { name: 'Extrude 1', exact: true })
      .click({ button: 'right' })
    await page
      .getByTestId('context-menu')
      .getByRole('menuitem', { name: 'Unsuppress', exact: true })
      .click()
    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'error')
    const lost = await expectOnlyNotification(page)
    await expect(lost).toHaveAttribute('data-severity', 'error')
    await expect(
      lost.getByText("'Fillet 1' lost the front top edge of 'Box 1'.", { exact: true }),
    ).toBeVisible()
    expect(await notificationActions(lost)).toEqual(["Show what 'Fillet 1' lost", 'Undo'])

    await lost.getByRole('button', { name: "Show what 'Fillet 1' lost", exact: true }).click()
    await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    // Without a ghost from the core (phase 4), the stored centroid is ringed.
    await expect(page.getByTestId('repair-marker')).toHaveCount(1)
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
    await timelineItem(page, 'Linear pattern 1')
      .getByRole('button', { name: 'Linear pattern 1', exact: true })
      .dblclick()
    await expect(inspector(page)).toHaveAccessibleName('Linear pattern 1')
    await fillInspector(page, 'Count', '2')
    await page.keyboard.press('Enter')

    await expect(timelineItem(page, 'Fillet 1')).toHaveAttribute('data-status', 'warning')
    const lost = await expectOnlyNotification(page)
    await expect(lost).toHaveAttribute('data-severity', 'warning')
    await expect(
      lost.getByText(
        "'Fillet 1' lost the right top edge of 'Box 1' of the third copy of 'Linear pattern 1'.",
        { exact: true },
      ),
    ).toBeVisible()
    expect(await notificationActions(lost)).toEqual(["Show what 'Fillet 1' lost", 'Undo'])
  })

  test('E-125 corpus R28: a fillet dropped above what makes its edge, moved back by its fix', async ({
    page,
  }) => {
    await openApp(page, { fileSystemAccess: false })
    await dropFile(page, 'boss-fillet.ptsm')
    const labels = ['Sketch 1', 'Extrude 1', 'Datum plane 1', 'Sketch 2', 'Extrude 2', 'Fillet 1']
    await expectTimeline(page, labels)

    await dragTo(page, timelineItem(page, 'Fillet 1'), await timelineGap(page, 2))
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
})
