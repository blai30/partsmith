import { fireEvent, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { setHoveredField } from '../app/actions/command'
import { undo } from '../app/actions/document'
import { enterSketch } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import type { BodyMeshData } from '../core/events'
import type { BoundingBox, ElementDescriptor, ElementRef, Resolution } from '../generated/protocol'
import {
  bodyItem,
  bodyMeshEvent,
  boxMesh,
  CUBE_BOUNDS,
  CUBE_FACES,
  cubeDocumentState,
  documentState,
  elementItem,
  featureItem,
  FILLET_EDGES_COMMAND,
  FILLETED_CUBE_FACES,
  meshSummary,
  readyState,
} from '../layout/fixtures.testing'
import {
  createModelApp,
  stubAnimationFrames,
  stubElementSize,
  stubMatchMedia,
  type FrameQueue,
} from '../layout/test-app.testing'
import { onUiRequest } from '../layout/requests'
import { MESSAGES } from '../messages'
import { DEFAULT_SETTINGS } from '../settings/settings'
import { INITIAL_STATE, type AppState, type DocumentState } from '../state/store'
import { createFakeBackend, type FakeBackend } from '../test/fakes/backend'
import type { FakeCoreApi } from '../test/fakes/core-api'
import { deferred } from '../test/fakes/deferred'
import {
  CUBE_FEATURES_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_TOP_FACE,
  CUBE_TOP_FRONT_EDGE,
} from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { ELEMENT_STATE } from './backend'
import {
  DEFAULT_CAMERA,
  fitBounds,
  forward,
  standardViewRotation,
  up,
  withProjection,
  type Camera,
} from './camera'
import { createViewportController, type ViewportController } from './controller'
import { drawnGridSpacing } from './grid'
import { createScene, type Scene } from './scene'

const WIDTH = 800
const HEIGHT = 600
const ASPECT = WIDTH / HEIGHT
const CUBE_MESH = boxMesh([-10, -10, 0], [10, 10, 20])

type Fixture = {
  readonly app: App
  readonly core: FakeCoreApi
  readonly backend: FakeBackend
  readonly scene: Scene
  readonly canvas: HTMLCanvasElement
  readonly overlay: HTMLElement
  readonly controller: ViewportController
  readonly frames: FrameQueue
}

let cleanups: (() => void)[] = []

afterEach(() => {
  for (const cleanup of cleanups.toReversed()) {
    cleanup()
  }
  cleanups = []
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

type SetUpOptions = {
  readonly state?: Partial<AppState>
  readonly document?: DocumentState
  readonly reducedMotion?: boolean
  readonly prepare?: (core: FakeCoreApi) => void
}

function setUp(options: SetUpOptions = {}): Fixture {
  stubMatchMedia({ reducedMotion: options.reducedMotion ?? true })
  const frames = stubAnimationFrames()
  cleanups.push(stubElementSize(HTMLCanvasElement.prototype, { width: WIDTH, height: HEIGHT }))
  const { app, core } = createModelApp({
    state: readyState(options.document ?? cubeDocumentState(), options.state),
  })
  options.prepare?.(core)
  const viewport = document.createElement('main')
  viewport.dataset.testid = 'viewport'
  const canvas = document.createElement('canvas')
  const overlay = document.createElement('div')
  viewport.append(canvas, overlay)
  document.body.append(viewport)
  const backend = createFakeBackend()
  const scene = createScene(backend)
  scene.applyEvent(bodyMeshEvent({ body: 'b-cube', meshId: 1, mesh: CUBE_MESH }))
  const controller = createViewportController({ app, canvas, overlay, backend, scene })
  cleanups.push(() => {
    controller.dispose()
    viewport.remove()
  })
  frames.flush()
  return { app, core, backend, scene, canvas, overlay, controller, frames }
}

/** Answers picks with one element of the cube mesh at distance 0. */
function pickElement(
  fixture: Fixture,
  element: 'face' | 'edge' | 'vertex',
  index: number,
  mesh: BodyMeshData = CUBE_MESH,
): void {
  const positions =
    element === 'face'
      ? mesh.positions
      : element === 'edge'
        ? mesh.edgePositions
        : mesh.vertexPositions
  fixture.backend.setPickHandler(({ frame }) => {
    const id = fixture.backend.idOf(positions)
    const draw = frame.draws.find((candidate) => candidate.id === id)
    return draw?.pickBase === null || draw === undefined
      ? []
      : [{ pickId: draw.pickBase + index, distance: 0, depth: 100 }]
  })
}

function indicesWith(fixture: Fixture, positions: Float32Array, bit: number): number[] {
  const states = fixture.backend.elementStates.get(fixture.backend.idOf(positions))
  return states === undefined
    ? []
    : [...states.keys()].filter((index) => ((states[index] ?? 0) & bit) !== 0)
}

function click(
  canvas: HTMLCanvasElement,
  x: number,
  y: number,
  modifiers: { shiftKey?: boolean } = {},
): void {
  const init = {
    pointerId: 1,
    pointerType: 'mouse',
    button: 0,
    clientX: x,
    clientY: y,
    ...modifiers,
  }
  fireEvent.pointerDown(canvas, { ...init, buttons: 1 })
  fireEvent.pointerUp(canvas, { ...init, buttons: 0 })
  fireEvent.click(canvas, { button: 0, clientX: x, clientY: y, ...modifiers })
}

/** A middle-button press and release at one point. */
function middleClick(canvas: HTMLCanvasElement, x: number, y: number): void {
  const init = { pointerId: 1, pointerType: 'mouse', button: 1, clientX: x, clientY: y }
  fireEvent.pointerDown(canvas, { ...init, buttons: 4 })
  fireEvent.pointerUp(canvas, { ...init, buttons: 0 })
}

function setDocument(app: App, change: Partial<DocumentState>): void {
  const current = app.store.getState().document
  if (current === null) {
    throw new Error('No document.')
  }
  app.store.setState({ document: { ...current, ...change } })
}

/** The largest difference between two cameras' numbers; a rotation and its negation are the same rotation. */
function cameraDifference(actual: Camera, expected: Camera): number {
  const rotation = (sign: number) =>
    Math.max(
      ...actual.rotation.map((value, i) =>
        Math.abs(value - sign * (expected.rotation[i] ?? Number.NaN)),
      ),
    )
  return Math.max(
    ...actual.target.map((value, i) => Math.abs(value - (expected.target[i] ?? Number.NaN))),
    Math.min(rotation(1), rotation(-1)),
    Math.abs(actual.distance - expected.distance),
    Math.abs(actual.orthographicHeight - expected.orthographicHeight),
    actual.projection === expected.projection ? 0 : Number.POSITIVE_INFINITY,
  )
}

describe('rendering on demand (3.12)', () => {
  it('renders a frame after mounting and nothing while idle', () => {
    const fixture = setUp()
    expect(fixture.backend.frames.length).toBeGreaterThan(0)
    fixture.backend.clearCalls()
    fixture.frames.flush()
    expect(fixture.backend.calls.filter((call) => call.method === 'render')).toEqual([])
  })

  it('renders once for several store changes in one animation frame', () => {
    const fixture = setUp()
    fixture.backend.clearCalls()
    fixture.app.store.setState({ selection: [bodyItem('b-cube', 'Cube')] })
    fixture.app.store.setState({ hover: { key: 'mesh:1:face:0', kind: 'face' } })
    fixture.frames.flush()
    expect(fixture.backend.calls.filter((call) => call.method === 'render')).toHaveLength(1)
  })

  it('renders after requestRender', () => {
    const fixture = setUp()
    fixture.backend.clearCalls()
    fixture.controller.requestRender()
    fixture.frames.flush()
    expect(fixture.backend.calls.filter((call) => call.method === 'render')).toHaveLength(1)
  })

  it('sizes the drawing buffer to the CSS size times the device pixel ratio, at most 2', () => {
    vi.stubGlobal('devicePixelRatio', 3)
    const fixture = setUp()
    expect(fixture.backend.size).toEqual({ width: WIDTH, height: HEIGHT, pixelRatio: 2 })
  })

  it('renders nothing after dispose', () => {
    const fixture = setUp()
    fixture.controller.dispose()
    fixture.backend.clearCalls()
    fixture.app.store.setState({ selection: [bodyItem('b-cube', 'Cube')] })
    fixture.frames.flush()
    expect(fixture.backend.calls.filter((call) => call.method === 'render')).toEqual([])
  })

  it('uploads everything again and renders when the WebGL context is restored', () => {
    const fixture = setUp()
    fixture.backend.loseContext()
    fixture.backend.clearCalls()
    fixture.backend.restoreContext()
    fixture.frames.flush()
    expect(fixture.backend.idOf(CUBE_MESH.positions)).toEqual(expect.any(String))
    expect(fixture.backend.calls.some((call) => call.method === 'render')).toBe(true)
  })

  it('renders with the dark viewport theme when the theme setting changes', () => {
    const fixture = setUp()
    fixture.app.store.setState({ settings: { ...DEFAULT_SETTINGS, theme: 'dark' } })
    fixture.frames.flush()
    const frame = fixture.backend.lastFrame()
    expect(frame?.theme.backgroundTop).not.toEqual(fixture.backend.frames[0]?.theme.backgroundTop)
  })
})

describe('the grid spacing (3.12, 3.21)', () => {
  it('mirrors the drawn grid’s minor spacing in the store, larger after zooming out', () => {
    const fixture = setUp()
    const camera = fixture.controller.getCamera()
    const state = fixture.app.store.getState()
    expect(state.view.gridSpacing).toBe(drawnGridSpacing(state, camera, HEIGHT))
    const before = state.view.gridSpacing ?? 0
    fixture.controller.setCamera({ ...camera, distance: camera.distance * 20 })
    fixture.frames.flush()
    expect(fixture.app.store.getState().view.gridSpacing).toBeGreaterThan(before)
  })

  it('mirrors null when no grid is drawn, as in Print', () => {
    const fixture = setUp()
    fixture.app.store.setState({ context: 'print' })
    fixture.frames.flush()
    expect(fixture.app.store.getState().view.gridSpacing).toBeNull()
  })
})

describe('the camera handle (2.11, 3.14)', () => {
  it('starts at the default camera', () => {
    const fixture = setUp()
    expect(cameraDifference(fixture.controller.getCamera(), DEFAULT_CAMERA)).toBeLessThan(1e-6)
  })

  it('setStandardView keeps target, distance and projection, and mirrors the view in the store', () => {
    const fixture = setUp()
    fixture.controller.setStandardView('front')
    fixture.frames.flush()
    expect(
      cameraDifference(fixture.controller.getCamera(), {
        ...DEFAULT_CAMERA,
        rotation: standardViewRotation('front'),
      }),
    ).toBeLessThan(1e-6)
    expect(fixture.app.store.getState().view.named).toBe('front')
  })

  it('animates camera changes for 250 ms unless reduced motion is requested', () => {
    const fixture = setUp({ reducedMotion: false })
    fixture.controller.setStandardView('top')
    fixture.frames.flush(2)
    const midway = fixture.controller.getCamera()
    expect(forward(midway)[2]).toBeGreaterThan(-1 + 1e-6)
    fixture.frames.flush(40)
    const done = fixture.controller.getCamera()
    expect(forward(done)[2]).toBeCloseTo(-1, 6)
  })

  it('changes the camera at once with reduced motion', () => {
    const fixture = setUp({ reducedMotion: true })
    fixture.controller.setStandardView('top')
    expect(forward(fixture.controller.getCamera())[2]).toBeCloseTo(-1, 6)
  })

  it('toggleProjection keeps the apparent size and mirrors the projection in the store', () => {
    const fixture = setUp()
    fixture.controller.toggleProjection()
    fixture.frames.flush()
    expect(
      cameraDifference(
        fixture.controller.getCamera(),
        withProjection(DEFAULT_CAMERA, 'orthographic'),
      ),
    ).toBeLessThan(1e-6)
    expect(fixture.app.store.getState().view.projection).toBe('orthographic')
    fixture.controller.toggleProjection()
    fixture.frames.flush()
    expect(fixture.app.store.getState().view.projection).toBe('perspective')
  })

  it('fit frames the visible bounds with margin 0.1', () => {
    const fixture = setUp()
    fixture.controller.fit()
    fixture.frames.flush()
    expect(
      cameraDifference(
        fixture.controller.getCamera(),
        fitBounds(DEFAULT_CAMERA, CUBE_BOUNDS, ASPECT, 0.1),
      ),
    ).toBeLessThan(1e-6)
  })

  it('fit frames a 100 mm cube at the origin when nothing is visible', () => {
    const fixture = setUp({ document: documentState(CUBE_FEATURES_DOCUMENT, { meshes: {} }) })
    fixture.app.store.setState({
      view: { ...fixture.app.store.getState().view, sessionVisible: { 'feature:f-sk1': false } },
    })
    fixture.scene.applyEvent(bodyMeshEvent({ body: 'b-cube', meshId: 2, mesh: null, revision: 2 }))
    fixture.controller.fit()
    fixture.frames.flush()
    expect(
      cameraDifference(
        fixture.controller.getCamera(),
        fitBounds(DEFAULT_CAMERA, { min: [-50, -50, -50], max: [50, 50, 50] }, ASPECT, 0.1),
      ),
    ).toBeLessThan(1e-6)
  })

  it('fit with bounds and zoomToBounds frame the given box with their margins', () => {
    const fixture = setUp()
    const box: BoundingBox = { min: [0, 0, 0], max: [4, 4, 4] }
    fixture.controller.fit(box)
    fixture.frames.flush()
    expect(
      cameraDifference(fixture.controller.getCamera(), fitBounds(DEFAULT_CAMERA, box, ASPECT, 0.1)),
    ).toBeLessThan(1e-6)
    fixture.controller.zoomToBounds(box, 0.3)
    fixture.frames.flush()
    expect(
      cameraDifference(fixture.controller.getCamera(), fitBounds(DEFAULT_CAMERA, box, ASPECT, 0.3)),
    ).toBeLessThan(1e-6)
  })

  it('lookAlong puts the eye on the normal side looking along -normal, with the up hint', () => {
    const fixture = setUp()
    fixture.controller.lookAlong([0, 0, 1], [0, 1, 0], [0, 0, 20])
    fixture.frames.flush()
    const camera = fixture.controller.getCamera()
    expect(forward(camera)[2]).toBeCloseTo(-1, 6)
    expect(up(camera)[1]).toBeCloseTo(1, 6)
    expect(camera.target).toEqual([0, 0, 20])
    expect(camera.distance).toBeCloseTo(DEFAULT_CAMERA.distance, 6)
    expect(camera.projection).toBe(DEFAULT_CAMERA.projection)
  })

  it('setCamera replaces the camera', () => {
    const fixture = setUp()
    const camera: Camera = { ...DEFAULT_CAMERA, target: [5, 5, 5], distance: 80 }
    fixture.controller.setCamera(camera, { animate: false })
    expect(cameraDifference(fixture.controller.getCamera(), camera)).toBeLessThan(1e-6)
  })
})

describe('navigation (3.13)', () => {
  it('zooms with the wheel at the cursor and prevents the page from scrolling', () => {
    const fixture = setUp()
    const allowed = fireEvent.wheel(fixture.canvas, {
      deltaY: 100,
      deltaMode: 0,
      clientX: WIDTH / 2,
      clientY: HEIGHT / 2,
    })
    fixture.frames.flush()
    expect(allowed).toBe(false)
    expect(fixture.controller.getCamera().distance).toBeCloseTo(
      DEFAULT_CAMERA.distance * 1.0015 ** 100,
      6,
    )
  })

  it('pans with a middle drag in the Fusion preset and prevents autoscroll', () => {
    const fixture = setUp()
    const press = {
      pointerId: 1,
      pointerType: 'mouse',
      button: 1,
      buttons: 4,
      clientX: 400,
      clientY: 300,
    }
    expect(fireEvent.pointerDown(fixture.canvas, press)).toBe(false)
    fireEvent.pointerMove(fixture.canvas, { ...press, clientX: 460, clientY: 300 })
    fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 460, clientY: 300 })
    fixture.frames.flush()
    const camera = fixture.controller.getCamera()
    expect(camera.rotation).toEqual(DEFAULT_CAMERA.rotation)
    expect(camera.target).not.toEqual(DEFAULT_CAMERA.target)
  })

  it('orbits with Shift and a middle drag in the Fusion preset', () => {
    const fixture = setUp()
    const press = {
      pointerId: 1,
      pointerType: 'mouse',
      button: 1,
      buttons: 4,
      shiftKey: true,
      clientX: 400,
      clientY: 300,
    }
    fireEvent.pointerDown(fixture.canvas, press)
    fireEvent.pointerMove(fixture.canvas, { ...press, clientX: 460 })
    fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 460 })
    fixture.frames.flush()
    expect(fixture.controller.getCamera().rotation).not.toEqual(DEFAULT_CAMERA.rotation)
  })

  it('orbits with middle and left buttons in the FreeCAD preset, middle pressed first', () => {
    const fixture = setUp({
      state: { settings: { ...DEFAULT_SETTINGS, theme: 'light', navigation: 'freecad' } },
    })
    const base = { pointerId: 1, pointerType: 'mouse', clientX: 400, clientY: 300 }
    fireEvent.pointerDown(fixture.canvas, { ...base, button: 1, buttons: 4 })
    fireEvent.pointerDown(fixture.canvas, { ...base, button: 0, buttons: 5 })
    fireEvent.pointerMove(fixture.canvas, { ...base, buttons: 5, clientX: 460 })
    fireEvent.pointerUp(fixture.canvas, { ...base, button: 0, buttons: 4, clientX: 460 })
    fireEvent.pointerUp(fixture.canvas, { ...base, button: 1, buttons: 0, clientX: 460 })
    fixture.frames.flush()
    expect(fixture.controller.getCamera().rotation).not.toEqual(DEFAULT_CAMERA.rotation)
  })

  it.each(['fusion', 'freecad'] as const)(
    'orbits with a right drag in the %s preset and opens no browser menu (3.13)',
    (navigation) => {
      const fixture = setUp({
        state: { settings: { ...DEFAULT_SETTINGS, theme: 'light', navigation } },
      })
      const press = {
        pointerId: 1,
        pointerType: 'mouse',
        button: 2,
        buttons: 2,
        clientX: 400,
        clientY: 300,
      }
      fireEvent.pointerDown(fixture.canvas, press)
      expect(fireEvent.contextMenu(fixture.canvas, press)).toBe(false)
      fireEvent.pointerMove(fixture.canvas, { ...press, clientX: 460 })
      fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 460 })
      fixture.frames.flush()
      expect(fixture.controller.getCamera().rotation).not.toEqual(DEFAULT_CAMERA.rotation)
    },
  )

  it.each(['fusion', 'freecad'] as const)(
    'pans with Shift and a right drag in the %s preset',
    (navigation) => {
      const fixture = setUp({
        state: { settings: { ...DEFAULT_SETTINGS, theme: 'light', navigation } },
      })
      const press = {
        pointerId: 1,
        pointerType: 'mouse',
        button: 2,
        buttons: 2,
        shiftKey: true,
        clientX: 400,
        clientY: 300,
      }
      fireEvent.pointerDown(fixture.canvas, press)
      fireEvent.pointerMove(fixture.canvas, { ...press, clientX: 460 })
      fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 460 })
      fixture.frames.flush()
      const camera = fixture.controller.getCamera()
      expect(camera.rotation).toEqual(DEFAULT_CAMERA.rotation)
      expect(camera.target).not.toEqual(DEFAULT_CAMERA.target)
    },
  )

  it('fits the view on a middle double-click in the Fusion preset', () => {
    const fixture = setUp()
    middleClick(fixture.canvas, 400, 300)
    middleClick(fixture.canvas, 401, 300)
    fixture.frames.flush()
    expect(
      cameraDifference(
        fixture.controller.getCamera(),
        fitBounds(DEFAULT_CAMERA, CUBE_BOUNDS, ASPECT, 0.1),
      ),
    ).toBeLessThan(1e-6)
  })

  it('does not fit on two middle clicks far apart in time', () => {
    const fixture = setUp()
    middleClick(fixture.canvas, 400, 300)
    // Each rendered frame advances the stubbed clock by 16 ms: 40 frames are 640 ms.
    for (let i = 0; i < 40; i++) {
      fixture.controller.requestRender()
      fixture.frames.flush(1)
    }
    middleClick(fixture.canvas, 400, 300)
    fixture.frames.flush()
    expect(cameraDifference(fixture.controller.getCamera(), DEFAULT_CAMERA)).toBeLessThan(1e-9)
  })

  it('does not fit on a middle double-click in the FreeCAD preset', () => {
    const fixture = setUp({
      state: { settings: { ...DEFAULT_SETTINGS, theme: 'light', navigation: 'freecad' } },
    })
    middleClick(fixture.canvas, 400, 300)
    middleClick(fixture.canvas, 400, 300)
    fixture.frames.flush()
    expect(cameraDifference(fixture.controller.getCamera(), DEFAULT_CAMERA)).toBeLessThan(1e-9)
  })

  it('records Home, a standard view and a fit, as one sketch view entry', async () => {
    const fixture = setUp()
    fixture.app.attachViewport(fixture.controller)
    await enterSketch(fixture.app, 'f-sk1')
    fixture.frames.flush()
    const before = fixture.controller.getCamera()
    fixture.controller.setStandardView('iso')
    fixture.controller.fit()
    fixture.frames.flush()
    await settle()
    expect(cameraDifference(fixture.controller.getCamera(), before)).toBeGreaterThan(1e-3)
    await undo(fixture.app)
    fixture.frames.flush()
    expect(cameraDifference(fixture.controller.getCamera(), before)).toBeLessThan(1e-6)
  })

  it('prevents the context menu on the canvas', () => {
    const fixture = setUp()
    expect(fireEvent.contextMenu(fixture.canvas)).toBe(false)
  })
})

describe('hover (3.6)', () => {
  it('picks under the pointer with a 4 pixel radius and sets the hover target', () => {
    const fixture = setUp()
    pickElement(fixture, 'face', 3)
    fireEvent.pointerMove(fixture.canvas, {
      pointerId: 1,
      pointerType: 'mouse',
      buttons: 0,
      clientX: 120,
      clientY: 80,
    })
    fixture.frames.flush()
    const pick = fixture.backend.calls.find((call) => call.method === 'pick')
    expect(pick).toEqual({ method: 'pick', x: 120, y: 80, radius: 4 })
    expect(fixture.app.store.getState().hover).toEqual({ key: 'mesh:1:face:3', kind: 'face' })
  })

  it('scales the pick radius with the UI scale', () => {
    const fixture = setUp({
      state: { settings: { ...DEFAULT_SETTINGS, theme: 'light', uiScale: 2 } },
    })
    fireEvent.pointerMove(fixture.canvas, {
      pointerId: 1,
      pointerType: 'mouse',
      buttons: 0,
      clientX: 10,
      clientY: 10,
    })
    fixture.frames.flush()
    expect(fixture.backend.calls.find((call) => call.method === 'pick')).toMatchObject({
      radius: 8,
    })
  })

  it('clears the hover over empty space', () => {
    const fixture = setUp({ state: { hover: { key: 'mesh:1:face:3', kind: 'face' } } })
    fireEvent.pointerMove(fixture.canvas, {
      pointerId: 1,
      pointerType: 'mouse',
      buttons: 0,
      clientX: 5,
      clientY: 5,
    })
    fixture.frames.flush()
    expect(fixture.app.store.getState().hover).toBeNull()
  })

  it('prefers a vertex over an edge over a face under the pointer', () => {
    const fixture = setUp()
    fixture.backend.setPickHandler(({ frame }) => {
      const base = (positions: Float32Array) =>
        frame.draws.find((draw) => draw.id === fixture.backend.idOf(positions))?.pickBase ?? 0
      return [
        { pickId: base(CUBE_MESH.positions) + 1, distance: 0, depth: 100 },
        { pickId: base(CUBE_MESH.edgePositions) + 2, distance: 1, depth: 100 },
        { pickId: base(CUBE_MESH.vertexPositions) + 5, distance: 3, depth: 100 },
      ]
    })
    fireEvent.pointerMove(fixture.canvas, {
      pointerId: 1,
      pointerType: 'mouse',
      buttons: 0,
      clientX: 50,
      clientY: 50,
    })
    fixture.frames.flush()
    expect(fixture.app.store.getState().hover).toEqual({ key: 'mesh:1:vertex:5', kind: 'vertex' })
  })
})

describe('hover while the camera moves (3.13)', () => {
  it('waits until the camera has been still for 100 ms before picking under the pointer', () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    cleanups.push(() => {
      vi.useRealTimers()
    })
    const fixture = setUp()
    let now = 5000
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    fireEvent.wheel(fixture.canvas, { deltaY: 100, deltaMode: 0, clientX: 400, clientY: 300 })
    fixture.frames.flush()
    const picks = () => fixture.backend.calls.filter((call) => call.method === 'pick').length
    const before = picks()
    now += 50
    fireEvent.pointerMove(fixture.canvas, {
      pointerId: 1,
      pointerType: 'mouse',
      buttons: 0,
      clientX: 120,
      clientY: 80,
    })
    fixture.frames.flush()
    expect(picks()).toBe(before)
    now += 50
    vi.advanceTimersByTime(50)
    fixture.frames.flush()
    expect(picks()).toBe(before + 1)
  })
})

describe('clicks (3.6)', () => {
  it('selects the picked face by its element name and highlights it', async () => {
    const fixture = setUp({
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    await settle()
    fixture.frames.flush()
    expect(fixture.core.callsTo('pickReference')).toEqual([
      [1, { mesh_id: 1, kind: 'face', index: 0 }],
    ])
    const selection = fixture.app.store.getState().selection
    expect(selection).toHaveLength(1)
    expect(selection[0]).toMatchObject({
      key: 'element:b-cube:f-ext1/end-cap',
      kind: 'face',
      body: 'b-cube',
      feature: 'f-ext1',
      element: CUBE_TOP_FACE,
    })
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.selected)).toEqual([0])
  })

  it('toggles the picked item with Shift', async () => {
    const fixture = setUp({
      state: { selection: [bodyItem('b-cube', 'Cube')] },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150, { shiftKey: true })
    await settle()
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual([
      'body:b-cube',
      'element:b-cube:f-ext1/end-cap',
    ])
    click(fixture.canvas, 200, 150, { shiftKey: true })
    await settle()
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
  })

  it('clears the selection on a click on empty space', async () => {
    const fixture = setUp({ state: { selection: [bodyItem('b-cube', 'Cube')] } })
    click(fixture.canvas, 5, 5)
    await settle()
    expect(fixture.app.store.getState().selection).toEqual([])
  })

  it('ignores a click whose pick went stale', async () => {
    const fixture = setUp({ state: { selection: [bodyItem('b-cube', 'Cube')] } })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
    expect(fixture.app.store.getState().notifications).toEqual([])
  })

  it('does not treat a press that moved more than 4 pixels as a click', async () => {
    const fixture = setUp({
      state: { selection: [bodyItem('b-cube', 'Cube')] },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    const init = { pointerId: 1, pointerType: 'mouse', button: 0 }
    fireEvent.pointerDown(fixture.canvas, { ...init, buttons: 1, clientX: 200, clientY: 150 })
    fireEvent.pointerMove(fixture.canvas, { ...init, buttons: 1, clientX: 210, clientY: 150 })
    fireEvent.pointerUp(fixture.canvas, { ...init, buttons: 0, clientX: 210, clientY: 150 })
    await settle()
    expect(fixture.core.callsTo('pickReference')).toEqual([])
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
  })

  it('treats a press that moved 4 pixels as a click', async () => {
    const fixture = setUp({
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    const init = { pointerId: 1, pointerType: 'mouse', button: 0 }
    fireEvent.pointerDown(fixture.canvas, { ...init, buttons: 1, clientX: 200, clientY: 150 })
    fireEvent.pointerMove(fixture.canvas, { ...init, buttons: 1, clientX: 204, clientY: 150 })
    fireEvent.pointerUp(fixture.canvas, { ...init, buttons: 0, clientX: 204, clientY: 150 })
    await settle()
    expect(fixture.core.callsTo('pickReference')).toHaveLength(1)
    expect(fixture.app.store.getState().selection.map((item) => item.kind)).toEqual(['face'])
  })

  it('sends clicks to the active selection field during a feature command', async () => {
    const fixture = setUp({
      state: { command: FILLET_EDGES_COMMAND },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FRONT_EDGE))
      },
    })
    pickElement(fixture, 'edge', 4)
    click(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.app.store.getState().selection).toEqual([])
    expect(fixture.app.store.getState().command?.params.edges).toEqual([
      CUBE_TOP_FRONT_EDGE.reference,
    ])
  })

  it('U-88 a double-click on a face whose descriptor has feature f-ext1 edits f-ext1', async () => {
    const fixture = setUp({
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    click(fixture.canvas, 200, 150)
    fireEvent.dblClick(fixture.canvas, { button: 0, clientX: 200, clientY: 150 })
    await waitFor(() => {
      expect(fixture.core.callsTo('setFocus')).toEqual([[1, 'f-ext1']])
    })
    expect(fixture.core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
  })

  it('a double-click on a face of a feature missing from the document edits nothing', async () => {
    const fixture = setUp({
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve({ ...CUBE_TOP_FACE, feature: 'f-gone' }))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    click(fixture.canvas, 200, 150)
    fireEvent.dblClick(fixture.canvas, { button: 0, clientX: 200, clientY: 150 })
    await settle()
    expect(fixture.core.callsTo('beginTransaction')).toEqual([])
  })
})

function featuresSetUp(
  listElements: (body: string) => Promise<readonly ElementDescriptor[]>,
): Fixture {
  return setUp({
    document: documentState(CUBE_FEATURES_DOCUMENT, {
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    }),
    prepare: (core) => {
      core.stub('listElements', (_documentHandle, body) => listElements(body))
    },
  })
}

async function select(fixture: Fixture, id: string, label: string): Promise<void> {
  fixture.app.store.setState({ selection: [featureItem(id, label)] })
  await settle()
  fixture.frames.flush()
}

describe('feature highlighting (3.6)', () => {
  it('U-93 selecting f-ext1 lists the cube faces once and selects exactly the faces f-ext1 made', async () => {
    const fixture = featuresSetUp(() => Promise.resolve(FILLETED_CUBE_FACES))
    await select(fixture, 'f-ext1', 'Extrude 1')
    expect(fixture.core.callsTo('listElements')).toEqual([[1, 'b-cube', 'face']])
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.selected)).toEqual([0, 2, 3, 5])
  })

  it('U-93 selecting it again with the same mesh id sends no request', async () => {
    const fixture = featuresSetUp(() => Promise.resolve(FILLETED_CUBE_FACES))
    await select(fixture, 'f-ext1', 'Extrude 1')
    await select(fixture, 'f-fil1', 'Fillet 1')
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.selected)).toEqual([1, 4])
    await select(fixture, 'f-ext1', 'Extrude 1')
    expect(fixture.core.callsTo('listElements')).toHaveLength(1)
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.selected)).toEqual([0, 2, 3, 5])
  })

  it('U-93 a new mesh for b-cube arriving before the response discards it and requests again', async () => {
    const answers = [
      deferred<readonly ElementDescriptor[]>(),
      deferred<readonly ElementDescriptor[]>(),
    ]
    let call = 0
    const fixture = featuresSetUp(() => answers[call++]?.promise ?? Promise.resolve([]))
    await select(fixture, 'f-ext1', 'Extrude 1')
    expect(fixture.core.callsTo('listElements')).toHaveLength(1)
    const rebuilt = boxMesh([-10, -10, 0], [10, 10, 20])
    fixture.scene.applyEvent(
      bodyMeshEvent({ body: 'b-cube', meshId: 2, mesh: rebuilt, revision: 2 }),
    )
    setDocument(fixture.app, { revision: 2, meshes: { 'b-cube': meshSummary(2, CUBE_BOUNDS) } })
    answers[0]?.resolve(CUBE_FACES)
    await settle()
    fixture.frames.flush()
    expect(fixture.core.callsTo('listElements')).toHaveLength(2)
    expect(indicesWith(fixture, rebuilt.positions, ELEMENT_STATE.selected)).toEqual([])
    answers[1]?.resolve(FILLETED_CUBE_FACES)
    await settle()
    fixture.frames.flush()
    expect(indicesWith(fixture, rebuilt.positions, ELEMENT_STATE.selected)).toEqual([0, 2, 3, 5])
  })

  it('U-93 selecting the sketch f-sk1 marks no face', async () => {
    const fixture = featuresSetUp(() => Promise.resolve(FILLETED_CUBE_FACES))
    await select(fixture, 'f-sk1', 'Sketch 1')
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.selected)).toEqual([])
  })

  it('highlights nothing for selected features while a feature command is active', async () => {
    const fixture = featuresSetUp(() => Promise.resolve(FILLETED_CUBE_FACES))
    fixture.app.store.setState({ command: FILLET_EDGES_COMMAND })
    await select(fixture, 'f-ext1', 'Extrude 1')
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.selected)).toEqual([])
  })
})

describe('selection after rebuilds (3.6)', () => {
  it('U-82 drops a selected face that resolves missing and keeps one that resolves', async () => {
    const fixture = setUp({
      state: { selection: [elementItem(CUBE_TOP_FACE), elementItem(CUBE_FRONT_FACE)] },
      prepare: (core) => {
        core.stub('resolveReferences', (_documentHandle, references: readonly ElementRef[]) =>
          Promise.resolve(
            references.map((reference): Resolution =>
              reference.reference.q === CUBE_FRONT_FACE.name
                ? {
                    status: 'resolved',
                    matches: [{ body: 'b-cube', mesh_id: 2, kind: 'face', index: 1 }],
                  }
                : { status: 'missing', matches: [] },
            ),
          ),
        )
      },
    })
    const rebuilt = boxMesh([-10, -10, 0], [10, 10, 25])
    fixture.scene.applyEvent(
      bodyMeshEvent({ body: 'b-cube', meshId: 2, mesh: rebuilt, revision: 2 }),
    )
    setDocument(fixture.app, { revision: 2, meshes: { 'b-cube': meshSummary(2, CUBE_BOUNDS) } })
    await settle()
    fixture.frames.flush()
    expect(fixture.core.callsTo('resolveReferences').at(-1)).toEqual([
      1,
      [
        { body: 'b-cube', reference: CUBE_TOP_FACE.reference },
        { body: 'b-cube', reference: CUBE_FRONT_FACE.reference },
      ],
    ])
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual([
      'element:b-cube:f-ext1/side-0',
    ])
    expect(indicesWith(fixture, rebuilt.positions, ELEMENT_STATE.selected)).toEqual([1])
  })
})

/** An edge descriptor named `name`; its list position is its display id. */
function edgeNamed(name: string): ElementDescriptor {
  return { ...CUBE_TOP_FRONT_EDGE, name, reference: { q: name, fp: `fp-${name}` } }
}

/** Moves the pointer over face 0 of the cube. */
function hoverFaceZero(fixture: Fixture): void {
  pickElement(fixture, 'face', 0)
  fireEvent.pointerMove(fixture.canvas, {
    pointerId: 1,
    pointerType: 'mouse',
    buttons: 0,
    clientX: 120,
    clientY: 80,
  })
  fixture.frames.flush()
}

describe('faces hovered as their edges (3.7)', () => {
  // Display edge ids 0 to 11; the top face's 4 edges are at positions 2, 5, 7 and 11.
  const EDGES = Array.from({ length: 12 }, (_, index) => {
    const top = [2, 5, 7, 11].indexOf(index)
    return edgeNamed(top === -1 ? `edge-${String(index)}` : (CUBE_TOP_FACE.adjacent[top] ?? ''))
  })

  function setUpFillet(): Fixture {
    return setUp({
      state: { command: FILLET_EDGES_COMMAND },
      prepare: (core) => {
        core.stub('listElements', (_handle, _body, kind) =>
          Promise.resolve(kind === 'edge' ? EDGES : CUBE_FACES),
        )
      },
    })
  }

  it('lights up the edges around a hovered face instead of the face in a Fillet', async () => {
    const fixture = setUpFillet()
    hoverFaceZero(fixture)
    await settle()
    fixture.frames.flush()
    expect(indicesWith(fixture, CUBE_MESH.edgePositions, ELEMENT_STATE.hover)).toEqual([
      2, 5, 7, 11,
    ])
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.hover)).toEqual([])
  })

  it('lists the faces and edges of a body once per mesh', async () => {
    const fixture = setUpFillet()
    hoverFaceZero(fixture)
    await settle()
    hoverFaceZero(fixture)
    await settle()
    expect(fixture.core.callsTo('listElements').map((call) => call[2])).toEqual(['face', 'edge'])
  })

  it('hovers the face itself when the active field takes faces', async () => {
    const fixture = setUp({
      state: {
        command: {
          ...FILLET_EDGES_COMMAND,
          featureType: 'shell',
          params: { thickness: '2 mm' },
          activeField: 'remove_faces',
        },
      },
    })
    hoverFaceZero(fixture)
    await settle()
    fixture.frames.flush()
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.hover)).toEqual([0])
    expect(fixture.core.callsTo('listElements')).toEqual([])
  })
})

describe('a hovered selection field box (3.7)', () => {
  it("draws the field's edges hovered while the pointer is over its box", async () => {
    const fixture = setUp({
      state: {
        command: {
          ...FILLET_EDGES_COMMAND,
          params: { radius: '2 mm', body: 'b-cube', edges: [CUBE_TOP_FRONT_EDGE.reference] },
        },
      },
      prepare: (core) => {
        core.stub('resolveReferences', (_documentHandle, references: readonly ElementRef[]) =>
          Promise.resolve(
            references.map((): Resolution => ({
              status: 'resolved',
              matches: [{ body: 'b-cube', mesh_id: 1, kind: 'edge', index: 4 }],
            })),
          ),
        )
      },
    })
    await settle()
    fixture.frames.flush()
    expect(indicesWith(fixture, CUBE_MESH.edgePositions, ELEMENT_STATE.selected)).toEqual([4])
    expect(indicesWith(fixture, CUBE_MESH.edgePositions, ELEMENT_STATE.hover)).toEqual([])
    setHoveredField(fixture.app, 'edges')
    fixture.frames.flush()
    expect(indicesWith(fixture, CUBE_MESH.edgePositions, ELEMENT_STATE.hover)).toEqual([4])
    setHoveredField(fixture.app, null)
    fixture.frames.flush()
    expect(indicesWith(fixture, CUBE_MESH.edgePositions, ELEMENT_STATE.hover)).toEqual([])
  })

  it("draws the field's body hovered for a body field", async () => {
    const fixture = setUp({
      state: {
        command: {
          ...FILLET_EDGES_COMMAND,
          featureType: 'shell',
          params: { thickness: '2 mm', body: 'b-cube' },
          activeField: null,
          hoveredField: 'body',
        },
      },
    })
    await settle()
    fixture.frames.flush()
    expect(indicesWith(fixture, CUBE_MESH.positions, ELEMENT_STATE.hover).length).toBeGreaterThan(0)
  })
})

describe('clicks into a command (3.7)', () => {
  it('a face clicked for its edges fills Edges and resolves them through the core', async () => {
    const edges = [0, 1, 2, 3].map((index) => edgeNamed(CUBE_TOP_FACE.adjacent[index] ?? ''))
    const fixture = setUp({
      state: { command: FILLET_EDGES_COMMAND },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
        core.stub('listElements', () => Promise.resolve(edges))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    await settle()
    fixture.frames.flush()
    expect(fixture.app.store.getState().command?.params['edges']).toEqual(
      edges.map((edge) => edge.reference),
    )
    // The four edges are not taken for the clicked face: the core resolves them.
    const [, references] = fixture.core.callsTo('resolveReferences').at(-1) ?? []
    expect(references).toHaveLength(4)
  })

  it('places a hole center where the click ray meets a flat face', async () => {
    const fixture = setUp({
      state: {
        command: {
          ...FILLET_EDGES_COMMAND,
          featureType: 'hole',
          title: 'Hole',
          params: { diameter: '3.2 mm', extent: { kind: 'through_all' } },
          activeField: 'points',
        },
      },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    const helper = fixture.app.store.getState().command?.helper
    const [entity] = helper?.params.entities ?? []
    expect(entity?.type).toBe('point')
    expect(helper?.params.plane).toMatchObject({ kind: 'face', body: 'b-cube' })
  })
})

describe('clicks while a plane is picked for a new sketch (3.16.1)', () => {
  const PLANE_PICKING: Partial<AppState> = {
    context: 'sketch',
    sketch: {
      sketchId: null,
      tool: null,
      construction: false,
      showConstraints: true,
      selection: [],
      dragging: false,
      cameraBefore: null,
    },
  }

  it('starts the sketch on a flat face', async () => {
    const fixture = setUp({
      state: PLANE_PICKING,
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    const [call] = fixture.core.callsTo('applyEdits')
    expect(call?.[1]).toBe('New sketch')
    expect(call?.[2]).toMatchObject([
      { type: 'add_feature', feature: { type: 'sketch', plane: { kind: 'face', body: 'b-cube' } } },
    ])
  })

  it('refuses a face that is not flat with pick_plane in the sketch hint, not a notification', async () => {
    const fixture = setUp({
      state: PLANE_PICKING,
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve({ ...CUBE_TOP_FACE, surface: 'cylinder' }))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    const state = fixture.app.store.getState()
    expect(state.pickHint).toEqual({ message: MESSAGES.pick_plane(), field: null })
    expect(state.notifications).toEqual([])
    expect(fixture.core.callsTo('applyEdits')).toEqual([])
  })
})

function layOnFaceState(): Partial<AppState> {
  return {
    context: 'print',
    print: { ...INITIAL_STATE.print, pickingFace: true },
  }
}

describe('clicks while Lay on face picks a face (3.17)', () => {
  it('lays a clicked flat face on the bed instead of selecting it', async () => {
    const fixture = setUp({
      state: layOnFaceState(),
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    const state = fixture.app.store.getState()
    expect(state.print).toMatchObject({
      pickingFace: false,
      orientation: { kind: 'face_down', body: 'b-cube', face: CUBE_TOP_FACE.reference },
    })
    expect(state.selection).toEqual([])
  })

  it('refuses a face that is not flat with pick_flat_face and keeps picking', async () => {
    const fixture = setUp({
      state: layOnFaceState(),
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve({ ...CUBE_TOP_FACE, surface: 'cylinder' }))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    const state = fixture.app.store.getState()
    expect(state.pickHint).toEqual({ message: MESSAGES.pick_flat_face(), field: null })
    expect(state.print).toMatchObject({ pickingFace: true, orientation: null })
  })

  it('accepts faces only: an edge under the pointer is not picked', async () => {
    const fixture = setUp({ state: layOnFaceState() })
    pickElement(fixture, 'edge', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    expect(fixture.core.callsTo('pickReference')).toEqual([])
    expect(fixture.app.store.getState().print.pickingFace).toBe(true)
  })
})

/** A right press and release at one point. */
function rightClick(canvas: HTMLCanvasElement, x: number, y: number): void {
  const init = { pointerId: 1, pointerType: 'mouse', button: 2, clientX: x, clientY: y }
  fireEvent.pointerDown(canvas, { ...init, buttons: 2 })
  fireEvent.contextMenu(canvas, init)
  fireEvent.pointerUp(canvas, { ...init, buttons: 0 })
}

function menuRequests(app: App): { readonly x: number; readonly y: number }[] {
  const requests: { x: number; y: number }[] = []
  cleanups.push(
    onUiRequest(app, (request) => {
      if (request.kind === 'viewport_menu') {
        requests.push({ x: request.x, y: request.y })
        return true
      }
      return false
    }),
  )
  return requests
}

describe('right clicks and the context menu (3.13)', () => {
  it('selects an unselected face under the pointer, as a click would, then opens the menu there', async () => {
    const fixture = setUp({
      state: { selection: [bodyItem('b-cube', 'Cube')] },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    const requests = menuRequests(fixture.app)
    pickElement(fixture, 'face', 0)
    rightClick(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual([
      'element:b-cube:f-ext1/end-cap',
    ])
    expect(requests).toEqual([{ x: 200, y: 150 }])
  })

  it('keeps a selection that already holds the item under the pointer', async () => {
    const fixture = setUp({
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    await settle()
    const top = fixture.app.store.getState().selection
    const front = elementItem(CUBE_FRONT_FACE)
    fixture.app.store.setState({ selection: [...top, front] })
    await settle()
    const requests = menuRequests(fixture.app)
    rightClick(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.app.store.getState().selection.map((item) => item.key)).toEqual([
      'element:b-cube:f-ext1/end-cap',
      front.key,
    ])
    expect(requests).toHaveLength(1)
  })

  it('opens the menu without picking during a feature command', async () => {
    const fixture = setUp({ state: { command: FILLET_EDGES_COMMAND } })
    const requests = menuRequests(fixture.app)
    pickElement(fixture, 'edge', 0)
    rightClick(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.core.callsTo('pickReference')).toEqual([])
    expect(requests).toEqual([{ x: 200, y: 150 }])
  })

  it('opens no menu after a right drag, which orbits', async () => {
    const fixture = setUp()
    const requests = menuRequests(fixture.app)
    const press = { pointerId: 1, pointerType: 'mouse', button: 2, clientX: 400, clientY: 300 }
    fireEvent.pointerDown(fixture.canvas, { ...press, buttons: 2 })
    fireEvent.pointerMove(fixture.canvas, { ...press, buttons: 2, clientX: 460 })
    fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 460 })
    await settle()
    expect(requests).toEqual([])
  })
})
