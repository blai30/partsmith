import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { commitEdits } from '../app/actions/edits'
import { pickPlane, reattachSketch } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type {
  Document,
  ElementDescriptor,
  FeatureStatus,
  Placement,
  SketchPlane,
  SketchSolution,
} from '../generated/protocol'
import { documentWithStatuses, readyState } from '../layout/fixtures.testing'
import { pickRow } from '../layout/pick-row'
import {
  createModelApp,
  renderWithApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import type { DocumentState } from '../state/store'
import { coreErrorFor, type FakeCoreApi } from '../test/fakes/core-api'
import { deferred } from '../test/fakes/deferred'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  featuresOf,
  type FeatureRecord,
} from '../test/fakes/documents'
import { datumItem, elementItem } from '../test/fakes/selection'
import { settle } from '../test/fakes/settle'
import { CUBE_BOUNDS, meshSummary } from '../test/fakes/state'

// Re-attaching asks the core for the plane (ui.md 3.16.1, U-250): the pick builds the plane as a
// new sketch's, asks the core with `attachPlane` and commits the plane it returns, both in one
// task of the mutation queue. A pick the core gives no plane for shows `attach_moved` and stays
// open. A datum plane that does not build is refused with `attach_datum_unbuilt` first.

// The WebGL2 backend cannot run in jsdom; the viewport gets a FakeBackend instead.
vi.mock('../viewport/webgl2/backend', async () => {
  const fakes = await import('../test/fakes/backend')
  return { createWebGl2Backend: () => fakes.createFakeBackend() }
})

let restoreSize: () => void
let frames: FrameQueue

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  restoreSize = stubElementSize(HTMLCanvasElement.prototype, { width: 800, height: 600 })
  stubMatchMedia({ reducedMotion: true })
  frames = stubAnimationFrames()
})

afterEach(() => {
  restoreSize()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

/** 'Extrude 3', a boss on the cube before 'Sketch 2'; its top is the face the tests pick. */
const EXTRUDE_3: FeatureRecord = {
  id: 'f-ext3',
  type: 'extrude',
  name: 'Extrude 3',
  body: 'b-cube',
  profile: { sketch: 'f-sk1', regions: [] },
  extent: { kind: 'distance', expr: '10 mm' },
  op: 'join',
}

/** 'Datum plane 1', 30 mm above XY, before 'Sketch 2'. */
const DATUM_1: FeatureRecord = {
  id: 'f-dp1',
  type: 'datum_plane',
  name: 'Datum plane 1',
  base: { kind: 'origin', plane: 'XY' },
  offset: '30 mm',
}

/** 'Sketch 2', a circle on a face the cube lost, with its fallback in body coordinates. */
const SKETCH_2: FeatureRecord = {
  id: 'f-sk2',
  type: 'sketch',
  plane: {
    kind: 'face',
    body: 'b-cube',
    face: { q: 'f-ext1/end-cap', fp: 'plane|n=0,0,1|c=0,0,20|a=400|adj=4,4' },
    fallback: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
    fallback_frame: 'body',
  },
  entities: [{ id: 'e-1', type: 'circle', center: [0, 0], radius: 3 }],
  constraints: [],
}

/** A feature after 'Sketch 2', whose faces the sketch cannot be placed on. */
const EXTRUDE_4: FeatureRecord = {
  id: 'f-ext4',
  type: 'extrude',
  name: 'Extrude 4',
  body: 'b-cube',
  profile: { sketch: 'f-sk2', regions: [] },
  extent: { kind: 'distance', expr: '5 mm' },
  op: 'join',
}

/** U-146's document with 'Extrude 3' and 'Datum plane 1' before 'Sketch 2', then `after`. */
function recipe(datum: FeatureRecord = DATUM_1, after: readonly FeatureRecord[] = []): Document {
  return {
    ...CUBE_DOCUMENT,
    features: [...featuresOf(CUBE_DOCUMENT), EXTRUDE_3, datum, SKETCH_2, ...after],
  }
}

/** A flat face of `feature` at height `z`, as `pick_reference` describes it. */
function topOf(feature: string, z: number): ElementDescriptor {
  const placement: Placement = { origin: [0, 0, z], x_axis: [1, 0, 0], normal: [0, 0, 1] }
  return {
    name: `${feature}/end-cap`,
    kind: 'face',
    body: 'b-cube',
    feature,
    reference: { q: `${feature}/end-cap`, fp: `plane|n=0,0,1|c=0,0,${String(z)}|a=100|adj=4,4` },
    surface: 'plane',
    centroid: [0, 0, z],
    area: 100,
    normal: [0, 0, 1],
    placement,
    fallback: placement,
    adjacent: [],
  }
}

/** The top of 'Extrude 3'. */
const EXTRUDE_3_TOP = topOf('f-ext3', 30)

/** The plane a host builds from the top of 'Extrude 3', as for a new sketch (3.16.1). */
const EXTRUDE_3_TOP_PLANE: SketchPlane = {
  kind: 'face',
  body: 'b-cube',
  face: EXTRUDE_3_TOP.reference,
  fallback: { origin: [0, 0, 30], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  fallback_frame: 'body',
}

/** What the core gives back for a sketch without motion: the same face in world coordinates. */
const EXTRUDE_3_TOP_IN_WORLD: SketchPlane = {
  kind: 'face',
  body: 'b-cube',
  face: EXTRUDE_3_TOP.reference,
  fallback: { origin: [0, 0, 130], x_axis: [1, 0, 0], normal: [0, 0, 1] },
}

/** The text of `attach_moved` for 'Sketch 2' on `target`. */
function attachMoved(target: string): string {
  return `'Sketch 2' cannot be placed on ${target}: the two do not move together, and what later features place on the faces made from 'Sketch 2' could shift with no message. Pick a plane or face that moves the same way as 'Sketch 2'.`
}

/** The solution of 'Sketch 2' at its saved position. */
const SKETCH_2_SOLUTION: SketchSolution = {
  ...CUBE_SKETCH_SOLUTION,
  feature: 'f-sk2',
  placement: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
}

function documentFor(document: Document, statuses: readonly FeatureStatus[] = []): DocumentState {
  return documentWithStatuses(document, statuses, {
    name: 'cube',
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    sketches: {
      'f-sk1': { solution: CUBE_SKETCH_SOLUTION, display: null },
      'f-sk2': { solution: SKETCH_2_SOLUTION, display: null },
    },
  })
}

type Harness = { readonly app: App; readonly core: FakeCoreApi }

function flush(): void {
  act(() => {
    frames.flush()
  })
}

/** Runs an action of the app and lets its requests and renders settle. */
async function run(action: () => Promise<void>): Promise<void> {
  await act(async () => {
    await action()
    await settle()
  })
  flush()
}

/** The whole shell over `document`, re-attaching 'Sketch 2'. */
async function reattaching(document: DocumentState = documentFor(recipe())): Promise<Harness> {
  const { app, core } = createModelApp({ state: readyState(document) })
  renderWithApp(app)
  flush()
  await run(() => reattachSketch(app, 'f-sk2'))
  return { app, core }
}

function header(): HTMLElement {
  return screen.getByTestId('sketch-header')
}

function hint(): string | null {
  return screen.getByTestId('sketch-hint').textContent
}

/** The planes of the `update_feature` edits that `applyEdits` was called with, in call order. */
function committedPlanes(core: FakeCoreApi): unknown[] {
  return core.callsTo('applyEdits').map((call) => {
    const [, label, edits] = call
    expect(label).toBe("Attach 'Sketch 2'")
    const [edit] = edits as readonly { readonly feature: { readonly plane: unknown } }[]
    return edit?.feature.plane
  })
}

/** Expects plane picking to be still open for 'Sketch 2'. */
function expectPickOpen(app: App): void {
  expect(header()).toHaveAttribute('data-state', 'picking')
  expect(app.store.getState().sketch).toMatchObject({ sketchId: null, reattaching: 'f-sk2' })
}

describe('re-attaching asks the core for the plane (3.16.1, U-250)', () => {
  it('U-250 (a) a flat face of Extrude 3 asks attachPlane once with the plane of a new sketch, then commits the plane it returns and enters f-sk2', async () => {
    const { app, core } = await reattaching()
    core.stub('attachPlane', () => Promise.resolve(EXTRUDE_3_TOP_IN_WORLD))
    await run(() => pickPlane(app, elementItem(EXTRUDE_3_TOP)))
    expect(core.callsTo('attachPlane')).toEqual([[1, 'f-sk2', EXTRUDE_3_TOP_PLANE]])
    expect(core.callsTo('applyEdits')).toStrictEqual([
      [
        1,
        "Attach 'Sketch 2'",
        [{ type: 'update_feature', feature: { ...SKETCH_2, plane: EXTRUDE_3_TOP_IN_WORLD } }],
      ],
    ])
    expect(app.store.getState().sketch).toMatchObject({ sketchId: 'f-sk2', reattaching: null })
  })

  it('U-250 (a) a change queued while attachPlane is pending reaches the core after the attach, which runs in one task of the mutation queue', async () => {
    const { app, core } = await reattaching()
    const answer = deferred<SketchPlane | null>()
    core.stub('attachPlane', () => answer.promise)
    const picked = pickPlane(app, elementItem(EXTRUDE_3_TOP))
    await act(() => settle())
    expect(core.callsTo('attachPlane')).toHaveLength(1)
    const extrude1 = featuresOf(CUBE_DOCUMENT).find((feature) => feature.id === 'f-ext1')
    if (extrude1 === undefined) {
      throw new Error('cube.ptsm has Extrude 1.')
    }
    const renamed = commitEdits(app, "Rename 'Extrude 1'", [
      { type: 'update_feature', feature: { ...extrude1, name: 'Base' } },
    ])
    await act(() => settle())
    answer.resolve(EXTRUDE_3_TOP_PLANE)
    await run(async () => {
      await picked
      await renamed
    })
    const labels = core.callsTo('applyEdits').map((call) => call[1])
    expect(labels).toEqual(["Attach 'Sketch 2'", "Rename 'Extrude 1'"])
  })

  it('U-250 (b) a face the core gives no plane for commits nothing, shows attach_moved and keeps the pick open', async () => {
    const { app, core } = await reattaching()
    core.stub('attachPlane', () => Promise.resolve(null))
    await run(() => pickPlane(app, elementItem(EXTRUDE_3_TOP)))
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(attachMoved("this face of 'Extrude 3'"))
    expectPickOpen(app)
    expect(app.store.getState().notifications).toEqual([])
  })

  it('U-250 (b) the next pick the core answers with a plane commits it', async () => {
    const { app, core } = await reattaching()
    core.stub('attachPlane', () => Promise.resolve(null))
    await run(() => pickPlane(app, elementItem(EXTRUDE_3_TOP)))
    core.stub('attachPlane', (_documentHandle, _feature, plane) => Promise.resolve(plane))
    await run(() => pickPlane(app, elementItem(EXTRUDE_3_TOP)))
    expect(committedPlanes(core)).toEqual([EXTRUDE_3_TOP_PLANE])
    expect(app.store.getState().sketch).toMatchObject({ sketchId: 'f-sk2', reattaching: null })
  })

  it('U-250 (c) the plane button Top asks with XY and names the Top plane (XY) when the core gives no plane', async () => {
    const { app, core } = await reattaching()
    const user = userEvent.setup()
    core.stub('attachPlane', () => Promise.resolve(null))
    await user.click(within(header()).getByRole('button', { name: /Top plane \(XY\)/ }))
    await run(() => Promise.resolve())
    expect(core.callsTo('attachPlane')).toEqual([[1, 'f-sk2', { kind: 'origin', plane: 'XY' }]])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(attachMoved('the Top plane (XY)'))
    expectPickOpen(app)
  })

  it("U-250 (c) the browser row of 'Datum plane 1' asks with the datum and names it when the core gives no plane", async () => {
    const { app, core } = await reattaching()
    core.stub('attachPlane', () => Promise.resolve(null))
    pickRow(app, datumItem('f-dp1', 'Datum plane 1'), false)
    await run(() => Promise.resolve())
    expect(core.callsTo('attachPlane')).toEqual([[1, 'f-sk2', { kind: 'datum', feature: 'f-dp1' }]])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(attachMoved("'Datum plane 1'"))
    expectPickOpen(app)
  })

  it('U-250 (d) a face made after f-sk2 is refused with attach_after without asking the core', async () => {
    const { app, core } = await reattaching(documentFor(recipe(DATUM_1, [EXTRUDE_4])))
    await run(() => pickPlane(app, elementItem(topOf('f-ext4', 35))))
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(
      "'Sketch 2' cannot be placed on 'Extrude 4', which comes after it. Pick an origin plane, or a flat face or datum plane made before 'Sketch 2'.",
    )
    expectPickOpen(app)
  })

  it('U-250 (d) a face that is not flat is refused with pick_plane without asking the core', async () => {
    const { app, core } = await reattaching()
    const side: ElementDescriptor = { ...EXTRUDE_3_TOP, surface: 'cylinder' }
    await run(() => pickPlane(app, elementItem(side)))
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expectPickOpen(app)
  })

  it("U-250 (e) a refusal of attachPlane shows the core's message as for any refused change and keeps the pick open", async () => {
    const { app, core } = await reattaching()
    const refusal = await coreErrorFor({
      code: 'unknown_body',
      message: 'There is no body with id b-cube.',
    })
    core.stub('attachPlane', () => Promise.reject(refusal))
    await run(() => pickPlane(app, elementItem(EXTRUDE_3_TOP)))
    expect(core.callsTo('applyEdits')).toEqual([])
    const notifications = app.store.getState().notifications
    expect(notifications.map((notification) => notification.code)).toEqual(['unknown_body'])
    expect(notifications[0]?.message).toBe('There is no body with id b-cube.')
    expectPickOpen(app)
  })

  it("U-250 (g) the row of a suppressed 'Datum plane 1' is refused with attach_datum_unbuilt without asking the core", async () => {
    const suppressed = { ...DATUM_1, suppressed: true }
    const status: FeatureStatus = { id: 'f-dp1', status: 'suppressed', messages: [], repairs: [] }
    const { app, core } = await reattaching(documentFor(recipe(suppressed), [status]))
    pickRow(app, datumItem('f-dp1', 'Datum plane 1'), false)
    await run(() => Promise.resolve())
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(
      "'Sketch 2' cannot be placed on 'Datum plane 1', which is suppressed. Unsuppress 'Datum plane 1', or pick another plane or face.",
    )
    expectPickOpen(app)
  })

  it("U-250 (g) the row of 'Datum plane 1' with an error is refused with attach_datum_unbuilt without asking the core", async () => {
    const status: FeatureStatus = {
      id: 'f-dp1',
      status: 'error',
      messages: [
        {
          severity: 'error',
          code: 'references_not_found',
          message: "The face 'Datum plane 1' is placed on can no longer be found.",
        },
      ],
      repairs: [],
    }
    const { app, core } = await reattaching(documentFor(recipe(), [status]))
    pickRow(app, datumItem('f-dp1', 'Datum plane 1'), false)
    await run(() => Promise.resolve())
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(
      "'Sketch 2' cannot be placed on 'Datum plane 1', which has an error. Fix 'Datum plane 1' first, or pick another plane or face.",
    )
    expectPickOpen(app)
  })

  it('U-250 (h) when a change queued before the pick removes f-sk2, the pick sends nothing and shows nothing', async () => {
    const { app, core } = await reattaching()
    const blocker = deferred<undefined>()
    const held = app.mutations.run(() => blocker.promise)
    const picked = pickPlane(app, elementItem(EXTRUDE_3_TOP))
    await act(() => settle())
    // The change ahead of the pick removed the sketch: the core's new recipe has no f-sk2.
    app.store.setState((state) =>
      state.document === null
        ? {}
        : {
            document: {
              ...state.document,
              recipe: {
                ...state.document.recipe,
                features: featuresOf(state.document.recipe).filter(
                  (feature) => feature.id !== 'f-sk2',
                ),
              },
            },
          },
    )
    blocker.resolve(undefined)
    await run(async () => {
      await held
      await picked
    })
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(app.store.getState().notifications).toEqual([])
  })

  it("U-250 (i) a suppress of 'Datum plane 1' queued before the pick of its row refuses it with attach_datum_unbuilt without asking the core", async () => {
    const { app, core } = await reattaching()
    const blocker = deferred<undefined>()
    const held = app.mutations.run(() => blocker.promise)
    // The row is picked while 'Datum plane 1' still builds: the store has not seen the suppress yet.
    const picked = pickPlane(app, datumItem('f-dp1', 'Datum plane 1'))
    await act(() => settle())
    // The change ahead of the pick suppressed the datum: the core's new recipe and status say so.
    const status: FeatureStatus = { id: 'f-dp1', status: 'suppressed', messages: [], repairs: [] }
    app.store.setState((state) =>
      state.document === null
        ? {}
        : {
            document: {
              ...state.document,
              recipe: recipe({ ...DATUM_1, suppressed: true }),
              featureStatus: { ...state.document.featureStatus, 'f-dp1': status },
            },
          },
    )
    blocker.resolve(undefined)
    await run(async () => {
      await held
      await picked
    })
    expect(core.callsTo('attachPlane')).toEqual([])
    expect(core.callsTo('applyEdits')).toEqual([])
    expect(hint()).toBe(
      "'Sketch 2' cannot be placed on 'Datum plane 1', which is suppressed. Unsuppress 'Datum plane 1', or pick another plane or face.",
    )
    expectPickOpen(app)
  })
})
