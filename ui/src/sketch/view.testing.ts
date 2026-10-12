// Test support for the sketch controller and overlay: a 1024 by 1024 pixel canvas inside a viewport
// region, an orthographic camera looking straight down at the XY plane at 0.125 mm per pixel with the
// sketch origin at the canvas center, pointer events at sketch coordinates, and a fake core that
// solves candidates as given and applies sketch edits to the store as the core's events would.

import { vi } from 'vitest'
import type { App } from '../app/createApp'
import type { CoreApi } from '../core/api'
import type {
  Edit,
  SketchConstraint,
  SketchEntity,
  SketchParams,
  SketchSolution,
} from '../generated/protocol'
import type { AppState } from '../state/store'
import { DEFAULT_CAMERA, standardViewRotation, type Camera } from '../viewport/camera'
import type { ViewportHandle } from '../viewport/controller'
import {
  SKETCH_ID,
  coreError,
  createFakeCore,
  createSketchTestApp,
  documentState,
  sketchRecord,
  sketchSession,
  solutionFor,
  solvedSketch,
} from './fixtures.testing'
import type { Vector2 } from './geometry'

/** The canvas is this many CSS pixels wide and high; powers of two keep the arithmetic exact. */
export const VIEW_PIXELS = 1024
/** Sketch millimeters per CSS pixel of the test view. */
export const MILLIMETERS_PER_PIXEL = 0.125

/** Looking down at the XY plane from +Z, +X right and +Y up, 128 mm high. */
export const TOP_CAMERA: Camera = {
  ...DEFAULT_CAMERA,
  target: [0, 0, 0],
  rotation: standardViewRotation('top'),
  projection: 'orthographic',
  orthographicHeight: VIEW_PIXELS * MILLIMETERS_PER_PIXEL,
}

/** The canvas pixel of sketch coordinates in the test view. */
export function pixelOf(point: Vector2): Vector2 {
  return [
    VIEW_PIXELS / 2 + point[0] / MILLIMETERS_PER_PIXEL,
    VIEW_PIXELS / 2 - point[1] / MILLIMETERS_PER_PIXEL,
  ]
}

export type TestView = {
  /** The viewport region, `data-testid="viewport"`, holding the focusable view and its canvas. */
  readonly region: HTMLElement
  readonly canvas: HTMLCanvasElement
  /** The focusable element around the canvas, as the viewport's application element. */
  readonly focusTarget: HTMLElement
  remove(): void
}

/** A viewport region with a sized canvas, attached to the document. */
export function mountView(): TestView {
  const region = document.createElement('main')
  region.dataset['testid'] = 'viewport'
  const focusTarget = document.createElement('div')
  focusTarget.tabIndex = 0
  const canvas = document.createElement('canvas')
  Object.defineProperty(canvas, 'clientWidth', { configurable: true, value: VIEW_PIXELS })
  Object.defineProperty(canvas, 'clientHeight', { configurable: true, value: VIEW_PIXELS })
  focusTarget.append(canvas)
  region.append(focusTarget)
  document.body.append(region)
  return {
    region,
    canvas,
    focusTarget,
    remove() {
      region.remove()
    },
  }
}

/** A viewport handle whose camera is `camera`; the other methods are mocks. */
export function fixedViewport(camera: Camera = TOP_CAMERA): ViewportHandle {
  return {
    getCamera: () => camera,
    setCamera: vi.fn<ViewportHandle['setCamera']>(),
    fit: vi.fn<ViewportHandle['fit']>(),
    setStandardView: vi.fn<ViewportHandle['setStandardView']>(),
    toggleProjection: vi.fn<ViewportHandle['toggleProjection']>(),
    lookAlong: vi.fn<ViewportHandle['lookAlong']>(),
    zoomToBounds: vi.fn<ViewportHandle['zoomToBounds']>(),
    screenRectangle: () => null,
    requestRender: vi.fn<ViewportHandle['requestRender']>(),
  }
}

/** Dispatches a pointer event at sketch coordinates on the canvas. */
export function pointerAt(
  canvas: HTMLCanvasElement,
  type: 'pointerdown' | 'pointermove' | 'pointerup',
  point: Vector2,
  init: PointerEventInit = {},
): PointerEvent {
  const [x, y] = pixelOf(point)
  const pressed = type === 'pointerup' ? 0 : type === 'pointerdown' ? 1 : 0
  const event = new PointerEvent(type, {
    bubbles: true,
    cancelable: true,
    pointerId: 1,
    button: type === 'pointermove' ? -1 : 0,
    buttons: pressed,
    clientX: x,
    clientY: y,
    ...init,
  })
  canvas.dispatchEvent(event)
  return event
}

/** A press and release at the same sketch point. */
export function clickAt(
  canvas: HTMLCanvasElement,
  point: Vector2,
  init: PointerEventInit = {},
): void {
  pointerAt(canvas, 'pointermove', point, init)
  pointerAt(canvas, 'pointerdown', point, init)
  pointerAt(canvas, 'pointerup', point, init)
}

/** A press at `from`, moves to `to` with the button held, and a release there. */
export function dragFrom(
  canvas: HTMLCanvasElement,
  from: Vector2,
  to: Vector2,
  init: PointerEventInit = {},
): void {
  pointerAt(canvas, 'pointerdown', from, init)
  pointerAt(canvas, 'pointermove', to, { buttons: 1, ...init })
  pointerAt(canvas, 'pointerup', to, init)
}

/** Dispatches a key press on a target (the document body by default). */
export function pressKey(
  key: string,
  init: KeyboardEventInit = {},
  target: EventTarget = document.body,
): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
  target.dispatchEvent(event)
  return event
}

/** The edits `applyEdits` received, with their labels. */
export type AppliedEdits = { readonly label: string | null; readonly edits: readonly Edit[] }[]

export type SketchHarness = {
  readonly app: App
  readonly core: CoreApi
  /** Every `applyEdits` call, in order. */
  readonly applied: AppliedEdits
  /** The candidates `solveSketch` probed, in order. */
  readonly probes: SketchParams[]
}

/**
 * An app editing `f-sk1` with these entities and constraints, the test viewport attached, and a
 * core that reports each probe solved as given (`solve` overrides that) and applies sketch updates
 * to the store.
 */
export function sketchHarness(
  options: {
    readonly entities?: SketchEntity[]
    readonly constraints?: SketchConstraint[]
    readonly solution?: SketchSolution
    readonly state?: Partial<AppState>
    readonly core?: Partial<CoreApi>
    readonly solve?: (candidate: SketchParams) => SketchSolution
  } = {},
): SketchHarness {
  const entities = options.entities ?? []
  const applied: AppliedEdits = []
  const probes: SketchParams[] = []
  const solve =
    options.solve ?? ((candidate: SketchParams) => solutionFor(candidate.entities ?? []))
  let revision = 1
  const holder: { app: App | null } = { app: null }
  const core = createFakeCore({
    solveSketch: (_handle, _feature, candidate) => {
      if (candidate !== undefined) {
        probes.push(candidate)
      }
      return Promise.resolve(candidate === undefined ? null : solvedSketch(solve(candidate)))
    },
    applyEdits: (_handle, label, edits) => {
      applied.push({ label, edits })
      revision += 1
      const app = holder.app
      const [edit] = edits
      if (app !== null && edit?.type === 'update_feature') {
        const feature = edit.feature
        app.store.setState((state) => {
          const document = state.document
          if (document === null) {
            return {}
          }
          const features = (document.recipe.features ?? []).map((record) =>
            record.id === feature.id ? feature : record,
          )
          const stored: unknown = feature['entities']
          const solved = solutionFor(Array.isArray(stored) ? (stored as SketchEntity[]) : [])
          return {
            document: {
              ...document,
              revision,
              recipe: { ...document.recipe, features },
              sketches: { ...document.sketches, [feature.id]: solvedSketch(solved) },
            },
          }
        })
      }
      return Promise.resolve(revision)
    },
    evaluate: (_handle, expression) => {
      const value = Number.parseFloat(expression)
      return Number.isFinite(value)
        ? Promise.resolve({ value, dimension: { length: 0, angle: 0 } })
        : Promise.reject(coreError('expression_error', `Unknown name '${expression}'.`))
    },
    ...options.core,
  })
  const app = createSketchTestApp({
    core,
    state: {
      context: 'sketch',
      sketch: sketchSession(),
      document: documentState({
        features: [sketchRecord({ entities, constraints: options.constraints ?? [] })],
        sketches: { [SKETCH_ID]: solvedSketch(options.solution ?? solutionFor(entities)) },
      }),
      ...options.state,
    },
  })
  holder.app = app
  app.attachViewport(fixedViewport())
  return { app, core, applied, probes }
}

/**
 * Waits until the sketch commits in flight have settled: the fake core answers at once, so every
 * step of a commit runs in the microtasks that finish before the next task.
 */
export async function settleCommits(): Promise<void> {
  await new Promise((resolve) => {
    setTimeout(resolve, 0)
  })
}
