import { fireEvent, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  cancelCommand,
  confirmCommand,
  setHoveredField,
  startFeatureCommand,
} from '../app/actions/command'
import { undo } from '../app/actions/document'
import { select as selectItems } from '../app/actions/selection'
import { enterSketch } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import type { BodyMeshData } from '../core/events'
import type {
  BoundingBox,
  ElementDescriptor,
  ElementRef,
  FeatureRecord,
  Resolution,
} from '../generated/protocol'
import {
  activeCommandWith,
  bodyMeshEvent,
  boxMesh,
  CUBE_FACES,
  cubeDocumentState,
  documentState,
  FILLET_EDGES_COMMAND,
  FILLETED_CUBE_FACES,
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
import { planePickingSession } from '../sketch/fixtures.testing'
import {
  INITIAL_STATE,
  type ActiveCommand,
  type AppState,
  type DocumentState,
} from '../state/store'
import { createFakeBackend, type FakeBackend } from '../test/fakes/backend'
import type { FakeCoreApi } from '../test/fakes/core-api'
import { deferred } from '../test/fakes/deferred'
import {
  CUBE_EXTRUDE,
  CUBE_FEATURES_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_TOP_FACE,
  CUBE_TOP_FACE_MOVED,
  CUBE_TOP_FRONT_EDGE,
} from '../test/fakes/documents'
import { attachCall, cubeWithSketch, lostFaceSketch } from '../test/fakes/repairs'
import { bodyItem, elementItem, featureItem } from '../test/fakes/selection'
import { stubResizeObserver, type FakeResizeObservers } from '../test/fakes/resize-observer'
import { settle } from '../test/fakes/settle'
import { CUBE_BOUNDS, meshSummary } from '../test/fakes/state'
import { ELEMENT_STATE, type RenderFrame } from './backend'
import {
  DEFAULT_CAMERA,
  eye,
  fitBounds,
  forward,
  orbit,
  standardViewRotation,
  up,
  windowProjection,
  withProjection,
  type Camera,
} from './camera'
import { createViewportController, type ViewportController } from './controller'
import { drawnGridSpacing } from './grid'
import { createScene, type Scene } from './scene'
import { worldToCanvas } from './projection.testing'

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
  /** The free-area element (3.2); unmeasured (an empty box) unless `free` is given. */
  readonly freeArea: HTMLElement
  /** Changes the free area's box relative to the canvas; the next measurement reads it. */
  setFree(next: Box): void
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
  /** The canvas size in CSS pixels; 800 by 600 by default. */
  readonly size?: { readonly width: number; readonly height: number }
  /** The free area's box relative to the canvas at (0, 0); empty, so unmeasured, by default. */
  readonly free?: Box
}

type Box = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** Stubs an element's bounding box, which the test can change through the returned setter. */
function stubBox(element: Element, initial: Box): (next: Box) => void {
  let current = initial
  vi.spyOn(element, 'getBoundingClientRect').mockImplementation(() => DOMRect.fromRect(current))
  return (next) => {
    current = next
  }
}

function setUp(options: SetUpOptions = {}): Fixture {
  stubMatchMedia({ reducedMotion: options.reducedMotion ?? true })
  const frames = stubAnimationFrames()
  cleanups.push(
    stubElementSize(HTMLCanvasElement.prototype, options.size ?? { width: WIDTH, height: HEIGHT }),
  )
  const { app, core } = createModelApp({
    state: readyState(options.document ?? cubeDocumentState(), options.state),
  })
  options.prepare?.(core)
  const viewport = document.createElement('main')
  viewport.dataset.testid = 'viewport'
  const canvas = document.createElement('canvas')
  const overlay = document.createElement('div')
  const freeArea = document.createElement('div')
  viewport.append(canvas, overlay, freeArea)
  document.body.append(viewport)
  const setFree = stubBox(freeArea, options.free ?? { x: 0, y: 0, width: 0, height: 0 })
  const backend = createFakeBackend()
  const scene = createScene(backend)
  scene.applyEvent(bodyMeshEvent({ body: 'b-cube', meshId: 1, mesh: CUBE_MESH }))
  const controller = createViewportController({ app, canvas, overlay, freeArea, backend, scene })
  cleanups.push(() => {
    controller.dispose()
    viewport.remove()
  })
  frames.flush()
  return { app, core, backend, scene, canvas, overlay, freeArea, setFree, controller, frames }
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
    expect(frame?.theme.background).not.toEqual(fixture.backend.frames[0]?.theme.background)
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

  it('itemBounds is the world box of items as drawn, and null for items that are not (4.2)', () => {
    const fixture = setUp()
    fixture.frames.flush()
    expect(fixture.controller.itemBounds([bodyItem('b-cube', 'Cube')])).toEqual({
      min: [-10, -10, 0],
      max: [10, 10, 20],
    })
    expect(fixture.controller.itemBounds([featureItem('f-ext1', 'Extrude 1')])).toBeNull()
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

describe('ps:frame (2.18)', () => {
  const FRAME = 'ps:frame'
  /** A Shift and middle press at the canvas center: an orbit in the Fusion preset. */
  const ORBIT = {
    pointerId: 1,
    pointerType: 'mouse',
    button: 1,
    buttons: 4,
    shiftKey: true,
    clientX: 400,
    clientY: 300,
  }

  function frameDurations(): number[] {
    return performance.getEntriesByName(FRAME, 'measure').map((entry) => entry.duration)
  }

  /**
   * A fixture whose `performance.now()` stands still after mounting, so only the animation
   * frames' times, 16 ms apart, can give a measure a duration.
   */
  function setUpStillClock(): Fixture {
    const fixture = setUp()
    vi.spyOn(performance, 'now').mockReturnValue(performance.now())
    performance.clearMeasures(FRAME)
    return fixture
  }

  /** Moves the pressed pointer to each x in turn, one animation frame per move. */
  function orbitThrough(fixture: Fixture, xs: readonly number[]): void {
    for (const x of xs) {
      fireEvent.pointerMove(fixture.canvas, { ...ORBIT, clientX: x })
      fixture.frames.flush(1)
    }
  }

  it('U-164 measures from one frame time to the next while an orbit moves the camera', () => {
    const fixture = setUpStillClock()
    fireEvent.pointerDown(fixture.canvas, ORBIT)
    orbitThrough(fixture, [420, 440, 460])
    expect(frameDurations()).toEqual([16, 16])
  })

  it('U-164 records none for a redraw with a still camera and no drag', () => {
    const fixture = setUpStillClock()
    fireEvent.pointerDown(fixture.canvas, ORBIT)
    orbitThrough(fixture, [420])
    fireEvent.pointerUp(fixture.canvas, { ...ORBIT, buttons: 0, clientX: 420 })
    // A hover highlight right after the last moving frame.
    const rendered = fixture.backend.frames.length
    fixture.app.store.setState({ hover: { key: 'mesh:1:face:3', kind: 'face' } })
    fixture.frames.flush(1)
    expect(fixture.backend.frames).toHaveLength(rendered + 1)
    // The wheel moves the camera again: its first frame starts a new motion.
    fireEvent.wheel(fixture.canvas, { deltaY: 100, deltaMode: 0, clientX: 400, clientY: 300 })
    fixture.frames.flush(1)
    expect(frameDurations()).toEqual([])
  })

  it('U-164 ends the motion at an animation frame with nothing to draw', () => {
    const fixture = setUpStillClock()
    fireEvent.pointerDown(fixture.canvas, ORBIT)
    orbitThrough(fixture, [420])
    // The pointer rests for one frame.
    fixture.frames.flush(1)
    orbitThrough(fixture, [440, 460])
    expect(frameDurations()).toEqual([16])
  })

  it('U-164 measures frames a drag renders while the camera stays still', () => {
    const fixture = setUpStillClock()
    const camera = fixture.controller.getCamera()
    const press = { ...ORBIT, button: 0, buttons: 1, shiftKey: false }
    fireEvent.pointerDown(fixture.canvas, press)
    fireEvent.pointerMove(fixture.canvas, { ...press, clientX: 460 })
    for (let i = 0; i < 3; i++) {
      fixture.controller.requestRender()
      fixture.frames.flush(1)
    }
    fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 460 })
    fixture.controller.requestRender()
    fixture.frames.flush()
    expect(fixture.controller.getCamera()).toBe(camera)
    expect(frameDurations()).toEqual([16, 16])
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

  it('a double-click on a face while OK runs edits its feature once the commit has landed (3.7)', async () => {
    const fixture = setUp({
      state: {
        command: {
          ...FILLET_EDGES_COMMAND,
          featureType: 'datum_plane',
          title: 'Datum plane',
          params: { base: { kind: 'origin', plane: 'XY' }, offset: '10 mm' },
          activeField: null,
          applied: true,
        },
      },
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(CUBE_TOP_FACE))
      },
    })
    const committed = deferred<{ revision: number; recorded: boolean }>()
    fixture.core.stub('commitTransaction', () => committed.promise)
    const confirmed = confirmCommand(fixture.app)
    await settle()
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 200, 150)
    click(fixture.canvas, 200, 150)
    fireEvent.dblClick(fixture.canvas, { button: 0, clientX: 200, clientY: 150 })
    committed.resolve({ revision: 2, recorded: true })
    await confirmed
    await waitFor(() => {
      expect(fixture.core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
    })
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

/** Extrude 1 of cube.ptsm edited, as `editFeature` opens it: applied, its preview drawn. */
const EDITING_EXTRUDE = activeCommandWith({
  featureType: 'extrude',
  featureId: 'f-ext1',
  title: 'Extrude 1',
  editing: true,
  applied: true,
  preview: 'shown',
  params: {
    body: CUBE_EXTRUDE['body'],
    profile: CUBE_EXTRUDE['profile'],
    extent: CUBE_EXTRUDE['extent'],
    op: CUBE_EXTRUDE['op'],
  },
})

/** A Fillet on the cube's top front edge: complete, its preview drawn. */
const COMPLETE_FILLET: ActiveCommand = {
  ...FILLET_EDGES_COMMAND,
  params: { body: 'b-cube', radius: '2 mm', edges: [CUBE_TOP_FRONT_EDGE.reference] },
  applied: true,
  preview: 'shown',
}

const TOP_FACE_KEY = `element:b-cube:${CUBE_TOP_FACE.name}`

type EndHeld = {
  readonly fixture: Fixture
  /** Answers the command's held cancel or commit; the cube's model mesh is drawn from then on. */
  answerEnd(): void
  /** The frames the backend picked in, oldest first. */
  readonly pickFrames: readonly RenderFrame[]
}

/**
 * The cube with `command` active and the core's answer to its cancel or commit held. Until that
 * answer, the backend picks nothing where the cube is, as the scene draws the cube's model mesh
 * only once the answer's events arrive (3.6); after it, the top face. `pickReference` answers
 * with `pick` (the top face by default).
 */
function withEndHeld(
  command: ActiveCommand,
  pick: () => Promise<ElementDescriptor> = () => Promise.resolve(CUBE_TOP_FACE),
): EndHeld {
  let drawn = false
  const ended = deferred<undefined>()
  const fixture = setUp({
    state: { command },
    prepare: (core) => {
      core.stub('pickReference', pick)
      core.stub('cancelTransaction', async () => {
        await ended.promise
        drawn = true
        return 2
      })
      core.stub('commitTransaction', async () => {
        await ended.promise
        drawn = true
        return { revision: 2, recorded: true }
      })
    },
  })
  const pickFrames: RenderFrame[] = []
  fixture.backend.setPickHandler(({ frame }) => {
    pickFrames.push(frame)
    const draw = frame.draws.find(
      (candidate) => candidate.id === fixture.backend.idOf(CUBE_MESH.positions),
    )
    return !drawn || draw === undefined || draw.pickBase === null
      ? []
      : [{ pickId: draw.pickBase, distance: 0, depth: 100 }]
  })
  return {
    fixture,
    pickFrames,
    answerEnd: () => {
      ended.resolve(undefined)
    },
  }
}

function doubleClick(canvas: HTMLCanvasElement, x: number, y: number): void {
  click(canvas, x, y)
  click(canvas, x, y)
  fireEvent.dblClick(canvas, { button: 0, clientX: x, clientY: y })
}

function selectionKeys(fixture: Fixture): string[] {
  return fixture.app.store.getState().selection.map((item) => item.key)
}

function shownCodes(fixture: Fixture): (string | undefined)[] {
  return fixture.app.store.getState().notifications.map((notification) => notification.code)
}

/** The index in the core's calls of the first call to `method` that matches `matches`, or -1. */
function callIndex(
  fixture: Fixture,
  method: keyof App['core'],
  matches: (args: readonly unknown[]) => boolean = () => true,
): number {
  return fixture.core.calls.findIndex((call) => call.method === method && matches(call.args))
}

describe('U-242 clicks in the view land in the order they were made (3.6, 3.7 step 1)', () => {
  it('U-242 (a) a click right after Esc is picked once the cancel is answered, against the model it restores', async () => {
    const { fixture, answerEnd } = withEndHeld(EDITING_EXTRUDE)
    const canceled = cancelCommand(fixture.app)
    click(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.core.callsTo('pickReference')).toEqual([])
    answerEnd()
    await canceled
    await settle()
    expect(fixture.core.callsTo('pickReference')).toEqual([
      [1, { mesh_id: 1, kind: 'face', index: 0 }],
    ])
    expect(selectionKeys(fixture)).toEqual([TOP_FACE_KEY])
  })

  it('U-242 (b) a click while OK runs is picked once the commit is answered, against the confirmed model', async () => {
    const { fixture, answerEnd } = withEndHeld(COMPLETE_FILLET)
    const confirmed = confirmCommand(fixture.app)
    await settle()
    click(fixture.canvas, 200, 150)
    await settle()
    answerEnd()
    await confirmed
    await settle()
    expect(fixture.app.store.getState().command).toBeNull()
    expect(selectionKeys(fixture)).toEqual([TOP_FACE_KEY])
  })

  it('U-242 (c) a double-click right after Esc edits what it finds once the cancel is answered', async () => {
    const { fixture, answerEnd } = withEndHeld(EDITING_EXTRUDE)
    const canceled = cancelCommand(fixture.app)
    doubleClick(fixture.canvas, 200, 150)
    await settle()
    expect(fixture.core.callsTo('beginTransaction')).toEqual([])
    answerEnd()
    await canceled
    await settle()
    expect(fixture.core.callsTo('beginTransaction')).toEqual([[1, 'Edit Extrude 1']])
  })

  it('U-242 (d) guard: "Select Cube" made while a click waits for the cancel lands after that click (E-89)', async () => {
    const picked = deferred<ElementDescriptor>()
    const { fixture, answerEnd } = withEndHeld(EDITING_EXTRUDE, () => picked.promise)
    const canceled = cancelCommand(fixture.app)
    click(fixture.canvas, 200, 150)
    selectItems(fixture.app, [bodyItem('b-cube', 'Cube')], 'replace')
    answerEnd()
    await canceled
    await settle()
    picked.resolve(CUBE_TOP_FACE)
    await settle()
    expect(selectionKeys(fixture)).toEqual(['body:b-cube'])
  })

  it('U-242 (e) a click that waits for the cancel is picked with the camera of the click', async () => {
    const { fixture, answerEnd, pickFrames } = withEndHeld(EDITING_EXTRUDE)
    const atClick = fixture.controller.getCamera()
    const canceled = cancelCommand(fixture.app)
    click(fixture.canvas, 200, 150)
    fixture.controller.setCamera(orbit(atClick, 90, 0, [0, 0, 10]))
    fixture.frames.flush()
    answerEnd()
    await canceled
    await settle()
    expect(pickFrames).toHaveLength(1)
    const expected = eye(atClick)
    pickFrames[0]?.eye.forEach((value, axis) => {
      expect(value).toBeCloseTo(expected[axis] ?? Number.NaN, 6)
    })
    expect(selectionKeys(fixture)).toEqual([TOP_FACE_KEY])
  })

  it('U-242 (f) Sketch pressed after a click that waits for the cancel puts the new sketch on that face', async () => {
    const picked = deferred<ElementDescriptor>()
    const { fixture, answerEnd } = withEndHeld(EDITING_EXTRUDE, () => picked.promise)
    const canceled = cancelCommand(fixture.app)
    click(fixture.canvas, 200, 150)
    const started = startFeatureCommand(fixture.app, 'sketch')
    answerEnd()
    await canceled
    await settle()
    picked.resolve(CUBE_TOP_FACE)
    await started
    await settle()
    const changes = fixture.core.callsTo('applyEdits')
    expect(changes.map((call) => call[1])).toEqual(['New sketch'])
    expect(changes[0]?.[2]).toMatchObject([
      {
        type: 'add_feature',
        feature: {
          type: 'sketch',
          plane: { kind: 'face', body: 'b-cube', face: CUBE_TOP_FACE.reference },
        },
      },
    ])
  })

  it('U-242 (g) Shell started after a click that waits for the cancel preselects that face', async () => {
    const picked = deferred<ElementDescriptor>()
    const { fixture, answerEnd } = withEndHeld(EDITING_EXTRUDE, () => picked.promise)
    const canceled = cancelCommand(fixture.app)
    click(fixture.canvas, 200, 150)
    const started = startFeatureCommand(fixture.app, 'shell')
    answerEnd()
    await canceled
    await settle()
    picked.resolve(CUBE_TOP_FACE)
    await started
    await settle()
    const command = fixture.app.store.getState().command
    expect(command).toMatchObject({ featureType: 'shell', history: [] })
    expect(command?.params['remove_faces']).toEqual([CUBE_TOP_FACE.reference])
  })

  it('U-242 (h) a feature key pressed after a click on a face during plane picking opens after the new sketch', async () => {
    const picked = deferred<ElementDescriptor>()
    const fixture = setUp({
      state: { context: 'sketch', sketch: planePickingSession() },
      prepare: (core) => {
        core.stub('pickReference', () => picked.promise)
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    const started = startFeatureCommand(fixture.app, 'extrude')
    picked.resolve(CUBE_TOP_FACE)
    await started
    await settle()
    const sketched = callIndex(fixture, 'applyEdits', (args) => args[1] === 'New sketch')
    expect(sketched).toBeGreaterThanOrEqual(0)
    expect(sketched).toBeLessThan(callIndex(fixture, 'beginTransaction'))
    expect(fixture.app.store.getState()).toMatchObject({ context: 'model', sketch: null })
    expect(fixture.app.store.getState().command?.featureType).toBe('extrude')
    expect(shownCodes(fixture)).not.toContain('command_canceled')
  })

  it('U-242 (i) a feature key pressed after a double-click on a face opens after the edit it asks for', async () => {
    const picked = deferred<ElementDescriptor>()
    const fixture = setUp({
      prepare: (core) => {
        core.stub('pickReference', () => picked.promise)
      },
    })
    pickElement(fixture, 'face', 0)
    doubleClick(fixture.canvas, 200, 150)
    const started = startFeatureCommand(fixture.app, 'fillet')
    picked.resolve(CUBE_TOP_FACE)
    await started
    await settle()
    expect(fixture.core.callsTo('beginTransaction')).toEqual([
      [1, 'Edit Extrude 1'],
      [1, 'Fillet'],
    ])
    expect(fixture.app.store.getState().command?.featureType).toBe('fillet')
    expect(shownCodes(fixture)).not.toContain('command_canceled')
  })

  it('U-242 (j) a right click right after Esc selects the face it finds once the cancel is answered, then opens the menu', async () => {
    const { fixture, answerEnd } = withEndHeld(EDITING_EXTRUDE)
    const requests = menuRequests(fixture.app)
    const canceled = cancelCommand(fixture.app)
    rightClick(fixture.canvas, 200, 150)
    await settle()
    expect(requests).toEqual([])
    answerEnd()
    await canceled
    await settle()
    expect(selectionKeys(fixture)).toEqual([TOP_FACE_KEY])
    expect(requests).toEqual([{ x: 200, y: 150 }])
  })

  it('U-242 (k) a right click while OK runs selects the face it finds once the commit is answered, then opens the menu', async () => {
    const { fixture, answerEnd } = withEndHeld(COMPLETE_FILLET)
    const requests = menuRequests(fixture.app)
    const confirmed = confirmCommand(fixture.app)
    await settle()
    rightClick(fixture.canvas, 200, 150)
    await settle()
    expect(requests).toEqual([])
    answerEnd()
    await confirmed
    await settle()
    expect(fixture.app.store.getState().command).toBeNull()
    expect(selectionKeys(fixture)).toEqual([TOP_FACE_KEY])
    expect(requests).toEqual([{ x: 200, y: 150 }])
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

/** Re-attaching `sketch2`, the last feature but `after`, by a click on `face` (3.16.1). */
async function attachByClick(
  sketch2: FeatureRecord,
  face: ElementDescriptor,
  after: readonly FeatureRecord[] = [],
): Promise<Fixture> {
  const fixture = setUp({
    document: documentState(cubeWithSketch(sketch2, after), {
      name: 'cube',
      meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    }),
    state: { context: 'sketch', sketch: planePickingSession('f-sk2') },
    prepare: (core) => {
      core.stub('pickReference', () => Promise.resolve(face))
    },
  })
  pickElement(fixture, 'face', 0)
  click(fixture.canvas, 400, 300)
  await settle()
  return fixture
}

describe('clicks while a plane is picked for a new or re-attached sketch (3.16.1)', () => {
  const PLANE_PICKING: Partial<AppState> = { context: 'sketch', sketch: planePickingSession() }

  /** The cube's top after a Move body by (100, 0, 0), with its full reference (engine.md 3.9). */
  const MOVED_TOP_FACE: ElementDescriptor = {
    ...CUBE_TOP_FACE_MOVED,
    reference: {
      ...CUBE_TOP_FACE.reference,
      merged: { 'f-ext1/end-cap': ['f-ext2/end-cap'] },
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

  it("U-142 a picked face of a moved body stores its full reference and the descriptor's fallback with fallback_frame body", async () => {
    const fixture = setUp({
      state: PLANE_PICKING,
      prepare: (core) => {
        core.stub('pickReference', () => Promise.resolve(MOVED_TOP_FACE))
      },
    })
    pickElement(fixture, 'face', 0)
    click(fixture.canvas, 400, 300)
    await settle()
    const [call] = fixture.core.callsTo('applyEdits')
    expect(call?.[1]).toBe('New sketch')
    expect(call?.[2]).toMatchObject([
      {
        type: 'add_feature',
        feature: {
          type: 'sketch',
          plane: {
            kind: 'face',
            body: 'b-cube',
            face: MOVED_TOP_FACE.reference,
            fallback: MOVED_TOP_FACE.fallback,
            fallback_frame: 'body',
          },
        },
      },
    ])
  })

  it("U-146 re-attaching 'Sketch 2' with its fallback in body coordinates: a picked flat face commits one update labeled \"Attach 'Sketch 2'\" with its full reference and its descriptor's fallback in body coordinates", async () => {
    const sketch2 = lostFaceSketch('body')
    const fixture = await attachByClick(sketch2, MOVED_TOP_FACE)
    expect(fixture.core.callsTo('applyEdits')).toEqual([
      attachCall(sketch2, {
        kind: 'face',
        body: 'b-cube',
        face: MOVED_TOP_FACE.reference,
        fallback: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
        fallback_frame: 'body',
      }),
    ])
    expect(fixture.app.store.getState().sketch).toMatchObject({ sketchId: 'f-sk2' })
  })

  it("U-146 re-attaching 'Sketch 2' with its fallback in world coordinates stores the picked face's placement in world coordinates, which keeps the sketch without motion", async () => {
    const sketch2 = lostFaceSketch('world')
    const fixture = await attachByClick(sketch2, MOVED_TOP_FACE)
    // Strictly: not even an undefined `fallback_frame`, so the plane reads as world coordinates.
    expect(fixture.core.callsTo('applyEdits')).toStrictEqual([
      attachCall(sketch2, {
        kind: 'face',
        body: 'b-cube',
        face: MOVED_TOP_FACE.reference,
        fallback: { origin: [100, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
      }),
    ])
    expect(fixture.app.store.getState().sketch).toMatchObject({ sketchId: 'f-sk2' })
  })

  it("U-146 refuses a face made by a feature after 'Sketch 2' with attach_after in the sketch hint, and keeps the pick open", async () => {
    const boss: FeatureRecord = {
      id: 'f-ext2',
      type: 'extrude',
      body: 'b-cube',
      sketch: 'f-sk2',
      distance: '5 mm',
      op: 'join',
    }
    const bossTop: ElementDescriptor = {
      ...CUBE_TOP_FACE,
      name: 'f-ext2/end-cap',
      feature: 'f-ext2',
      reference: { q: 'f-ext2/end-cap', fp: 'plane|n=0,0,1|c=0,0,25|a=28.274334|adj=1' },
    }
    const fixture = await attachByClick(lostFaceSketch('body'), bossTop, [boss])
    const state = fixture.app.store.getState()
    expect(fixture.core.callsTo('applyEdits')).toEqual([])
    expect(state.pickHint).toEqual({
      message: MESSAGES.attach_after('Sketch 2', 'Extrude 2'),
      field: null,
    })
    expect(state.sketch).toMatchObject({ sketchId: null, reattaching: 'f-sk2' })
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

/** The canvas box a world box is drawn in by the last frame, from its view and projection. */
function drawnBox(
  fixture: Fixture,
  bounds: BoundingBox,
  size: { readonly width: number; readonly height: number },
): Box {
  const frame = fixture.backend.lastFrame()
  if (frame === null) {
    throw new Error('Nothing was rendered.')
  }
  const xs: number[] = []
  const ys: number[] = []
  for (const x of [bounds.min[0], bounds.max[0]]) {
    for (const y of [bounds.min[1], bounds.max[1]]) {
      for (const z of [bounds.min[2], bounds.max[2]]) {
        const pixel = worldToCanvas(frame, [x, y, z], size)
        xs.push(pixel.x)
        ys.push(pixel.y)
      }
    }
  }
  const left = Math.min(...xs)
  const top = Math.min(...ys)
  return { x: left, y: top, width: Math.max(...xs) - left, height: Math.max(...ys) - top }
}

/** Expects a drawn box to span x from `left` to `right` and y from `top` to `bottom`, within 0.5 pixel. */
function expectSpan(box: Box, left: number, right: number, top: number, bottom: number): void {
  expect(Math.abs(box.x - left)).toBeLessThanOrEqual(0.5)
  expect(Math.abs(box.x + box.width - right)).toBeLessThanOrEqual(0.5)
  expect(Math.abs(box.y - top)).toBeLessThanOrEqual(0.5)
  expect(Math.abs(box.y + box.height - bottom)).toBeLessThanOrEqual(0.5)
}

describe('framing in the free area (U-177)', () => {
  const CANVAS = { width: 1200, height: 800 }
  const FREE = { x: 280, y: 64, width: 640, height: 600 }
  const CUBE: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }
  const FRONT = withProjection(
    { ...DEFAULT_CAMERA, rotation: standardViewRotation('front') },
    'orthographic',
  )

  function setUpFront(free: Box = FREE): Fixture & { readonly observers: FakeResizeObservers } {
    const observers = stubResizeObserver()
    const fixture = setUp({ size: CANVAS, free })
    fixture.controller.setCamera(FRONT)
    fixture.frames.flush()
    return { ...fixture, observers }
  }

  it('U-177 projects every frame with windowProjection for the view window', () => {
    const fixture = setUpFront()
    expect(fixture.controller.viewWindow()).toEqual({ ...CANVAS, free: FREE })
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const projection = fixture.backend.lastFrame()?.projection ?? new Float32Array(16)
    // An orthographic projection holds 2 / (near - far) and (far + near) / (near - far).
    const depthScale = projection[10] ?? 0
    const depthShift = projection[14] ?? 0
    const near = (depthShift + 1) / depthScale
    const far = (depthShift - 1) / depthScale
    const expected = windowProjection(
      fixture.controller.getCamera(),
      { ...CANVAS, free: FREE },
      near,
      far,
    )
    expected.forEach((value, index) => {
      expect(projection[index]).toBeCloseTo(value, 5)
    })
  })

  it('U-177 fits the box into 80 percent of the free area height around its center', () => {
    const fixture = setUpFront()
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    expectSpan(drawnBox(fixture, CUBE, CANVAS), 360, 840, 124, 604)
    const viewport = fixture.canvas.closest('[data-testid="viewport"]')
    expect(viewport).toHaveAttribute('data-camera-target', '0,0,10')
  })

  it('U-177 moves a fitted model to the new free area center when an island closes or collapses', () => {
    const fixture = setUpFront()
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const fitted = fixture.controller.getCamera()
    fixture.setFree({ x: 0, y: 64, width: 920, height: 600 })
    fixture.observers.fire()
    fixture.frames.flush()
    expectSpan(drawnBox(fixture, CUBE, CANVAS), 220, 700, 124, 604)
    expect(cameraDifference(fixture.controller.getCamera(), fitted)).toBeLessThan(1e-9)
    fixture.setFree({ x: 0, y: 64, width: 920, height: 656 })
    fixture.observers.fire()
    fixture.frames.flush()
    // The taller free area scales the model up around its new center (460, 392).
    expectSpan(drawnBox(fixture, CUBE, CANVAS), 197.6, 722.4, 129.6, 654.4)
  })

  it('U-177 pans and spaces the grid by the free area height, and keeps a panned camera when the area changes', () => {
    const fixture = setUpFront({ x: 0, y: 64, width: 920, height: 656 })
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const before = fixture.controller.getCamera()
    const press = { pointerId: 1, pointerType: 'mouse', button: 1, buttons: 4, clientY: 300 }
    fireEvent.pointerDown(fixture.canvas, { ...press, clientX: 400 })
    fireEvent.pointerMove(fixture.canvas, { ...press, clientX: 500 })
    fireEvent.pointerUp(fixture.canvas, { ...press, buttons: 0, clientX: 500 })
    fixture.frames.flush()
    const panned = fixture.controller.getCamera()
    expect(before.target[0] - panned.target[0]).toBeCloseTo((100 * 25) / 656, 9)
    expect(fixture.app.store.getState().view.gridSpacing).toBe(
      drawnGridSpacing(fixture.app.store.getState(), panned, 656),
    )
    fixture.setFree(FREE)
    fixture.observers.fire()
    fixture.frames.flush()
    expect(fixture.controller.getCamera()).toBe(panned)
  })

  it('U-177 frames a fitted wide box again when the free area narrows', () => {
    const fixture = setUpFront()
    const wide: BoundingBox = { min: [-100, -10, 0], max: [100, 10, 40] }
    fixture.controller.fit(wide)
    fixture.frames.flush()
    fixture.setFree({ x: 280, y: 64, width: 400, height: 600 })
    fixture.observers.fire()
    fixture.frames.flush()
    const drawn = drawnBox(fixture, wide, CANVAS)
    expect(Math.abs(drawn.x - 320)).toBeLessThanOrEqual(0.5)
    expect(Math.abs(drawn.x + drawn.width - 640)).toBeLessThanOrEqual(0.5)
  })

  it('U-177 fits beside an open inspector, which moves no camera when it moves or closes', () => {
    const observers = stubResizeObserver()
    const canvasSize = { width: 1280, height: 720 }
    const free = { x: 280, y: 64, width: 992, height: 548 }
    const fixture = setUp({
      size: canvasSize,
      free,
      state: { selection: [bodyItem('b-cube', 'Cube')] },
    })
    fixture.controller.setCamera(FRONT)
    const inspector = document.createElement('div')
    inspector.dataset['testid'] = 'inspector'
    fixture.freeArea.append(inspector)
    const moveInspector = stubBox(inspector, { x: 280 + 74, y: 64 + 16, width: 320, height: 400 })
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const drawn = drawnBox(fixture, CUBE, canvasSize)
    expect(drawn.x + drawn.width / 2).toBeCloseTo(973, 1)
    expect(drawn.y + drawn.height / 2).toBeCloseTo(338, 1)
    expect(drawn.height).toBeCloseTo(438.4, 1)
    const fitted = fixture.controller.getCamera()
    moveInspector({ x: 900, y: 80, width: 320, height: 400 })
    observers.fire()
    fixture.frames.flush()
    expect(fixture.controller.getCamera()).toBe(fitted)
    inspector.remove()
    observers.fire()
    fixture.frames.flush()
    expect(fixture.controller.getCamera()).toBe(fitted)
  })
})

describe('framing beside an inspector under an island (2.11)', () => {
  const CUBE: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }
  const FRONT = withProjection(
    { ...DEFAULT_CAMERA, rotation: standardViewRotation('front') },
    'orthographic',
  )

  function setUpBesideSidePanel(inspectorBox: {
    readonly x: number
    readonly y: number
    readonly width: number
    readonly height: number
  }) {
    // 1280 by 720 with the browser and the Parameters panel docked: the free area is 624 wide.
    const fixture = setUp({
      size: { width: 1280, height: 720 },
      free: { x: 280, y: 64, width: 624, height: 548 },
      state: { selection: [bodyItem('b-cube', 'Cube')] },
    })
    fixture.controller.setCamera(FRONT)
    const inspector = document.createElement('div')
    inspector.dataset['testid'] = 'inspector'
    fixture.freeArea.append(inspector)
    stubBox(inspector, inspectorBox)
    return fixture
  }

  it('frames the whole free area when the card lies entirely under the side panel', () => {
    const fixture = setUpBesideSidePanel({ x: 952, y: 80, width: 320, height: 400 })
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const drawn = drawnBox(fixture, CUBE, { width: 1280, height: 720 })
    expect(drawn.x + drawn.width / 2).toBeCloseTo(280 + 624 / 2, 1)
  })

  it('frames the whole free area when the card lies entirely under the browser', () => {
    const fixture = setUpBesideSidePanel({ x: 280 - 400, y: 80, width: 320, height: 400 })
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const drawn = drawnBox(fixture, CUBE, { width: 1280, height: 720 })
    expect(drawn.x + drawn.width / 2).toBeCloseTo(280 + 624 / 2, 1)
  })

  it('frames beside the part of a card that reaches into the free area', () => {
    // The card spans 500 to 820 in the free area, so 500 to 624 of it is inside: frame left of it.
    const fixture = setUpBesideSidePanel({ x: 280 + 500, y: 80, width: 320, height: 400 })
    fixture.controller.fit(CUBE)
    fixture.frames.flush()
    const drawn = drawnBox(fixture, CUBE, { width: 1280, height: 720 })
    expect(drawn.x + drawn.width / 2).toBeCloseTo(280 + 250, 1)
  })
})

describe('islands above the canvas (U-178)', () => {
  it('U-178 ends the hover when the pointer leaves the canvas for an island, which takes no wheel or click', () => {
    const fixture = setUp()
    const topBar = document.createElement('header')
    topBar.dataset['testid'] = 'top-bar'
    document.body.append(topBar)
    cleanups.push(() => {
      topBar.remove()
    })
    pickElement(fixture, 'face', 3)
    const move = { pointerId: 1, pointerType: 'mouse', buttons: 0, clientX: 120, clientY: 80 }
    fireEvent.pointerMove(fixture.canvas, move)
    fixture.frames.flush()
    expect(fixture.app.store.getState().hover).toEqual({ key: 'mesh:1:face:3', kind: 'face' })
    fireEvent.pointerLeave(fixture.canvas, move)
    expect(fixture.app.store.getState().hover).toBeNull()
    const camera = fixture.controller.getCamera()
    const selection = fixture.app.store.getState().selection
    fireEvent.wheel(topBar, { deltaY: 100, clientX: 120, clientY: 80 })
    fireEvent.pointerDown(topBar, { ...move, button: 0, buttons: 1 })
    fireEvent.pointerUp(topBar, { ...move, button: 0 })
    fireEvent.click(topBar, { button: 0, clientX: 120, clientY: 80 })
    fixture.frames.flush()
    expect(fixture.controller.getCamera()).toBe(camera)
    expect(fixture.app.store.getState().selection).toBe(selection)
    expect(fixture.app.store.getState().hover).toBeNull()
  })
})

describe('overlay elements in the free area (U-189)', () => {
  it('U-189 hides an anchored overlay element outside the free area and places one inside it', () => {
    const fixture = setUp({
      size: { width: 1200, height: 800 },
      free: { x: 280, y: 64, width: 640, height: 600 },
    })
    const top = withProjection(
      { ...DEFAULT_CAMERA, rotation: standardViewRotation('top'), target: [0, 0, 0] },
      'orthographic',
    )
    fixture.controller.setCamera({ ...top, orthographicHeight: 600 })
    // 1 mm per pixel, the target at the free area's center (600, 364).
    const outside = document.createElement('span')
    outside.dataset['anchor'] = '-500,0,0'
    const inside = document.createElement('span')
    inside.dataset['anchor'] = '0,0,0'
    fixture.overlay.append(outside, inside)
    fixture.controller.requestRender()
    fixture.frames.flush()
    expect(outside.hidden).toBe(true)
    expect(inside.hidden).toBe(false)
    expect(inside.getAttribute('style')).toMatch(/^translate: 600px 364px;?$/)
  })
})

describe('framing as the inspector closes (2.11)', () => {
  it('frames the whole free area once the store closes the inspector, before React removes it', () => {
    const canvasSize = { width: 1280, height: 720 }
    const fixture = setUp({ size: canvasSize, free: { x: 280, y: 64, width: 992, height: 548 } })
    fixture.controller.setCamera(
      withProjection(
        { ...DEFAULT_CAMERA, rotation: standardViewRotation('front') },
        'orthographic',
      ),
    )
    // The selection inspector of a selection that was just cleared, still in the DOM.
    const stale = document.createElement('div')
    stale.dataset['testid'] = 'inspector'
    fixture.freeArea.append(stale)
    stubBox(stale, { x: 280 + 74, y: 64 + 16, width: 320, height: 400 })
    fixture.controller.fit({ min: [-10, -10, 0], max: [10, 10, 20] })
    fixture.frames.flush()
    const viewport = fixture.canvas.closest('[data-testid="viewport"]')
    expect(viewport).toHaveAttribute('data-camera-target', '0,0,10')
  })
})

describe('Look at beside the inspector (3.14)', () => {
  it('puts the target at the center of the part of the free area beside an open inspector', () => {
    const fixture = setUp({
      size: { width: 1280, height: 720 },
      free: { x: 280, y: 64, width: 992, height: 548 },
      state: { selection: [bodyItem('b-cube', 'Cube')] },
    })
    const inspector = document.createElement('div')
    inspector.dataset['testid'] = 'inspector'
    fixture.freeArea.append(inspector)
    stubBox(inspector, { x: 280 + 74, y: 64 + 16, width: 320, height: 400 })
    fixture.controller.lookAlong([0, -1, 0], [0, 0, 1], [0, 0, 10])
    fixture.frames.flush()
    const frame = fixture.backend.lastFrame()
    const pixel = worldToCanvas(
      {
        view: frame?.view ?? new Float32Array(16),
        projection: frame?.projection ?? new Float32Array(16),
      },
      [0, 0, 10],
      { width: 1280, height: 720 },
    )
    expect(pixel.x).toBeCloseTo(973, 1)
    expect(pixel.y).toBeCloseTo(338, 1)
  })
})
