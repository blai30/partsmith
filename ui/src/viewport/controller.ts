import { editFeature } from '../app/actions/command'
import { layOnFace } from '../app/actions/print'
import { clearSelection, elementSelectionItem, pickAt } from '../app/actions/selection'
import { createSketchOn, enterSketch, recordSketchView } from '../app/actions/sketch'
import { onCoreEvent, type App } from '../app/createApp'
import { CoreError } from '../core/client'
import { createLatestOnly, type LatestOnly } from '../core/queue'
import type {
  BoundingBox,
  DocHandle,
  ElementDescriptor,
  ElementRef,
  MeshElement,
  Reference,
  Resolution,
} from '../generated/protocol'
import type { Rectangle } from '../inspector/placement'
import { requestUi } from '../layout/requests'
import { measureTiming, TIMING_NAMES } from '../performance/marks'
import type { AppState, SelectionItem } from '../state/store'
import type { RenderBackend, RenderFrame } from './backend'
import {
  DEFAULT_CAMERA,
  classifyView,
  easeOutCubic,
  eye,
  fitBounds,
  forward,
  interpolateCameras,
  normalizedPosition,
  orbit,
  pan,
  pointAtDepth,
  rotationLookingAlong,
  standardViewRotation,
  wheelZoomFactor,
  withProjection,
  zoomAt,
  type Camera,
  type StandardView,
  type Vector3,
} from './camera'
import { drawnGridSpacing } from './grid'
import { dot, normalize, scale, subtract } from './math'
import {
  gestureFor,
  GESTURE_THRESHOLD_PIXELS,
  NAVIGATION_PRESETS,
  type KeyNavigation,
} from './navigation'
import { positionElement } from './overlay'
import { choosePick, type PickCandidate, type PickResult, type PointerRay } from './picking'
import { commandSelectionFields, type CommandSelectionField, type Scene } from './scene'

// The viewport controller (ui.md 2.11, 3.6, 3.12, 3.13): the camera, rendering on demand, mouse
// and wheel navigation, hover and click picking, and the highlights that need the core. It
// subscribes to the store itself, so the viewport bypasses React (ADR 0002).

export type ViewportHandle = {
  getCamera(): Camera
  setCamera(camera: Camera, options?: { animate?: boolean; record?: boolean }): void
  fit(bounds?: BoundingBox): void
  setStandardView(view: StandardView): void
  toggleProjection(): void
  /** Puts the eye on the `normal` side of `target`, looking along -normal, with up from `upHint`. */
  lookAlong(normal: Vector3, upHint: Vector3, target: Vector3): void
  zoomToBounds(bounds: BoundingBox, margin: number): void
  screenRectangle(items: readonly SelectionItem[]): Rectangle | null
  requestRender(): void
}

/** What a rendered frame offers the chrome that follows the camera in the same animation frame. */
export type RenderedFrame = {
  readonly camera: Camera
  readonly size: { readonly width: number; readonly height: number }
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
const FIT_MARGIN = 0.1

/** A second middle click within this long of the first is a double-click (3.13). */
const DOUBLE_CLICK_MILLISECONDS = 500
/** Hover picking waits until the camera has been still this long (3.13). */
const CAMERA_STILL_MILLISECONDS = 100

const PRIMARY_BUTTON = 1
const MIDDLE_BUTTONS = 4
const MIDDLE_BUTTON_INDEX = 1
const SECONDARY_BUTTONS = 2
const SECONDARY_BUTTON_INDEX = 2

/** Pick errors that mean the geometry under the pointer is gone or not pickable. */
const IGNORED_PICK_ERRORS: ReadonlySet<string> = new Set(['stale_pick', 'preview_not_pickable'])

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
  gesture: 'orbit' | 'pan' | 'none' | null
  pivot: Vector3 | null
}

type CameraAnimation = {
  readonly from: Camera
  readonly to: Camera
  readonly startedAt: number
}

/** A resolved reference: its display ids in the mesh its body had when it was resolved. */
type ResolvedReference = {
  readonly body: string
  readonly meshId: number
  /** Null until the reference is resolved; selected elements found by a click are known at once. */
  readonly matches: readonly MeshElement[] | null
}

/** A face list of one body, in list_elements order (display ids are positions). */
type FaceList = { readonly meshId: number; readonly faces: readonly ElementDescriptor[] }

function referenceKey(body: string, reference: Reference): string {
  return `${body}|${reference.q}|${reference.fp}`
}

function isReference(value: unknown): value is Reference {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof Reflect.get(value, 'q') === 'string' &&
    typeof Reflect.get(value, 'fp') === 'string'
  )
}

/** Whether the active selection field takes a face as its edges (Fillet, Chamfer; 3.7). */
function facesStandForEdges(state: AppState): boolean {
  const command = state.command
  const field = commandSelectionFields(state).find(
    (candidate) => candidate.path === command?.activeField,
  )
  return field?.accepts === 'edge' && field.multiple
}

/** The element references the viewport highlights: the app selection's elements, or the command's field values. */
function highlightedReferences(state: AppState): {
  readonly references: readonly ElementRef[]
  /** Selection keys of the app selection's elements by reference key, to drop lost ones. */
  readonly selectionKeys: ReadonlyMap<string, string>
} {
  const references: ElementRef[] = []
  const selectionKeys = new Map<string, string>()
  if (state.command === null) {
    for (const item of state.selection) {
      if (item.element !== null) {
        const key = referenceKey(item.element.body, item.element.reference)
        references.push({ body: item.element.body, reference: item.element.reference })
        selectionKeys.set(key, item.key)
      }
    }
    return { references, selectionKeys }
  }
  for (const field of commandSelectionFields(state)) {
    references.push(...fieldReferences(field))
  }
  return { references, selectionKeys }
}

/** The model elements a command selection field names: its faces or edges, or the face or edge of a plane or axis. */
function fieldReferences(field: CommandSelectionField): ElementRef[] {
  const references: ElementRef[] = []
  const values = Array.isArray(field.value) ? field.value : [field.value]
  const body = field.siblings['body']
  for (const value of values) {
    if ((field.accepts === 'face' || field.accepts === 'edge') && typeof body === 'string') {
      if (isReference(value)) {
        references.push({ body, reference: value })
      }
      continue
    }
    if (field.accepts !== 'plane' && field.accepts !== 'axis') {
      continue
    }
    if (typeof value !== 'object' || value === null) {
      continue
    }
    const valueBody: unknown = Reflect.get(value, 'body')
    const element: unknown = Reflect.get(value, 'face') ?? Reflect.get(value, 'edge')
    if (typeof valueBody === 'string' && isReference(element)) {
      references.push({ body: valueBody, reference: element })
    }
  }
  return references
}

/** The model elements of the selection field whose box the pointer is over (3.7), or none. */
function hoveredFieldReferences(state: AppState): readonly ElementRef[] {
  const path = state.command?.hoveredField ?? null
  const field =
    path === null
      ? undefined
      : commandSelectionFields(state).find((candidate) => candidate.path === path)
  return field === undefined ? [] : fieldReferences(field)
}

function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches
}

/** Animation frames, or a timer where the environment has none. */
function requestFrame(callback: (time: number) => void): () => void {
  if (typeof requestAnimationFrame === 'function') {
    const id = requestAnimationFrame(callback)
    return () => {
      cancelAnimationFrame(id)
    }
  }
  const timer = setTimeout(() => {
    callback(performance.now())
  }, 16)
  return () => {
    clearTimeout(timer)
  }
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

/** Reports a viewport action that failed. */
function report(error: unknown): void {
  console.error('A viewport action failed.', error)
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

/** The features whose faces are highlighted: the selected ones while no command is active (3.6). */
function selectedFeatures(state: AppState): ReadonlySet<string> {
  if (state.command !== null) {
    return new Set()
  }
  return new Set(
    state.selection.flatMap((item) =>
      (item.kind === 'feature' || item.kind === 'datum_plane') && item.feature !== null
        ? [item.feature]
        : [],
    ),
  )
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

export function createViewportController(options: {
  app: App
  canvas: HTMLCanvasElement
  overlay: HTMLElement
  backend: RenderBackend
  scene: Scene
}): ViewportController {
  const { app, canvas, overlay, backend, scene } = options
  const store = app.store
  let camera: Camera = DEFAULT_CAMERA
  let animation: CameraAnimation | null = null
  let size = { width: 0, height: 0, pixelRatio: 1 }
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
  let hoverTimer: ReturnType<typeof setTimeout> | null = null
  /** The last middle-button click, for the Fit to view double-click (3.13). */
  let lastMiddleClick: { readonly at: number; readonly x: number; readonly y: number } | null = null
  /** Whether this task already recorded a sketch view entry: one user action is one entry. */
  let recordedInTask = false
  const renderListeners = new Set<(frame: RenderedFrame) => void>()
  const cleanups: (() => void)[] = []

  // Highlights that need the core (3.6, 3.7).
  const resolved = new Map<string, ResolvedReference>()
  const faceLists = new Map<string, FaceList>()
  /** The mesh id each body's face list was last asked for. */
  const requestedFaces = new Map<string, number>()
  const faceRequests = new Map<string, LatestOnly<{ documentHandle: DocHandle; meshId: number }>>()

  function aspect(): number {
    return size.width > 0 && size.height > 0 ? size.width / size.height : 1
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

  function step(): void {
    cancelFrame = null
    if (disposed) {
      return
    }
    advanceAnimation()
    if (renderNeeded) {
      renderNeeded = false
      render()
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

  function render(): void {
    const started = performance.now()
    const state = store.getState()
    if (camera !== renderedCamera) {
      renderedCamera = camera
      cameraChangedAt = started
    }
    lastFrame = scene.frame(state, camera, size)
    backend.render(lastFrame)
    measureTiming(TIMING_NAMES.frame, started)
    const rendered: RenderedFrame = { camera, size, project }
    for (const listener of Array.from(renderListeners)) {
      listener(rendered)
    }
    positionAnchors()
    mirrorCameraTarget()
    mirrorGridSpacing(state)
  }

  /** Mirrors the drawn grid's minor spacing in the store when it changes, for the status bar (3.12, 3.21). */
  function mirrorGridSpacing(state: AppState): void {
    const spacing = drawnGridSpacing(state, camera, size.height)
    if (state.view.gridSpacing !== spacing) {
      store.setState((current) => ({ view: { ...current.view, gridSpacing: spacing } }))
    }
  }

  /** The CSS pixel position of a world point, or null behind the eye. */
  function project(point: Vector3): { x: number; y: number } | null {
    const position = normalizedPosition(camera, point, aspect())
    if (position === null) {
      return null
    }
    return {
      x: ((position.x + 1) / 2) * size.width,
      y: ((1 - position.y) / 2) * size.height,
    }
  }

  /** Moves the overlay's anchored elements (`data-anchor="x,y,z"`) to their projections (3.12). */
  function positionAnchors(): void {
    for (const element of overlay.querySelectorAll<HTMLElement>('[data-anchor]')) {
      const coordinates = (element.dataset['anchor'] ?? '').split(',').map(Number)
      const [x = Number.NaN, y = Number.NaN, z = Number.NaN] = coordinates
      const position = Number.isFinite(x + y + z) ? project([x, y, z]) : null
      const inside =
        position !== null &&
        position.x >= 0 &&
        position.y >= 0 &&
        position.x <= size.width &&
        position.y <= size.height
      if (!inside) {
        element.hidden = true
        continue
      }
      element.hidden = false
      positionElement(element, position.x, position.y)
    }
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

  function fit(bounds?: BoundingBox): void {
    const framed = bounds ?? scene.visibleBounds(store.getState()) ?? emptyFitBounds()
    moveCamera(fitBounds(goalCamera(), framed, aspect(), FIT_MARGIN))
  }

  // Sizes.

  function resize(): void {
    const width = canvas.clientWidth
    const height = canvas.clientHeight
    const pixelRatio = Math.min(
      typeof devicePixelRatio === 'number' && devicePixelRatio > 0 ? devicePixelRatio : 1,
      MAXIMUM_PIXEL_RATIO,
    )
    if (width === size.width && height === size.height && pixelRatio === size.pixelRatio) {
      return
    }
    size = { width, height, pixelRatio }
    backend.resize(width, height, pixelRatio)
    requestRender()
  }

  // Picking.

  function canvasPoint(event: { clientX: number; clientY: number }): { x: number; y: number } {
    const rectangle = canvas.getBoundingClientRect()
    return { x: event.clientX - rectangle.left, y: event.clientY - rectangle.top }
  }

  function currentFrame(): RenderFrame {
    if (lastFrame === null || renderNeeded) {
      renderNeeded = false
      render()
    }
    return lastFrame ?? scene.frame(store.getState(), camera, size)
  }

  /** The pick the pointer means at (x, y), with the world point under it when the depth is known. */
  function pickAtPoint(x: number, y: number): PickResult | null {
    const frame = currentFrame()
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
        kind: result.kind === 'mesh' ? result.element : result.item.kind,
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
      found.depth === null
        ? null
        : pointAtDepth(
            camera,
            [(x / Math.max(size.width, 1)) * 2 - 1, 1 - (y / Math.max(size.height, 1)) * 2],
            aspect(),
            found.depth,
          )
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
   * The camera ray through the canvas point (x, y), and the world size of one CSS pixel times the
   * UI scale at the picked point, or at the camera target without one (3.7 hole centers).
   */
  function pointerRay(x: number, y: number, picked: Vector3 | null): PointerRay {
    const cursor: [number, number] = [
      (x / Math.max(size.width, 1)) * 2 - 1,
      1 - (y / Math.max(size.height, 1)) * 2,
    ]
    const near = pointAtDepth(camera, cursor, aspect(), 0)
    const far = pointAtDepth(camera, cursor, aspect(), 1)
    const direction =
      camera.projection === 'perspective'
        ? (normalize(subtract(far, eye(camera))) ?? forward(camera))
        : forward(camera)
    const origin = camera.projection === 'perspective' ? eye(camera) : near
    const depth = picked === null ? null : dot(subtract(picked, eye(camera)), forward(camera))
    const visibleHeight =
      camera.projection === 'perspective'
        ? 2 * (depth ?? camera.distance) * Math.tan(camera.verticalFieldOfView / 2)
        : camera.orthographicHeight
    const millimetersPerPixel =
      (visibleHeight / Math.max(size.height, 1)) * store.getState().settings.uiScale
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
    return pointAtDepth(
      camera,
      [(x / Math.max(size.width, 1)) * 2 - 1, 1 - (y / Math.max(size.height, 1)) * 2],
      aspect(),
      sample.depth,
    )
  }

  /** The selection item of a mesh pick through the core, or null when the pick went stale. */
  async function meshItem(
    result: Extract<PickResult, { kind: 'mesh' }>,
  ): Promise<SelectionItem | null> {
    const document = store.getState().document
    if (document === null) {
      return null
    }
    try {
      const element = await app.core.pickReference(document.documentHandle, {
        mesh_id: result.meshId,
        kind: result.element,
        index: result.index,
      })
      return elementSelectionItem(app, element)
    } catch (error) {
      if (error instanceof CoreError && IGNORED_PICK_ERRORS.has(error.code)) {
        return null
      }
      throw error
    }
  }

  /** Remembers a clicked element's display id, so it is highlighted without a core round trip. */
  function rememberClick(
    before: ReadonlySet<string>,
    result: Extract<PickResult, { kind: 'mesh' }>,
  ): void {
    const { references } = highlightedReferences(store.getState())
    const added = references.filter(
      (reference) =>
        !before.has(referenceKey(reference.body, reference.reference)) &&
        reference.body === result.body,
    )
    // A click adds one element; anything else resolves through the core.
    const [reference, ...others] = added
    if (reference === undefined || others.length > 0) {
      return
    }
    resolved.set(referenceKey(reference.body, reference.reference), {
      body: result.body,
      meshId: result.meshId,
      matches: [
        { body: result.body, mesh_id: result.meshId, kind: result.element, index: result.index },
      ],
    })
  }

  async function click(x: number, y: number, toggle: boolean): Promise<void> {
    const state = store.getState()
    const sketch = state.context === 'sketch' ? state.sketch : null
    // A sketch tool takes the clicks itself (3.16.3).
    if (sketch !== null && sketch.sketchId !== null && sketch.tool !== null) {
      return
    }
    const result = pickAtPoint(x, y)
    if (sketch !== null && sketch.sketchId === null) {
      await pickSketchPlane(result)
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
        clearSelection(app)
      }
      return
    }
    const before = new Set(
      highlightedReferences(state).references.map((reference) =>
        referenceKey(reference.body, reference.reference),
      ),
    )
    // A face that stands for its edges adds edge references, which resolve through the core.
    const standsForEdges =
      result.kind === 'mesh' && result.element === 'face' && facesStandForEdges(state)
    await pickAt(app, result, { toggle }, pointerRay(x, y, result.world))
    if (result.kind === 'mesh') {
      if (!standsForEdges) {
        rememberClick(before, result)
      }
      updateHighlights()
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
    const item = await meshItem(result)
    if (item !== null) {
      layOnFace(app, item)
    }
  }

  async function pickSketchPlane(result: PickResult | null): Promise<void> {
    if (result === null) {
      return
    }
    const item = result.kind === 'item' ? result.item : await meshItem(result)
    if (item === null) {
      return
    }
    // A face that is not flat is refused with `pick_plane` in the sketch hint (3.16.1).
    await createSketchOn(app, item)
  }

  async function doubleClick(x: number, y: number): Promise<void> {
    const state = store.getState()
    if (state.command !== null || state.context === 'sketch') {
      return
    }
    const result = pickAtPoint(x, y)
    if (result === null) {
      return
    }
    const features = state.document?.recipe.features ?? []
    if (result.kind === 'mesh') {
      if (result.element !== 'face') {
        return
      }
      const item = await meshItem(result)
      const feature = item?.element?.feature ?? null
      if (feature !== null && features.some((record) => record.id === feature)) {
        await editFeature(app, feature)
      }
      return
    }
    const item = result.item
    if (
      (item.kind === 'sketch_curve' ||
        item.kind === 'sketch_point' ||
        item.kind === 'sketch_region') &&
      item.feature !== null
    ) {
      await enterSketch(app, item.feature)
    } else if (item.kind === 'datum_plane' && item.feature !== null) {
      await editFeature(app, item.feature)
    }
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
      press.gesture = gestureFor(store.getState().settings.navigation, press.buttons, {
        shift: event.shiftKey,
      })
      if (press.gesture !== 'none') {
        recordSketchView(app, goalCamera())
        animation = null
        if (press.gesture === 'orbit') {
          press.pivot = pivotAt(press.startX, press.startY)
        }
      }
    } else if (press.gesture === 'none') {
      // A chord added after the press started can still become a gesture (FreeCAD orbit).
      const gesture = gestureFor(store.getState().settings.navigation, press.buttons, {
        shift: event.shiftKey,
      })
      if (gesture !== 'none') {
        press.gesture = gesture
        recordSketchView(app, goalCamera())
        if (gesture === 'orbit') {
          press.pivot = pivotAt(press.startX, press.startY)
        }
      }
    }
    if (press.gesture === 'orbit') {
      camera = orbit(camera, deltaX, deltaY, press.pivot ?? camera.target)
      requestRender()
    } else if (press.gesture === 'pan') {
      camera = pan(camera, deltaX, deltaY, Math.max(size.height, 1))
      requestRender()
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
    if (isClick && ended.startButtons === PRIMARY_BUTTON && event.button === 0) {
      click(point.x, point.y, event.shiftKey || event.ctrlKey || event.metaKey).catch(report)
    } else if (
      released &&
      ended.gesture === null &&
      ended.startButtons === SECONDARY_BUTTONS &&
      event.button === SECONDARY_BUTTON_INDEX
    ) {
      // A right press released before it became an orbit or pan opens the context menu (3.13).
      contextClick(ended.startX, ended.startY).catch(report)
    } else if (
      isClick &&
      ended.startButtons === MIDDLE_BUTTONS &&
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
    return highlightedReferences(state).references.some((reference) =>
      (resolved.get(referenceKey(reference.body, reference.reference))?.matches ?? []).some(
        (match) =>
          match.body === result.body &&
          match.mesh_id === result.meshId &&
          match.kind === result.element &&
          match.index === result.index,
      ),
    )
  }

  /**
   * A right click (3.13): a pick target under the pointer that is not selected becomes the
   * selection, as a click would, then the context menu opens there. During a command, a face pick
   * or a sketch tool the click selects nothing.
   */
  async function contextClick(x: number, y: number): Promise<void> {
    const state = store.getState()
    const sketch = state.context === 'sketch' ? state.sketch : null
    const picking =
      state.command !== null ||
      (state.context === 'print' && state.print.pickingFace) ||
      (sketch !== null && (sketch.sketchId === null || sketch.tool !== null))
    if (!picking) {
      const result = pickAtPoint(x, y)
      if (result !== null && !pickSelected(result, state)) {
        await click(x, y, false)
      }
    }
    requestUi(app, { kind: 'viewport_menu', x, y })
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
    const factor = wheelZoomFactor(event, Math.max(size.height, 1))
    camera = zoomAt(
      camera,
      factor,
      [(point.x / Math.max(size.width, 1)) * 2 - 1, 1 - (point.y / Math.max(size.height, 1)) * 2],
      aspect(),
    )
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
    doubleClick(point.x, point.y).catch(report)
  }

  // Highlights through the core.

  const resolutions = createLatestOnly<
    {
      readonly documentHandle: DocHandle
      readonly references: readonly ElementRef[]
      readonly meshIds: readonly number[]
    },
    readonly Resolution[]
  >(
    (request) => app.core.resolveReferences(request.documentHandle, request.references),
    (results, request) => {
      const lost = new Set<string>()
      request.references.forEach((reference, index) => {
        const key = referenceKey(reference.body, reference.reference)
        const resolution = results[index]
        const found = resolution?.status === 'resolved'
        resolved.set(key, {
          body: reference.body,
          meshId: request.meshIds[index] ?? 0,
          matches: found ? resolution.matches : [],
        })
        if (!found) {
          lost.add(key)
        }
      })
      dropLostSelection(lost)
      updateHighlights()
    },
    (error) => {
      console.warn('Partsmith could not highlight the selection after a rebuild.', error)
    },
  )

  /** Selected faces, edges and vertices that no longer resolve leave the selection silently (3.6). */
  function dropLostSelection(lost: ReadonlySet<string>): void {
    if (lost.size === 0) {
      return
    }
    const state = store.getState()
    if (state.command !== null) {
      return
    }
    const { selectionKeys } = highlightedReferences(state)
    const keys = new Set(
      Array.from(lost).flatMap((key) => {
        const selectionKey = selectionKeys.get(key)
        return selectionKey === undefined ? [] : [selectionKey]
      }),
    )
    if (keys.size > 0) {
      store.setState((current) => ({
        selection: current.selection.filter((item) => !keys.has(item.key)),
      }))
    }
  }

  /** Resolves the highlighted references whose body got a new mesh, or that were never resolved during a command. */
  function resolveStale(state: AppState): void {
    const document = state.document
    if (document === null) {
      return
    }
    const { references } = highlightedReferences(state)
    const stale: ElementRef[] = []
    const meshIds: number[] = []
    for (const reference of references) {
      const meshId = document.meshes[reference.body]?.meshId
      if (meshId === undefined) {
        continue
      }
      const key = referenceKey(reference.body, reference.reference)
      const known = resolved.get(key)
      if (known === undefined && state.command === null) {
        // Selected before the viewport knew it: highlighted once its body is rebuilt.
        resolved.set(key, { body: reference.body, meshId, matches: null })
        continue
      }
      if (known === undefined || known.meshId !== meshId) {
        stale.push(reference)
        meshIds.push(meshId)
      }
    }
    if (stale.length > 0) {
      resolutions.submit({ documentHandle: document.documentHandle, references: stale, meshIds })
    }
  }

  function faceRequestOf(body: string): LatestOnly<{ documentHandle: DocHandle; meshId: number }> {
    let request = faceRequests.get(body)
    if (request === undefined) {
      request = createLatestOnly<
        { documentHandle: DocHandle; meshId: number },
        readonly ElementDescriptor[]
      >(
        (input) => app.core.listElements(input.documentHandle, body, 'face'),
        (faces, input) => {
          const current = store.getState().document?.meshes[body]?.meshId
          if (current !== input.meshId) {
            // The body was rebuilt while the list was on its way: ask again for the new mesh.
            requestedFaces.delete(body)
            requestFaces(store.getState())
            return
          }
          faceLists.set(body, { meshId: input.meshId, faces })
          updateHighlights()
        },
        (error) => {
          console.warn('Partsmith could not list the faces to highlight.', error)
        },
      )
      faceRequests.set(body, request)
    }
    return request
  }

  /** Lists the faces of visible bodies while features are selected (3.6 feature highlighting). */
  function requestFaces(state: AppState): void {
    const document = state.document
    if (document === null || selectedFeatures(state).size === 0) {
      return
    }
    for (const body of document.recipe.bodies ?? []) {
      const mesh = document.meshes[body.id]
      if (body.visible === false || mesh === undefined) {
        continue
      }
      const cached = faceLists.get(body.id)
      if (cached?.meshId === mesh.meshId || requestedFaces.get(body.id) === mesh.meshId) {
        continue
      }
      faceLists.delete(body.id)
      requestedFaces.set(body.id, mesh.meshId)
      faceRequestOf(body.id).submit({
        documentHandle: document.documentHandle,
        meshId: mesh.meshId,
      })
    }
  }

  /** The display ids the resolved references stand for, per body, at the current meshes. */
  function resolvedIndices(
    state: AppState,
    references: readonly ElementRef[],
  ): Record<string, { meshId: number; face: number[]; edge: number[]; vertex: number[] }> {
    const document = state.document
    const highlights: Record<
      string,
      { meshId: number; face: number[]; edge: number[]; vertex: number[] }
    > = {}
    if (document === null) {
      return highlights
    }
    for (const reference of references) {
      const known = resolved.get(referenceKey(reference.body, reference.reference))
      for (const match of known?.matches ?? []) {
        if (document.meshes[match.body]?.meshId !== match.mesh_id) {
          continue
        }
        const existing = highlights[match.body]
        const entry =
          existing !== undefined && existing.meshId === match.mesh_id
            ? existing
            : { meshId: match.mesh_id, face: [], edge: [], vertex: [] }
        highlights[match.body] = entry
        entry[match.kind].push(match.index)
      }
    }
    return highlights
  }

  /** Hands the scene the display ids to draw selected and hovered, and renders. */
  function updateHighlights(): void {
    const state = store.getState()
    const document = state.document
    const highlights = resolvedIndices(state, highlightedReferences(state).references)
    const entryOf = (body: string, meshId: number) => {
      const existing = highlights[body]
      if (existing !== undefined && existing.meshId === meshId) {
        return existing
      }
      const created = { meshId, face: [], edge: [], vertex: [] }
      highlights[body] = created
      return created
    }
    if (document !== null) {
      const features = selectedFeatures(state)
      if (features.size > 0) {
        for (const [body, list] of faceLists) {
          if (document.meshes[body]?.meshId !== list.meshId) {
            continue
          }
          list.faces.forEach((face, index) => {
            if (features.has(face.feature)) {
              entryOf(body, list.meshId).face.push(index)
            }
          })
        }
      }
    }
    scene.setElementHighlights(highlights)
    scene.setFieldHover(resolvedIndices(state, hoveredFieldReferences(state)))
    requestRender()
  }

  // Store and core subscriptions.

  cleanups.push(
    store.subscribe((state, previous) => {
      if (disposed || !relevantChange(state, previous)) {
        return
      }
      if (
        state.document !== previous.document ||
        state.selection !== previous.selection ||
        state.command !== previous.command
      ) {
        if (state.document?.documentHandle !== previous.document?.documentHandle) {
          resolved.clear()
          faceLists.clear()
          requestedFaces.clear()
        }
        resolveStale(state)
        requestFaces(state)
        updateHighlights()
        return
      }
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

  const listeners: readonly [string, EventListener, AddEventListenerOptions?][] = [
    ['pointerdown', onPointerDown as EventListener],
    ['pointermove', onPointerMove as EventListener],
    ['pointerup', onPointerUp as EventListener],
    ['pointercancel', onPointerCancel as EventListener],
    ['lostpointercapture', onPointerCancel as EventListener],
    ['pointerleave', onPointerLeave],
    ['mousedown', onMouseDown as EventListener],
    ['wheel', onWheel as EventListener, { passive: false }],
    ['contextmenu', onContextMenu],
    ['dblclick', onDoubleClick as EventListener],
  ]
  for (const [type, listener, listenerOptions] of listeners) {
    canvas.addEventListener(type, listener, listenerOptions)
  }
  cleanups.push(() => {
    for (const [type, listener] of listeners) {
      canvas.removeEventListener(type, listener)
    }
  })

  if (typeof ResizeObserver === 'function') {
    const observer = new ResizeObserver(resize)
    observer.observe(canvas)
    cleanups.push(() => {
      observer.disconnect()
    })
  } else {
    window.addEventListener('resize', resize)
    cleanups.push(() => {
      window.removeEventListener('resize', resize)
    })
  }

  resize()
  resolveStale(store.getState())
  requestFaces(store.getState())
  updateHighlights()

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
      moveCamera({ ...goal, target, rotation: rotationLookingAlong(scale(normal, -1), upHint) })
    },
    zoomToBounds(bounds, margin) {
      moveCamera(fitBounds(goalCamera(), bounds, aspect(), margin))
    },
    screenRectangle(items) {
      return scene.screenRectangle(items, camera, size)
    },
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
          const height = Math.max(size.height, 1)
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
      if (wheelBurst !== null) {
        clearTimeout(wheelBurst.timer)
        wheelBurst = null
      }
      if (hoverTimer !== null) {
        clearTimeout(hoverTimer)
        hoverTimer = null
      }
      resolutions.cancel()
      for (const request of faceRequests.values()) {
        request.cancel()
      }
      for (const cleanup of cleanups.splice(0).toReversed()) {
        cleanup()
      }
      renderListeners.clear()
    },
  }
}
