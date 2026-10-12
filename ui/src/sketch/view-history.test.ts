import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { redo, undo } from '../app/actions/document'
import { commitSketchChange, enterSketch } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import { createAppStore, type AppStore } from '../state/store'
import { createFakeBackend } from '../test/fakes/backend'
import {
  DEFAULT_CAMERA,
  classifyView,
  forward,
  orbit,
  up,
  type Camera,
  type Vector3,
} from '../viewport/camera'
import { createViewportController, type ViewportController } from '../viewport/controller'
import { createScene } from '../viewport/scene'
import {
  SKETCH_ID,
  createFakeCore,
  createSketchTestApp,
  documentState,
  pointEntity,
  sketchRecord,
  solutionFor,
  solvedSketch,
} from './fixtures.testing'
import type { SketchChange } from './session'

const TOLERANCE = 1e-9

// The camera the user had before entering the sketch: an orbited iso view, not a standard view.
const PRE_ENTRY_CAMERA = orbit(DEFAULT_CAMERA, 37, -12, [0, 0, 0])

const POINT_CHANGE: SketchChange = {
  add: { entities: [pointEntity('e-1', [5, 5])], constraints: [] },
  requested: [],
  inferred: [],
}

// Camera moves are instant under reduced motion (2.11), so restored cameras can be read at once.
function reducedMotion(query: string): MediaQueryList {
  return {
    matches: query.includes('prefers-reduced-motion: reduce'),
    media: query,
    onchange: null,
    addListener: () => undefined,
    removeListener: () => undefined,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => false,
  }
}

function expectVector(actual: Vector3, expected: Vector3): void {
  const gap = Math.hypot(actual[0] - expected[0], actual[1] - expected[1], actual[2] - expected[2])
  expect(gap).toBeLessThan(TOLERANCE)
}

function expectCamera(actual: Camera, expected: Camera): void {
  expect(actual.projection).toBe(expected.projection)
  expectVector(actual.target, expected.target)
  expectVector(forward(actual), forward(expected))
  expectVector(up(actual), up(expected))
  expect(actual.distance).toBeCloseTo(expected.distance, 9)
  expect(actual.orthographicHeight).toBeCloseTo(expected.orthographicHeight, 9)
}

/** Updates the undo and redo labels as the core's document_changed event would. */
function setLabels(store: AppStore, undoLabel: string | null, redoLabel: string | null): void {
  const document = store.getState().document
  if (document !== null) {
    store.setState({ document: { ...document, undoLabel, redoLabel } })
  }
}

type Fixture = {
  readonly app: App
  readonly controller: ViewportController
  /** Core undo and redo calls, in order. */
  readonly coreCalls: string[]
  /** The camera right after entering the sketch. */
  readonly entered: Camera
}

const mounted: { controller: ViewportController; elements: HTMLElement[] }[] = []

/** An app with a real viewport controller on a fake backend, in sketch mode on the empty XY sketch f-sk1. */
async function enterEmptySketch(): Promise<Fixture> {
  const store = createAppStore({
    document: documentState({
      features: [sketchRecord()],
      sketches: { [SKETCH_ID]: solvedSketch(solutionFor([])) },
      undoLabel: 'New sketch',
    }),
  })
  const coreCalls: string[] = []
  const core = createFakeCore({
    solveSketch: (_documentHandle, _feature, candidate) =>
      Promise.resolve(solvedSketch(solutionFor(candidate?.entities ?? []))),
    applyEdits: (_documentHandle, label) => {
      setLabels(store, label, null)
      return Promise.resolve(2)
    },
    undo: () => {
      coreCalls.push('undo')
      setLabels(store, 'New sketch', 'Point')
      return Promise.resolve({ revision: 3, label: 'Point' })
    },
    redo: () => {
      coreCalls.push('redo')
      setLabels(store, 'Point', null)
      return Promise.resolve({ revision: 4, label: 'Point' })
    },
  })
  const app = createSketchTestApp({ core, store })
  const canvas = document.createElement('canvas')
  const overlay = document.createElement('div')
  document.body.append(canvas, overlay)
  const backend = createFakeBackend()
  const controller = createViewportController({
    app,
    canvas,
    overlay,
    backend,
    scene: createScene(backend),
  })
  mounted.push({ controller, elements: [canvas, overlay] })
  app.attachViewport(controller)
  controller.setCamera(PRE_ENTRY_CAMERA, { animate: false, record: false })

  await enterSketch(app, SKETCH_ID)

  return { app, controller, coreCalls, entered: controller.getCamera() }
}

/** A completed orbit gesture: the controller records the camera from before it. */
function orbitView(fixture: Fixture): Camera {
  const before = fixture.controller.getCamera()
  const orbited = orbit(before, 100, 0, before.target)
  fixture.controller.setCamera(orbited, { animate: false, record: true })
  return orbited
}

async function commitPoint(fixture: Fixture): Promise<void> {
  const result = await commitSketchChange(fixture.app, 'Point', POINT_CHANGE)
  expect(result).toMatchObject({ outcome: 'committed' })
}

beforeEach(() => {
  vi.stubGlobal('matchMedia', reducedMotion)
})

afterEach(() => {
  for (const { controller, elements } of mounted.splice(0)) {
    controller.dispose()
    for (const element of elements) {
      element.remove()
    }
  }
  vi.unstubAllGlobals()
})

describe('entering a sketch (3.16.1)', () => {
  it('enters sketch mode looking at the XY sketch from +Z with +Y up', async () => {
    const fixture = await enterEmptySketch()
    expect(fixture.app.store.getState().context).toBe('sketch')
    expect(fixture.app.store.getState().sketch?.sketchId).toBe(SKETCH_ID)
    expect(classifyView(fixture.entered)).toBe('top')
  })
})

describe('sketch view history (3.20)', () => {
  it('U-62 after enter, an edit and an orbit, three Mod+Z presses restore the camera from before the orbit, call core undo once, then restore the pre-entry camera', async () => {
    const fixture = await enterEmptySketch()
    await commitPoint(fixture)
    orbitView(fixture)

    await undo(fixture.app)
    expectCamera(fixture.controller.getCamera(), fixture.entered)
    expect(fixture.coreCalls).toEqual([])

    await undo(fixture.app)
    expect(fixture.coreCalls).toEqual(['undo'])
    expectCamera(fixture.controller.getCamera(), fixture.entered)

    await undo(fixture.app)
    expectCamera(fixture.controller.getCamera(), PRE_ENTRY_CAMERA)
    expect(fixture.coreCalls).toEqual(['undo'])
  })

  it('U-62 after enter, an edit and an orbit, Mod+Shift+Z replays the three undos in reverse', async () => {
    const fixture = await enterEmptySketch()
    await commitPoint(fixture)
    const orbited = orbitView(fixture)
    await undo(fixture.app)
    await undo(fixture.app)
    await undo(fixture.app)

    await redo(fixture.app)
    expectCamera(fixture.controller.getCamera(), fixture.entered)
    expect(fixture.coreCalls).toEqual(['undo'])

    await redo(fixture.app)
    expect(fixture.coreCalls).toEqual(['undo', 'redo'])
    expectCamera(fixture.controller.getCamera(), fixture.entered)

    await redo(fixture.app)
    expectCamera(fixture.controller.getCamera(), orbited)
    expect(fixture.coreCalls).toEqual(['undo', 'redo'])
  })

  it('U-62 after enter, an orbit and an edit, Mod+Z pops the newest entry first: core undo, then the camera from before the orbit, then the pre-entry camera', async () => {
    const fixture = await enterEmptySketch()
    const orbited = orbitView(fixture)
    await commitPoint(fixture)

    await undo(fixture.app)
    expect(fixture.coreCalls).toEqual(['undo'])
    expectCamera(fixture.controller.getCamera(), orbited)

    await undo(fixture.app)
    expectCamera(fixture.controller.getCamera(), fixture.entered)

    await undo(fixture.app)
    expectCamera(fixture.controller.getCamera(), PRE_ENTRY_CAMERA)
    expect(fixture.coreCalls).toEqual(['undo'])
  })

  it('U-62 after enter, an orbit and an edit, Mod+Shift+Z replays the three undos in reverse', async () => {
    const fixture = await enterEmptySketch()
    const orbited = orbitView(fixture)
    await commitPoint(fixture)
    await undo(fixture.app)
    await undo(fixture.app)
    await undo(fixture.app)

    await redo(fixture.app)
    expectCamera(fixture.controller.getCamera(), fixture.entered)

    await redo(fixture.app)
    expectCamera(fixture.controller.getCamera(), orbited)
    expect(fixture.coreCalls).toEqual(['undo'])

    await redo(fixture.app)
    expect(fixture.coreCalls).toEqual(['undo', 'redo'])
    expectCamera(fixture.controller.getCamera(), orbited)
  })

  it('with the stack empty, Mod+Z calls core undo directly', async () => {
    const fixture = await enterEmptySketch()

    await undo(fixture.app)
    expectCamera(fixture.controller.getCamera(), PRE_ENTRY_CAMERA)
    expect(fixture.coreCalls).toEqual([])

    await undo(fixture.app)
    expect(fixture.coreCalls).toEqual(['undo'])
    expectCamera(fixture.controller.getCamera(), PRE_ENTRY_CAMERA)
  })

  it('a new view change clears the redo stack, so Mod+Shift+Z then calls core redo directly', async () => {
    const fixture = await enterEmptySketch()
    orbitView(fixture)
    await undo(fixture.app)
    const reorbited = orbitView(fixture)
    // The core has an edit to redo, so core redo is available.
    setLabels(fixture.app.store, 'New sketch', 'Point')

    await redo(fixture.app)
    expect(fixture.coreCalls).toEqual(['redo'])
    expectCamera(fixture.controller.getCamera(), reorbited)
  })
})
