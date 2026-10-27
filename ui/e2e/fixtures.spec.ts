/// <reference lib="dom" />
import { expect, test, type Page } from '@playwright/test'
import {
  bodyRow,
  dropFile,
  inspector,
  inspectorField,
  openApp,
  runCommand,
  timelineItem,
} from './helpers'
import { expectTimeline } from './support/documents'
import { fillInspector } from './support/modeling'
import { chooseContext, expectCurrentReport, healthItems, openPrintCheck } from './support/print'

// Fixture checks (ui.md 6.2, 6.5 E-108): each fixture with a stated result opens in the real core
// with that result, so a test that uses a fixture fails for its own reason, never for the
// fixture's. `notes.txt` is E-5; the perf fixtures are the inputs of 6.7 and state no result.

/** One fixture of ui.md 6.2 and the check of its Result row. */
type FixtureCheck = {
  readonly file: string
  readonly check: (page: Page) => Promise<void>
}

/** Selects a body by its browser row and expects its size and center. */
async function expectBody(page: Page, name: string, size: string, center: string): Promise<void> {
  await bodyRow(page, name).click()
  await expect(page.getByTestId('selection-title')).toHaveText(name)
  await expect(page.getByTestId('body-size')).toHaveAttribute('data-value', size)
  await expect(page.getByTestId('body-center')).toHaveAttribute('data-value', center)
}

/** Edits "Sketch 1" by double-clicking its timeline item and expects its degrees of freedom. */
async function editSketch(page: Page, dof: number): Promise<void> {
  await timelineItem(page, 'Sketch 1')
    .getByRole('button', { name: 'Sketch 1', exact: true })
    .dblclick()
  await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
  await expect(page.getByTestId('sketch-header')).toHaveAttribute('data-dof', String(dof))
}

/** Finishes the sketch being edited and expects its one region to be selected (3.16.11). */
async function finishWithOneRegion(page: Page): Promise<void> {
  await page.keyboard.press('ControlOrMeta+Enter')
  await expect(page.getByTestId('app')).toHaveAttribute('data-context', 'model')
  await expect(page.getByTestId('selection-title')).toHaveText('1 region')
}

const FIXTURES: readonly FixtureCheck[] = [
  {
    file: 'cube.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1', 'Extrude 1'])
      await expectBody(page, 'Cube', '20 × 20 × 20 mm', '0, 0, 10 mm')
    },
  },
  {
    file: 'rect-sketch.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1'])
      await editSketch(page, 0)
      await expect(page.getByTestId('sketch-header')).toHaveAttribute('data-state', 'full')
      await finishWithOneRegion(page)
      // An extrude of the region shows where it lies: x in [0, 40], y in [0, 20].
      await page.keyboard.press('e')
      await expect(inspector(page)).toHaveAccessibleName('Extrude')
      await expect(inspectorField(page, 'profile').getByTestId('selection-field')).toHaveAttribute(
        'data-count',
        '1',
      )
      await fillInspector(page, 'Distance', '10')
      await page.keyboard.press('Enter')
      await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok', {
        timeout: 15_000,
      })
      await expectBody(page, 'Body 1', '40 × 20 × 10 mm', '20, 10, 5 mm')
    },
  },
  {
    file: 'underconstrained.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1'])
      await editSketch(page, 7)
    },
  },
  {
    file: 'parametric.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1', 'Extrude 1'])
      await expectBody(page, 'Cube', '20 × 20 × 20 mm', '0, 0, 10 mm')
      await runCommand(page, 'Parameters')
      const size = page.locator('[data-testid=parameter-row][data-id=p-size]')
      await expect(size.getByRole('textbox', { name: 'Name of size', exact: true })).toHaveValue(
        'size',
      )
      await expect(size.getByTestId('parameter-value')).toHaveText('20 mm')
    },
  },
  {
    file: 'cube-features.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1', 'Extrude 1', 'Fillet 1', 'Shell 1'])
    },
  },
  {
    file: 'two-bodies.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1', 'Extrude 1', 'Cylinder 1'])
    },
  },
  {
    file: 'fillet-error.ptsm',
    check: async (page) => {
      await expect(page.getByTestId('timeline-item')).toHaveCount(3)
      await expect(timelineItem(page, 'Extrude 1')).toHaveAttribute('data-status', 'ok')
      const fillet = timelineItem(page, 'Fillet 1')
      await expect(fillet).toHaveAttribute('data-status', 'error')
      await fillet.getByRole('button', { name: 'Fillet 1', exact: true }).dblclick()
      await expect(inspector(page)).toHaveAccessibleName('Fillet 1')
      // `fillet_too_large` on `/radius`: its message shows under the radius field, and its
      // suggestion is the banner's "Use" action (3.7, 4.2).
      await expect(inspectorField(page, 'radius').getByTestId('field-error')).toContainText(
        'Fillet radius 15 mm is too large',
      )
      await expect(
        inspector(page).getByTestId('inspector-error').getByRole('button', { name: /^Use / }),
      ).toBeVisible()
    },
  },
  {
    file: 'hole-cube.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1', 'Extrude 1', 'Sketch 2'])
    },
  },
  {
    file: 'revolve-sketch.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1'])
      await expect(
        page.getByTestId('browser').getByRole('button', { name: 'Hide Sketch 1', exact: true }),
      ).toBeVisible()
      await editSketch(page, 0)
      await finishWithOneRegion(page)
    },
  },
  {
    file: 'offset-cube.ptsm',
    check: async (page) => {
      await expectTimeline(page, ['Sketch 1', 'Extrude 1'])
      await expectBody(page, 'Cube', '20 × 20 × 20 mm', '20, 0, 10 mm')
    },
  },
  {
    file: 'overhang.ptsm',
    check: async (page) => {
      await chooseContext(page, 'Print')
      await expectCurrentReport(page)
      await openPrintCheck(page)
      await expect(healthItems(page, 'overhangs')).not.toHaveCount(0)
    },
  },
  {
    file: 'oversize.ptsm',
    check: async (page) => {
      await chooseContext(page, 'Print')
      await expectCurrentReport(page)
      await openPrintCheck(page)
      await expect(
        healthItems(page, 'bed_fit').and(page.locator('[data-severity=error]')),
      ).not.toHaveCount(0)
    },
  },
  {
    file: 'hollow-resin.ptsm',
    check: async (page) => {
      await chooseContext(page, 'Print')
      await expectCurrentReport(page)
      await openPrintCheck(page)
      await expect(
        healthItems(page, 'trapped_volumes').and(page.locator('[data-severity=warning]')),
      ).not.toHaveCount(0)
    },
  },
]

test.describe('E-108 fixtures', () => {
  for (const fixture of FIXTURES) {
    test(`E-108 ${fixture.file} opens with its stated result`, async ({ page }) => {
      await openApp(page)
      await dropFile(page, fixture.file)
      await fixture.check(page)
    })
  }
})
