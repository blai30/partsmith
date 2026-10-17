import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { runFixAction, showLost } from '../app/actions/fixes'
import { reattachSketch, startNewSketch } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type {
  Diagnostic,
  Document,
  FeatureStatus,
  FixAction,
  FixRef,
  Reference,
  RepairCandidate,
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
import type { ElementRepair } from '../repair/repair'
import type { DocumentState } from '../state/store'
import type { FakeBackend } from '../test/fakes/backend'
import { coreErrorFor, type FakeCoreApi } from '../test/fakes/core-api'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  featuresOf,
  type FeatureRecord,
} from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { CUBE_BOUNDS, meshSummary } from '../test/fakes/state'

// Re-attaching a sketch whose face is lost (ui.md 3.16.1, U-146): "Show what '{label}' lost" and a
// `pick` action at the sketch's plane run the plane picking of a new sketch for that sketch, with
// its prompt, the lost face's ghost and the numbered candidates in the header; a pick stores the
// new plane in one undo entry labeled "Attach '{label}'".

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

let restoreSize: () => void
let frames: FrameQueue

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
  created.backends.length = 0
})

afterEach(() => {
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** The face 'Sketch 2' was drawn on: the top of 'Extrude 1', which an edit removed. */
const LOST_TOP: Reference = { q: 'f-ext1/end-cap', fp: 'plane|n=0,0,1|c=0,0,20|a=400|adj=4,4' }

/** 'Sketch 2', a circle on the lost top face, with its fallback in body coordinates. */
const SKETCH_2: FeatureRecord = {
  id: 'f-sk2',
  type: 'sketch',
  plane: {
    kind: 'face',
    body: 'b-cube',
    face: LOST_TOP,
    fallback: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
    fallback_frame: 'body',
  },
  entities: [{ id: 'e-1', type: 'circle', center: [0, 0], radius: 3 }],
  constraints: [],
}

/** cube.ptsm with 'Sketch 2' at the end. */
const DOCUMENT: Document = { ...CUBE_DOCUMENT, features: [...featuresOf(CUBE_DOCUMENT), SKETCH_2] }

/** A FixRef of the `plane_fallback` message of 'Sketch 2'. */
function attachFix(index: number): FixRef {
  return {
    feature: 'f-sk2',
    code: 'plane_fallback',
    field: '/plane/face',
    cause: false,
    index,
    revision: 3,
  }
}

const ATTACH_TOP: FixAction = {
  label: "Attach to the top of 'Extrude 3'",
  run: { kind: 'apply', fix: attachFix(0) },
}

const ATTACH_FRONT: FixAction = {
  label: "Attach to the front side of 'Extrude 3'",
  run: { kind: 'apply', fix: attachFix(1) },
}

const FALLBACK_MESSAGE: Diagnostic = {
  severity: 'warning',
  code: 'plane_fallback',
  message:
    "The top of 'Extrude 1', which this sketch is on, is gone, so the sketch uses the position saved with it.",
  field: '/plane/face',
  actions: [ATTACH_TOP, ATTACH_FRONT],
}

const TOP_OF_EXTRUDE_3: RepairCandidate = {
  reference: { q: 'f-ext3/end-cap', fp: 'plane|n=0,0,1|c=0,0,30|a=400|adj=4,4' },
  description: "the top of 'Extrude 3'",
  relation: "facing the way the top of 'Extrude 1' did, 10 mm from where it was",
  meaning: false,
  distance: 10,
  score: 0.61,
}

const FRONT_OF_EXTRUDE_3: RepairCandidate = {
  reference: { q: 'f-ext3/side-0', fp: 'plane|n=0,-1,0|c=0,-10,25|a=200|adj=4,4' },
  description: "the front side of 'Extrude 3'",
  relation: 'about 15 mm from where it was',
  meaning: false,
  distance: 15,
  score: 0.42,
}

/** The ghost of the lost face, buffer 0: two triangles of the 20 mm square at z = 20. */
const GHOST_POINTS = [-10, -10, 20, 10, -10, 20, 10, 10, 20, -10, -10, 20, 10, 10, 20, -10, 10, 20]

const FACE_REPAIR: ElementRepair = {
  kind: 'element',
  path: '/plane/face',
  body: 'b-cube',
  reference: LOST_TOP,
  resolution: 'missing',
  description: "the top of 'Extrude 1'",
  centroid: [0, 0, 20],
  ghost: { positions: 0, kind: 'triangles', frame: 'world' },
  candidates: [TOP_OF_EXTRUDE_3, FRONT_OF_EXTRUDE_3],
}

const SKETCH_2_STATUS: FeatureStatus = {
  id: 'f-sk2',
  status: 'warning',
  messages: [FALLBACK_MESSAGE],
  repairs: [FACE_REPAIR],
}

/** Where 'Sketch 2' is placed now: its saved position. */
const SKETCH_2_SOLUTION: SketchSolution = {
  ...CUBE_SKETCH_SOLUTION,
  feature: 'f-sk2',
  placement: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
}

function documentWithLostFace(): DocumentState {
  return documentWithStatuses(DOCUMENT, [SKETCH_2_STATUS], {
    name: 'cube',
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    sketches: {
      'f-sk1': { solution: CUBE_SKETCH_SOLUTION, display: null },
      'f-sk2': { solution: SKETCH_2_SOLUTION, display: null },
    },
    ghosts: { 0: Float32Array.from(GHOST_POINTS) },
  })
}

type Harness = { readonly app: App; readonly core: FakeCoreApi }

function flush(): void {
  act(() => {
    frames.flush()
  })
}

/** The whole shell over cube.ptsm with 'Sketch 2' on a lost face, at rest. */
function shell(): Harness {
  const { app, core } = createModelApp({ state: readyState(documentWithLostFace()) })
  renderWithApp(app)
  flush()
  return { app, core }
}

/** Runs an action of the app and lets its requests and renders settle. */
async function run(action: () => Promise<void>): Promise<void> {
  await act(async () => {
    await action()
    await settle()
  })
  flush()
}

function header(): HTMLElement {
  return screen.getByTestId('sketch-header')
}

describe('re-attaching a sketch (3.16.1, U-146)', () => {
  it("U-146 \"Show what 'Sketch 2' lost\" runs reattachSketch: plane picking for 'Sketch 2'", async () => {
    const { app } = shell()
    await run(() => showLost(app, 'f-sk2'))
    expect(screen.getByTestId('app')).toHaveAttribute('data-context', 'sketch')
    expect(app.store.getState().sketch).toMatchObject({ sketchId: null, reattaching: 'f-sk2' })
    expect(header()).toHaveAttribute('data-state', 'picking')
    expect(header()).toHaveTextContent("Pick a plane or a flat face for 'Sketch 2'")
  })

  it('U-146 lists the candidates in the header as numbered buttons with their relation', async () => {
    const { app } = shell()
    await run(() => reattachSketch(app, 'f-sk2'))
    const buttons = within(header()).getAllByTestId('repair-candidate-button')
    expect(buttons).toHaveLength(2)
    const [first, second] = buttons
    expect(first).toHaveTextContent("1. The top of 'Extrude 3'")
    expect(first).toHaveTextContent(TOP_OF_EXTRUDE_3.relation)
    expect(second).toHaveTextContent("2. The front side of 'Extrude 3'")
    expect(second).toHaveTextContent(FRONT_OF_EXTRUDE_3.relation)
    // Candidates are described by meaning and distance, never by a score (3.7).
    for (const button of buttons) {
      expect(button.textContent).not.toMatch(/%|0\.61|0\.42/)
    }
  })

  it("U-146 draws the lost face's ghost while picking", async () => {
    const { app } = shell()
    await run(() => reattachSketch(app, 'f-sk2'))
    const ghosts = screen.getAllByTestId('repair-ghost')
    expect(ghosts).toHaveLength(1)
    expect(ghosts[0]?.querySelector('title')).toHaveTextContent("The top of 'Extrude 1' (missing)")
  })

  it('U-146 keeps the three plane buttons of plane picking', async () => {
    const { app } = shell()
    await run(() => reattachSketch(app, 'f-sk2'))
    for (const plane of [/Top plane \(XY\)/, /Front plane \(XZ\)/, /Right plane \(YZ\)/]) {
      expect(within(header()).getByRole('button', { name: plane })).toBeInTheDocument()
    }
  })

  it('U-146 the plane button Top commits one apply_edits labeled "Attach \'Sketch 2\'" with the stored entities, then enters f-sk2', async () => {
    const { app, core } = shell()
    const user = userEvent.setup()
    await run(() => reattachSketch(app, 'f-sk2'))
    await user.click(within(header()).getByRole('button', { name: /Top plane \(XY\)/ }))
    await run(() => Promise.resolve())
    expect(core.callsTo('applyEdits')).toEqual([
      [
        1,
        "Attach 'Sketch 2'",
        [
          {
            type: 'update_feature',
            feature: { ...SKETCH_2, plane: { kind: 'origin', plane: 'XY' } },
          },
        ],
      ],
    ])
    expect(core.callsTo('applyFix')).toEqual([])
    const state = app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch).toMatchObject({ sketchId: 'f-sk2', reattaching: null })
  })

  it("U-146 candidate button 1 calls applyFix with the first action's FixRef", async () => {
    const { app, core } = shell()
    const user = userEvent.setup()
    await run(() => reattachSketch(app, 'f-sk2'))
    const [first] = within(header()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? document.body)
    await run(() => Promise.resolve())
    expect(core.callsTo('applyFix')).toEqual([[1, attachFix(0)]])
    expect(core.callsTo('applyEdits')).toEqual([])
  })

  it("U-146 once candidate 1's action attaches 'Sketch 2', f-sk2 is edited, as after a pick", async () => {
    const { app, core } = shell()
    const user = userEvent.setup()
    // The recompute of the fix reaches the store before its reply settles (CoreClient.request).
    core.stub('applyFix', () => {
      app.store.setState((state) =>
        state.document === null
          ? {}
          : {
              document: {
                ...state.document,
                featureStatus: {
                  ...state.document.featureStatus,
                  'f-sk2': { id: 'f-sk2', status: 'ok', messages: [], repairs: [] },
                },
              },
            },
      )
      return Promise.resolve(4)
    })
    await run(() => reattachSketch(app, 'f-sk2'))
    const [first] = within(header()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? document.body)
    await run(() => Promise.resolve())
    expect(core.callsTo('applyFix')).toEqual([[1, attachFix(0)]])
    expect(app.store.getState().context).toBe('sketch')
    expect(app.store.getState().sketch).toMatchObject({ sketchId: 'f-sk2', reattaching: null })
  })

  it('U-146 a candidate whose fix no longer applies leaves the pick open', async () => {
    const { app, core } = shell()
    const user = userEvent.setup()
    const stale = await coreErrorFor({
      code: 'stale_fix',
      message:
        'This fix no longer applies because the document changed after it was offered. Look at the problem again.',
    })
    core.stub('applyFix', () => Promise.reject(stale))
    await run(() => reattachSketch(app, 'f-sk2'))
    const [first] = within(header()).getAllByTestId('repair-candidate-button')
    await user.click(first ?? document.body)
    await run(() => Promise.resolve())
    expect(app.store.getState().sketch).toMatchObject({ sketchId: null, reattaching: 'f-sk2' })
    expect(app.store.getState().notifications.map((notification) => notification.code)).toEqual([
      'stale_fix',
    ])
  })

  it('U-146 Esc returns to Model with nothing changed', async () => {
    const { app, core } = shell()
    const user = userEvent.setup()
    await run(() => reattachSketch(app, 'f-sk2'))
    expect(app.store.getState().sketch?.reattaching).toBe('f-sk2')
    await user.keyboard('{Escape}')
    await run(() => Promise.resolve())
    expect(app.store.getState().context).toBe('model')
    expect(app.store.getState().sketch).toBeNull()
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(core.callsTo('applyFix')).toEqual([])
  })

  it.each(['/plane/face', '/plane'])(
    'U-146 a pick action at %s of a sketch runs reattachSketch',
    async (path) => {
      const { app } = shell()
      const pick: FixAction = {
        label: 'Pick a face again',
        run: { kind: 'pick', feature: 'f-sk2', path },
      }
      await run(() => runFixAction(app, pick, 'panel'))
      expect(app.store.getState().context).toBe('sketch')
      expect(app.store.getState().sketch).toMatchObject({ sketchId: null, reattaching: 'f-sk2' })
      expect(header()).toHaveTextContent("Pick a plane or a flat face for 'Sketch 2'")
    },
  )

  it('a new sketch picks its plane with the plain prompt and nothing being re-attached', async () => {
    const { app } = shell()
    await run(() => startNewSketch(app))
    expect(app.store.getState().sketch).toMatchObject({ sketchId: null, reattaching: null })
    expect(header()).toHaveTextContent('Pick a plane or a flat face to draw on')
    expect(within(header()).queryAllByTestId('repair-candidate-button')).toEqual([])
  })
})
