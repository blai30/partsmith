import { describe, expect, it, vi, type Mock } from 'vitest'
import type { BoundingBox, ProfileFinding } from '../../generated/protocol'
import {
  actionLabels,
  createTestApp,
  notificationWith,
  startWithDocument,
} from '../../test/fakes/app'
import { coreErrorFor, createFakeCoreApi } from '../../test/fakes/core-api'
import type { Document, SketchSolution } from '../../generated/protocol'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH,
  CUBE_SKETCH_SOLUTION,
  RECT_SKETCH_SOLUTION,
} from '../../test/fakes/documents'
import { documentStateFor } from '../../test/fakes/state'
import { settle } from '../../test/fakes/settle'
import { createAppStore } from '../../state/store'
import {
  classifyView,
  DEFAULT_CAMERA,
  orbit,
  standardViewRotation,
  type Camera,
  type StandardView,
  wholeCanvas,
} from '../../viewport/camera'
import type { Vector3 } from '../../viewport/math'
import type { App } from '../createApp'
import { redo, undo } from './document'
import { runFixAction } from './fixes'
import {
  enterSketch,
  finishSketch,
  recordSketchEdit,
  recordSketchView,
  sketchPlaneOf,
  startNewSketch,
} from './sketch'

// The sketch view history of ui.md 3.20 with a recording viewport, and entering and finishing.

type RecordingViewport = NonNullable<ReturnType<App['viewport']>> & {
  readonly cameras: Camera[]
  readonly lookAlong: Mock<(normal: Vector3, upHint: Vector3, target: Vector3) => void>
  readonly zoomToBounds: Mock<(bounds: BoundingBox, margin: number) => void>
}

function recordingViewport(start: Camera): RecordingViewport {
  let camera = start
  const cameras: Camera[] = []
  return {
    cameras,
    getCamera: () => camera,
    setCamera: (next: Camera) => {
      camera = next
      cameras.push(next)
    },
    fit: vi.fn<(bounds?: BoundingBox) => void>(),
    setStandardView: vi.fn<(view: StandardView) => void>(),
    toggleProjection: vi.fn<() => void>(),
    lookAlong: vi.fn<(normal: Vector3, upHint: Vector3, target: Vector3) => void>(),
    zoomToBounds: vi.fn<(bounds: BoundingBox, margin: number) => void>(),
    itemBounds: () => null,
    screenRectangle: () => null,
    viewWindow: () => wholeCanvas({ width: 800, height: 600 }),
    requestRender: vi.fn<() => void>(),
  }
}

const BEFORE = orbit(DEFAULT_CAMERA, 30, 10, [0, 0, 0])
const LATER = orbit(BEFORE, 50, 0, [0, 0, 0])

function sketchApp(
  recipe: Document = CUBE_DOCUMENT,
  solution: SketchSolution = CUBE_SKETCH_SOLUTION,
) {
  const core = createFakeCoreApi()
  const viewport = recordingViewport(BEFORE)
  const app = createTestApp({
    core,
    viewport,
    store: createAppStore({
      document: documentStateFor(recipe, {
        sketches: { 'f-sk1': { solution, display: null } },
      }),
    }),
  })
  return { app, core, viewport }
}

/** An entity of the cube's sketch 30 mm further right. */
function shiftedRight(
  entity: SketchSolution['entities'][number],
): SketchSolution['entities'][number] {
  if (entity.type !== 'line') {
    return entity
  }
  return {
    ...entity,
    start: [entity.start[0] + 30, entity.start[1]],
    end: [entity.end[0] + 30, entity.end[1]],
  }
}

/** The cube's sketch solved 30 mm to the right of the origin, so its entities leave it out. */
const SHIFTED_SOLUTION: SketchSolution = {
  ...CUBE_SKETCH_SOLUTION,
  entities: CUBE_SKETCH_SOLUTION.entities.map(shiftedRight),
}

describe('entering a sketch', () => {
  it('looks along the placement normal with its y axis up, and frames the entities with margin 0.1', async () => {
    const { app, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    expect(viewport.lookAlong).toHaveBeenCalledWith([0, 0, 1], [0, 1, 0], [0, 0, 0])
    expect(viewport.zoomToBounds).toHaveBeenCalledWith(
      { min: [-10, -10, 0], max: [10, 10, 0] },
      0.1,
    )
  })
})

describe('framing an entered sketch (3.16.1)', () => {
  it('frames the sketch origin too when a constraint refers to it, so a dimension to it shows', async () => {
    // The cube's sketch places its corner by distances from the origin.
    const { app, viewport } = sketchApp(CUBE_DOCUMENT, SHIFTED_SOLUTION)
    await enterSketch(app, 'f-sk1')
    expect(viewport.zoomToBounds).toHaveBeenCalledWith({ min: [0, -10, 0], max: [40, 10, 0] }, 0.1)
  })

  it('frames only the entities when no constraint refers to the origin or an axis', async () => {
    const constraints = (CUBE_SKETCH['constraints'] as readonly { readonly type: string }[]).filter(
      (constraint) => constraint.type === 'coincident' || constraint.type === 'horizontal',
    )
    const recipe: Document = {
      ...CUBE_DOCUMENT,
      features: [{ ...CUBE_SKETCH, constraints }, ...(CUBE_DOCUMENT.features ?? []).slice(1)],
    }
    const { app, viewport } = sketchApp(recipe, SHIFTED_SOLUTION)
    await enterSketch(app, 'f-sk1')
    expect(viewport.zoomToBounds).toHaveBeenCalledWith({ min: [20, -10, 0], max: [40, 10, 0] }, 0.1)
  })
})

describe('sketch view history (3.20)', () => {
  it('undo restores the camera from before entering, then calls the core', async () => {
    const { app, core, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    viewport.setCamera(LATER)
    await undo(app)
    expect(viewport.getCamera()).toBe(BEFORE)
    expect(core.callsTo('undo')).toEqual([])
    await undo(app)
    expect(core.callsTo('undo')).toHaveLength(1)
  })

  it('a recorded view and a recorded edit undo newest first, and redo replays them', async () => {
    const { app, core, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    recordSketchEdit(app)
    recordSketchView(app, viewport.getCamera())
    viewport.setCamera(LATER)
    await undo(app)
    expect(core.callsTo('undo')).toEqual([])
    await undo(app)
    expect(core.callsTo('undo')).toHaveLength(1)
    await redo(app)
    expect(core.callsTo('redo')).toHaveLength(1)
    await redo(app)
    expect(viewport.getCamera()).toBe(LATER)
  })

  it("U-145 a fix applied in a sketch session is a document entry, so the next Mod+Z calls the core's undo", async () => {
    const { app, core } = sketchApp()
    await enterSketch(app, 'f-sk1')
    const fix = {
      feature: 'f-sk1',
      code: 'open_end',
      field: '/profile_findings/0',
      cause: false,
      index: 0,
      revision: 1,
    }
    await runFixAction(
      app,
      { label: 'Connect the ends (0.3 mm apart)', run: { kind: 'apply', fix } },
      'panel',
    )
    expect(core.callsTo('applyFix')).toEqual([[1, fix]])
    await undo(app)
    expect(core.callsTo('undo')).toHaveLength(1)
  })

  it('a refused solve shows the core message, and the history still starts with the camera from before', async () => {
    const core = createFakeCoreApi()
    const refusal = await coreErrorFor({
      code: 'invalid_params',
      message: "Sketch 1 has a constraint on 'e-9', which does not exist.",
    })
    core.stub('solveSketch', () => Promise.reject(refusal))
    const viewport = recordingViewport(BEFORE)
    const app = createTestApp({
      core,
      viewport,
      store: createAppStore({ document: documentStateFor(CUBE_DOCUMENT) }),
    })
    await enterSketch(app, 'f-sk1')
    expect(notificationWith(app, refusal.message).severity).toBe('error')
    expect(app.store.getState().sketch?.sketchId).toBe('f-sk1')
    expect(viewport.lookAlong).not.toHaveBeenCalled()
    viewport.setCamera(LATER)
    await undo(app)
    expect(viewport.getCamera()).toBe(BEFORE)
    expect(core.callsTo('undo')).toEqual([])
  })

  it('finishing the sketch discards the history', async () => {
    const { app, core, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    await finishSketch(app)
    await undo(app)
    expect(core.callsTo('undo')).toHaveLength(1)
    expect(viewport.getCamera()).toBe(BEFORE)
  })

  it('records nothing outside sketch mode', () => {
    const { app } = sketchApp()
    recordSketchView(app, DEFAULT_CAMERA)
    recordSketchEdit(app)
    expect(app.store.getState().sketch).toBeNull()
  })
})

describe('finishSketch (3.16.11)', () => {
  it('reports profile problems with "Back to sketch"', async () => {
    const openEnd: ProfileFinding = { type: 'open_endpoint', entity: 'e-1', point: [0, 0] }
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { solutions: { 'f-sk1': { ...RECT_SKETCH_SOLUTION, profile_findings: [openEnd] } } },
    })
    await enterSketch(fake.app, 'f-sk1')
    await finishSketch(fake.app)
    await settle()
    const notification = notificationWith(
      fake.app,
      'Sketch 1 has 1 open end, so some shapes cannot be extruded.',
    )
    expect(actionLabels(notification)).toEqual(['Back to sketch'])
  })
})

describe('the camera after a sketch (3.16.11)', () => {
  // Looking straight down at the XY plane of f-sk1, as entering the sketch leaves the camera.
  const STRAIGHT_ON: Camera = { ...BEFORE, rotation: standardViewRotation('top') }

  it('keeps the camera from before entering as cameraBefore', async () => {
    const { app } = sketchApp()
    await enterSketch(app, 'f-sk1')
    expect(app.store.getState().sketch?.cameraBefore).toBe(BEFORE)
  })

  it('U-126 animates a camera still looking straight at the sketch back to the camera from before', async () => {
    const { app, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    viewport.setCamera(STRAIGHT_ON)
    const setCamera = vi.spyOn(viewport, 'setCamera')
    await finishSketch(app)
    expect(setCamera).toHaveBeenCalledExactlyOnceWith(BEFORE, { animate: true })
    expect(viewport.getCamera()).toBe(BEFORE)
  })

  it('U-126 turns to iso when the camera from before looked straight at the sketch too', async () => {
    const { app, viewport } = sketchApp()
    viewport.setCamera(STRAIGHT_ON)
    await enterSketch(app, 'f-sk1')
    await finishSketch(app)
    expect(classifyView(viewport.getCamera())).toBe('iso')
    expect(viewport.getCamera().target).toEqual(STRAIGHT_ON.target)
  })

  it('U-126 leaves a camera the user turned away from the sketch plane where it is', async () => {
    const { app, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    viewport.setCamera(LATER)
    const setCamera = vi.spyOn(viewport, 'setCamera')
    await finishSketch(app)
    expect(setCamera).not.toHaveBeenCalled()
  })

  it('keeps the first camera from before when the session moves on to another sketch', async () => {
    const { app, viewport } = sketchApp()
    await enterSketch(app, 'f-sk1')
    viewport.setCamera(STRAIGHT_ON)
    await enterSketch(app, 'f-sk1')
    expect(app.store.getState().sketch?.cameraBefore).toBe(BEFORE)
  })

  it('has no camera from before while a plane is picked', async () => {
    const { app } = sketchApp()
    await startNewSketch(app)
    expect(app.store.getState().sketch).toMatchObject({ sketchId: null, cameraBefore: null })
  })
})

describe('sketchPlaneOf', () => {
  it('maps a datum plane row to a datum plane reference and refuses other items', () => {
    const datum = {
      key: 'feature:f-dp1',
      kind: 'datum_plane',
      label: 'Datum plane 1',
      body: null,
      feature: 'f-dp1',
      element: null,
      sketchItem: null,
      origin: null,
    } as const
    expect(sketchPlaneOf(datum)).toEqual({ kind: 'datum', feature: 'f-dp1' })
    expect(sketchPlaneOf({ ...datum, kind: 'body', key: 'body:b-cube' })).toBeNull()
  })
})
