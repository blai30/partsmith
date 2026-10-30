/// <reference lib="dom" />
import { expect, type Locator, type Page } from '@playwright/test'
import {
  bodyRow,
  dropFile,
  inspector,
  inspectorInput,
  openApp,
  runCommand,
  test,
  timelineItem,
} from './helpers'
import { openProblems } from './support/documents'
import { editFeatureFromTimeline, focusViewport } from './support/modeling'
import { chooseContext, expectCurrentReport, openPrintCheck } from './support/print'
import { editSketchFromTimeline } from './support/sketch'

// Keyboard and target size (ui.md 6.5 E-162 and E-163, 3.24, ADR 0049): the tab order and focus
// returns of 3.24 and the 24 by 24 pixel minimum of every target, measured in the browser, since
// jsdom has neither the tab order across the shell's regions nor layout.

/** The regions that hold a stop or a target, in the tab order of 3.24. */
const REGIONS = [
  'top-bar',
  'browser',
  'viewport',
  'top-center-slot',
  'inspector',
  'timeline',
  'side-panel',
  'status-bar',
  'notification',
] as const

/** The controls that 3.24's target size rule covers. */
const TARGETS = [
  'button',
  'input',
  'select',
  'textarea',
  '[role=button]',
  '[role=slider]',
  '[role=treeitem]',
  '[role=option]',
  '[role=menuitem]',
  '[role=menuitemradio]',
  '[role=menuitemcheckbox]',
].join(', ')

/**
 * The marks that keep the sizes their sections give them (3.24): the view cube's cells (every
 * button of `view-cube` but Home view), the sketch's constraint glyphs and dimension labels, the
 * repair candidate markers and the timeline's rollback marker.
 */
const MARKS = [
  '[data-testid=view-cube] button:not([aria-label="Home view"])',
  '[data-testid=constraint-glyph]',
  '[data-testid=sketch-dimension]',
  '[data-testid=repair-candidate]',
  '[data-testid=timeline-marker]',
].join(', ')

/** The smallest width and height of a target, in CSS pixels at UI scale 1 (3.24). */
const MINIMUM_TARGET_PIXELS = 24

/** A run that reads fewer targets than this measured the wrong page. */
const MINIMUM_TARGET_COUNT = 30

/** The most key presses a walk takes before it gives up on its last stop. */
const MAXIMUM_PRESSES = 60

/** What the probes read of an element: the region that holds it, its test id and its name. */
type Described = {
  readonly region: string
  readonly testId: string | null
  readonly name: string
}

/** A target under the minimum, as E-163 reports it. */
type SmallTarget = Described & {
  readonly tag: string
  readonly width: number
  readonly height: number
}

/** What one run of E-163's check read. */
type TargetRun = {
  readonly count: number
  readonly small: readonly SmallTarget[]
}

/** The probes the init script installs in the page, which both tests read through. */
type Probes = {
  readonly describe: (element: Element) => Described
}

/**
 * Installs `window.partsmithProbes` before the app loads. An element's region is the nearest of
 * `regions` that holds it or is it (`none` outside them); its name is its accessible name, read as
 * the text of its `aria-labelledby` elements, else its `aria-label`, else the text of a form
 * control's labels, else its own text, leaving out what is `aria-hidden`.
 */
function installProbes(regions: readonly string[]): void {
  const regionSelector = regions.map((region) => `[data-testid="${region}"]`).join(', ')
  // The init script runs in the page, so its helpers live inside it.
  // oxlint-disable-next-line unicorn/consistent-function-scoping
  const shownText = (element: Element): string => {
    const copy = element.cloneNode(true)
    if (!(copy instanceof Element)) {
      return ''
    }
    for (const hidden of copy.querySelectorAll('[aria-hidden="true"]')) {
      hidden.remove()
    }
    return copy.textContent ?? ''
  }
  const accessibleName = (element: Element): string => {
    const labelledBy = (element.getAttribute('aria-labelledby') ?? '')
      .split(/\s+/)
      .filter((id) => id !== '')
    if (labelledBy.length > 0) {
      return labelledBy.map((id) => document.getElementById(id)?.textContent ?? '').join(' ')
    }
    const label = element.getAttribute('aria-label') ?? ''
    if (label.trim() !== '') {
      return label
    }
    const labels =
      element instanceof HTMLInputElement ||
      element instanceof HTMLSelectElement ||
      element instanceof HTMLTextAreaElement
        ? [...(element.labels ?? [])]
        : []
    return labels.length > 0 ? labels.map(shownText).join(' ') : shownText(element)
  }
  const probes: Probes = {
    describe: (element) => ({
      region: element.closest(regionSelector)?.getAttribute('data-testid') ?? 'none',
      testId: element.getAttribute('data-testid'),
      name: accessibleName(element).replace(/\s+/g, ' ').trim(),
    }),
  }
  Reflect.set(window, 'partsmithProbes', probes)
}

/** Opens the app with the probes installed. */
async function openWithProbes(page: Page): Promise<void> {
  await page.addInitScript(installProbes, REGIONS)
  await openApp(page)
}

/**
 * The stop that has focus, as E-162 names it: `{region}: {name}`, where the name is the element's
 * `data-testid` when it has one and its accessible name otherwise, and `none: document.body` when
 * focus has dropped to the page.
 */
function focusedStop(page: Page): Promise<string> {
  return page.evaluate(() => {
    const focused = document.activeElement
    if (focused === null || focused === document.body) {
      return 'none: document.body'
    }
    const probes = Reflect.get(window, 'partsmithProbes') as Probes
    const { region, testId, name } = probes.describe(focused)
    return `${region}: ${testId ?? name}`
  })
}

/**
 * Presses `key` until `isLast` holds for the focused stop, and returns every stop after the first
 * press, the last one included.
 */
async function walk(
  page: Page,
  key: 'Tab' | 'Shift+Tab',
  isLast: (stop: string) => boolean,
): Promise<string[]> {
  const stops: string[] = []
  for (let presses = 0; presses < MAXIMUM_PRESSES; presses++) {
    // Each press starts from where the one before left focus, so the presses are sequential.
    // oxlint-disable-next-line no-await-in-loop
    await page.keyboard.press(key)
    // oxlint-disable-next-line no-await-in-loop
    const stop = await focusedStop(page)
    stops.push(stop)
    if (isLast(stop)) {
      break
    }
  }
  return stops
}

/**
 * The stops of E-162 (a) on `cube.ptsm`, with the body "Cube" selected, the Parameters panel open
 * and the error notification of `notes.txt`, from "Toggle browser" to the notification's "Dismiss".
 */
const REGION_STOPS = [
  'top-bar: Toggle browser',
  'top-bar: document-name',
  'top-bar: Model',
  'top-bar: Sketch',
  'top-bar: palette-button',
  'top-bar: Export',
  'top-bar: theme-toggle',
  'browser: browser-row',
  'viewport: 3D view',
  'inspector: Clear selection',
  'inspector: Color of Cube',
  'inspector: Material',
  'inspector: Visible',
  'inspector: Move body',
  'inspector: Mirror',
  'inspector: Split body',
  'timeline: Collapse timeline',
  'timeline: Sketch 1',
  'side-panel: Close Parameters',
  'side-panel: Add parameter',
  'status-bar: status-units',
  'status-bar: status-printer',
  'status-bar: health-badge',
  'notification: Dismiss',
] as const

/** The stops of E-162 (c) after the 3D view in sketch mode on "Sketch 1" (3.16.2). */
const SKETCH_STOPS = [
  'viewport: constraint-glyph',
  'viewport: constraint-glyph',
  'viewport: constraint-glyph',
  'viewport: constraint-glyph',
  'viewport: sketch-dimension',
  'viewport: sketch-dimension',
  'viewport: sketch-dimension',
  'viewport: sketch-dimension',
  'top-center-slot: sketch-header',
  'timeline: Collapse timeline',
] as const

/** The 3D view's focusable surface (3.13). */
function view(page: Page): Locator {
  return page.getByRole('application', { name: '3D view', exact: true })
}

/** A timeline item's button, which holds the strip's one stop (3.9). */
function timelineButton(page: Page, label: string): Locator {
  return timelineItem(page, label).getByRole('button', { name: label, exact: true })
}

/**
 * Measures every rendered target of 3.24 that is not a mark by its bounding client rect, a form
 * control with labels by the smallest box that holds its rect and its labels' rects, and returns
 * how many it read and the ones under the minimum.
 */
function measureTargets(page: Page): Promise<TargetRun> {
  return page.evaluate(
    ({ targets, marks, minimum }) => {
      const probes = Reflect.get(window, 'partsmithProbes') as Probes
      const rendered = [...document.querySelectorAll(targets)].filter((element) => {
        const rect = element.getBoundingClientRect()
        return (
          element.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true }) &&
          rect.width > 0 &&
          rect.height > 0 &&
          !element.matches(marks)
        )
      })
      // The function runs in the page, so its helpers live inside it.
      // oxlint-disable-next-line unicorn/consistent-function-scoping
      const rounded = (pixels: number): number => Math.round(pixels * 100) / 100
      const small = rendered.flatMap((element) => {
        const boxes = [element.getBoundingClientRect()]
        if (
          element instanceof HTMLInputElement ||
          element instanceof HTMLSelectElement ||
          element instanceof HTMLTextAreaElement
        ) {
          boxes.push(...[...(element.labels ?? [])].map((label) => label.getBoundingClientRect()))
        }
        const width = rounded(
          Math.max(...boxes.map((box) => box.right)) - Math.min(...boxes.map((box) => box.left)),
        )
        const height = rounded(
          Math.max(...boxes.map((box) => box.bottom)) - Math.min(...boxes.map((box) => box.top)),
        )
        if (width >= minimum && height >= minimum) {
          return []
        }
        return [{ ...probes.describe(element), tag: element.tagName.toLowerCase(), width, height }]
      })
      return { count: rendered.length, small }
    },
    { targets: TARGETS, marks: MARKS, minimum: MINIMUM_TARGET_PIXELS },
  )
}

/** E-163's check in the state the page is in; `state` names the run in its failures. */
async function expectTargetsLargeEnough(page: Page, state: string): Promise<void> {
  const { count, small } = await measureTargets(page)
  expect(count, `${state}: the number of targets read`).toBeGreaterThanOrEqual(MINIMUM_TARGET_COUNT)
  const described = small.map(
    (target) =>
      `${target.tag} "${target.name}" in ${target.region}: ${String(target.width)} by ${String(target.height)}`,
  )
  expect(described, `${state}: the targets under 24 by 24 pixels`).toEqual([])
}

/**
 * Opens `popup` with `open`, measures the targets while it shows (E-163), then closes it with
 * Escape and waits until it is hidden.
 */
async function expectPopupTargetsLargeEnough(
  page: Page,
  popup: Locator,
  state: string,
  open: () => Promise<void>,
): Promise<void> {
  await open()
  await expect(popup).toBeVisible()
  await expectTargetsLargeEnough(page, state)
  await page.keyboard.press('Escape')
  await expect(popup).toBeHidden()
}

test.describe('keyboard and target size (3.24)', () => {
  test('E-162 tab order and focus return on cube.ptsm', async ({ page }) => {
    await openWithProbes(page)
    await dropFile(page, 'cube.ptsm')
    // Its error notification stays until it is dismissed (3.22).
    await dropFile(page, 'notes.txt')
    await bodyRow(page, 'Cube').click()
    await expect(inspector(page)).toHaveAttribute('data-mode', 'selection')
    await runCommand(page, 'Parameters')
    await expect(page.getByTestId('side-panel')).toBeVisible()
    // The palette gives focus back to the row (3.4).
    await expect(bodyRow(page, 'Cube')).toBeFocused()

    // (a) The regions in order, forward and back.
    await page.getByRole('button', { name: 'Toggle browser', exact: true }).focus()
    const forward = await walk(page, 'Tab', (stop) => stop === REGION_STOPS.at(-1))
    expect(forward, 'the stops after Toggle browser, pressing Tab').toEqual(REGION_STOPS.slice(1))
    const backward = await walk(page, 'Shift+Tab', (stop) => stop === REGION_STOPS[0])
    expect(backward, 'the stops after Dismiss, pressing Shift+Tab').toEqual(
      REGION_STOPS.slice(0, -1).toReversed(),
    )

    // (b) Focus return. A side panel closed from inside gives focus back to its opener.
    await page.getByRole('button', { name: 'Close Parameters', exact: true }).focus()
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('side-panel')).toBeHidden()
    await expect(bodyRow(page, 'Cube')).toBeFocused()

    // A notification that goes away with focus inside gives it to the 3D view.
    await page
      .getByTestId('notification')
      .getByRole('button', { name: 'Dismiss', exact: true })
      .focus()
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('notification')).toHaveCount(0)
    await expect(view(page)).toBeFocused()

    // A command's inspector gives focus back to the control that had it.
    await timelineButton(page, 'Sketch 1').focus()
    await page.keyboard.press('ArrowRight')
    await expect(timelineButton(page, 'Extrude 1')).toBeFocused()
    await page.keyboard.press('Enter')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await expect(inspectorInput(page, 'Distance')).toBeFocused()
    await page.keyboard.press('Escape')
    // The body "Cube" is still selected.
    await expect(inspector(page)).toHaveAttribute('data-mode', 'selection')
    await expect(timelineButton(page, 'Extrude 1')).toBeFocused()

    // When that control is gone, to the 3D view: a suggestion of the selection inspector goes with
    // the selection, which the command takes (3.7 step 4).
    await inspector(page).getByRole('button', { name: 'Move body', exact: true }).focus()
    await page.keyboard.press('Enter')
    await expect(inspector(page)).toHaveAttribute('data-mode', 'command')
    await page.keyboard.press('Escape')
    await expect(inspector(page)).toBeHidden()
    await expect(view(page)).toBeFocused()

    // (c) In sketch mode the overlay's glyphs and labels follow the 3D view, then the slot.
    await editSketchFromTimeline(page, 'Sketch 1')
    await view(page).focus()
    const sketchStops = await walk(page, 'Tab', (stop) => stop.startsWith('timeline: '))
    expect(sketchStops, 'the stops after the 3D view in sketch mode').toEqual(SKETCH_STOPS)
  })

  test('E-163 every target is at least 24 by 24 CSS pixels at scale 1', async ({ page }) => {
    // Fifteen runs over three documents, two print checks and a sketch.
    test.slow()
    await openWithProbes(page)
    // The empty document, with its empty-state card.
    await expectTargetsLargeEnough(page, 'the empty document')

    await dropFile(page, 'cube.ptsm')
    await bodyRow(page, 'Cube').click()
    await runCommand(page, 'Parameters')
    await page.getByRole('button', { name: 'Add parameter', exact: true }).click()
    await dropFile(page, 'notes.txt')
    await expectTargetsLargeEnough(page, 'a selection, a draft parameter and a notification')
    await page.getByRole('button', { name: 'Close Parameters', exact: true }).click()
    await expect(page.getByTestId('side-panel')).toBeHidden()

    const popups: [string, Locator, () => Promise<void>][] = [
      [
        'the Document menu',
        page.getByTestId('document-menu'),
        () => page.getByTestId('document-name').click(),
      ],
      [
        'the export popover',
        page.getByTestId('export-popover'),
        () =>
          page.getByTestId('top-bar').getByRole('button', { name: 'Export', exact: true }).click(),
      ],
      ['the palette', page.getByTestId('palette'), () => page.keyboard.press('ControlOrMeta+K')],
      [
        'the color popover',
        page.getByTestId('color-popover'),
        () =>
          page
            .getByTestId('browser')
            .getByRole('button', { name: 'Color of Cube', exact: true })
            .click(),
      ],
      [
        'the Units menu',
        page.getByRole('menu', { name: 'Units', exact: true }),
        () => page.getByTestId('status-units').click(),
      ],
      [
        'the printer menu',
        page.getByTestId('printer-menu'),
        () => page.getByTestId('status-printer').click(),
      ],
      [
        'the timeline item menu',
        page.getByTestId('context-menu'),
        async () => {
          await timelineButton(page, 'Extrude 1').focus()
          await page.keyboard.press('Shift+F10')
        },
      ],
      [
        'the viewport menu',
        page.getByTestId('viewport-menu'),
        async () => {
          const free = await page.getByTestId('viewport').boundingBox()
          if (free === null) {
            throw new Error('The viewport is not visible.')
          }
          await page.mouse.click(free.x + free.width / 2, free.y + free.height / 2, {
            button: 'right',
          })
        },
      ],
      [
        'the section popover',
        page.getByTestId('section-popover'),
        async () => {
          await focusViewport(page)
          await page.keyboard.press('Shift+X')
        },
      ],
    ]
    for (const [state, popup, open] of popups) {
      // One popup at a time: each opens once the one before it has closed.
      // oxlint-disable-next-line no-await-in-loop
      await expectPopupTargetsLargeEnough(page, popup, state, open)
    }
    // The section stays on once its popover closes; Shift+X turns it off again (3.15).
    await focusViewport(page)
    await page.keyboard.press('Shift+X')
    await expect(page.getByTestId('viewport')).toHaveAttribute('data-section', 'off')

    await editFeatureFromTimeline(page, 'Extrude 1')
    await page.getByTestId('inspector-more').click()
    await expect(page.getByTestId('inspector-more')).toHaveAttribute('aria-expanded', 'true')
    await expectTargetsLargeEnough(page, 'the Extrude inspector with More shown')
    await page.keyboard.press('Escape')
    await expect(inspector(page)).not.toHaveAttribute('data-mode', 'command')

    await editSketchFromTimeline(page, 'Sketch 1')
    await expectTargetsLargeEnough(page, 'sketch context')
    await page.keyboard.press('ControlOrMeta+Enter')
    await chooseContext(page, 'Print')
    await expectCurrentReport(page)
    await openPrintCheck(page)
    await expectTargetsLargeEnough(page, 'print context with the print check')

    await chooseContext(page, 'Model')
    await dropFile(page, 'fillet-error.ptsm')
    await editFeatureFromTimeline(page, 'Fillet 1')
    await expect(page.getByTestId('inspector-error')).toBeVisible()
    await openProblems(page)
    await expectTargetsLargeEnough(page, 'the Problems panel and the error banner of Fillet 1')
  })
})
