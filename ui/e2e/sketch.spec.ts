/// <reference lib="dom" />
import { expect, test, type Page } from '@playwright/test'
import {
  bodyRow,
  canvas,
  clickTool,
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
  sketchToScreen,
  timelineItem,
  viewport,
  type PagePoint,
} from './helpers'
import {
  app,
  clickAt,
  constraintGlyphs,
  dimensionLabel,
  dragBetween,
  editSketchFromBrowser,
  editSketchFromTimeline,
  expectDof,
  expectNoHover,
  inferenceGlyphs,
  newSketchOnTopPlane,
  offset,
  sketchDimensions,
  sketchHeader,
  sketchHint,
} from './support/sketch'

// Sketch mode against the production build and the real core (ui.md 3.16, 6.5): entering and
// plane picking, drawing tools and their step prompts, inference and its suppression, dimensions
// typed while drawing and placed with the dimension tool, constraints and their conflicts, point
// drags, box selection, the sketch header and its problems, and finishing.

/** Ends the line tool's chain with Esc, then leaves the tool with a second Esc (ui.md 3.5). */
async function leaveLineTool(page: Page): Promise<void> {
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await expect(sketchHint(page)).not.toContainText('Click to')
}

/** Draws one line with the line tool from `start` to `end`, optionally holding Control (Mod). */
async function drawLine(
  page: Page,
  start: PagePoint,
  end: PagePoint,
  { free = false }: { readonly free?: boolean } = {},
): Promise<void> {
  await page.keyboard.press('l')
  await expect(sketchHint(page)).toContainText('Click to start a line.')
  if (free) {
    await page.keyboard.down('Control')
  }
  await page.mouse.move(start.x, start.y)
  await clickAt(page, start)
  await page.mouse.move(end.x, end.y, { steps: 4 })
  await clickAt(page, end)
  if (free) {
    await page.keyboard.up('Control')
  }
}

/**
 * E-12's steps: a new sketch on the top plane picked in the browser, a 2-point rectangle from the
 * origin, and its width and height dimensioned to 40 and 20 with the dimension tool.
 */
async function drawDimensionedRectangle(page: Page): Promise<void> {
  await clickTool(page, 'Sketch')
  await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')
  await expandOrigin(page)
  await (await originRow(page, 'Top plane (XY)')).click()
  await expect(timelineItem(page, 'Sketch 1')).toBeVisible()
  await expect(viewport(page)).toHaveAttribute('data-view', 'top')
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'empty')

  const box = await canvas(page)
  await page.keyboard.press('r')
  await page.mouse.move(box.cx, box.cy)
  await expect(page.locator('[data-testid=inference-glyph][data-type=coincident]')).toBeVisible()
  await clickAt(page, offset(box, 0, 0))
  await clickAt(page, offset(box, 200, -100))
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'under')
  await expectDof(page, 2)

  await page.keyboard.press('d')
  await clickAt(page, offset(box, 100, 0))
  await clickAt(page, offset(box, 100, 40))
  await expect(page.getByTestId('dimension-editor')).toBeVisible()
  await page.keyboard.type('40')
  await page.keyboard.press('Enter')
  await expect(dimensionLabel(page, '40')).toHaveCount(1)
  await expectDof(page, 1)

  await clickAt(page, offset(box, 0, -50))
  await clickAt(page, offset(box, -40, -50))
  await expect(page.getByTestId('dimension-editor')).toBeVisible()
  await page.keyboard.type('20')
  await page.keyboard.press('Enter')
  await expectDof(page, 0)
  await expect(sketchHeader(page)).toHaveAttribute('data-state', 'full')
  await expect(sketchHeader(page)).toContainText('Fully constrained')
}

test.describe('sketching', () => {
  test('E-12 a rectangle from the origin, dimensioned to fully constrained', async ({ page }) => {
    await openApp(page)
    await drawDimensionedRectangle(page)
  })

  // The spec allows either start; continuing from E-12 keeps the double-click on the timeline
  // to E-18 and E-104.
  test('E-13 Extrude from a sketch takes its region and builds the body', async ({ page }) => {
    await openApp(page)
    await drawDimensionedRectangle(page)
    await page.keyboard.press('e')
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    await expect(inspectorField(page, 'profile').getByTestId('selection-field')).toHaveAttribute(
      'data-count',
      '1',
    )
    await expect(inspectorInput(page, 'Distance')).toBeFocused()
    await page.keyboard.type('10')
    await expect(inspectorField(page, 'extent.expr').getByTestId('expression-result')).toHaveText(
      'mm',
    )
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('timeline-item')).toHaveCount(2)
    await expect(timelineItem(page, 'Sketch 1')).toHaveAttribute('data-status', 'ok')
    await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok', {
      timeout: 15_000,
    })
    await bodyRow(page, 'Body 1').click()
    await expect(page.getByTestId('body-size')).toHaveText('40 × 20 × 10 mm')
    await expect(page.getByTestId('body-center')).toHaveText('20, 10, 5 mm')
  })

  test('E-14 the Extrude distance is checked live as a length', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await fitView(page, 'Top')
    await page.keyboard.press('e')
    await expect(inspector(page)).toHaveAccessibleName('Extrude')
    const box = await canvas(page)
    await clickAt(page, offset(box, 0, 0))
    const profile = inspectorField(page, 'profile').getByTestId('selection-field')
    await expect(profile).toHaveAttribute('data-count', '1')
    const distance = inspectorInput(page, 'Distance')
    const field = inspectorField(page, 'extent.expr')
    await distance.fill('10 deg')
    await expect(field.getByTestId('field-error')).toBeVisible()
    await expect(field.getByTestId('field-error')).toContainText('must be a length')
    await expect(inspector(page).getByTestId('inspector-ok')).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    await distance.fill('2 * 5')
    await expect(field.getByTestId('expression-result')).toHaveText('= 10 mm')
    // The error element stays mounted as a live region; without an error it is empty and hidden.
    await expect(inspector(page).getByTestId('field-error').filter({ hasText: /\S/ })).toHaveCount(
      0,
    )
    await expect(inspector(page).getByTestId('inspector-ok')).not.toHaveAttribute(
      'aria-disabled',
      'true',
    )
  })

  test('E-15 inference makes a near-horizontal line horizontal, and Control suppresses it', async ({
    page,
  }) => {
    await openApp(page)
    await newSketchOnTopPlane(page)
    const box = await canvas(page)
    await page.keyboard.press('l')
    await clickAt(page, offset(box, 0, 0))
    // 2.9 degrees, outside the 6-pixel axis snap.
    await page.mouse.move(box.cx + 200, box.cy - 10, { steps: 4 })
    await expect(page.locator('[data-testid=inference-glyph][data-type=horizontal]')).toBeVisible()
    await clickAt(page, offset(box, 200, -10))
    await leaveLineTool(page)
    await expectDof(page, 1)

    await page.keyboard.press('l')
    await page.keyboard.down('Control')
    await clickAt(page, offset(box, 0, -100))
    await page.mouse.move(box.cx + 200, box.cy - 103, { steps: 4 })
    await expect(inferenceGlyphs(page)).toHaveCount(0)
    await clickAt(page, offset(box, 200, -103))
    await page.keyboard.up('Control')
    await leaveLineTool(page)
    await expectDof(page, 5)
  })

  test('E-16 a length typed while drawing becomes a driving dimension', async ({ page }) => {
    await openApp(page)
    await newSketchOnTopPlane(page)
    const box = await canvas(page)
    await page.keyboard.press('l')
    await clickAt(page, offset(box, 0, 0))
    await page.mouse.move(box.cx + 150, box.cy, { steps: 4 })
    await page.keyboard.type('25')
    const length = page.getByRole('textbox', { name: 'Length', exact: true })
    await expect(length).toHaveAttribute('data-testid', 'draw-input')
    await expect(length).toHaveValue('25')
    await page.keyboard.press('Enter')
    await expect(dimensionLabel(page, '25')).toHaveCount(1)
    await leaveLineTool(page)
    await expectDof(page, 0)
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'full')
  })

  test('E-17 a conflicting constraint is refused and can replace what it conflicts with', async ({
    page,
  }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    await expectDof(page, 0)
    const box = await canvas(page)
    const { scale } = sketchToScreen(box, [0, 0], [40, 20])
    await runCommand(page, 'Equal')
    // The bottom line lies on the sketch's X axis and the left line on its Y axis.
    await clickAt(page, offset(box, 0, 10 * scale))
    await clickAt(page, offset(box, -20 * scale, 0))
    const warning = notification(
      page,
      'warning',
      'Equal conflicts with Distance 40 and Distance 20, so it was not added.',
    )
    await expect(warning).toBeVisible()
    await expectDof(page, 0)
    await expect(sketchDimensions(page)).toHaveCount(2)
    await warning.getByRole('button', { name: 'Replace them', exact: true }).click()
    await expect(constraintGlyphs(page, 'equal')).toHaveCount(1)
    await expect(sketchDimensions(page)).toHaveCount(0)
    await expectDof(page, 1)
  })

  test('E-18 a dimension the sketch already sets is offered as a reference', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromTimeline(page, 'Sketch 1')
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'full')
    const box = await canvas(page)
    const { scale } = sketchToScreen(box, [0, 0], [40, 20])
    const dimensionsBefore = await sketchDimensions(page).count()
    await page.keyboard.press('d')
    await clickAt(page, offset(box, 0, -10 * scale))
    await clickAt(page, offset(box, 0, -10 * scale - 30))
    const editor = page.getByTestId('dimension-editor')
    await expect(editor).toBeVisible()
    await page.keyboard.type('40')
    await page.keyboard.press('Enter')
    const alert = editor.getByRole('alert')
    await expect(alert).toContainText(
      'Other dimensions and constraints already set this size. Add it as a reference dimension that only shows the measurement?',
    )
    await alert.getByRole('button', { name: 'Add reference', exact: true }).click()
    await expect(sketchDimensions(page)).toHaveCount(dimensionsBefore + 1)
    const reference = page.locator('[data-testid=sketch-dimension][data-driving=false]')
    await expect(reference).toHaveCount(1)
    await expect(dimensionLabel(page, '(40)')).toHaveAttribute('data-driving', 'false')
  })

  test('E-19 two selected lines offer their constraints, and Perpendicular applies', async ({
    page,
  }) => {
    await openApp(page)
    await newSketchOnTopPlane(page)
    const box = await canvas(page)
    await drawLine(page, offset(box, -150, 60), offset(box, 150, 60), { free: true })
    await leaveLineTool(page)
    await drawLine(page, offset(box, -100, -60), offset(box, 100, -140), { free: true })
    await leaveLineTool(page)
    await expectDof(page, 8)

    await clickAt(page, offset(box, 0, 60))
    await page.keyboard.down('Shift')
    await clickAt(page, offset(box, 0, -100))
    await page.keyboard.up('Shift')
    const selection = inspector(page)
    await Promise.all(
      ['Parallel', 'Perpendicular', 'Collinear', 'Equal'].map((name) =>
        expect(selection.getByRole('button', { name: new RegExp(`^${name}`) })).toBeVisible(),
      ),
    )
    await selection.getByRole('button', { name: /^Perpendicular/ }).click()
    await expectDof(page, 7)
    await expect(constraintGlyphs(page, 'perpendicular')).toHaveCount(1)
  })

  test('E-20 dragging a free point moves it, and undo puts it back', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'underconstrained.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    await expectDof(page, 7)
    const box = await canvas(page)
    const mapping = sketchToScreen(box, [0, -5], [55, 5])
    const old = mapping.toPage([30, 0])
    const moved = mapping.toPage([30, 10])
    await expectHover(page, old.x, old.y, 'sketch_point')
    await dragBetween(page, old, moved, 10)
    await expectDof(page, 7)
    await expectHover(page, moved.x, moved.y, 'sketch_point')
    await expectNoHover(page, old, 'sketch_point')
    await page.keyboard.press('ControlOrMeta+z')
    await expectHover(page, old.x, old.y, 'sketch_point')
  })

  test('E-21 window and crossing box selection', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    const box = await canvas(page)
    const mapping = sketchToScreen(box, [0, 0], [40, 20])
    const topLeft = mapping.toPage([0, 20])
    const bottomRight = mapping.toPage([40, 0])
    await dragBetween(
      page,
      { x: topLeft.x - 30, y: topLeft.y - 30 },
      { x: bottomRight.x + 30, y: bottomRight.y + 30 },
    )
    await expect(page.getByTestId('selection-title')).toHaveText('4 lines')

    // Right to left across the middle of the bottom line only: a crossing selection.
    const bottomMiddle = mapping.toPage([20, 0])
    await dragBetween(
      page,
      { x: bottomMiddle.x + 20, y: bottomMiddle.y - 15 },
      { x: bottomMiddle.x - 20, y: bottomMiddle.y + 15 },
    )
    await expect(page.getByTestId('selection-title')).toHaveText('1 line')
  })

  test('E-22 Mod+Enter finishes the sketch, which stays visible', async ({ page }) => {
    await openApp(page)
    await newSketchOnTopPlane(page)
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    await expect(
      page.getByTestId('browser').getByRole('button', { name: 'Hide Sketch 1', exact: true }),
    ).toBeVisible()
  })
})

test.describe('sketch mode for everyone', () => {
  test('E-64 Sketch in the empty state picks a plane, and Esc goes back', async ({ page }) => {
    await openApp(page)
    const emptyState = page.getByTestId('empty-state')
    await emptyState.getByRole('button', { name: 'Sketch', exact: true }).click()
    await expect(app(page)).toHaveAttribute('data-context', 'sketch')
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'picking')
    await expect(emptyState).toHaveCount(0)
    await page.keyboard.press('Escape')
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    await expect(emptyState).toBeVisible()
  })

  test('E-70 step prompts lead through drawing a line, and Esc names the way out', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1024, height: 720 })
    await openApp(page)
    await newSketchOnTopPlane(page)
    await expect(
      page
        .getByRole('toolbar', { name: 'Sketch tools' })
        .getByRole('button', { name: 'Finish sketch', exact: true }),
    ).toHaveText('Finish sketch')
    await expect(sketchHint(page)).toHaveText(
      'Choose Line (L), Rectangle (R) or Circle (C) to start drawing.',
    )
    await page.keyboard.press('l')
    await expect(sketchHint(page)).toHaveText('Click to start a line.')
    const box = await canvas(page)
    await clickAt(page, offset(box, 0, 0))
    await page.mouse.move(box.cx + 150, box.cy, { steps: 4 })
    await expect(sketchHint(page)).toContainText('type a length')
    await expect(page.getByRole('textbox', { name: 'Length', exact: true })).toHaveAttribute(
      'placeholder',
      /^\d+(\.\d+)?$/,
    )
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await expect(sketchHint(page)).toHaveText(
      'To leave the sketch, click Finish sketch or press Ctrl+Enter.',
    )
    await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  })

  test('E-71 a draw input takes an expression with a parameter', async ({ page }) => {
    await openApp(page)
    await newSketchOnTopPlane(page)
    await runCommand(page, 'Parameters')
    const panel = page.getByTestId('parameters-panel')
    await panel.getByRole('button', { name: 'Add parameter', exact: true }).click()
    await page.keyboard.type('width')
    await page.keyboard.press('Enter')
    await page.keyboard.type('40 mm')
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('parameter-row')).toHaveCount(1)
    await expect(page.getByTestId('parameter-value')).toHaveText('40 mm')
    await page
      .getByTestId('side-panel')
      .getByRole('button', { name: /^Close/ })
      .click()
    await expect(panel).toHaveCount(0)

    const box = await canvas(page)
    await clickAt(page, offset(box, 0, 100))
    await page.keyboard.press('l')
    await clickAt(page, offset(box, 0, 0))
    await page.mouse.move(box.cx + 100, box.cy, { steps: 4 })
    await page.keyboard.press('Tab')
    await expect(page.getByRole('textbox', { name: 'Length', exact: true })).toBeFocused()
    await page.keyboard.type('width / 2')
    await page.keyboard.press('Enter')
    await expect(dimensionLabel(page, 'fx 20')).toHaveCount(1)
  })

  test('E-81 open ends show in the header and when finishing', async ({ page }) => {
    await openApp(page)
    await newSketchOnTopPlane(page)
    const box = await canvas(page)
    await page.keyboard.press('l')
    await clickAt(page, offset(box, 0, 0))
    await clickAt(page, offset(box, 100, 0))
    await clickAt(page, offset(box, 100, -80))
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await expect(sketchHeader(page)).toHaveAttribute('data-outline', '2')
    await expect(sketchHeader(page)).toContainText('2 open ends')
    await page.keyboard.press('ControlOrMeta+Enter')
    const warning = notification(
      page,
      'warning',
      'Sketch 1 has 2 open ends, so some shapes cannot be extruded.',
    )
    await expect(warning).toBeVisible()
    await expect(warning.getByRole('button', { name: 'Back to sketch', exact: true })).toBeVisible()
  })

  test('E-82 the header lists free geometry and selects it', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'underconstrained.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    await expect(sketchHeader(page)).toContainText('Not fully constrained · 7 DOF')
    await sketchHeader(page).click()
    const problems = page.getByTestId('sketch-problems')
    await expect(problems).toContainText('Free geometry: 1 line, 1 circle')
    await expect(problems).toContainText('you can extrude at any time')
    await problems.getByRole('button', { name: 'Select them', exact: true }).click()
    await expect(page.getByTestId('selection-title')).toHaveText('2 items')
  })

  test('E-86 plane picking labels the origin planes and starts on the one clicked', async ({
    page,
  }) => {
    await openApp(page)
    await clickTool(page, 'Sketch')
    const labels = page.getByTestId('plane-label')
    await expect(labels).toHaveText(['Top (XY)', 'Front (XZ)', 'Right (YZ)'])
    await expect(sketchHint(page)).toHaveText('Top plane (XY) is the print bed.')
    await page.getByRole('button', { name: 'Sketch on Front plane (XZ)', exact: true }).click()
    await expect(viewport(page)).toHaveAttribute('data-view', 'front')
    await expect(timelineItem(page, 'Sketch 1')).toBeVisible()
    await expect(sketchHeader(page)).toHaveAttribute('data-state', 'empty')
  })

  test('E-91 the size field of a selected line or circle adds its dimension', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'underconstrained.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    const box = await canvas(page)
    const mapping = sketchToScreen(box, [0, -5], [55, 5])
    await clickAt(page, mapping.toPage([15, 0]))
    await expect(page.getByTestId('selection-title')).toHaveText('1 line')
    const size = page.getByTestId('entity-size')
    await expect(size).toHaveAccessibleName('Length')
    await expect(size).toHaveValue('30')
    await size.fill('40')
    await size.press('Enter')
    await expect(dimensionLabel(page, '40')).toHaveCount(1)
    await expectDof(page, 6)

    await clickAt(page, mapping.toPage([55, 0]))
    await expect(page.getByTestId('selection-title')).toHaveText('1 circle')
    await expect(size).toHaveAccessibleName('Diameter')
    await expect(size).toHaveValue('10')
    await size.fill('12')
    await size.press('Enter')
    await expect(dimensionLabel(page, '⌀12')).toHaveCount(1)
    await expectDof(page, 5)
  })

  test('E-102 Esc never leaves a sketch being edited', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    for (let i = 0; i < 5; i++) {
      // Each press is one step of the Esc ladder, so they run in order.
      // oxlint-disable-next-line no-await-in-loop
      await page.keyboard.press('Escape')
    }
    await expect(app(page)).toHaveAttribute('data-context', 'sketch')
  })

  test('E-103 a constraint the sketch already implies is not added', async ({ page }) => {
    await openApp(page)
    await dropFile(page, 'rect-sketch.ptsm')
    await editSketchFromBrowser(page, 'Sketch 1')
    const box = await canvas(page)
    const { scale } = sketchToScreen(box, [0, 0], [40, 20])
    const horizontal = constraintGlyphs(page, 'horizontal')
    await expect(horizontal).toHaveCount(2)
    await clickAt(page, offset(box, 0, -10 * scale))
    await expect(page.getByTestId('selection-title')).toHaveText('1 line')
    await page.keyboard.press('h')
    await expect(
      notification(
        page,
        'info',
        'Horizontal is already implied by other constraints, so it was not added.',
      ),
    ).toBeVisible()
    await expect(horizontal).toHaveCount(2)
  })

  test('E-104 finishing turns a straight-on view to iso, and keeps a view the user turned', async ({
    page,
  }) => {
    await openApp(page)
    await page
      .getByTestId('empty-state')
      .getByRole('button', { name: 'Sketch', exact: true })
      .click()
    await page.getByRole('button', { name: 'Sketch on Top plane (XY)', exact: true }).click()
    await expect(viewport(page)).toHaveAttribute('data-view', 'top')
    const box = await canvas(page)
    await page.keyboard.press('r')
    await clickAt(page, offset(box, 0, 0))
    await clickAt(page, offset(box, 100, 60))
    await expect(sketchHeader(page)).toHaveAttribute('data-state', /^(under|full)$/)
    await page.keyboard.press('ControlOrMeta+Enter')
    await expect(viewport(page)).toHaveAttribute('data-view', 'iso')
    await expect(page.getByTestId('selection-title')).toHaveText('1 region')

    await editSketchFromTimeline(page, 'Sketch 1')
    await page.mouse.move(box.cx, box.cy)
    await page.mouse.down({ button: 'right' })
    await page.mouse.move(box.cx, box.cy - 100, { steps: 10 })
    await page.mouse.up({ button: 'right' })
    await expect(viewport(page)).toHaveAttribute('data-view', 'custom')
    await page
      .getByRole('toolbar', { name: 'Sketch tools' })
      .getByRole('button', { name: 'Finish sketch', exact: true })
      .click()
    await expect(app(page)).toHaveAttribute('data-context', 'model')
    await expect(viewport(page)).toHaveAttribute('data-view', 'custom')
  })
})
