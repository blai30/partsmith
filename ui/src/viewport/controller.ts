import { commandEnding, openFeatureEdit } from '../app/actions/command'
import { layOnFace } from '../app/actions/print'
import {
  clearSelection,
  landInTurn,
  landPick,
  pickAt,
  pickedItem,
  setSelection,
} from '../app/actions/selection'
import { openSketch, pickPlane, recordSketchView } from '../app/actions/sketch'
import { startInProgress, startInTurn } from '../app/actions/starts'
import { onCoreEvent, type App } from '../app/createApp'
import { reportFailure } from '../app/report'
import type { BoundingBox, ElementDescriptor } from '../generated/protocol'
import type { Rectangle } from '../inspector/placement'
import { requestUi } from '../layout/requests'
import { selectInspectorOpen } from '../state/selectors'
import { measureTiming, TIMING_NAMES } from '../performance/marks'
import type { AppState, SelectionItem } from '../state/store'
import type { RenderBackend, RenderFrame } from './backend'
import {
  aspectOf,
  DEFAULT_CAMERA,
  DEFAULT_FIT_MARGIN,
  classifyView,
  easeOutCubic,
  eye,
  fitBounds,
  forward,
  framedArea,
  interpolateCameras,
  normalizedPosition,
  orbit,
  pan,
  pointAtDepth,
  rotationLookingAlong,
  standardViewRotation,
  visibleHeightAtDepth,
  wheelZoomFactor,
  windowAspect,
  windowCursor,
  windowPosition,
  withProjection,
  zoomAt,
  type Camera,
  type StandardView,
  type Vector3,
  type ViewRectangle,
  type ViewWindow,
} from './camera'
import { drawnGridSpacing } from './grid'
import { dot, extendBounds, normalize, scale, subtract } from './math'
import {
  gestureFor,
  GESTURE_THRESHOLD_PIXELS,
  LEFT_BUTTON,
  MIDDLE_BUTTON,
  NAVIGATION_PRESETS,
  RIGHT_BUTTON,
  type Gesture,
  type KeyNavigation,
} from './navigation'
import { hideLabelsUnderCards, placeElement, visiblePosition } from './overlay'
import {
  candidateKind,
  choosePick,
  type PickCandidate,
  type PickResult,
  type PointerRay,
} from './picking'
import { requestFrame } from './frames'
import { createHighlights } from './highlights'
import { selectionFieldAt } from './picking-rules'
import type { Scene } from './scene'

// The viewport controller (ui.md 2.11, 3.6, 3.12, 3.13): the camera, rendering on demand, mouse
// and wheel navigation, and hover and click picking; `highlights.ts` keeps the highlights that
// need the core. It subscribes to the store itself, so the viewport bypasses React (ADR 0002).

export type ViewportHandle = {
  getCamera(): Camera
  setCamera(camera: Camera, options?: { animate?: boolean; record?: boolean }): void
  fit(bounds?: BoundingBox): void
  setStandardView(view: StandardView): void
  toggleProjection(): void
  /** Puts the eye on the `normal` side of `target`, looking along -normal, with up from `upHint`. */
  lookAlong(normal: Vector3, upHint: Vector3, target: Vector3): void
  zoomToBounds(bounds: BoundingBox, margin: number): void
  /** The world box of selection items as drawn (3.6): faces, edges and vertices from their meshes, sketch items and bodies; null when none is drawn. */
  itemBounds(items: readonly SelectionItem[]): BoundingBox | null
  screenRectangle(items: readonly SelectionItem[]): Rectangle | null
  /** The canvas size and the free area last measured (3.2). */
  viewWindow(): ViewWindow
  requestRender(): void
}

/** What a rendered frame offers the chrome that follows the camera in the same animation frame. */
export type RenderedFrame = {
  readonly camera: Camera
  readonly view: ViewWindow
  /** The CSS pixel position of a world point in the canvas, or null behind the eye. */
  project(point: Vector3): { readonly x: number; readonly y: number } | null
}

export type ViewportController = ViewportHandle & {
  dispose(): void
  /** Applies a camera step asked for from the keyboard (3.5). */
  navigate(step: KeyNavigation): void
  /** Hears every rendered frame, for chrome that follows the camera (the view cube rotor). */
  onRender(listener: (frame: RenderedFrame) => void): () => void
}

/** Camera animations (2.11). */
const ANIMATION_MILLISECONDS = 250
/** The drawing buffer is the CSS size times the device pixel ratio, at most 2 (3.12). */
const MAXIMUM_PIXEL_RATIO = 2
/** A click moves at most this far (CSS pixels) and lasts at most this long (3.6). */
const CLICK_DISTANCE = 4
const CLICK_MILLISECONDS = 400
/** The pick radius in CSS pixels at 100 percent UI scale (3.6). */
const PICK_RADIUS = 4
/** A wheel burst ends after this long without wheel events (3.20). */
const WHEEL_BURST_MILLISECONDS = 300
/** The side of the cube framed when nothing is visible (3.14). */
const EMPTY_FIT_SIDE = 100

/**
 * How a click lands in the selection or the active command (3.6): in its place among the clicks
 * and selection changes on their way, or at once, for a click whose turn has come after it waited
 * for a command's end.
 */
type ClickLanding = {
  readonly pick: (
    app: App,
    sample: PickResult,
    modifiers: { readonly toggle: boolean },
    pointer?: PointerRay,
  ) => Promise<void>
  /** Empties the selection, for a click on empty space. */
  readonly clear: (app: App) => void
}

const LANDS_IN_ORDER: ClickLanding = { pick: pickAt, clear: clearSelection }

const LANDS_NOW: ClickLanding = {
  pick: landPick,
  clear: (app) => {
    setSelection(app, [], 'replace')
  },
}

/** A second middle click within this long of the first is a double-click (3.13). */
const DOUBLE_CLICK_MILLISECONDS = 500
/** Hover picking waits until the camera has been still this long (3.13). */
const CAMERA_STILL_MILLISECONDS = 100

// PointerEvent.button values.
const MIDDLE_BUTTON_INDEX = 1
const SECONDARY_BUTTON_INDEX = 2

type Press = {
  readonly pointerId: number
  readonly startX: number
  readonly startY: number
  readonly startedAt: number
  /** The buttons the press started with. */
  readonly startButtons: number
  buttons: number
  lastX: number
  lastY: number
  /** The camera gesture once the press moved past the threshold; 'none' for a left drag. */
  gesture: Gesture | null
  pivot: Vector3 | null
}

type CameraAnimation = {
  readonly from: Camera
  readonly to: Camera
  readonly startedAt: number
}

/** Whether the active selection field takes a face as its edges (Fillet, Chamfer; 3.7). */
function facesStandForEdges(state: AppState): boolean {
  const field = selectionFieldAt(state, state.command?.activeField ?? null)
  return field?.accepts === 'edge' && field.multiple
}

function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches
}

/** A coordinate rounded to 2 decimals, trailing zeros trimmed, -0 written 0 (3.13). */
function coordinateText(value: number): string {
  const rounded = Math.round(value * 100) / 100
  return String(rounded + 0)
}

function emptyFitBounds(): BoundingBox {
  const half = EMPTY_FIT_SIDE / 2
  return { min: [-half, -half, -half], max: [half, half, half] }
}

/** A middle-button press never starts the browser's autoscroll (3.13). */
function onMouseDown(event: MouseEvent): void {
  if (event.button === MIDDLE_BUTTON_INDEX) {
    event.preventDefault()
  }
}

/** The browser's menu never opens on the canvas: on macOS and Linux it fires on the press, before a right drag can orbit (3.13). */
function onContextMenu(event: Event): void {
  event.preventDefault()
}

/** Whether a store change can change what the view draws. */
function relevantChange(state: AppState, previous: AppState): boolean {
  return (
    state.document !== previous.document ||
    state.selection !== previous.selection ||
    state.hover !== previous.hover ||
    state.command !== previous.command ||
    state.sketch !== previous.sketch ||
    state.view !== previous.view ||
    state.settings !== previous.settings ||
    state.context !== previous.context ||
    state.print !== previous.print ||
    state.printers !== previous.printers ||
    state.catalog !== previous.catalog
  )
}

/** What the last fit or zoom to bounds framed and the camera it set (2.11). */
type Fitted = { readonly camera: Camera; readonly bounds: BoundingBox; readonly margin: number }

/**
 * `overlay` is the overlay layer of 3.12, whose anchored children the controller moves each frame;
 * `freeArea` is the `viewport` element, the free area of 3.2, which the camera frames.
 */
export function createViewportController(options: {
  app: App
  canvas: HTMLCanvasElement
  overlay: HTMLElement
  freeArea: HTMLElement
  backend: RenderBackend
  scene: Scene
}): ViewportController {
  const { app, canvas, overlay, freeArea, backend, scene } = options
  const store = app.store
  let camera: Camera = DEFAULT_CAMERA
  let animation: CameraAnimation | null = null
  let size = { width: 0, height: 0, pixelRatio: 1 }
  /** The free area in canvas CSS pixels, as last measured. */
  let free: ViewRectangle = { x: 0, y: 0, width: 0, height: 0 }
  /** The last fit, while the camera is still the one it set: a new free area frames it again. */
  let fitted: Fitted | null = null
  let cancelFrame: (() => void) | null = null
  let renderNeeded = true
  let disposed = false
  let lastFrame: RenderFrame | null = null
  let press: Press | null = null
  let hoverAt: { readonly x: number; readonly y: number } | null = null
  let wheelBurst: { readonly before: Camera; timer: ReturnType<typeof setTimeout> } | null = null
  let pointerClass = false
  /** The camera of the last frame and when a frame last showed a different one, for the hover pause (3.13). */
  let renderedCamera: Camera = camera
  let cameraChangedAt = Number.NEGATIVE_INFINITY
  /** The time of the last animation frame when it rendered motion: where the next ps:frame starts (2.18). */
  let motionFrameAt: number | null = null
  let hoverTimer: ReturnType<typeof setTimeout> | null = null
  /** The last middle-button click, for the Fit to view double-click (3.13). */
  let lastMiddleClick: { readonly at: number; readonly x: number; readonly y: number } | null = null
  /** Whether this task already recorded a sketch view entry: one user action is one entry. */
  let recordedInTask = false
  const renderListeners = new Set<(frame: RenderedFrame) => void>()
  const cleanups: (() => void)[] = []

  const highlights = createHighlights({ app, scene, onChange: requestRender })

  /** The canvas size and the free area last measured. */
  function viewWindow(): ViewWindow {
    return { width: size.width, height: size.height, free }
  }

  /** The canvas point (x, y) in the free area's normalized device coordinates (2.11). */
  function cursorAt(x: number, y: number): [number, number] {
    return windowCursor(viewWindow(), x, y)
  }

  /** The aspect every camera function gets: the free area's. */
  function aspect(): number {
    return windowAspect(viewWindow())
  }

  /** The free area's height in CSS pixels, which pans, zooms and pixel sizes measure. */
  function freeHeight(): number {
    return Math.max(framedArea(viewWindow()).height, 1)
  }

  function radius(): number {
    return PICK_RADIUS * store.getState().settings.uiScale
  }

  // Rendering on demand.

  function schedule(): void {
    if (disposed || cancelFrame !== null) {
      return
    }
    cancelFrame = requestFrame(step)
  }

  function requestRender(): void {
    renderNeeded = true
    schedule()
  }

  function step(frameTime: number): void {
    cancelFrame = null
    if (disposed) {
      return
    }
    advanceAnimation()
    if (renderNeeded) {
      renderNeeded = false
      render(frameTime)
    } else {
      // A frame with nothing to draw ends the motion that ps:frame times.
      motionFrameAt = null
    }
    if (hoverAt !== null && press === null) {
      // Hover picking waits for the camera to be still, so a moving camera renders no ID pass (3.13).
      const wait = cameraChangedAt + CAMERA_STILL_MILLISECONDS - performance.now()
      if (animation === null && wait <= 0) {
        const point = hoverAt
        hoverAt = null
        hover(point.x, point.y)
      } else if (animation === null) {
        retryHoverAfter(wait)
      }
    }
  }

  /** Asks for another frame once the camera has been still long enough to hover. */
  function retryHoverAfter(milliseconds: number): void {
    if (hoverTimer !== null) {
      return
    }
    hoverTimer = setTimeout(() => {
      hoverTimer = null
      schedule()
    }, milliseconds)
  }

  /** Renders the frame of `frameTime`: the animation frame's time, or now outside one. */
  function render(frameTime: number): void {
    const state = store.getState()
    const moving = camera !== renderedCamera || (press !== null && press.gesture !== null)
    if (camera !== renderedCamera) {
      renderedCamera = camera
      cameraChangedAt = performance.now()
    }
    const view = viewWindow()
    lastFrame = scene.frame(state, camera, { ...view, pixelRatio: size.pixelRatio })
    backend.render(lastFrame)
    timeFrame(frameTime, moving)
    const rendered: RenderedFrame = { camera, view, project }
    for (const listener of Array.from(renderListeners)) {
      listener(rendered)
    }
    positionAnchors()
    mirrorCameraTarget()
    mirrorGridSpacing(state)
  }

  /**
   * Records ps:frame (2.18) from the previous animation frame to this one when both rendered while
   * the camera moved or a drag ran, then asks for the next frame to see whether the motion goes on.
   */
  function timeFrame(frameTime: number, moving: boolean): void {
    if (!moving) {
      motionFrameAt = null
      return
    }
    if (motionFrameAt !== null) {
      measureTiming(TIMING_NAMES.frame, motionFrameAt, frameTime)
    }
    motionFrameAt = frameTime
    schedule()
  }

  /** Mirrors the drawn grid's minor spacing in the store when it changes, for the status bar (3.12, 3.21). */
  function mirrorGridSpacing(state: AppState): void {
    const spacing = drawnGridSpacing(state, camera, framedArea(viewWindow()).height)
    if (state.view.gridSpacing !== spacing) {
      store.setState((current) => ({ view: { ...current.view, gridSpacing: spacing } }))
    }
  }

  /** The canvas CSS pixel position of a world point, or null behind the eye. */
  function project(point: Vector3): { x: number; y: number } | null {
    const position = normalizedPosition(camera, point, aspect())
    return position === null ? null : windowPosition(viewWindow(), position)
  }

  /**
   * Moves the overlay's anchored labels (`data-anchor="x,y,z"`) to their projections, hiding those
   * whose anchor is behind the eye or outside the free area, or whose box meets a card there (3.12).
   */
  function positionAnchors(): void {
    const area = framedArea(viewWindow())
    const labels = [...overlay.querySelectorAll<HTMLElement>('[data-anchor]')]
    for (const element of labels) {
      const coordinates = (element.dataset['anchor'] ?? '').split(',').map(Number)
      const [x = Number.NaN, y = Number.NaN, z = Number.NaN] = coordinates
      const position = Number.isFinite(x + y + z) ? project([x, y, z]) : null
      placeElement(element, visiblePosition(position, area))
    }
    hideLabelsUnderCards(labels, freeArea)
  }

  function viewportElement(): HTMLElement | null {
    return canvas.closest<HTMLElement>('[data-testid="viewport"]')
  }

  function mirrorCameraTarget(): void {
    const element = viewportElement()
    const text = camera.target.map(coordinateText).join(',')
    if (element !== null && element.dataset['cameraTarget'] !== text) {
      element.dataset['cameraTarget'] = text
    }
  }

  // The camera.

  function goalCamera(): Camera {
    return animation === null ? camera : animation.to
  }

  /** Mirrors the settled camera's standard view and projection in the store (3.13). */
  function mirrorCamera(settled: Camera): void {
    const named = classifyView(settled)
    const view = store.getState().view
    if (view.named !== named || view.projection !== settled.projection) {
      store.setState((state) => ({
        view: { ...state.view, named, projection: settled.projection },
      }))
    }
  }

  function advanceAnimation(): void {
    if (animation === null) {
      return
    }
    const elapsed = performance.now() - animation.startedAt
    const amount = Math.min(Math.max(elapsed / ANIMATION_MILLISECONDS, 0), 1)
    camera = interpolateCameras(animation.from, animation.to, easeOutCubic(amount))
    renderNeeded = true
    if (amount >= 1) {
      camera = animation.to
      animation = null
      mirrorCamera(camera)
      return
    }
    schedule()
  }

  function setCamera(next: Camera, change: { animate?: boolean; record?: boolean } = {}): void {
    // Several recorded moves in one task (Home is a standard view and a fit) are one entry.
    if (change.record === true && !recordedInTask) {
      recordSketchView(app, goalCamera())
      recordedInTask = true
      queueMicrotask(() => {
        recordedInTask = false
      })
    }
    if (change.animate === true && !prefersReducedMotion()) {
      animation = { from: camera, to: next, startedAt: performance.now() }
      requestRender()
      return
    }
    animation = null
    camera = next
    mirrorCamera(camera)
    requestRender()
  }

  /** A user camera change: animated, and recorded in the sketch view history. */
  function moveCamera(next: Camera): void {
    setCamera(next, { animate: true, record: true })
  }

  /**
   * The part of the free area the camera frames into, in free-area pixels: beside an open
   * inspector, the side away from it when that side is at least half the free area, else all of
   * it (2.11).
   */
  function framingPart(): { readonly x: number; readonly width: number } {
    const area = framedArea(viewWindow())
    const whole = { x: 0, width: area.width }
    // The store decides whether the inspector is open: an action that frames as it clears the
    // selection (entering a sketch) runs before React removes the closing inspector.
    const inspector = selectInspectorOpen(store.getState())
      ? freeArea.querySelector('[data-testid="inspector"]')
      : null
    if (inspector === null) {
      return whole
    }
    // Only the part of the card inside the free area counts; a card under an island covers none.
    const box = inspector.getBoundingClientRect()
    const offset = box.left - freeArea.getBoundingClientRect().left
    const left = Math.min(Math.max(offset, 0), area.width)
    const right = Math.min(Math.max(offset + box.width, 0), area.width)
    if (right <= left) {
      return whole
    }
    const part =
      (left + right) / 2 < area.width / 2
        ? { x: right, width: area.width - right }
        : { x: 0, width: left }
    return part.width < area.width / 2 ? whole : part
  }

  /** Pans a camera so what it shows at the free area's center goes to the center of `part`. */
  function centeredIn(part: { readonly x: number; readonly width: number }, next: Camera): Camera {
    const offset = part.x + part.width / 2 - framedArea(viewWindow()).width / 2
    return offset === 0 ? next : pan(next, offset, 0, freeHeight())
  }

  /** The camera that frames `bounds` with `margin` in the framing part, as fit does (2.11). */
  function framing(start: Camera, bounds: BoundingBox, margin: number): Camera {
    const part = framingPart()
    const partAspect = aspectOf({ width: part.width, height: freeHeight() })
    return centeredIn(part, fitBounds(start, bounds, partAspect, margin))
  }

  /** Frames bounds as a user camera change, and remembers them for a new free area. */
  function frameBounds(bounds: BoundingBox, margin: number): void {
    const next = framing(goalCamera(), bounds, margin)
    moveCamera(next)
    fitted = { camera: next, bounds, margin }
  }

  function fit(bounds?: BoundingBox): void {
    const framed = bounds ?? scene.visibleBounds(store.getState()) ?? emptyFitBounds()
    frameBounds(framed, DEFAULT_FIT_MARGIN)
  }

  // Sizes.

  /** Measures the canvas and the free area in it, after layout (3.2). */
  function resize(): void {
    const width = canvas.clientWidth
    const height = canvas.clientHeight
    const pixelRatio = Math.min(
      typeof devicePixelRatio === 'number' && devicePixelRatio > 0 ? devicePixelRatio : 1,
      MAXIMUM_PIXEL_RATIO,
    )
    const canvasBox = canvas.getBoundingClientRect()
    const freeBox = freeArea.getBoundingClientRect()
    const measured: ViewRectangle = {
      x: freeBox.left - canvasBox.left,
      y: freeBox.top - canvasBox.top,
      width: freeBox.width,
      height: freeBox.height,
    }
    const sizeChanged =
      width !== size.width || height !== size.height || pixelRatio !== size.pixelRatio
    const freeChanged =
      measured.x !== free.x ||
      measured.y !== free.y ||
      measured.width !== free.width ||
      measured.height !== free.height
    if (!sizeChanged && !freeChanged) {
      return
    }
    if (sizeChanged) {
      size = { width, height, pixelRatio }
      backend.resize(width, height, pixelRatio)
    }
    free = measured
    reframe()
    requestRender()
  }

  /**
   * Frames the last fit's bounds again in a new free area, at once and without history, while the
   * camera is still the one that fit set; any other camera keeps its target, rotation and visible
   * height (2.11).
   */
  function reframe(): void {
    if (fitted === null || goalCamera() !== fitted.camera) {
      return
    }
    const next = framing(fitted.camera, fitted.bounds, fitted.margin)
    animation = null
    camera = next
    fitted = { ...fitted, camera: next }
    mirrorCamera(camera)
  }

  // Picking.

  function canvasPoint(event: { clientX: number; clientY: number }): { x: number; y: number } {
    const rectangle = canvas.getBoundingClientRect()
    return { x: event.clientX - rectangle.left, y: event.clientY - rectangle.top }
  }

  function currentFrame(): RenderFrame {
    if (lastFrame === null || renderNeeded) {
      renderNeeded = false
      render(performance.now())
    }
    return (
      lastFrame ??
      scene.frame(store.getState(), camera, { ...viewWindow(), pixelRatio: size.pixelRatio })
    )
  }

  /**
   * A frame of the model drawn now as `view` shows it, for a click picked with the camera kept at
   * the click (3.6). Its pick ids replace those of the frame drawn, so the next draw or pick renders
   * the current camera's frame again.
   */
  function frameWith(view: Camera): RenderFrame {
    const frame = scene.frame(store.getState(), view, {
      ...viewWindow(),
      pixelRatio: size.pixelRatio,
    })
    requestRender()
    return frame
  }

  /**
   * The pick the pointer means at (x, y), with the world point under it when the depth is known,
   * as `view` shows the model drawn now: the camera, or the camera kept at a click that waited (3.6).
   */
  function pickAtPoint(x: number, y: number, view: Camera = camera): PickResult | null {
    const frame = view === camera ? currentFrame() : frameWith(view)
    const pickRadius = radius()
    const started = performance.now()
    const picked = backend.pick(frame, x, y, pickRadius)
    measureTiming(TIMING_NAMES.pick, started)
    const samples = picked.toSorted((first, second) => first.pickId - second.pickId)
    const candidates: (PickCandidate & {
      readonly result: PickResult
      readonly depth: number | null
    })[] = []
    for (const sample of samples) {
      const target = scene.targetFor(sample.pickId)
      if (target === null) {
        continue
      }
      const { result } = target
      candidates.push({
        key: result.kind === 'mesh' ? result.key : result.item.key,
        kind: result.kind === 'mesh' ? result.element : candidateKind(result.item),
        pickClass: target.pickClass,
        distance: sample.distance,
        result,
        depth: sample.depth,
      })
    }
    const chosen = choosePick(candidates, () => true, pickRadius)
    if (chosen === null) {
      return null
    }
    const found = candidates.find((candidate) => candidate === chosen)
    if (found === undefined) {
      return null
    }
    const world =
      found.depth === null ? null : pointAtDepth(view, cursorAt(x, y), aspect(), found.depth)
    return { ...found.result, world }
  }

  /** Element lists of bodies, for faces hovered as their edges, per model mesh (3.7). */
  const hoverLists = new Map<
    string,
    { readonly meshId: number; readonly list: Promise<readonly ElementDescriptor[]> }
  >()

  function listedElements(
    body: string,
    kind: 'face' | 'edge',
    meshId: number,
  ): Promise<readonly ElementDescriptor[]> | null {
    const document = store.getState().document
    if (document === null) {
      return null
    }
    const key = `${body}:${kind}`
    const cached = hoverLists.get(key)
    if (cached?.meshId === meshId) {
      return cached.list
    }
    const list = app.core.listElements(document.documentHandle, body, kind)
    hoverLists.set(key, { meshId, list })
    list.catch(() => {
      // A failed list is asked again on the next hover.
      if (hoverLists.get(key)?.list === list) {
        hoverLists.delete(key)
      }
    })
    return list
  }

  /** Lights up the edges around a hovered face, instead of the face, while it stands for them (3.7). */
  function updateHoverEdges(result: PickResult | null): void {
    if (
      result === null ||
      result.kind !== 'mesh' ||
      result.element !== 'face' ||
      !facesStandForEdges(store.getState())
    ) {
      scene.setHoverEdges(null)
      return
    }
    const faces = listedElements(result.body, 'face', result.meshId)
    const edges = listedElements(result.body, 'edge', result.meshId)
    if (faces === null || edges === null) {
      scene.setHoverEdges(null)
      return
    }
    Promise.all([faces, edges]).then(
      ([faceList, edgeList]) => {
        if (disposed || store.getState().hover?.key !== result.key) {
          return
        }
        const adjacent = new Set(faceList[result.index]?.adjacent ?? [])
        scene.setHoverEdges({
          faceKey: result.key,
          body: result.body,
          meshId: result.meshId,
          edges: edgeList.flatMap((edge, index) => (adjacent.has(edge.name) ? [index] : [])),
        })
        requestRender()
      },
      (error: unknown) => {
        console.warn('Partsmith could not list the edges around the face.', error)
      },
    )
  }

  function hover(x: number, y: number): void {
    const result = pickAtPoint(x, y)
    updateHoverEdges(result)
    const target =
      result === null
        ? null
        : result.kind === 'mesh'
          ? { key: result.key, kind: result.element }
          : { key: result.item.key, kind: result.item.kind }
    const current = store.getState().hover
    if (current?.key !== target?.key || current?.kind !== target?.kind) {
      store.setState({ hover: target })
    }
    if ((target !== null) !== pointerClass) {
      pointerClass = target !== null
      canvas.classList.toggle('cursor-pointer', pointerClass)
    }
  }

  /**
   * The ray of `view` through the canvas point (x, y), and the world size of one CSS pixel times
   * the UI scale at the picked point, or at the camera target without one (3.7 hole centers).
   */
  function pointerRay(x: number, y: number, picked: Vector3 | null, view: Camera): PointerRay {
    const cursor = cursorAt(x, y)
    const near = pointAtDepth(view, cursor, aspect(), 0)
    const far = pointAtDepth(view, cursor, aspect(), 1)
    const direction =
      view.projection === 'perspective'
        ? (normalize(subtract(far, eye(view))) ?? forward(view))
        : forward(view)
    const origin = view.projection === 'perspective' ? eye(view) : near
    const depth = picked === null ? null : dot(subtract(picked, eye(view)), forward(view))
    const visibleHeight = visibleHeightAtDepth(view, depth ?? view.distance)
    const millimetersPerPixel = (visibleHeight / freeHeight()) * store.getState().settings.uiScale
    return { origin, direction, millimetersPerPixel }
  }

  /** The world point under the cursor from the ID pass depth, or the target (3.13 pivot). */
  function pivotAt(x: number, y: number): Vector3 {
    const frame = currentFrame()
    const [sample] = backend
      .pick(frame, x, y, 0)
      .filter((candidate) => candidate.depth !== null)
      .toSorted((first, second) => (first.depth ?? 0) - (second.depth ?? 0))
    if (sample?.depth === null || sample === undefined) {
      return camera.target
    }
    return pointAtDepth(camera, cursorAt(x, y), aspect(), sample.depth)
  }

  /**
   * A click: in its place among the clicks and selection changes (3.6). One made while a feature
   * command's end is on its way holds its place from now and is picked once that end has settled,
   * against the model drawn then, with the camera kept at the click, and goes where a click goes
   * then.
   */
  function click(x: number, y: number, toggle: boolean): Promise<void> {
    return afterCommandEnd((view, landing) => clickAt(x, y, toggle, view, landing))
  }

  /**
   * Runs `act` with the camera and the way a click lands (3.6): at once with the camera, or, while
   * a feature command's end is on its way, in the click's place from now, once that end has
   * settled, with the camera kept at the click.
   */
  function afterCommandEnd(
    act: (view: Camera, landing: ClickLanding) => Promise<void>,
  ): Promise<void> {
    const ending = commandEnding(app)
    if (ending === null) {
      return act(camera, LANDS_IN_ORDER)
    }
    const atClick = camera
    return landInTurn(app, () => act(atClick, LANDS_NOW), ending)
  }

  /** Where a click at (x, y) with `view` goes now, landing as `landing` says. */
  async function clickAt(
    x: number,
    y: number,
    toggle: boolean,
    view: Camera,
    landing: ClickLanding,
  ): Promise<void> {
    const state = store.getState()
    const sketch = state.context === 'sketch' ? state.sketch : null
    // A sketch tool takes the clicks itself (3.16.3).
    if (sketch !== null && sketch.sketchId !== null && sketch.tool !== null) {
      return
    }
    const result = pickAtPoint(x, y, view)
    if (sketch !== null && sketch.sketchId === null) {
      // A start takes its turn among the starts (3.7 step 1); the clicks after it go on.
      pickSketchPlane(result).catch(reportFailure('Placing the sketch'))
      return
    }
    if (state.context === 'print' && state.print.pickingFace && state.command === null) {
      await pickFaceToLay(result)
      return
    }
    if (sketch !== null) {
      selectInSketch(result, toggle)
      return
    }
    if (result === null) {
      if (state.command === null) {
        landing.clear(app)
      }
      return
    }
    const before = highlights.referenceKeys(state)
    // A face that stands for its edges adds edge references, which resolve through the core.
    const standsForEdges =
      result.kind === 'mesh' && result.element === 'face' && facesStandForEdges(state)
    await landing.pick(app, result, { toggle }, pointerRay(x, y, result.world, view))
    if (result.kind === 'mesh') {
      if (!standsForEdges) {
        highlights.rememberClick(before, result)
      }
      highlights.update()
    }
  }

  function selectInSketch(result: PickResult | null, toggle: boolean): void {
    const item = result?.kind === 'item' ? result.item : null
    store.setState((state) => {
      if (state.sketch === null) {
        return {}
      }
      const current = state.sketch.selection
      let selection: readonly SelectionItem[]
      if (item === null) {
        selection = toggle ? current : []
      } else if (toggle) {
        selection = current.some((selected) => selected.key === item.key)
          ? current.filter((selected) => selected.key !== item.key)
          : [...current, item]
      } else {
        selection = [item]
      }
      return { sketch: { ...state.sketch, selection } }
    })
  }

  /** The face clicked during Lay on face: a flat face rests on the bed, another face is refused (3.17). */
  async function pickFaceToLay(result: PickResult | null): Promise<void> {
    if (result?.kind !== 'mesh' || result.element !== 'face') {
      return
    }
    const item = await pickedItem(app, result)
    if (item !== null) {
      layOnFace(app, item)
    }
  }

  /**
   * A click during plane picking: a new sketch or the re-attached one goes on what it picks, and a
   * face that is not flat is refused with `pick_plane` in the sketch hint (3.16.1). It takes its
   * turn among the starts now, before the core answers its pick (3.7 step 1).
   */
  async function pickSketchPlane(result: PickResult | null): Promise<void> {
    if (result !== null) {
      await pickPlane(app, pickedItem(app, result))
    }
  }

  /**
   * A double-click (3.6): edits the feature of a face, or enters the sketch of a sketch item, in
   * its turn among the starts, which it takes now, before the core answers its pick (3.7 step 1).
   * One made while a feature command's end is on its way is picked once that end has settled, with
   * the camera kept at the click.
   */
  function doubleClick(x: number, y: number): Promise<void> {
    const ending = commandEnding(app)
    const state = store.getState()
    if (
      ending === null &&
      !startInProgress(app) &&
      (state.command !== null || state.context === 'sketch')
    ) {
      // Nothing on its way changes the command or sketch mode a double-click finds there.
      return Promise.resolve()
    }
    const atClick = camera
    const picking = ending === null ? doubleClickTarget(x, y, atClick) : null
    // Its turn comes once the clicks of the double-click have landed (3.6).
    return startInTurn(app, async () => {
      await ending
      const target = await (picking ?? doubleClickTarget(x, y, atClick))
      const current = store.getState()
      if (target === null || current.command !== null || current.context === 'sketch') {
        return
      }
      if (target.kind === 'sketch') {
        await openSketch(app, target.feature)
      } else if (
        (current.document?.recipe.features ?? []).some(({ id }) => id === target.feature)
      ) {
        await openFeatureEdit(app, target.feature)
      }
    })
  }

  /**
   * What a double-click at (x, y) with `view` edits: the feature of a face, asked of the core now,
   * a datum plane, or the sketch of a sketch curve, point or region; null for anything else.
   */
  async function doubleClickTarget(
    x: number,
    y: number,
    view: Camera,
  ): Promise<{ readonly kind: 'feature' | 'sketch'; readonly feature: string } | null> {
    const result = pickAtPoint(x, y, view)
    if (result?.kind === 'mesh') {
      const feature =
        result.element === 'face'
          ? ((await pickedItem(app, result))?.element?.feature ?? null)
          : null
      return feature === null ? null : { kind: 'feature', feature }
    }
    const item = result?.item ?? null
    if (item === null || item.feature === null) {
      return null
    }
    if (
      item.kind === 'sketch_curve' ||
      item.kind === 'sketch_point' ||
      item.kind === 'sketch_region'
    ) {
      return { kind: 'sketch', feature: item.feature }
    }
    return item.kind === 'datum_plane' ? { kind: 'feature', feature: item.feature } : null
  }

  // Pointer and wheel input (3.13).

  function onPointerDown(event: PointerEvent): void {
    if (event.button === MIDDLE_BUTTON_INDEX) {
      // No browser autoscroll.
      event.preventDefault()
    }
    if (press !== null) {
      if (press.pointerId === event.pointerId) {
        press.buttons = event.buttons
      }
      return
    }
    const point = canvasPoint(event)
    press = {
      pointerId: event.pointerId,
      startX: point.x,
      startY: point.y,
      startedAt: performance.now(),
      startButtons: event.buttons,
      buttons: event.buttons,
      lastX: point.x,
      lastY: point.y,
      gesture: null,
      pivot: null,
    }
    hoverAt = null
    try {
      canvas.setPointerCapture(event.pointerId)
    } catch {
      // A pointer that is gone cannot be captured; the press still works.
    }
  }

  function onPointerMove(event: PointerEvent): void {
    const point = canvasPoint(event)
    if (press === null) {
      if (event.buttons === 0) {
        hoverAt = point
        schedule()
      }
      return
    }
    if (press.pointerId !== event.pointerId) {
      return
    }
    if (event.buttons === 0) {
      endPress(event, false)
      return
    }
    press.buttons = event.buttons
    const deltaX = point.x - press.lastX
    const deltaY = point.y - press.lastY
    press.lastX = point.x
    press.lastY = point.y
    if (press.gesture === null) {
      const moved = Math.hypot(point.x - press.startX, point.y - press.startY)
      if (moved <= GESTURE_THRESHOLD_PIXELS) {
        return
      }
      const gesture = gestureFor(store.getState().settings.navigation, press.buttons, {
        shift: event.shiftKey,
      })
      beginGesture(press, gesture, { stopAnimation: true })
    } else if (press.gesture === 'none') {
      // A chord added after the press started can still become a gesture (FreeCAD orbit).
      const gesture = gestureFor(store.getState().settings.navigation, press.buttons, {
        shift: event.shiftKey,
      })
      beginGesture(press, gesture, { stopAnimation: false })
    }
    if (press.gesture === 'orbit') {
      camera = orbit(camera, deltaX, deltaY, press.pivot ?? camera.target)
      requestRender()
    } else if (press.gesture === 'pan') {
      camera = pan(camera, deltaX, deltaY, freeHeight())
      requestRender()
    }
  }

  /**
   * Gives a press its gesture. A camera gesture records the view in the sketch view history and,
   * for an orbit, takes the pivot under the press; a press that moved past the threshold also stops
   * a camera animation.
   */
  function beginGesture(
    current: Press,
    gesture: Gesture,
    { stopAnimation }: { readonly stopAnimation: boolean },
  ): void {
    current.gesture = gesture
    if (gesture === 'none') {
      return
    }
    recordSketchView(app, goalCamera())
    if (stopAnimation) {
      animation = null
    }
    if (gesture === 'orbit') {
      current.pivot = pivotAt(current.startX, current.startY)
    }
  }

  function endPress(event: PointerEvent, released: boolean): void {
    const ended = press
    if (ended === null) {
      return
    }
    press = null
    try {
      if (canvas.hasPointerCapture(ended.pointerId)) {
        canvas.releasePointerCapture(ended.pointerId)
      }
    } catch {
      // The capture is already gone.
    }
    if (ended.gesture === 'orbit' || ended.gesture === 'pan') {
      mirrorCamera(camera)
      return
    }
    const point = canvasPoint(event)
    // A press that passed the gesture threshold without a camera gesture is still a click within 4 pixels (3.6).
    const isClick =
      released &&
      Math.hypot(point.x - ended.startX, point.y - ended.startY) <= CLICK_DISTANCE &&
      performance.now() - ended.startedAt <= CLICK_MILLISECONDS
    if (isClick && ended.startButtons === LEFT_BUTTON && event.button === 0) {
      click(point.x, point.y, event.shiftKey || event.ctrlKey || event.metaKey).catch(
        reportFailure('A viewport action'),
      )
    } else if (
      released &&
      ended.gesture === null &&
      ended.startButtons === RIGHT_BUTTON &&
      event.button === SECONDARY_BUTTON_INDEX
    ) {
      // A right press released before it became an orbit or pan opens the context menu (3.13).
      contextClick(ended.startX, ended.startY).catch(reportFailure('A viewport action'))
    } else if (
      isClick &&
      ended.startButtons === MIDDLE_BUTTON &&
      event.button === MIDDLE_BUTTON_INDEX
    ) {
      middleClick(point)
    }
  }

  /** Whether a pick is in the selection already, so a right click on it keeps the selection (3.13). */
  function pickSelected(result: PickResult, state: AppState): boolean {
    if (result.kind === 'item') {
      const selection =
        state.context === 'sketch' ? (state.sketch?.selection ?? []) : state.selection
      return selection.some((item) => item.key === result.item.key)
    }
    if (state.view.filter.body && !state.view.filter.face && result.element === 'face') {
      return state.selection.some((item) => item.kind === 'body' && item.body === result.body)
    }
    return highlights.isPicked(state, result)
  }

  /**
   * A right click (3.13): a pick target under the pointer that is not selected becomes the
   * selection, as a click would, then the context menu opens there. During a command, a face pick
   * or a sketch tool the click selects nothing. One made while a feature command's end is on its
   * way decides this once that end has settled, as a click does (3.6), and the menu opens then.
   */
  async function contextClick(x: number, y: number): Promise<void> {
    await afterCommandEnd((view, landing) => selectForMenu(x, y, view, landing))
    requestUi(app, { kind: 'viewport_menu', x, y })
  }

  /** What a right click at (x, y) with `view` selects before its menu opens (3.13). */
  async function selectForMenu(
    x: number,
    y: number,
    view: Camera,
    landing: ClickLanding,
  ): Promise<void> {
    const state = store.getState()
    const sketch = state.context === 'sketch' ? state.sketch : null
    const picking =
      state.command !== null ||
      (state.context === 'print' && state.print.pickingFace) ||
      (sketch !== null && (sketch.sketchId === null || sketch.tool !== null))
    if (picking) {
      return
    }
    const result = pickAtPoint(x, y, view)
    if (result !== null && !pickSelected(result, state)) {
      await clickAt(x, y, false, view, landing)
    }
  }

  /** A second middle click close to the first fits the view in presets that ask for it (3.13). */
  function middleClick(point: { readonly x: number; readonly y: number }): void {
    const now = performance.now()
    const previous = lastMiddleClick
    const double =
      previous !== null &&
      now - previous.at <= DOUBLE_CLICK_MILLISECONDS &&
      Math.hypot(point.x - previous.x, point.y - previous.y) <= CLICK_DISTANCE
    if (!double) {
      lastMiddleClick = { at: now, x: point.x, y: point.y }
      return
    }
    lastMiddleClick = null
    const preset = store.getState().settings.navigation
    if (NAVIGATION_PRESETS.find((entry) => entry.id === preset)?.middleDoubleClickFits === true) {
      fit()
    }
  }

  function onPointerUp(event: PointerEvent): void {
    if (press === null || press.pointerId !== event.pointerId) {
      return
    }
    if (event.buttons !== 0) {
      // Another button is still down (FreeCAD chords).
      press.buttons = event.buttons
      return
    }
    endPress(event, true)
  }

  function onPointerCancel(event: PointerEvent): void {
    if (press !== null && press.pointerId === event.pointerId) {
      endPress(event, false)
    }
  }

  function onPointerLeave(): void {
    hoverAt = null
    if (press === null && store.getState().hover !== null) {
      store.setState({ hover: null })
    }
  }

  function onWheel(event: WheelEvent): void {
    event.preventDefault()
    const point = canvasPoint(event)
    if (wheelBurst === null) {
      wheelBurst = {
        before: goalCamera(),
        timer: setTimeout(endWheelBurst, WHEEL_BURST_MILLISECONDS),
      }
    } else {
      clearTimeout(wheelBurst.timer)
      wheelBurst.timer = setTimeout(endWheelBurst, WHEEL_BURST_MILLISECONDS)
    }
    animation = null
    const factor = wheelZoomFactor(event, freeHeight())
    camera = zoomAt(camera, factor, cursorAt(point.x, point.y), aspect())
    requestRender()
  }

  function endWheelBurst(): void {
    const burst = wheelBurst
    wheelBurst = null
    if (burst !== null && !disposed) {
      recordSketchView(app, burst.before)
      mirrorCamera(camera)
    }
  }

  function onDoubleClick(event: MouseEvent): void {
    if (event.button !== 0) {
      return
    }
    const point = canvasPoint(event)
    doubleClick(point.x, point.y).catch(reportFailure('A viewport action'))
  }

  // Store and core subscriptions.

  cleanups.push(
    store.subscribe((state, previous) => {
      if (disposed || !relevantChange(state, previous)) {
        return
      }
      highlights.storeChanged(state, previous)
      requestRender()
    }),
  )

  cleanups.push(
    onCoreEvent(app, (event) => {
      scene.applyEvent(event)
      requestRender()
    }),
  )

  cleanups.push(
    backend.onContextLost(() => {
      lastFrame = null
    }),
  )
  cleanups.push(
    backend.onContextRestored(() => {
      scene.restore()
      requestRender()
    }),
  )

  /** Adds an event listener that dispose removes. */
  function listen<Type extends keyof HTMLElementEventMap>(
    target: HTMLElement,
    type: Type,
    handler: (event: HTMLElementEventMap[Type]) => void,
    listenerOptions?: AddEventListenerOptions,
  ): void {
    target.addEventListener(type, handler, listenerOptions)
    cleanups.push(() => {
      target.removeEventListener(type, handler)
    })
  }

  listen(canvas, 'pointerdown', onPointerDown)
  listen(canvas, 'pointermove', onPointerMove)
  listen(canvas, 'pointerup', onPointerUp)
  listen(canvas, 'pointercancel', onPointerCancel)
  listen(canvas, 'lostpointercapture', onPointerCancel)
  listen(canvas, 'pointerleave', onPointerLeave)
  listen(canvas, 'mousedown', onMouseDown)
  listen(canvas, 'wheel', onWheel, { passive: false })
  listen(canvas, 'contextmenu', onContextMenu)
  listen(canvas, 'dblclick', onDoubleClick)

  cleanups.push(() => {
    if (wheelBurst !== null) {
      clearTimeout(wheelBurst.timer)
      wheelBurst = null
    }
    if (hoverTimer !== null) {
      clearTimeout(hoverTimer)
      hoverTimer = null
    }
  })

  // Islands opening, closing or collapsing change the free area without resizing the canvas.
  if (typeof ResizeObserver === 'function') {
    const observer = new ResizeObserver(resize)
    observer.observe(canvas)
    observer.observe(freeArea)
    cleanups.push(() => {
      observer.disconnect()
    })
  }
  window.addEventListener('resize', resize)
  cleanups.push(() => {
    window.removeEventListener('resize', resize)
  })

  resize()
  // Highlight what the store already selects.
  highlights.storeChanged(store.getState(), null)

  return {
    getCamera: () => camera,
    setCamera,
    fit,
    setStandardView(view) {
      moveCamera({ ...goalCamera(), rotation: standardViewRotation(view) })
    },
    toggleProjection() {
      const goal = goalCamera()
      moveCamera(
        withProjection(goal, goal.projection === 'perspective' ? 'orthographic' : 'perspective'),
      )
    },
    lookAlong(normal, upHint, target) {
      const goal = goalCamera()
      const rotation = rotationLookingAlong(scale(normal, -1), upHint)
      moveCamera(centeredIn(framingPart(), { ...goal, target, rotation }))
    },
    zoomToBounds: frameBounds,
    itemBounds(items) {
      const bounds = extendBounds(null, scene.worldPoints(items))
      return bounds === null ? null : { min: [...bounds.min], max: [...bounds.max] }
    },
    screenRectangle(items) {
      return scene.screenRectangle(items, camera, viewWindow())
    },
    viewWindow,
    requestRender,
    navigate(navigation) {
      const goal = goalCamera()
      animation = null
      camera = goal
      switch (navigation.kind) {
        case 'orbit':
          setCamera(orbit(goal, navigation.deltaX, navigation.deltaY, goal.target), {
            record: true,
          })
          return
        case 'pan': {
          const height = freeHeight()
          setCamera(pan(goal, navigation.deltaX * height, navigation.deltaY * height, height), {
            record: true,
          })
          return
        }
        case 'zoom':
          setCamera(zoomAt(goal, navigation.factor, [0, 0], aspect()), { record: true })
      }
    },
    onRender(listener) {
      renderListeners.add(listener)
      return () => {
        renderListeners.delete(listener)
      }
    },
    dispose() {
      if (disposed) {
        return
      }
      disposed = true
      cancelFrame?.()
      cancelFrame = null
      highlights.dispose()
      for (const cleanup of cleanups.splice(0).toReversed()) {
        cleanup()
      }
      renderListeners.clear()
    },
  }
}
