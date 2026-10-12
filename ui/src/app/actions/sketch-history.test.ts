import { describe, expect, it, vi } from 'vitest'
import { MESSAGES } from '../../messages'
import {
  actionLabels,
  createTestApp,
  notificationWith,
  startWithDocument,
} from '../../test/fakes/app'
import { createFakeCoreApi } from '../../test/fakes/core-api'
import {
  CUBE_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  RECT_SKETCH_SOLUTION,
} from '../../test/fakes/documents'
import { documentStateFor } from '../../test/fakes/state'
import { settle } from '../../test/fakes/settle'
import { createAppStore } from '../../state/store'
import { DEFAULT_CAMERA, orbit, type Camera } from '../../viewport/camera'
import type { ViewportHandle } from '../../viewport/controller'
import { redo, undo } from './document'
import {
  enterSketch,
  finishSketch,
  recordSketchEdit,
  recordSketchView,
  sketchPlaneOf,
} from './sketch'

// The sketch view history of ui.md 3.20 with a recording viewport, and entering and finishing.

function recordingViewport(start: Camera): ViewportHandle & { readonly cameras: Camera[] } {
  let camera = start
  const cameras: Camera[] = []
  return {
    cameras,
    getCamera: () => camera,
    setCamera: (next) => {
      camera = next
      cameras.push(next)
    },
    fit: vi.fn(),
    setStandardView: vi.fn(),
    toggleProjection: vi.fn(),
    lookAlong: vi.fn(),
    zoomToBounds: vi.fn(),
    screenRectangle: () => null,
    requestRender: vi.fn(),
  }
}

const BEFORE = orbit(DEFAULT_CAMERA, 30, 10, [0, 0, 0])
const LATER = orbit(BEFORE, 50, 0, [0, 0, 0])

function sketchApp() {
  const core = createFakeCoreApi()
  const viewport = recordingViewport(BEFORE)
  const app = createTestApp({
    core,
    viewport,
    store: createAppStore({
      document: documentStateFor(CUBE_DOCUMENT, {
        sketches: { 'f-sk1': { solution: CUBE_SKETCH_SOLUTION, display: null } },
      }),
    }),
  })
  return { app, core, viewport }
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
    const solution = {
      ...RECT_SKETCH_SOLUTION,
      profile_findings: [{ type: 'open_endpoint', entity: 'e-1', point: [0, 0] }],
    } as const
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: {
        solutions: { 'f-sk1': { ...solution, profile_findings: [...solution.profile_findings] } },
      },
    })
    await enterSketch(fake.app, 'f-sk1')
    await finishSketch(fake.app)
    await settle()
    const notification = notificationWith(fake.app, MESSAGES.profile_problems('Sketch 1', 1))
    expect(actionLabels(notification)).toEqual(['Back to sketch'])
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
