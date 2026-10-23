import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { editFeature } from '../app/actions/command'
import { clearSelection, select } from '../app/actions/selection'
import type { App } from '../app/createApp'
import { installSelectionUpkeep } from '../app/selection-upkeep'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type {
  Cause,
  Diagnostic,
  FeatureStatus,
  FixAction,
  FixRef,
  SketchSolution,
} from '../generated/protocol'
import { documentWithStatuses, readyState } from '../layout/fixtures.testing'
import {
  createModelApp,
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import { INITIAL_STATE, type AppState, type DocumentState } from '../state/store'
import type { FakeBackend } from '../test/fakes/backend'
import type { FakeCoreApi } from '../test/fakes/core-api'
import { CUBE_FEATURES_DOCUMENT, CUBE_SKETCH_SOLUTION } from '../test/fakes/documents'
import { featureItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { meshSummary } from '../test/fakes/state'
import { COLOR_CLASSES } from '../theme/roles'
import { worldToCanvas } from '../viewport/projection.testing'
import { pathNumbers } from './overlay.testing'

// The cause line of a message (ui.md 4.1, 4.2, 4.3, U-131): under the message and its actions, in
// `textMuted`, followed by the cause's own actions; a `show` of the cause selects what changed,
// frames it, and draws the cause's ghost, the entity as it was before the edit, until the
// selection changes.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
const created = vi.hoisted(() => ({ backends: [] as FakeBackend[] }))

vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return {
    createWebGl2Backend: () => {
      const backend = fakes.createFakeBackend()
      created.backends.push(backend)
      return backend
    },
  }
})

const WIDTH = 800
const HEIGHT = 600

let restoreSize: () => void
let frames: FrameQueue
let stopUpkeep: () => void = () => undefined

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: WIDTH, height: HEIGHT })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
  created.backends.length = 0
})

afterEach(() => {
  stopUpkeep()
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** A FixRef of the message of 'Fillet 1' at revision 4. */
function fixRef(cause: boolean, index: number): FixRef {
  return {
    feature: 'f-fil1',
    code: 'reference_missing',
    field: '/edges/0',
    cause,
    index,
    revision: 4,
  }
}

/** A: uses the edge that replaced the lost one. */
const USE_RIM: FixAction = {
  label: "Use the top rim of the cut made by 'Extrude 2'",
  run: { kind: 'apply', fix: fixRef(false, 0) },
}

/** B: picks an edge again. */
const PICK_AGAIN: FixAction = {
  label: 'Pick an edge again',
  run: { kind: 'pick', feature: 'f-fil1', path: '/edges/0' },
}

/** C: sets the distance back, an action of the cause line. */
const SET_BACK: FixAction = {
  label: "Set Distance of 'Extrude 2' back to 5 mm",
  run: { kind: 'apply', fix: fixRef(true, 0) },
}

/** D: undoes the edit. */
const UNDO: FixAction = { label: 'Undo', run: { kind: 'undo' } }

const DISTANCE_CHANGED: Cause = {
  text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.",
  actions: [SET_BACK, UNDO],
}

const LOST_RIM: Diagnostic = {
  severity: 'warning',
  code: 'reference_missing',
  message:
    "The top rim of 'Extrude 2' selected for Edges is gone, so it was skipped. Pick an edge again, or use the rim of the cut.",
  field: '/edges/0',
  actions: [USE_RIM, PICK_AGAIN],
  cause: DISTANCE_CHANGED,
}

function filletStatus(message: Diagnostic): FeatureStatus {
  return { id: 'f-fil1', status: 'warning', messages: [message], repairs: [] }
}

/** The line from (80, 40) to (70, 50) of 'Sketch 1' as it was before it was deleted, in buffer 1. */
const LINE_POINTS: readonly number[] = [80, 40, 70, 50]

/** A repair's ghost in world points, an edge from (0, 0, 20) to (20, 0, 20). */
const REPAIR_POINTS: readonly number[] = [0, 0, 20, 20, 0, 20]

/** "Show the line", a `show` of the deleted entity e-5 of 'Sketch 1'. */
const SHOW_LINE: FixAction = {
  label: 'Show the line',
  run: { kind: 'show', feature: 'f-sk1', entity: 'e-5' },
}

/** The cause of a deleted line, whose ghost is the line before the edit in sketch coordinates. */
const LINE_DELETED: Cause = {
  text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
  actions: [UNDO, SHOW_LINE],
  ghost: { positions: 1, kind: 'lines', frame: 'sketch' },
}

const LOST_BEVEL: Diagnostic = {
  severity: 'warning',
  code: 'reference_missing',
  message:
    "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.",
  field: '/edges/0',
  actions: [PICK_AGAIN],
  cause: LINE_DELETED,
}

/** cube-features.ptsm after the edit, with 'Fillet 1' in `status`, the cube's mesh and the line's ghost. */
function lostDocument(
  status: FeatureStatus,
  solution: SketchSolution = CUBE_SKETCH_SOLUTION,
): DocumentState {
  return documentWithStatuses(CUBE_FEATURES_DOCUMENT, [status], {
    name: 'cube-features',
    revision: 4,
    sketches: { 'f-sk1': { solution, display: null } },
    meshes: { 'b-cube': meshSummary(1, { min: [-10, -10, 0], max: [10, 10, 20] }) },
    ghosts: { 1: Float32Array.from(LINE_POINTS) },
  })
}

type Harness = { readonly app: App; readonly core: FakeCoreApi }

/**
 * The whole shell over `document`, with `state` on top of a ready app, kept as `createApp` keeps
 * it: what a `show` selected stops being shown once the selection changes.
 */
function shell(document: DocumentState, state: Partial<AppState> = {}): Harness {
  const harness = createModelApp({ state: readyState(document, state) })
  stopUpkeep = installSelectionUpkeep(harness.app)
  renderWithApp(harness.app)
  flush()
  return harness
}

/** The whole shell with the Problems panel open. */
function problemsShell(document: DocumentState): Harness {
  return shell(document, {
    panels: { ...INITIAL_STATE.panels, browserOpen: true, side: 'problems' },
  })
}

function flush(): void {
  act(() => {
    frames.flush()
  })
}

async function settled(): Promise<void> {
  await act(async () => {
    await settle()
  })
  flush()
}

function filletRow(): HTMLElement {
  const row = screen
    .getAllByTestId('problem-row')
    .find((element) => element.getAttribute('data-feature-id') === 'f-fil1')
  if (row === undefined) {
    throw new Error("No Problems row for 'Fillet 1'.")
  }
  return row
}

/** Whether the elements follow each other in document order. */
function inDocumentOrder(elements: readonly HTMLElement[]): boolean {
  return elements.slice(1).every((element, index) => {
    const previous = elements[index]
    return (
      previous !== undefined &&
      (previous.compareDocumentPosition(element) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0
    )
  })
}

/** The cause line inside `container`, and the message, A, B, the cause line, C and D there in the order U-131 expects. */
function causeLayout(container: HTMLElement): {
  readonly causeLine: HTMLElement
  readonly expectedOrder: readonly HTMLElement[]
} {
  const button = (action: FixAction) =>
    within(container).getByRole('button', { name: action.label })
  const causeLine = within(container).getByTestId('cause-line')
  return {
    causeLine,
    expectedOrder: [
      within(container).getByText(LOST_RIM.message),
      button(USE_RIM),
      button(PICK_AGAIN),
      causeLine,
      button(SET_BACK),
      button(UNDO),
    ],
  }
}

/** Expects U-131's layout: the cause's text in `textMuted`, after the message and its actions, before its own. */
function expectCauseLayout(container: HTMLElement): void {
  const { causeLine, expectedOrder } = causeLayout(container)
  expect(causeLine).toHaveTextContent(DISTANCE_CHANGED.text)
  expect(causeLine).toHaveClass(...COLOR_CLASSES.textMuted.foreground.split(' '))
  expect(inDocumentOrder(expectedOrder)).toBe(true)
}

describe('the cause line (4.1, U-131)', () => {
  it('U-131 follows the message and its actions in the Problems panel, with its own actions after it', () => {
    problemsShell(lostDocument(filletStatus(LOST_RIM)))
    const { causeLine, expectedOrder } = causeLayout(filletRow())
    expect(causeLine).toHaveTextContent(DISTANCE_CHANGED.text)
    expect(causeLine).toHaveClass(...COLOR_CLASSES.textMuted.foreground.split(' '))
    expect(inDocumentOrder(expectedOrder)).toBe(true)
  })

  it('U-131 follows the message and its actions in the selection inspector', async () => {
    const { app } = shell(lostDocument(filletStatus(LOST_RIM)))
    act(() => {
      select(app, [featureItem('f-fil1', 'Fillet 1')], 'replace')
    })
    await settled()
    const inspector = screen.getByTestId('inspector')
    expect(inspector).toHaveAttribute('data-mode', 'selection')
    expectCauseLayout(inspector)
  })

  it("U-131 follows the message and its actions in the command's banner", async () => {
    const { app } = shell(lostDocument(filletStatus(LOST_RIM)))
    await act(async () => {
      await editFeature(app, 'f-fil1')
      await settle()
    })
    flush()
    const banner = within(screen.getByTestId('inspector')).getByTestId('inspector-error')
    expect(banner).toHaveAttribute('data-severity', 'warning')
    expectCauseLayout(banner)
  })

  it("U-131 C runs apply_fix with the cause's FixRef", async () => {
    const { core } = problemsShell(lostDocument(filletStatus(LOST_RIM)))
    const user = userEvent.setup()
    await user.click(within(filletRow()).getByRole('button', { name: SET_BACK.label }))
    await settled()
    expect(core.callsTo('applyFix')).toEqual([[1, fixRef(true, 0)]])
    expect(core.callsTo('beginTransaction')).toEqual([])
  })

  it('U-131 D runs undo', async () => {
    const { core } = problemsShell(lostDocument(filletStatus(LOST_RIM)))
    const user = userEvent.setup()
    await user.click(within(filletRow()).getByRole('button', { name: UNDO.label }))
    await settled()
    expect(core.callsTo('undo')).toEqual([[1]])
  })

  it('shows no cause line for a message without a cause', () => {
    const { cause: _cause, ...withoutCause } = LOST_RIM
    problemsShell(lostDocument(filletStatus(withoutCause)))
    expect(within(filletRow()).queryByTestId('cause-line')).toBeNull()
  })
})

// The ghost of a cause (4.2 Show, 4.3 cause_ghost).

function lastBackend(): FakeBackend {
  const last = created.backends.at(-1)
  if (last === undefined) {
    throw new Error('The viewport made no backend.')
  }
  return last
}

/** Where the last frame draws a world point on the canvas, in CSS pixels. */
function canvasPosition(point: readonly [number, number, number]): { x: number; y: number } {
  const frame = lastBackend().lastFrame()
  if (frame === null) {
    throw new Error('No frame was rendered.')
  }
  return worldToCanvas(frame, point, { width: WIDTH, height: HEIGHT })
}

/** The cube sketch with the line e-5 still drawn, from (75, 40) to (70, 45). */
const SOLUTION_WITH_LINE: SketchSolution = {
  ...CUBE_SKETCH_SOLUTION,
  entities: [
    ...CUBE_SKETCH_SOLUTION.entities,
    { id: 'e-5', type: 'line', start: [75, 40], end: [70, 45], constrained: 'under_constrained' },
  ],
  points: [
    ...CUBE_SKETCH_SOLUTION.points,
    { point: 'e-5.start', at: [75, 40] },
    { point: 'e-5.end', at: [70, 45] },
  ],
}

/** Clicks "Show the line" on the cause line of 'Fillet 1' in `container`, the Problems row by default. */
async function showTheLine(container: HTMLElement = filletRow()): Promise<void> {
  const user = userEvent.setup()
  await user.click(within(container).getByRole('button', { name: SHOW_LINE.label }))
  await settled()
}

/** The numbers of the one cause-ghost path. */
function causeGhostNumbers(): number[] {
  const ghosts = screen.getAllByTestId('cause-ghost')
  const [ghost] = ghosts
  if (ghosts.length !== 1 || ghost === undefined) {
    throw new Error(`Expected one cause ghost, found ${String(ghosts.length)}.`)
  }
  return pathNumbers(ghost)
}

/** The path numbers along the deleted line, from (80, 40) to (70, 50) on the XY sketch, to a pixel. */
function alongLine(): unknown[] {
  return [canvasPosition([80, 40, 0]), canvasPosition([70, 50, 0])].flatMap((position) => [
    expect.closeTo(position.x, 0),
    expect.closeTo(position.y, 0),
  ])
}

/** Replaces the ghost buffers as a later recompute does, which numbers them anew (2.7). */
function renumberGhosts(app: App): void {
  app.store.setState((state) =>
    state.document === null
      ? {}
      : {
          document: {
            ...state.document,
            // A repair's world points first, then the line under another index.
            ghosts: { 1: Float32Array.from(REPAIR_POINTS), 2: Float32Array.from(LINE_POINTS) },
          },
        },
  )
}

describe("a cause's Show and its ghost (4.2, U-131)", () => {
  it('U-131 selects the sketch when the deleted entity is gone, and keeps the cause in AppState.shown', async () => {
    const { app } = problemsShell(lostDocument(filletStatus(LOST_BEVEL)))
    await showTheLine()
    const state = app.store.getState()
    expect(state.selection.map((item) => item.key)).toEqual(['feature:f-sk1'])
    expect(state.shown).toEqual({ feature: 'f-sk1', entity: 'e-5', ghost: LINE_DELETED.ghost })
  })

  it('U-131 selects the entity itself while the sketch still has it', async () => {
    const { app } = problemsShell(lostDocument(filletStatus(LOST_BEVEL), SOLUTION_WITH_LINE))
    await showTheLine()
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['sketch:f-sk1:e-5'])
  })

  it('U-131 draws one dashed cause-ghost path titled "Before this change"', async () => {
    problemsShell(lostDocument(filletStatus(LOST_BEVEL)))
    await showTheLine()
    const ghosts = screen.getAllByTestId('cause-ghost')
    expect(ghosts).toHaveLength(1)
    const [ghost] = ghosts
    expect(ghost?.tagName.toLowerCase()).toBe('path')
    expect(ghost).toHaveAttribute('stroke-dasharray', '4 3')
    expect(ghost?.querySelector('title')).toHaveTextContent('Before this change')
    expect(ghost).toHaveClass(...COLOR_CLASSES.errorMark.stroke.split(' '))
  })

  it("U-131 draws the ghost on the sketch's placement, where the line was", async () => {
    problemsShell(lostDocument(filletStatus(LOST_BEVEL)))
    await showTheLine()
    expect(causeGhostNumbers()).toEqual(alongLine())
  })

  it('U-131 keeps the ghost through a later recompute that numbers its buffers anew', async () => {
    const { app } = problemsShell(lostDocument(filletStatus(LOST_BEVEL)))
    await showTheLine()
    act(() => {
      renumberGhosts(app)
    })
    flush()
    expect(causeGhostNumbers()).toEqual(alongLine())
  })

  it("U-131 a Show from the command's banner draws the ghost of the recompute it was offered in", async () => {
    const { app, core } = shell(lostDocument(filletStatus(LOST_BEVEL)))
    await act(async () => {
      await editFeature(app, 'f-fil1')
      await settle()
    })
    flush()
    // The implicit cancel's recompute numbers the ghost buffers anew.
    core.stub('cancelTransaction', async () => {
      renumberGhosts(app)
      return 5
    })
    await showTheLine(within(screen.getByTestId('inspector')).getByTestId('inspector-error'))
    expect(core.callsTo('cancelTransaction')).toHaveLength(1)
    expect(app.store.getState().command).toBeNull()
    expect(causeGhostNumbers()).toEqual(alongLine())
  })

  it('U-131 frames what it selected together with the ghost', async () => {
    const { app } = problemsShell(lostDocument(filletStatus(LOST_BEVEL)))
    const viewport = app.viewport()
    if (viewport === null) {
      throw new Error('The shell attached no viewport.')
    }
    const zoom = vi.spyOn(viewport, 'zoomToBounds')
    await showTheLine()
    // The sketch's square from (-10, -10) to (10, 10), and the line from (80, 40) to (70, 50).
    const [bounds] = zoom.mock.lastCall ?? []
    expect(bounds?.min.slice(0, 2)).toEqual([-10, -10])
    expect(bounds?.max.slice(0, 2)).toEqual([80, 50])
  })

  it('U-131 a selection change removes the ghost', async () => {
    const { app } = problemsShell(lostDocument(filletStatus(LOST_BEVEL)))
    await showTheLine()
    expect(screen.getAllByTestId('cause-ghost')).toHaveLength(1)
    act(() => {
      clearSelection(app)
    })
    flush()
    expect(screen.queryByTestId('cause-ghost')).toBeNull()
    expect(app.store.getState().shown).toBeNull()
  })

  it("a message's own Show draws no ghost", async () => {
    const showSketch: FixAction = {
      label: "Show 'Sketch 1'",
      run: { kind: 'show', feature: 'f-sk1' },
    }
    const { app } = problemsShell(
      lostDocument(filletStatus({ ...LOST_BEVEL, actions: [showSketch], cause: null })),
    )
    const user = userEvent.setup()
    await user.click(within(filletRow()).getByRole('button', { name: showSketch.label }))
    await settled()
    expect(app.store.getState().shown).toEqual({ feature: 'f-sk1', entity: null, ghost: null })
    expect(screen.queryByTestId('cause-ghost')).toBeNull()
  })
})
