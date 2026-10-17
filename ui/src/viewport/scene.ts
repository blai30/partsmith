import type { BodyMeshData, CoreEvent, SketchDisplayData } from '../core/events'
import { featureLabel, type WorldAxis } from '../core/labels'
import type {
  BoundingBox,
  Body,
  DocHandle,
  ElementKind,
  Placement,
  SketchSolution,
} from '../generated/protocol'
import type { Rectangle } from '../inspector/placement'
import { originItem } from '../inspector/selection-values'
import { resolvedTheme } from '../layout/color-scheme'
import { bedOutline } from '../print/bed'
import { overhangAnalysis, overhangFields } from '../print/overhang'
import { selectRegionCandidates } from '../repair/repair'
import { regionSelectionItem } from '../sketch/hit'
import type { AppState, HelperSketch, SelectionItem } from '../state/store'
import { isSessionVisible } from '../state/visibility'
import {
  AXIS_ROLES,
  contrastRatio,
  encodedFromHex,
  linearFromHex,
  VIEWPORT_COLORS,
  viewportTheme,
  type ThemeName,
  type ViewportRole,
} from '../theme/roles'
import {
  ELEMENT_STATE,
  type DrawCall,
  type Geometry,
  type RenderBackend,
  type RenderFrame,
  type Rgb,
} from './backend'
import {
  depthRange,
  eye,
  framedArea,
  normalizedPosition,
  visibleHeightAtTarget,
  viewMatrix,
  windowAspect,
  windowPosition,
  windowProjection,
  type Camera,
  type Vector3,
  type ViewWindow,
} from './camera'
import {
  drawnGridSpacing,
  gridCorners,
  gridHalfSide,
  gridPlacement,
  gridSegments,
  millimetersPerPixelAtTarget,
  type GridSegments,
} from './grid'
import { elementExtentPoints } from './element-extent'
import { boundsCenter, boundsCorners, boundsDiagonal, extendBounds, type Bounds } from './math'
import type { PickClass, PickResult, SelectionItemKind } from './picking'
import {
  activeFieldKind,
  hoveredKeys,
  pickingFor,
  shownSelection,
  type Picking,
} from './picking-rules'
import {
  drawnOriginPlanes,
  ORIGIN_PLANE_NORMAL_AXES,
  ORIGIN_PLANE_PLACEMENTS,
  planeExtent,
  sketchWorldAxes,
  squareCorners,
} from './planes'
import { sectionFrameClip, sectionOffsetMillimeters } from './section'
import {
  curveLines,
  entityItem,
  entityWorldPoints,
  onPlane,
  plainItem,
  pointItem,
  pointsOnPlane,
  segmentsOnPlane,
  sequentialIds,
  sketchBounds,
  sketchAxisSpans,
  sketchOriginItem,
  yAxisOf,
  type SolvedEntity,
} from './sketch-drawables'

// The scene (ui.md 2.11, 3.12): every drawable's CPU-side data, so a lost WebGL context can be
// restored; pick id ranges and their targets; and the render frame for a camera and the store.

/** The hover and pick key of a mesh element by display id: `mesh:{meshId}:{kind}:{index}` (3.6). */
export function meshElementKey(meshId: number, kind: ElementKind, index: number): string {
  return `mesh:${String(meshId)}:${kind}:${String(index)}`
}

/**
 * The edges around a hovered face, by display id, while a multiple edge field takes the face as
 * its edges (3.7): they are drawn hovered and the face is not.
 */
export type HoverEdges = {
  /** The hover key of the face (`mesh:{meshId}:face:{index}`). */
  readonly faceKey: string
  readonly body: string
  readonly meshId: number
  readonly edges: readonly number[]
}

/** Display ids of model elements to draw selected, by body, for the mesh they index. */
export type ElementHighlights = Readonly<
  Record<
    string,
    {
      readonly meshId: number
      readonly face: readonly number[]
      readonly edge: readonly number[]
      readonly vertex: readonly number[]
    }
  >
>

export type Scene = {
  applyEvent(event: CoreEvent): void
  /** The pick target for a pick id of the last frame, or null. */
  targetFor(pickId: number): { readonly result: PickResult; readonly pickClass: PickClass } | null
  /** Bounds of visible model geometry, sketches and datum planes; null when nothing is visible. */
  visibleBounds(state: AppState): BoundingBox | null
  /** World points that cover selection items as drawn (3.6), a datum plane's square included: what framing them shows; none for items not drawn. */
  worldPoints(items: readonly SelectionItem[]): Vector3[]
  /** Canvas rectangle (CSS pixels) covering selection items, or null. */
  screenRectangle(
    items: readonly SelectionItem[],
    camera: Camera,
    view: ViewWindow,
  ): Rectangle | null
  /** `frame.projection` is `windowProjection(camera, view, near, far)` (2.11). */
  frame(
    state: AppState,
    camera: Camera,
    view: ViewWindow & { readonly pixelRatio: number },
  ): RenderFrame
  restore(): void
  /** Model elements drawn selected besides selected bodies: resolved selection references and feature highlighting. */
  setElementHighlights(highlights: ElementHighlights): void
  /** Edges drawn hovered in place of a hovered face that stands for its edges (3.7), or null. */
  setHoverEdges(hover: HoverEdges | null): void
  /** Model elements drawn hovered for the selection field whose box the pointer is over (3.7). */
  setFieldHover(highlights: ElementHighlights): void
  /** The evaluated section offset (mm) of an offset expression the store holds as text. */
  setSectionOffset(expression: string, millimeters: number): void
}

// Sizes in CSS pixels at 100 percent UI scale (3.6, 3.12, 3.16.2).
const EDGE_WIDTH = 1.25
const EDGE_PICK_WIDTH = 7
const VERTEX_PICK_SIZE = 9
const SKETCH_CURVE_WIDTH = 1.5
const REFERENCE_CURVE_WIDTH = 1.25
const SKETCH_CURVE_PICK_WIDTH = 7
const SKETCH_POINT_SIZE = 5
const SKETCH_POINT_ENTITY_SIZE = 6
const SKETCH_POINT_PICK_SIZE = 11
const ORIGIN_POINT_SIZE = 9
const PLANE_OUTLINE_WIDTH = 1
const BED_WIDTH = 1.5
/** Grid lines: 1 CSS pixel wide, minor at 35 percent and major at 70 percent opacity (3.12). */
const GRID_LINE_WIDTH = 1
const GRID_MINOR_OPACITY = 0.35
const GRID_MAJOR_OPACITY = 0.7
/** Chord tolerance of sketch circles and arcs. */
const SKETCH_TOLERANCE_PIXELS = 0.25
/** A tessellation is kept until the scale changes by more than this factor. */
const RETESSELLATION_FACTOR = 1.5

const REGION_OPACITY = 0.12
/** The draw of repair mode's region candidates (3.7). */
const REPAIR_REGIONS_ID = 'repair:regions'
/** Profile problem halos reach this many CSS pixels (times the UI scale) beyond each side of a curve (3.16.2). */
const PROBLEM_HALO_PIXELS = 4
/** How much of `problem` a halo mixes over the viewport background (3.16.2). */
const PROBLEM_HALO_AMOUNT = 0.3
const PLANE_FILL_OPACITY = 0.15
const PLANE_OUTLINE_OPACITY = 0.6
const NON_PART_OPACITY = 0.4

const ORIGIN_AXES: readonly { readonly name: WorldAxis; readonly direction: Vector3 }[] = [
  { name: 'X', direction: [1, 0, 0] },
  { name: 'Y', direction: [0, 1, 0] },
  { name: 'Z', direction: [0, 0, 1] },
]

type MeshEntry = {
  readonly documentHandle: DocHandle
  readonly body: string
  readonly meshId: number
  readonly data: BodyMeshData
}

/** A pickable draw's pick ids and how its elements become pick targets. */
type PickTarget = { readonly result: PickResult; readonly pickClass: PickClass }

type PickRange = {
  readonly first: number
  readonly count: number
  readonly target: (index: number) => PickTarget | null
}

/** A draw being built: its call, its element count and how its elements are picked. */
type PendingDraw = {
  readonly call: Omit<DrawCall, 'pickBase'>
  readonly elementCount: number
  /** Null when the draw is not pickable now. */
  readonly target: ((index: number) => PickTarget | null) | null
  readonly states: Uint8Array | null
  readonly scalars?: Float32Array | null
}

/** A body's draws: its faces, then its edges and vertices, which the grid goes between (3.12). */
type BodyDraws = { readonly surfaces: PendingDraw[]; readonly lines: PendingDraw[] }

/** Line segments, two points each, as unpicked line geometry with one element. */
function linesOf(positions: Float32Array): Geometry {
  return {
    kind: 'lines',
    positions,
    elementIds: new Uint32Array(positions.length / 6),
    elementCount: 1,
  }
}

function bodyMeshKey(documentHandle: DocHandle, body: string): string {
  return `${String(documentHandle)}:${body}`
}

function colorOf(theme: ThemeName, role: ViewportRole): Rgb {
  return linearFromHex(VIEWPORT_COLORS[theme][role])
}

/** One linear color per element, in the roles of these world axes (3.12, 3.16.2). */
function axisColors(theme: ThemeName, axes: readonly WorldAxis[]): Float32Array {
  return new Float32Array(axes.flatMap((axis) => [...colorOf(theme, AXIS_ROLES[axis])]))
}

/**
 * A profile problem halo's color: `problem` at 30 percent over the viewport's color, mixed
 * sRGB-encoded as blending would mix it, then drawn opaque, so the overlapping segment quads of a
 * tessellated curve do not darken it in beads (3.16.2).
 */
function problemHaloColor(theme: ThemeName): Rgb {
  const background = encodedFromHex(VIEWPORT_COLORS[theme].viewport)
  const problem = encodedFromHex(VIEWPORT_COLORS[theme].problem)
  const channels = background.map((channel, index) => {
    const mixed = channel + ((problem[index] ?? channel) - channel) * PROBLEM_HALO_AMOUNT
    return Math.round(mixed * 255)
      .toString(16)
      .padStart(2, '0')
  })
  return linearFromHex(`#${channels.join('')}`)
}

/** A box with every -0 written 0, so equal boxes compare equal. */
function normalizedBox(box: Bounds): BoundingBox {
  return {
    min: [box.min[0] + 0, box.min[1] + 0, box.min[2] + 0],
    max: [box.max[0] + 0, box.max[1] + 0, box.max[2] + 0],
  }
}

/** The display ids of one element kind in a body's highlights, or none when they index another mesh. */
function elementIndexSet(
  entry: ElementHighlights[string] | undefined,
  meshId: number,
  element: 'face' | 'edge' | 'vertex',
): ReadonlySet<number> {
  return entry !== undefined && entry.meshId === meshId ? new Set(entry[element]) : new Set()
}

/** A pick target for the item at an index, or null past the end. */
function itemTarget(
  items: readonly (SelectionItem | null)[],
  pickClass: PickClass,
): (index: number) => PickTarget | null {
  return (index) => {
    const item = items[index]
    return item === undefined || item === null
      ? null
      : { result: { kind: 'item', item, world: null }, pickClass }
  }
}

function bodyVisible(body: Body): boolean {
  return body.visible !== false
}

/** The keys drawn hovered and drawn selected. */
type Highlight = {
  readonly hover: ReadonlySet<string>
  readonly selected: ReadonlySet<string>
}

/** One element state per item from its key; a missing item has none. */
function itemStates(items: readonly (SelectionItem | null)[], highlight: Highlight): Uint8Array {
  const states = new Uint8Array(items.length)
  for (let i = 0; i < items.length; i++) {
    const key = items[i]?.key ?? ''
    if (highlight.hover.has(key)) {
      states[i] = (states[i] ?? 0) | ELEMENT_STATE.hover
    }
    if (highlight.selected.has(key)) {
      states[i] = (states[i] ?? 0) | ELEMENT_STATE.selected
    }
  }
  return states
}

function sameBytes(first: Uint8Array | undefined, second: Uint8Array): boolean {
  if (first === undefined || first.length !== second.length) {
    return false
  }
  return first.every((value, index) => value === second[index])
}

/** A sketch the scene draws; the active one is the sketch being edited. */
type VisibleSketch = {
  readonly id: string
  readonly solution: SketchSolution
  readonly active: boolean
  /** The world axes along its x and y axes on an origin plane, else null (3.16.2). */
  readonly worldAxes: readonly [WorldAxis, WorldAxis] | null
}

/** A datum plane the scene draws. */
type VisibleDatum = { readonly id: string; readonly placement: Placement }

function visibleSketches(state: AppState): readonly VisibleSketch[] {
  const document = state.document
  if (document === null) {
    return []
  }
  const activeId = state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null
  return (document.recipe.features ?? []).flatMap((feature) => {
    if (feature.type !== 'sketch') {
      return []
    }
    const solution = document.sketches[feature.id]?.solution
    const active = feature.id === activeId
    if (solution === undefined || (!active && !isSessionVisible(state, `feature:${feature.id}`))) {
      return []
    }
    return [{ id: feature.id, solution, active, worldAxes: sketchWorldAxes(feature) }]
  })
}

function visibleDatums(state: AppState): readonly VisibleDatum[] {
  const document = state.document
  if (document === null) {
    return []
  }
  return (document.recipe.features ?? []).flatMap((feature) => {
    const placement = document.datums[feature.id] ?? null
    if (
      feature.type !== 'datum_plane' ||
      placement === null ||
      !isSessionVisible(state, `feature:${feature.id}`)
    ) {
      return []
    }
    return [{ id: feature.id, placement }]
  })
}

function overlayCall(
  color: Rgb,
  layer: DrawCall['layer'],
  opacity: number,
  width: number,
  dashed: boolean,
  pickable: boolean,
): Omit<DrawCall, 'pickBase' | 'id'> {
  return {
    layer,
    color,
    elementColors: null,
    opacity,
    size: width,
    dashed,
    shading: 'flat',
    depthTest: false,
    clipped: false,
    pickSize: pickable ? width : 0,
    analysis: false,
  }
}

/** The chord tolerance in mm for the camera, quantized so small zoom changes keep a tessellation. */
function sketchTolerance(camera: Camera, height: number): number {
  const exact = SKETCH_TOLERANCE_PIXELS * millimetersPerPixelAtTarget(camera, height, 1)
  const step = Math.log(RETESSELLATION_FACTOR)
  return Math.exp(Math.floor(Math.log(exact) / step) * step)
}

/** Bounds of visible bodies, sketches (through `boundsOf`, which may cache) and datum planes. */
function modelBounds(
  state: AppState,
  sketches: readonly VisibleSketch[],
  datums: readonly VisibleDatum[],
  boundsOf: (solution: SketchSolution) => Bounds | null,
): Bounds | null {
  const document = state.document
  if (document === null) {
    return null
  }
  let bounds: Bounds | null = null
  for (const body of document.recipe.bodies ?? []) {
    const mesh = document.meshes[body.id]
    if (bodyVisible(body) && mesh !== undefined) {
      bounds = extendBounds(bounds, boundsCorners(mesh.boundingBox))
    }
  }
  for (const sketch of sketches) {
    const box = boundsOf(sketch.solution)
    if (box !== null) {
      bounds = extendBounds(bounds, boundsCorners(box))
    }
  }
  for (const datum of datums) {
    bounds = extendBounds(bounds, [datum.placement.origin])
  }
  return bounds
}

/** Near and far depths that cover the sphere around the scene bounds, or the target without any. */
function sceneDepthRange(
  camera: Camera,
  sceneBounds: Bounds | null,
): { near: number; far: number } {
  if (sceneBounds === null) {
    return depthRange(camera, camera.target, Math.max(visibleHeightAtTarget(camera), 1))
  }
  return depthRange(camera, boundsCenter(sceneBounds), Math.max(boundsDiagonal(sceneBounds) / 2, 1))
}

/** The four sides of a square as line segments, two points each. */
function squareOutline(corners: readonly Vector3[]): number[] {
  const segments: number[] = []
  for (let i = 0; i < 4; i++) {
    segments.push(...(corners[i] ?? [0, 0, 0]), ...(corners[(i + 1) % 4] ?? [0, 0, 0]))
  }
  return segments
}

export function createScene(backend: RenderBackend): Scene {
  /** CPU copies of everything uploaded, for restore(). */
  const uploaded = new Map<string, Geometry>()
  /** The element states last sent per draw. */
  const sentStates = new Map<string, Uint8Array>()
  /** The scalar field last sent per draw. */
  const sentScalars = new Map<string, Float32Array | null>()
  /** Signatures of generated geometry, so it is uploaded only when it changes. */
  const signatures = new Map<string, string>()
  const models = new Map<string, MeshEntry>()
  const previews = new Map<string, MeshEntry>()
  let overhang: Readonly<Record<string, Float32Array>> = {}
  let highlights: ElementHighlights = {}
  let fieldHover: ElementHighlights = {}
  let hoverEdges: HoverEdges | null = null
  const sectionOffsets = new Map<string, number>()
  let ranges: readonly PickRange[] = []
  let lastState: AppState | null = null
  /** The side and center of the planes the last frame drew (3.12), whose squares framing shows. */
  let lastPlanes: { readonly side: number; readonly center: Vector3 } | null = null
  // Solutions are replaced, never changed, so their bounds and identities can be kept by object.
  const sketchBoxes = new WeakMap<SketchSolution, Bounds | null>()
  const solutionNumbers = new WeakMap<SketchSolution, number>()
  let nextSolutionNumber = 1

  function cachedSketchBounds(solution: SketchSolution): Bounds | null {
    let box = sketchBoxes.get(solution)
    if (box === undefined) {
      box = sketchBounds(solution)
      sketchBoxes.set(solution, box)
    }
    return box
  }

  /** A number for each solution object, so generated geometry is signed without serializing it. */
  function solutionNumber(solution: SketchSolution): number {
    let number = solutionNumbers.get(solution)
    if (number === undefined) {
      number = nextSolutionNumber
      nextSolutionNumber += 1
      solutionNumbers.set(solution, number)
    }
    return number
  }

  function visibleBounds(state: AppState): BoundingBox | null {
    const bounds = modelBounds(
      state,
      visibleSketches(state),
      visibleDatums(state),
      cachedSketchBounds,
    )
    return bounds === null ? null : normalizedBox(bounds)
  }

  function upload(id: string, geometry: Geometry): void {
    backend.upload(id, geometry)
    uploaded.set(id, geometry)
    sentStates.delete(id)
    sentScalars.delete(id)
  }

  function remove(id: string): void {
    if (uploaded.delete(id)) {
      backend.remove(id)
    }
    sentStates.delete(id)
    sentScalars.delete(id)
    signatures.delete(id)
  }

  /** Uploads generated geometry under an id when its signature changed. */
  function uploadGenerated(id: string, signature: string, make: () => Geometry): void {
    if (signatures.get(id) === signature && uploaded.has(id)) {
      return
    }
    upload(id, make())
    signatures.set(id, signature)
  }

  function meshIds(layer: 'model' | 'preview', entry: MeshEntry) {
    const base = `${layer}:${String(entry.documentHandle)}:${entry.body}`
    return { faces: `${base}:faces`, edges: `${base}:edges`, vertices: `${base}:vertices` }
  }

  function uploadMesh(layer: 'model' | 'preview', entry: MeshEntry): void {
    const ids = meshIds(layer, entry)
    const mesh = entry.data
    upload(ids.faces, {
      kind: 'triangles',
      positions: mesh.positions,
      normals: mesh.normals,
      indices: mesh.indices,
      elementIds: mesh.faceIds,
      elementCount: mesh.faceCount,
    })
    upload(ids.edges, {
      kind: 'lines',
      positions: mesh.edgePositions,
      elementIds: mesh.edgeIds,
      elementCount: mesh.edgeCount,
    })
    if (layer === 'model') {
      upload(ids.vertices, {
        kind: 'points',
        positions: mesh.vertexPositions,
        elementIds: mesh.vertexIds,
        elementCount: mesh.vertexCount,
      })
    }
  }

  function removeMesh(layer: 'model' | 'preview', entry: MeshEntry): void {
    const ids = meshIds(layer, entry)
    remove(ids.faces)
    remove(ids.edges)
    remove(ids.vertices)
  }

  function applyMesh(event: Extract<CoreEvent, { type: 'body_mesh' }>): void {
    const entries = event.layer === 'model' ? models : previews
    const key = bodyMeshKey(event.documentHandle, event.body)
    const previous = entries.get(key)
    if (previous !== undefined) {
      removeMesh(event.layer, previous)
      entries.delete(key)
    }
    if (event.mesh === null) {
      return
    }
    const entry: MeshEntry = {
      documentHandle: event.documentHandle,
      body: event.body,
      meshId: event.meshId,
      data: event.mesh,
    }
    entries.set(key, entry)
    uploadMesh(event.layer, entry)
  }

  /** Forgets the meshes of documents older than the shown one. */
  function pruneDocuments(documentHandle: DocHandle | null): void {
    for (const [entries, layer] of [
      [models, 'model'],
      [previews, 'preview'],
    ] as const) {
      for (const [key, entry] of Array.from(entries)) {
        if (documentHandle === null || entry.documentHandle < documentHandle) {
          removeMesh(layer, entry)
          entries.delete(key)
        }
      }
    }
  }

  function modelOf(state: AppState, body: string): MeshEntry | undefined {
    const document = state.document
    return document === null ? undefined : models.get(bodyMeshKey(document.documentHandle, body))
  }

  function previewOf(state: AppState, body: string): MeshEntry | undefined {
    const document = state.document
    return document === null ? undefined : previews.get(bodyMeshKey(document.documentHandle, body))
  }

  function frame(
    state: AppState,
    camera: Camera,
    view: ViewWindow & { readonly pixelRatio: number },
  ): RenderFrame {
    // Pixel-sized quantities measure the free area, which the camera frames (2.11).
    const freeHeight = framedArea(view).height
    lastState = state
    const document = state.document
    pruneDocuments(document?.documentHandle ?? null)
    const theme = resolvedTheme(state.settings)
    const scale = state.settings.uiScale
    const picking = pickingFor(state)
    const hoverKey = state.hover?.key ?? null
    const selection = shownSelection(state)
    const highlight: Highlight = {
      hover: hoveredKeys(state),
      selected: new Set(selection.map((item) => item.key)),
    }
    const sketches = visibleSketches(state)
    const datums = visibleDatums(state)
    const pending: PendingDraw[] = []
    const bounds = modelBounds(state, sketches, datums, cachedSketchBounds)
    let sceneBounds = bounds

    // Bodies (3.12 passes 2 to 5): every body's faces, then the grid, then every body's edges and
    // vertices.
    const bodyLines: PendingDraw[] = []
    if (document !== null) {
      const previewShown = state.command !== null && state.command.preview !== 'none'
      const selectedBodies = new Set(
        selection.flatMap((item) =>
          item.kind === 'body' && item.body !== null ? [item.body] : [],
        ),
      )
      const fields = overhangFields(state, overhang)
      for (const body of document.recipe.bodies ?? []) {
        const model = modelOf(state, body.id)
        const preview = previewShown ? previewOf(state, body.id) : undefined
        // A body the command creates has a preview but no model mesh yet.
        if (!bodyVisible(body) || (model === undefined && preview === undefined)) {
          continue
        }
        if (model !== undefined) {
          sceneBounds = extendBounds(sceneBounds, boundsCorners(model.data.boundingBox))
        }
        const draws = bodyDraws(state, {
          body,
          model,
          preview,
          theme,
          scale,
          picking,
          hoverKey,
          hoverKeys: highlight.hover,
          selectedBody: selectedBodies.has(body.id),
          field: fields[body.id] ?? null,
        })
        pending.push(...draws.surfaces)
        bodyLines.push(...draws.lines)
        if (preview !== undefined) {
          sceneBounds = extendBounds(sceneBounds, boundsCorners(preview.data.boundingBox))
        }
      }
    }

    // The ground or sketch grid (3.12 pass 4), never picked: after the faces, which hide it, and
    // before the edges, so an edge lying on the grid (a part's bottom edges on the ground) is drawn
    // over the grid line it coincides with instead of under it.
    const gridDraws = drawGrid(state, bounds, camera, freeHeight, theme, scale)
    for (const corner of gridDraws.corners) {
      sceneBounds = extendBounds(sceneBounds, [corner])
    }
    pending.push(...gridDraws.draws, ...bodyLines)

    // Datum and origin planes, origin axes (3.12 pass 6).
    const planes = planeExtent(bounds)
    lastPlanes = planes
    const drawnPlanes = planeDraws(state, datums, planes, theme, scale, picking, highlight)
    for (const corners of drawnPlanes.corners) {
      sceneBounds = extendBounds(sceneBounds, corners)
    }
    pending.push(...drawnPlanes.draws)
    pending.push(...originDraws(state, theme, scale, planes, picking, highlight))

    // Sketches (3.12 pass 7, 3.16.2).
    for (const sketch of sketches) {
      const box = cachedSketchBounds(sketch.solution)
      if (box !== null) {
        sceneBounds = extendBounds(sceneBounds, boundsCorners(box))
      }
      pending.push(
        ...sketchDraws(state, sketch, {
          theme,
          scale,
          camera,
          view,
          picking,
          highlight,
        }),
      )
    }

    const candidateRegions = repairRegionsDraw(state, theme)
    if (candidateRegions === null) {
      remove(REPAIR_REGIONS_ID)
    } else {
      pending.push(candidateRegions)
    }

    // The hole centers a command placed on a face, drawn from the command before any preview (3.7).
    const helper = state.command?.helper ?? null
    const helperPoints = helperDraw(helper, theme, scale, picking, highlight)
    if (helperPoints === null) {
      remove('helper:points')
    } else {
      pending.push(helperPoints)
    }

    // Print bed (3.12 pass 8).
    const bed = bedDraw(state, theme, scale)
    if (bed !== null) {
      sceneBounds = extendBounds(sceneBounds, bed.points)
      pending.push(bed.draw)
    }

    const draws = assignPickIds(pending)
    const { near, far } = sceneDepthRange(camera, sceneBounds)
    return {
      view: viewMatrix(camera),
      projection: windowProjection(camera, view, near, far),
      eye: eye(camera),
      orthographic: camera.projection === 'orthographic',
      pixelRatio: view.pixelRatio,
      uiScale: scale,
      theme: viewportTheme(theme),
      draws,
      section: sectionClipFor(state),
      analysis: overhangAnalysis(state, theme),
    }
  }

  /**
   * Gives each pickable draw its pick ids, consecutive from 1 with a range as long as its element
   * count, and sends the element states and scalar fields that changed.
   */
  function assignPickIds(pending: readonly PendingDraw[]): DrawCall[] {
    const nextRanges: PickRange[] = []
    let next = 1
    const draws: DrawCall[] = []
    for (const draw of pending) {
      let pickBase: number | null = null
      const target = draw.target
      if (target !== null && draw.elementCount > 0) {
        pickBase = next
        nextRanges.push({ first: next, count: draw.elementCount, target })
        next += draw.elementCount
      }
      if (draw.states !== null && !sameBytes(sentStates.get(draw.call.id), draw.states)) {
        backend.setElementStates(draw.call.id, draw.states)
        sentStates.set(draw.call.id, draw.states)
      }
      if (draw.scalars !== undefined && sentScalars.get(draw.call.id) !== draw.scalars) {
        if (draw.scalars !== null || sentScalars.has(draw.call.id)) {
          backend.setScalarField(draw.call.id, draw.scalars)
        }
        sentScalars.set(draw.call.id, draw.scalars)
      }
      draws.push({ ...draw.call, pickBase })
    }
    ranges = nextRanges
    return draws
  }

  /** The section of the frame, at the evaluated offset of its expression or a plain length, or null. */
  function sectionClipFor(state: AppState): RenderFrame['section'] {
    const section = state.view.section
    if (section === null) {
      return null
    }
    const units = state.document?.recipe.units ?? 'mm'
    const offset =
      sectionOffsets.get(section.offset) ?? sectionOffsetMillimeters(section.offset, units) ?? 0
    return sectionFrameClip(section, offset)
  }

  /**
   * The fill and outline draws of the visible datum planes and the drawn origin planes (3.12 pass
   * 6), with each square's corners.
   */
  function planeDraws(
    state: AppState,
    datums: readonly VisibleDatum[],
    planes: { side: number; center: Vector3 },
    theme: ThemeName,
    scale: number,
    picking: Picking,
    highlight: Highlight,
  ): { readonly draws: PendingDraw[]; readonly corners: readonly (readonly Vector3[])[] } {
    const document = state.document
    const datumColor = colorOf(theme, 'datum')
    // Datum planes in `datum`; origin planes in their normal's axis role (3.12).
    const planeKinds: {
      readonly key: string
      readonly item: SelectionItem
      readonly placement: Placement
      readonly color: Rgb
    }[] = [
      ...datums.map((datum) => ({
        key: `feature:${datum.id}`,
        item: plainItem(`feature:${datum.id}`, 'datum_plane', datum.id, { feature: datum.id }),
        placement: datum.placement,
        color: datumColor,
      })),
      ...drawnOriginPlanes(state).map((plane) => ({
        key: `origin:${plane}`,
        item: originItem(plane),
        placement: ORIGIN_PLANE_PLACEMENTS[plane],
        color: colorOf(theme, AXIS_ROLES[ORIGIN_PLANE_NORMAL_AXES[plane]]),
      })),
    ]
    const draws: PendingDraw[] = []
    const allCorners: (readonly Vector3[])[] = []
    for (const plane of planeKinds) {
      const corners = squareCorners(plane.placement, planes.side, planes.center)
      allCorners.push(corners)
      const signature = JSON.stringify(corners)
      const fillId = `plane:${plane.key}:fill`
      const outlineId = `plane:${plane.key}:outline`
      uploadGenerated(fillId, signature, () => ({
        kind: 'fill',
        positions: new Float32Array(corners.flat()),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
        elementIds: new Uint32Array(4),
        elementCount: 1,
      }))
      uploadGenerated(outlineId, signature, () => linesOf(new Float32Array(squareOutline(corners))))
      const label =
        plane.item.kind === 'datum_plane' && document !== null
          ? featureLabel(document.recipe, plane.item.feature ?? '', state.catalog)
          : plane.item.label
      const item = { ...plane.item, label }
      const states = itemStates([plane.item], highlight)
      draws.push({
        call: {
          ...overlayCall(plane.color, 'translucent', PLANE_FILL_OPACITY, 0, false, false),
          id: fillId,
          depthTest: true,
        },
        elementCount: 1,
        target: picking.accepts(item.kind) ? itemTarget([item], 'area') : null,
        states,
      })
      draws.push({
        call: {
          ...overlayCall(
            plane.color,
            'translucent',
            PLANE_OUTLINE_OPACITY,
            PLANE_OUTLINE_WIDTH * scale,
            false,
            false,
          ),
          id: outlineId,
          depthTest: true,
        },
        elementCount: 1,
        target: null,
        states,
      })
    }
    return { draws, corners: allCorners }
  }

  /** The print bed outline's draw and its points (3.12 pass 8), or null without a bed. */
  function bedDraw(
    state: AppState,
    theme: ThemeName,
    scale: number,
  ): { readonly draw: PendingDraw; readonly points: readonly Vector3[] } | null {
    const bed = bedOutline(state)
    if (bed === null) {
      remove('bed')
      return null
    }
    const points: Vector3[] = []
    for (let i = 0; i + 2 < bed.length; i += 3) {
      points.push([bed[i] ?? 0, bed[i + 1] ?? 0, bed[i + 2] ?? 0])
    }
    if (uploaded.get('bed')?.positions !== bed) {
      upload('bed', linesOf(bed))
    }
    return {
      draw: {
        call: {
          ...overlayCall(colorOf(theme, 'bed'), 'opaque', 1, BED_WIDTH * scale, false, false),
          id: 'bed',
          depthTest: true,
        },
        elementCount: 1,
        target: null,
        states: null,
      },
      points,
    }
  }

  /** The grid's two draws, minor and major lines, and its square's corners; nothing when no grid is drawn. */
  function drawGrid(
    state: AppState,
    bounds: Bounds | null,
    camera: Camera,
    height: number,
    theme: ThemeName,
    scale: number,
  ): { readonly draws: PendingDraw[]; readonly corners: readonly Vector3[] } {
    const placement = gridPlacement(state)
    const spacing = drawnGridSpacing(state, camera, height)
    if (placement === null || spacing === null) {
      remove('grid:minor')
      remove('grid:major')
      return { draws: [], corners: [] }
    }
    const half = gridHalfSide(bounds, spacing)
    const signature = JSON.stringify([placement, spacing, half])
    // Built at most once, and only when a draw's signature changed.
    let segments: GridSegments | null = null
    const lines = () => (segments ??= gridSegments(placement, spacing, half))
    uploadGenerated('grid:minor', signature, () => linesOf(lines().minor))
    uploadGenerated('grid:major', signature, () => linesOf(lines().major))
    const color = colorOf(theme, 'grid')
    const draws: PendingDraw[] = (
      [
        ['grid:minor', GRID_MINOR_OPACITY],
        ['grid:major', GRID_MAJOR_OPACITY],
      ] as const
    ).map(([id, opacity]) => ({
      call: {
        ...overlayCall(color, 'opaque', opacity, GRID_LINE_WIDTH * scale, false, false),
        id,
        depthTest: true,
      },
      elementCount: 1,
      target: null,
      states: null,
    }))
    return { draws, corners: gridCorners(placement, half) }
  }

  type BodyInput = {
    readonly body: Body
    /** Undefined for a body that exists only in the preview layer. */
    readonly model: MeshEntry | undefined
    readonly preview: MeshEntry | undefined
    readonly theme: ThemeName
    readonly scale: number
    readonly picking: Picking
    readonly hoverKey: string | null
    /** Every key drawn hovered (`hoveredKeys`). */
    readonly hoverKeys: ReadonlySet<string>
    readonly selectedBody: boolean
    readonly field: Float32Array | null
  }

  function bodyDraws(state: AppState, input: BodyInput): BodyDraws {
    const { body, model, preview, theme, scale } = input
    const base = linearFromHex(body.color ?? VIEWPORT_COLORS[theme].body)
    const baseHex = body.color ?? VIEWPORT_COLORS[theme].body
    const edgeRole: ViewportRole =
      contrastRatio(VIEWPORT_COLORS[theme].edgeDark, baseHex) >=
      contrastRatio(VIEWPORT_COLORS[theme].edgeLight, baseHex)
        ? 'edgeDark'
        : 'edgeLight'
    const edgeColor = colorOf(theme, edgeRole)
    const part = (body.role ?? 'part') === 'part'
    const layer: DrawCall['layer'] = part ? 'opaque' : 'translucent'
    const opacity = part ? 1 : NON_PART_OPACITY
    const shaded = (id: string, color: Rgb): Omit<DrawCall, 'pickBase'> => ({
      id,
      layer,
      color,
      elementColors: null,
      opacity,
      size: 0,
      dashed: false,
      shading: 'matcap',
      depthTest: true,
      clipped: true,
      pickSize: 0,
      analysis: input.field !== null,
    })
    const lines = (id: string, width: number): Omit<DrawCall, 'pickBase'> => ({
      id,
      layer: 'opaque',
      color: edgeColor,
      elementColors: null,
      opacity: 1,
      size: width,
      dashed: false,
      shading: 'flat',
      depthTest: true,
      clipped: true,
      pickSize: EDGE_PICK_WIDTH * scale,
      analysis: false,
    })
    const draws: BodyDraws =
      model === undefined
        ? { surfaces: [], lines: [] }
        : modelDraws(state, input, model, {
            base,
            shaded,
            lines,
            previewing: preview !== undefined,
          })
    if (preview !== undefined) {
      const previewIds = meshIds('preview', preview)
      draws.surfaces.push({
        call: shaded(previewIds.faces, base),
        elementCount: preview.data.faceCount,
        target: null,
        states: null,
        scalars: null,
      })
      draws.lines.push({
        call: lines(previewIds.edges, EDGE_WIDTH * scale),
        elementCount: preview.data.edgeCount,
        target: null,
        states: null,
      })
    }
    return draws
  }

  type BodyStyle = {
    readonly base: Rgb
    readonly shaded: (id: string, color: Rgb) => Omit<DrawCall, 'pickBase'>
    readonly lines: (id: string, width: number) => Omit<DrawCall, 'pickBase'>
    /** During a preview the model stays pickable but only shows its highlighted edges and vertices. */
    readonly previewing: boolean
  }

  /** The model-layer faces, edges and vertices of a body, with their pick targets and states. */
  function modelDraws(
    state: AppState,
    input: BodyInput,
    model: MeshEntry,
    style: BodyStyle,
  ): BodyDraws {
    const { body, scale, picking, hoverKey, hoverKeys } = input
    const { base, shaded, lines, previewing } = style
    const locked = state.view.locked.includes(body.id)
    const mesh = model.data
    const ids = meshIds('model', model)
    const bodyKey = `body:${body.id}`
    const bodyHovered = hoverKeys.has(bodyKey)
    const meshKey = (element: ElementKind, index: number): string =>
      meshElementKey(model.meshId, element, index)
    // A hovered face that stands for its edges lights them up instead of itself (3.7).
    const redirect =
      hoverEdges !== null &&
      hoverEdges.body === body.id &&
      hoverEdges.meshId === model.meshId &&
      hoverEdges.faceKey === hoverKey
        ? new Set(hoverEdges.edges)
        : null
    const hoveredByField = {
      face: elementIndexSet(fieldHover[body.id], model.meshId, 'face'),
      edge: elementIndexSet(fieldHover[body.id], model.meshId, 'edge'),
      vertex: elementIndexSet(fieldHover[body.id], model.meshId, 'vertex'),
    }
    function hovered(element: 'face' | 'edge' | 'vertex', index: number): boolean {
      if (redirect !== null && element === 'face') {
        return false
      }
      if (redirect !== null && element === 'edge') {
        return redirect.has(index)
      }
      return (
        hoverKey === meshKey(element, index) ||
        hoveredByField[element].has(index) ||
        (element === 'face' && bodyHovered)
      )
    }
    function elementStates(element: 'face' | 'edge' | 'vertex', count: number): Uint8Array {
      const selected = elementIndexSet(highlights[body.id], model.meshId, element)
      const states = new Uint8Array(count)
      for (let i = 0; i < count; i++) {
        let value = 0
        if (hovered(element, i)) {
          value |= ELEMENT_STATE.hover
        }
        if (selected.has(i) || (element === 'face' && input.selectedBody)) {
          value |= ELEMENT_STATE.selected
        }
        states[i] = value
      }
      return states
    }
    function meshTarget(element: 'face' | 'edge' | 'vertex', pickClass: PickClass) {
      return (index: number) => {
        if (element === 'face' && picking.facesPickBodies) {
          return {
            result: {
              kind: 'item' as const,
              item: plainItem(bodyKey, 'body', body.name, { body: body.id }),
              world: null,
            },
            pickClass,
          }
        }
        return {
          result: {
            kind: 'mesh' as const,
            body: body.id,
            meshId: model.meshId,
            element,
            index,
            key: meshKey(element, index),
            world: null,
          },
          pickClass,
        }
      }
    }
    const faceKind: SelectionItemKind = picking.facesPickBodies ? 'body' : 'face'
    const pickable = (kind: SelectionItemKind): boolean => !locked && picking.accepts(kind)
    return {
      surfaces: [
        {
          call: previewing
            ? { ...shaded(ids.faces, base), layer: 'translucent', opacity: 0, analysis: false }
            : shaded(ids.faces, base),
          elementCount: mesh.faceCount,
          target: pickable(faceKind) ? meshTarget('face', 'area') : null,
          states: elementStates('face', mesh.faceCount),
          scalars: previewing ? null : input.field,
        },
      ],
      lines: [
        {
          call: lines(ids.edges, previewing ? 0 : EDGE_WIDTH * scale),
          elementCount: mesh.edgeCount,
          target: pickable('edge') ? meshTarget('edge', 'curve') : null,
          states: elementStates('edge', mesh.edgeCount),
        },
        {
          call: {
            ...lines(ids.vertices, 0),
            pickSize: VERTEX_PICK_SIZE * scale,
          },
          elementCount: mesh.vertexCount,
          target: pickable('vertex') ? meshTarget('vertex', 'point') : null,
          states: elementStates('vertex', mesh.vertexCount),
        },
      ],
    }
  }

  function originDraws(
    state: AppState,
    theme: ThemeName,
    scale: number,
    planes: { side: number; center: Vector3 },
    picking: Picking,
    highlight: Highlight,
  ): PendingDraw[] {
    const editingSketch = state.context === 'sketch' && state.sketch?.sketchId !== null
    if (editingSketch) {
      remove('origin:axes')
      remove('origin:point')
      return []
    }
    const axisFieldActive = activeFieldKind(state) === 'axis'
    const axes = ORIGIN_AXES.filter(
      (axis) => axisFieldActive || isSessionVisible(state, `origin:${axis.name}`),
    )
    // The origin point in `sketchReference`, each axis in its own role (3.12).
    const color = colorOf(theme, 'sketchReference')
    const draws: PendingDraw[] = []
    if (axes.length === 0) {
      remove('origin:axes')
    } else {
      const half = planes.side / 2
      const axisItems = axes.map((axis) => originItem(axis.name))
      const signature = JSON.stringify([axes.map((axis) => axis.name), half])
      uploadGenerated('origin:axes', signature, () => ({
        kind: 'lines',
        positions: new Float32Array(
          axes.flatMap((axis) => [
            ...axis.direction.map((component) => component * -half),
            ...axis.direction.map((component) => component * half),
          ]),
        ),
        elementIds: sequentialIds(axes.length),
        elementCount: axes.length,
      }))
      draws.push({
        call: {
          ...overlayCall(color, 'translucent', 1, REFERENCE_CURVE_WIDTH * scale, true, true),
          elementColors: axisColors(
            theme,
            axes.map((axis) => axis.name),
          ),
          id: 'origin:axes',
          pickSize: SKETCH_CURVE_PICK_WIDTH * scale,
        },
        elementCount: axes.length,
        target: picking.accepts('origin_axis') ? itemTarget(axisItems, 'curve') : null,
        states: itemStates(axisItems, highlight),
      })
    }
    if (!isSessionVisible(state, 'origin:O')) {
      remove('origin:point')
    } else {
      const origin = originItem('O')
      uploadGenerated('origin:point', 'origin', () => ({
        kind: 'points',
        positions: new Float32Array([0, 0, 0]),
        elementIds: new Uint32Array([0]),
        elementCount: 1,
      }))
      draws.push({
        call: {
          ...overlayCall(color, 'translucent', 1, ORIGIN_POINT_SIZE * scale, false, true),
          id: 'origin:point',
          pickSize: SKETCH_POINT_PICK_SIZE * scale,
        },
        elementCount: 1,
        target: picking.accepts('origin_point') ? itemTarget([origin], 'point') : null,
        states: itemStates([origin], highlight),
      })
    }
    return draws
  }

  type SketchInput = {
    readonly theme: ThemeName
    readonly scale: number
    readonly camera: Camera
    readonly view: ViewWindow
    readonly picking: Picking
    readonly highlight: Highlight
  }

  /** The active sketch's origin point and axes (3.16.2), or nothing for another sketch. */
  function sketchOriginDraws(sketch: VisibleSketch, input: SketchInput): PendingDraw[] {
    const { theme, scale, picking, highlight } = input
    const { solution } = sketch
    const prefix = `sketch:${sketch.id}`
    const reference = colorOf(theme, 'sketchReference')
    const axesId = `${prefix}:axes`
    const originId = `${prefix}:origin`
    if (!sketch.active) {
      remove(axesId)
      remove(originId)
      return []
    }
    const {
      center: [centerX, centerY],
      reach,
      quantum,
    } = sketchAxisSpans(solution.placement, input.camera, input.view)
    const axisSignature = JSON.stringify([
      solution.placement,
      centerX,
      centerY,
      Math.round(reach / quantum),
    ])
    uploadGenerated(axesId, axisSignature, () => {
      const { positions } = segmentsOnPlane(solution.placement, [
        { element: 0, points: new Float32Array([centerX - reach, 0, centerX + reach, 0]) },
        { element: 1, points: new Float32Array([0, centerY - reach, 0, centerY + reach]) },
      ])
      return { kind: 'lines', positions, elementIds: sequentialIds(2), elementCount: 2 }
    })
    uploadGenerated(originId, JSON.stringify(solution.placement), () => ({
      kind: 'points',
      positions: new Float32Array(solution.placement.origin),
      elementIds: new Uint32Array([0]),
      elementCount: 1,
    }))
    const axisItems = [sketchOriginItem(sketch.id, 'x_axis'), sketchOriginItem(sketch.id, 'y_axis')]
    const sketchOrigin = sketchOriginItem(sketch.id, 'point')
    return [
      {
        call: {
          ...overlayCall(reference, 'overlay', 1, REFERENCE_CURVE_WIDTH * scale, true, true),
          // On an origin plane the axes are world axes, in their roles (3.16.2).
          elementColors: sketch.worldAxes === null ? null : axisColors(theme, sketch.worldAxes),
          id: axesId,
          pickSize: SKETCH_CURVE_PICK_WIDTH * scale,
        },
        elementCount: 2,
        target: picking.accepts('sketch_curve') ? itemTarget(axisItems, 'curve') : null,
        states: itemStates(axisItems, highlight),
      },
      {
        call: {
          ...overlayCall(reference, 'overlay', 1, ORIGIN_POINT_SIZE * scale, false, true),
          id: originId,
          pickSize: SKETCH_POINT_PICK_SIZE * scale,
        },
        elementCount: 1,
        target: picking.accepts('sketch_point') ? itemTarget([sketchOrigin], 'point') : null,
        states: itemStates([sketchOrigin], highlight),
      },
    ]
  }

  /** Uploads a solution's region fills on its placement under `id` when they or the placement changed. */
  function uploadRegionFills(
    id: string,
    display: SketchDisplayData,
    solution: SketchSolution,
  ): void {
    const placement = solution.placement
    const signature = JSON.stringify(placement)
    const current = uploaded.get(id)
    if (
      current !== undefined &&
      signatures.get(id) === signature &&
      current.elementIds === display.regionIds
    ) {
      return
    }
    const yAxis = yAxisOf(placement)
    const positions = new Float32Array((display.regionPositions.length / 2) * 3)
    for (let i = 0; 2 * i + 1 < display.regionPositions.length; i++) {
      const point = onPlane(placement, yAxis, [
        display.regionPositions[2 * i] ?? 0,
        display.regionPositions[2 * i + 1] ?? 0,
      ])
      positions.set(point, 3 * i)
    }
    upload(id, {
      kind: 'fill',
      positions,
      indices: display.regionIndices,
      elementIds: display.regionIds,
      elementCount: solution.regions.length,
    })
    signatures.set(id, signature)
  }

  /**
   * Repair mode's region candidates (3.7): their regions drawn in the hover state, which fills a
   * region in highlight at 20 percent (3.12 pass 7), and every other region not at all, whether or
   * not the sketch is shown. Null while repair mode shows no region repair.
   */
  function repairRegionsDraw(state: AppState, theme: ThemeName): PendingDraw | null {
    const places = selectRegionCandidates(state)
    const sketch = places === null ? undefined : state.document?.sketches[places.sketch]
    const display = sketch?.display ?? null
    if (places === null || sketch === undefined || display === null) {
      return null
    }
    const count = sketch.solution.regions.length
    uploadRegionFills(REPAIR_REGIONS_ID, display, sketch.solution)
    const states = new Uint8Array(count)
    for (const index of places.regions.flat()) {
      states[index] = ELEMENT_STATE.hover
    }
    return {
      call: {
        ...overlayCall(colorOf(theme, 'highlight'), 'overlay', 0, 0, false, false),
        id: REPAIR_REGIONS_ID,
      },
      elementCount: count,
      target: null,
      states,
    }
  }

  function sketchDraws(state: AppState, sketch: VisibleSketch, input: SketchInput): PendingDraw[] {
    const { theme, scale, picking, highlight } = input
    const { solution } = sketch
    const tolerance = sketchTolerance(input.camera, framedArea(input.view).height)
    const prefix = `sketch:${sketch.id}`
    const reference = colorOf(theme, 'sketchReference')
    // In sketch mode only the active sketch is pickable; elsewhere visible sketches are (3.6).
    const inSketchContext = state.context === 'sketch'
    const pickableSketch = sketch.active || !inSketchContext
    const curves = solution.entities.filter((entity) => entity.type !== 'point')
    // Areas first, then the sketch's origin and axes, then its curves and points: in the ID pass a
    // later draw covers an earlier one at equal depth, so curves stay pickable where they lie on an
    // axis or bound a region (3.6).
    const draws: PendingDraw[] = []
    // Region fills from the solution's display buffers.
    const regionsId = `${prefix}:regions`
    const display = state.document?.sketches[sketch.id]?.display ?? null
    if (display === null || solution.regions.length === 0) {
      remove(regionsId)
    } else {
      uploadRegionFills(regionsId, display, solution)
      const items = solution.regions.map((region, index) =>
        regionSelectionItem(sketch.id, region, `Region ${String(index + 1)}`),
      )
      // The edited sketch fills its regions; in the Model context they fill only while the active
      // selection field takes regions, so the shapes that can be clicked show (3.12 pass 7).
      const filled = sketch.active || (state.command !== null && picking.accepts('sketch_region'))
      draws.push({
        call: {
          ...overlayCall(
            colorOf(theme, 'sketchRegion'),
            'overlay',
            filled ? REGION_OPACITY : 0,
            0,
            false,
            false,
          ),
          id: regionsId,
        },
        elementCount: solution.regions.length,
        target:
          pickableSketch && picking.accepts('sketch_region') ? itemTarget(items, 'area') : null,
        states: itemStates(items, highlight),
      })
    }

    draws.push(...sketchOriginDraws(sketch, input))
    const problemEntities = new Set(
      solution.profile_findings.flatMap((finding) =>
        'entities' in finding ? finding.entities : 'entity' in finding ? [finding.entity] : [],
      ),
    )
    // Curves named by profile findings get a halo under them, so they keep their DOF color.
    const haloId = `${prefix}:halo`
    const haloCurves = sketch.active
      ? curves.filter((entity) => problemEntities.has(entity.id))
      : []
    if (haloCurves.length === 0) {
      remove(haloId)
    } else {
      const signature = `${String(solutionNumber(solution))}:${String(tolerance)}:${haloCurves.map((entity) => entity.id).join(',')}`
      uploadGenerated(haloId, signature, () =>
        curveLines(solution.placement, haloCurves, tolerance),
      )
      draws.push({
        call: {
          ...overlayCall(
            problemHaloColor(theme),
            'overlay',
            1,
            (SKETCH_CURVE_WIDTH + 2 * PROBLEM_HALO_PIXELS) * scale,
            false,
            false,
          ),
          id: haloId,
        },
        elementCount: haloCurves.length,
        target: null,
        states: null,
      })
    }
    const entityColor = (entity: SolvedEntity): Rgb => {
      if (!sketch.active) {
        return reference
      }
      switch (entity.constrained) {
        case 'under_constrained':
          return colorOf(theme, 'sketchUnder')
        case 'conflicting':
          return colorOf(theme, 'sketchOver')
        case 'fully_constrained':
          return colorOf(theme, entity.construction === true ? 'sketchConstruction' : 'sketchFull')
      }
    }

    // Curves: solid and construction (dashed) draws.
    for (const construction of [false, true]) {
      const id = `${prefix}:${construction ? 'construction' : 'curves'}`
      const chosen = curves.filter(
        (entity) => (entity.construction === true && sketch.active) === construction,
      )
      if (chosen.length === 0) {
        remove(id)
        continue
      }
      // The curves follow from the solution, the tolerance and whether the sketch is active.
      const signature = `${String(solutionNumber(solution))}:${String(tolerance)}:${String(sketch.active)}`
      uploadGenerated(id, signature, () => curveLines(solution.placement, chosen, tolerance))
      const elementColors = new Float32Array(chosen.flatMap((entity) => [...entityColor(entity)]))
      const items = chosen.map((entity) => entityItem(sketch.id, entity))
      const states = itemStates(items, highlight)
      draws.push({
        call: {
          ...overlayCall(
            reference,
            'overlay',
            1,
            (sketch.active ? SKETCH_CURVE_WIDTH : REFERENCE_CURVE_WIDTH) * scale,
            construction,
            pickableSketch,
          ),
          id,
          elementColors: sketch.active ? elementColors : null,
          pickSize: SKETCH_CURVE_PICK_WIDTH * scale,
        },
        elementCount: chosen.length,
        target:
          pickableSketch && picking.accepts('sketch_curve') ? itemTarget(items, 'curve') : null,
        states,
      })
    }

    // Points: solved points of the active sketch, point entities of others.
    const pointsId = `${prefix}:points`
    const solvedPoints = sketch.active
      ? solution.points
      : solution.entities.flatMap((entity) =>
          entity.type === 'point' ? [{ point: entity.id, at: entity.position }] : [],
        )
    if (solvedPoints.length === 0) {
      remove(pointsId)
    } else {
      const signature = `${String(solutionNumber(solution))}:${String(sketch.active)}`
      uploadGenerated(pointsId, signature, () =>
        pointsOnPlane(
          solution.placement,
          solvedPoints.map((point) => point.at),
        ),
      )
      const owners = new Map(solution.entities.map((entity) => [entity.id, entity]))
      const items = solvedPoints.map((point) => pointItem(sketch.id, point.point))
      const colors = solvedPoints.flatMap((point) => {
        const ownerId = point.point.split('.')[0] ?? point.point
        const owner = owners.get(ownerId)
        return [...(owner === undefined ? reference : entityColor(owner))]
      })
      const pointEntities = solvedPoints.some((point) => owners.get(point.point)?.type === 'point')
      draws.push({
        call: {
          ...overlayCall(
            reference,
            'overlay',
            1,
            (pointEntities ? SKETCH_POINT_ENTITY_SIZE : SKETCH_POINT_SIZE) * scale,
            false,
            pickableSketch,
          ),
          id: pointsId,
          elementColors: sketch.active ? new Float32Array(colors) : null,
          pickSize: SKETCH_POINT_PICK_SIZE * scale,
        },
        elementCount: solvedPoints.length,
        target:
          pickableSketch && picking.accepts('sketch_point') ? itemTarget(items, 'point') : null,
        states: itemStates(items, highlight),
      })
    }

    return draws
  }

  /** The point entities of a command's helper sketch as pickable sketch points, or null without any. */
  function helperDraw(
    helper: HelperSketch | null,
    theme: ThemeName,
    scale: number,
    picking: Picking,
    highlight: Highlight,
  ): PendingDraw | null {
    const plane = helper?.params.plane
    const entities = (helper?.params.entities ?? []).flatMap((entity) =>
      entity.type === 'point' ? [entity] : [],
    )
    if (helper === null || plane?.kind !== 'face' || entities.length === 0) {
      return null
    }
    const placement = plane.fallback
    const id = 'helper:points'
    uploadGenerated(id, JSON.stringify([placement, entities]), () =>
      pointsOnPlane(
        placement,
        entities.map((entity) => entity.position),
      ),
    )
    const items = entities.map((entity) => pointItem(helper.id, entity.id))
    return {
      call: {
        ...overlayCall(
          colorOf(theme, 'sketchUnder'),
          'overlay',
          1,
          SKETCH_POINT_ENTITY_SIZE * scale,
          false,
          true,
        ),
        id,
        pickSize: SKETCH_POINT_PICK_SIZE * scale,
      },
      elementCount: entities.length,
      target: picking.accepts('sketch_point') ? itemTarget(items, 'point') : null,
      states: itemStates(items, highlight),
    }
  }

  /** The corners of a datum plane's square as the last frame drew it, or null for another item. */
  function datumSquare(item: SelectionItem): Vector3[] | null {
    const placement =
      item.kind === 'datum_plane' && item.feature !== null
        ? (lastState?.document?.datums[item.feature] ?? null)
        : null
    return placement === null || lastPlanes === null
      ? null
      : squareCorners(placement, lastPlanes.side, lastPlanes.center)
  }

  /** World points that stand for a selection item, for its screen rectangle. */
  function itemPoints(item: SelectionItem): Vector3[] {
    const state = lastState
    if (item.kind === 'body' && item.body !== null) {
      const box =
        state?.document?.meshes[item.body]?.boundingBox ??
        Array.from(models.values()).find((entry) => entry.body === item.body)?.data.boundingBox
      return box === undefined ? [] : boundsCorners(box)
    }
    if (item.element !== null) {
      const { body } = item.element
      const model = Array.from(models.values()).find((entry) => entry.body === body)
      return elementExtentPoints(item.element, model?.data ?? null)
    }
    const reference = item.sketchItem
    const solution =
      reference === null ? undefined : state?.document?.sketches[reference.sketch]?.solution
    if (reference !== null && solution !== undefined) {
      const placement = solution.placement
      const yAxis = yAxisOf(placement)
      switch (reference.kind) {
        case 'entity': {
          const entity = solution.entities.find((candidate) => candidate.id === reference.entity)
          return entity === undefined ? [] : entityWorldPoints(placement, entity)
        }
        case 'point': {
          const point = solution.points.find((candidate) => candidate.point === reference.point)
          const entity = solution.entities.find((candidate) => candidate.id === reference.point)
          const at = point?.at ?? (entity?.type === 'point' ? entity.position : null)
          return at === null ? [] : [onPlane(placement, yAxis, at)]
        }
        case 'region':
          return [onPlane(placement, yAxis, reference.region.centroid)]
        default:
          return [placement.origin]
      }
    }
    if (item.kind === 'datum_plane' && item.feature !== null) {
      const placement = state?.document?.datums[item.feature] ?? null
      return placement === null ? [] : [placement.origin]
    }
    if (item.origin !== null) {
      return [[0, 0, 0]]
    }
    return []
  }

  return {
    applyEvent(event) {
      if (event.type === 'body_mesh') {
        applyMesh(event)
      } else if (event.type === 'print_health') {
        overhang = event.overhang
      }
    },

    targetFor(pickId) {
      for (const range of ranges) {
        if (pickId >= range.first && pickId < range.first + range.count) {
          return range.target(pickId - range.first)
        }
      }
      return null
    },

    visibleBounds,

    worldPoints(items) {
      return items.flatMap((item) => datumSquare(item) ?? itemPoints(item))
    },

    screenRectangle(items, camera, view) {
      const aspect = windowAspect(view)
      let left = Number.POSITIVE_INFINITY
      let top = Number.POSITIVE_INFINITY
      let rightEdge = Number.NEGATIVE_INFINITY
      let bottom = Number.NEGATIVE_INFINITY
      for (const item of items) {
        for (const point of itemPoints(item)) {
          const position = normalizedPosition(camera, point, aspect)
          if (position === null) {
            continue
          }
          const { x, y } = windowPosition(view, position)
          left = Math.min(left, x)
          rightEdge = Math.max(rightEdge, x)
          top = Math.min(top, y)
          bottom = Math.max(bottom, y)
        }
      }
      if (!Number.isFinite(left) || !Number.isFinite(top)) {
        return null
      }
      return { x: left, y: top, width: rightEdge - left, height: bottom - top }
    },

    frame,

    restore() {
      for (const [id, geometry] of uploaded) {
        backend.upload(id, geometry)
      }
      sentStates.clear()
      sentScalars.clear()
    },

    setElementHighlights(next) {
      highlights = next
    },

    setHoverEdges(next) {
      hoverEdges = next
    },

    setFieldHover(next) {
      fieldHover = next
    },

    setSectionOffset(expression, millimeters) {
      sectionOffsets.set(expression, millimeters)
    },
  }
}
