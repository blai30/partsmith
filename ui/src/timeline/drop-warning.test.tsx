import { act, cleanup, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Use } from '../generated/protocol'
import { documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { TIMING_NAMES } from '../performance/marks'
import { deferred } from '../test/fakes/deferred'
import { settle } from '../test/fakes/settle'
import { DROP_DOCUMENT, DROP_USES } from '../test/fakes/uses'
import { COLOR_CLASSES } from '../theme/roles'
import {
  movePointer,
  pressItem,
  releasePointer,
  stubItemRectangles,
  timelineItem,
} from './strip.testing'

// The timeline's drop warning (ui.md 3.9, U-135): while a dragged feature's nearest gap would put
// it above what it uses, or below what uses it, the insertion bar turns `warningMark` and a chip
// says what would break; dropping there anyway is allowed. Each pointer move of the drag records
// `ps:drop-check` (2.18).

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
  performance.clearMeasures(TIMING_NAMES.dropCheck)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

// The items of 'Extrude 1', 'Extrude 2' and 'Fillet 1' as `stubItemRectangles` lays them out: gap
// g lies at x = 98 + 44 g, and item i's center at x = 120 + 44 i.
const GAP_X = [98, 142, 186, 230] as const
const CENTER_X = [120, 164, 208] as const

function renderDropTimeline(
  listUses: () => Promise<readonly Use[]> = () => Promise.resolve(DROP_USES),
) {
  stubItemRectangles(DROP_DOCUMENT)
  const test = createModelApp({ state: readyState(documentState(DROP_DOCUMENT)) })
  test.core.stub('listUses', listUses)
  renderWithApp(test.app)
  return test
}

/** Presses an item at `x` and moves 10 pixels left, which starts the drag (3.9). */
function startDrag(from: HTMLElement, x: number): void {
  pressItem(from, x)
  movePointer(from, x - 10)
}

/** Lets the drag's `list_uses` answer arrive. */
async function usesArrive(): Promise<void> {
  await act(async () => {
    await settle()
  })
}

/** The insertion bar, positioned at the gap under the pointer (3.9, 3.12). */
function insertionBar(): HTMLElement {
  const bar = screen.getByTestId('insertion-bar')
  expect(bar).toHaveAttribute('data-positioned')
  return bar
}

function hasClasses(element: HTMLElement, classes: string): boolean {
  return classes.split(' ').every((name) => element.classList.contains(name))
}

/** The text of the drop hint, empty while there is none. */
function dropHintText(): string {
  return screen.queryByTestId('drop-hint')?.textContent ?? ''
}

const BELOW_HINT = "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?"
const ABOVE_HINT = "'Fillet 1' uses an edge made by 'Extrude 2'. Drop above 'Fillet 1' instead?"

describe('the drop warning (3.9, U-135)', () => {
  it("U-135 dragging 'Fillet 1' above 'Extrude 2' turns the bar warningMark and shows the hint", async () => {
    renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    movePointer(fillet, GAP_X[1])
    const bar = insertionBar()
    expect(hasClasses(bar, COLOR_CLASSES.warningMark.background)).toBe(true)
    expect(hasClasses(bar, COLOR_CLASSES.accent.background)).toBe(false)
    const hint = screen.getByTestId('drop-hint')
    expect(hint).toHaveTextContent(BELOW_HINT)
    expect(hint).toHaveAttribute('role', 'status')
    expect(hint).toHaveAttribute('data-positioned')
  })

  it('U-135 over its own gaps, nothing warns: the bar stays accent and there is no hint', async () => {
    renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    for (const gap of [GAP_X[3], GAP_X[2]]) {
      movePointer(fillet, gap)
      expect(hasClasses(insertionBar(), COLOR_CLASSES.accent.background)).toBe(true)
      expect(hasClasses(insertionBar(), COLOR_CLASSES.warningMark.background)).toBe(false)
      expect(dropHintText()).toBe('')
    }
  })

  it('U-135 the warning follows the pointer: it goes when the nearest gap breaks nothing', async () => {
    renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    movePointer(fillet, GAP_X[1])
    expect(dropHintText()).toBe(BELOW_HINT)
    movePointer(fillet, GAP_X[3])
    expect(dropHintText()).toBe('')
    expect(hasClasses(insertionBar(), COLOR_CLASSES.accent.background)).toBe(true)
  })

  it("U-135 dragging 'Extrude 2' to the end asks to drop above 'Fillet 1'", async () => {
    renderDropTimeline()
    const extrude = timelineItem('f-ext2')
    startDrag(extrude, CENTER_X[1])
    await usesArrive()
    movePointer(extrude, GAP_X[3])
    expect(screen.getByTestId('drop-hint')).toHaveTextContent(ABOVE_HINT)
    expect(hasClasses(insertionBar(), COLOR_CLASSES.warningMark.background)).toBe(true)
  })

  it('U-135 dropping at a warned gap sends one move_feature labeled "Reorder", with no confirmation', async () => {
    const { core } = renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    movePointer(fillet, GAP_X[1])
    expect(dropHintText()).toBe(BELOW_HINT)
    releasePointer(fillet, GAP_X[1])
    await usesArrive()
    expect(core.callsTo('applyEdits')).toEqual([
      [1, 'Reorder', [{ type: 'move_feature', id: 'f-fil1', index: 1 }]],
    ])
    expect(screen.queryByRole('alertdialog')).toBeNull()
    expect(dropHintText()).toBe('')
  })

  it('U-135 asks for the uses once for the revision of the drag', async () => {
    const { core } = renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    releasePointer(fillet, CENTER_X[2] - 10)
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    releasePointer(fillet, CENTER_X[2] - 10)
    expect(core.callsTo('listUses')).toEqual([[1]])
  })

  it('U-135 no gap warns while the uses have not arrived', async () => {
    const answer = deferred<readonly Use[]>()
    renderDropTimeline(() => answer.promise)
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    movePointer(fillet, GAP_X[1])
    movePointer(fillet, GAP_X[0])
    expect(dropHintText()).toBe('')
    expect(hasClasses(insertionBar(), COLOR_CLASSES.accent.background)).toBe(true)
    answer.resolve(DROP_USES)
    await usesArrive()
    releasePointer(fillet, GAP_X[3])
  })

  it('U-135 uses that arrive during the drag warn at the gap under the pointer without another move', async () => {
    const answer = deferred<readonly Use[]>()
    renderDropTimeline(() => answer.promise)
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    movePointer(fillet, GAP_X[0])
    answer.resolve(DROP_USES)
    await usesArrive()
    expect(dropHintText()).toBe(BELOW_HINT)
    expect(hasClasses(insertionBar(), COLOR_CLASSES.warningMark.background)).toBe(true)
    releasePointer(fillet, GAP_X[3])
  })
})

/** How many `ps:drop-check` measures there are. */
function dropChecks(): number {
  return performance.getEntriesByName(TIMING_NAMES.dropCheck, 'measure').length
}

describe('ps:drop-check (2.18)', () => {
  it('records one measure for each pointer move while a feature is dragged', async () => {
    renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    startDrag(fillet, CENTER_X[2])
    await usesArrive()
    const before = dropChecks()
    movePointer(fillet, GAP_X[2])
    movePointer(fillet, GAP_X[1])
    movePointer(fillet, GAP_X[0])
    expect(dropChecks()).toBe(before + 3)
  })

  it('records none for pointer moves without a drag', () => {
    renderDropTimeline()
    const fillet = timelineItem('f-fil1')
    movePointer(fillet, GAP_X[1])
    pressItem(fillet, CENTER_X[2])
    // Within the drag threshold of 4 pixels, the press is still a click.
    movePointer(fillet, CENTER_X[2] - 3)
    releasePointer(fillet, CENTER_X[2] - 3)
    expect(dropChecks()).toBe(0)
  })
})
