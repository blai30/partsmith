import type { BodyMeshData, CoreEvent } from '../core/events'
import { featureLabel, type OriginPlane } from '../core/labels'
import type {
  BoundingBox,
  Body,
  DocHandle,
  Document,
  Placement,
  SelectionKind,
  SketchSolution,
  Units,
} from '../generated/protocol'
import { buildInspectorModel, getParam } from '../inspector/model'
import type { Rectangle } from '../inspector/placement'
import { itemsFromValue, originItem } from '../inspector/selection-values'
import { resolvedTheme } from '../layout/color-scheme'
import { bedOutline } from '../print/bed'
import { overhangAnalysis, overhangFields } from '../print/overhang'
import { tessellateArc, type Vector2 } from '../sketch/geometry'
import { selectOriginVisible } from '../state/selectors'
import type { AppState, HelperSketch, SelectionItem, SketchItemReference } from '../state/store'
import {
  contrastRatio,
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
  normalizedPosition,
  projectionMatrix,
  visibleHeightAtTarget,
  viewMatrix,
  type Camera,
  type Vector3,
} from './camera'
import {
  drawnGridSpacing,
  gridCorners,
  gridHalfSide,
  gridPlacement,
  gridSegments,
  type GridSegments,
} from './grid'
import { addScaled, cross, dot, normalize, subtract } from './math'
import type { PickClass, PickResult, SelectionItemKind } from './picking'
import { sectionClip } from './section'

// The scene (ui.md 2.11, 3.12): every drawable's CPU-side data, so a lost WebGL context can be
// restored; pick id ranges and their targets; and the render frame for a camera and the store.

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
  /** Screen rectangle (CSS pixels) covering selection items, or null. */
  screenRectangle(
    items: readonly SelectionItem[],
    camera: Camera,
    size: { width: number; height: number },
  ): Rectangle | null
  frame(
    state: AppState,
    camera: Camera,
    size: { width: number; height: number; pixelRatio: number },
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
const PLANE_FILL_OPACITY = 0.15
const PLANE_OUTLINE_OPACITY = 0.6
const NON_PART_OPACITY = 0.4
/** The smallest side of a drawn datum or origin plane, in mm. */
const MINIMUM_PLANE_SIDE = 50
const PLANE_SIDE_FACTOR = 1.2
/** The length of the sketch axes, as a multiple of the larger visible extent at the target. */
const SKETCH_AXIS_REACH = 4

const ORIGIN_PLANE_PLACEMENTS: Readonly<Record<OriginPlane, Placement>> = {
  XY: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  XZ: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  YZ: { origin: [0, 0, 0], x_axis: [0, 1, 0], normal: [1, 0, 0] },
}

const ORIGIN_PLANES: readonly OriginPlane[] = ['XY', 'XZ', 'YZ']

const ORIGIN_AXES: readonly { readonly name: 'X' | 'Y' | 'Z'; readonly direction: Vector3 }[] = [
  { name: 'X', direction: [1, 0, 0] },
  { name: 'Y', direction: [0, 1, 0] },
  { name: 'Z', direction: [0, 0, 1] },
]

/** The item kinds each selection field kind takes (2.12 table); faces, edges and vertices of a body field pick their body. */
const FIELD_ACCEPTS: Readonly<Record<SelectionKind, ReadonlySet<SelectionItemKind>>> = {
  body: new Set(['body', 'face', 'edge', 'vertex']),
  sketch: new Set(['sketch_curve', 'sketch_region']),
  sketch_region: new Set(['sketch_region']),
  sketch_point: new Set(['sketch_point']),
  face: new Set(['face']),
  edge: new Set(['edge']),
  plane: new Set(['origin_plane', 'datum_plane', 'face']),
  axis: new Set(['origin_axis', 'sketch_curve', 'edge', 'face']),
}

const PLANE_PICKING: ReadonlySet<SelectionItemKind> = new Set([
  'origin_plane',
  'datum_plane',
  'face',
])

const ACTIVE_SKETCH_KINDS: ReadonlySet<SelectionItemKind> = new Set([
  'sketch_curve',
  'sketch_point',
  'sketch_region',
])

/** What may be picked now (3.6). */
type Picking = {
  readonly accepts: (kind: SelectionItemKind) => boolean
  /** A face pick stands for its body (Bodies on, Faces off). */
  readonly facesPickBodies: boolean
}

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

type Point3 = readonly [number, number, number]

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

function plainItem(
  key: string,
  kind: SelectionItemKind,
  label: string,
  extra: Partial<SelectionItem> = {},
): SelectionItem {
  return {
    key,
    kind,
    label,
    body: null,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
    ...extra,
  }
}

function colorOf(theme: ThemeName, role: ViewportRole): Rgb {
  return linearFromHex(VIEWPORT_COLORS[theme][role])
}

/** A box's union with points; null stays null when there are none. */
function extend(bounds: BoundingBox | null, points: Iterable<Point3>): BoundingBox | null {
  let result = bounds
  for (const [x, y, z] of points) {
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
      continue
    }
    result =
      result === null
        ? { min: [x, y, z], max: [x, y, z] }
        : {
            min: [
              Math.min(result.min[0], x),
              Math.min(result.min[1], y),
              Math.min(result.min[2], z),
            ],
            max: [
              Math.max(result.max[0], x),
              Math.max(result.max[1], y),
              Math.max(result.max[2], z),
            ],
          }
  }
  return result
}

function boxCorners(box: BoundingBox): Point3[] {
  const corners: Point3[] = []
  for (const x of [box.min[0], box.max[0]]) {
    for (const y of [box.min[1], box.max[1]]) {
      for (const z of [box.min[2], box.max[2]]) {
        corners.push([x, y, z])
      }
    }
  }
  return corners
}

/** A box with every -0 written 0, so equal boxes compare equal. */
function normalizedBox(box: BoundingBox): BoundingBox {
  return {
    min: [box.min[0] + 0, box.min[1] + 0, box.min[2] + 0],
    max: [box.max[0] + 0, box.max[1] + 0, box.max[2] + 0],
  }
}

/** The 3D point of sketch coordinates on a placement. */
function onPlane(placement: Placement, yAxis: Vector3, point: Vector2): Point3 {
  return addScaled(addScaled(placement.origin, placement.x_axis, point[0]), yAxis, point[1])
}

function yAxisOf(placement: Placement): Vector3 {
  return cross(placement.normal, placement.x_axis)
}

// Sketch geometry.

type SolvedEntity = SketchSolution['entities'][number]

/** The 2D polyline of an entity's curve, or null for a point entity. */
function curvePolyline(entity: SolvedEntity, tolerance: number): Float32Array | null {
  switch (entity.type) {
    case 'point':
      return null
    case 'line':
      return new Float32Array([...entity.start, ...entity.end])
    case 'circle':
      return tessellateArc(entity.center, entity.radius, 0, 2 * Math.PI, tolerance)
    case 'arc':
      return tessellateArc(
        entity.center,
        entity.radius,
        entity.start_angle,
        entity.end_angle,
        tolerance,
      )
  }
}

/** 2D points that bound an entity (curve tessellations, point positions). */
function entityPoints(entity: SolvedEntity, tolerance: number): Vector2[] {
  if (entity.type === 'point') {
    return [entity.position]
  }
  const polyline = curvePolyline(entity, tolerance) ?? new Float32Array()
  const points: Vector2[] = []
  for (let i = 0; i + 1 < polyline.length; i += 2) {
    points.push([polyline[i] ?? 0, polyline[i + 1] ?? 0])
  }
  return points
}

/** Line segments (6 floats each) and their element ids for polylines on a placement. */
function segmentsOnPlane(
  placement: Placement,
  polylines: readonly { readonly element: number; readonly points: Float32Array }[],
): { positions: Float32Array; elementIds: Uint32Array } {
  const yAxis = yAxisOf(placement)
  const positions: number[] = []
  const elementIds: number[] = []
  for (const { element, points } of polylines) {
    for (let i = 0; i + 3 < points.length; i += 2) {
      const start = onPlane(placement, yAxis, [points[i] ?? 0, points[i + 1] ?? 0])
      const end = onPlane(placement, yAxis, [points[i + 2] ?? 0, points[i + 3] ?? 0])
      positions.push(...start, ...end)
      elementIds.push(element)
    }
  }
  return { positions: new Float32Array(positions), elementIds: new Uint32Array(elementIds) }
}

/** The selection item of a sketch point reference. */
function pointItem(sketch: string, point: string): SelectionItem {
  return plainItem(`point:${sketch}:${point}`, 'sketch_point', `Point ${point}`, {
    feature: sketch,
    sketchItem: { kind: 'point', sketch, point },
  })
}

/** The selection item of a sketch entity: a point entity by its point reference (its id), a curve by entity. */
function entityItem(sketch: string, entity: SolvedEntity): SelectionItem {
  if (entity.type === 'point') {
    return pointItem(sketch, entity.id)
  }
  const noun = entity.type === 'line' ? 'Line' : entity.type === 'circle' ? 'Circle' : 'Arc'
  return plainItem(`sketch:${sketch}:${entity.id}`, 'sketch_curve', `${noun} ${entity.id}`, {
    feature: sketch,
    sketchItem: { kind: 'entity', sketch, entity: entity.id, entityType: entity.type },
  })
}

function sketchOriginItem(sketch: string, which: 'point' | 'x_axis' | 'y_axis'): SelectionItem {
  const reference: SketchItemReference = { kind: 'origin', sketch, which }
  return which === 'point'
    ? plainItem(`point:${sketch}:origin`, 'sketch_point', 'Origin', {
        feature: sketch,
        sketchItem: reference,
      })
    : plainItem(
        `sketch:${sketch}:${which}`,
        'sketch_curve',
        which === 'x_axis' ? 'X axis' : 'Y axis',
        { feature: sketch, sketchItem: reference },
      )
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

function regionItem(sketch: string, solution: SketchSolution, index: number): SelectionItem | null {
  const region = solution.regions[index]
  if (region === undefined) {
    return null
  }
  return plainItem(
    `region:${sketch}:${region.reference.key}`,
    'sketch_region',
    `Region ${String(index + 1)}`,
    { feature: sketch, sketchItem: { kind: 'region', sketch, region } },
  )
}

// Visibility.

/** Whether some feature other than the sketch refers to it through a `sketch` key (3.10). */
function referencesSketch(value: unknown, sketchId: string): boolean {
  if (Array.isArray(value)) {
    return value.some((entry) => referencesSketch(entry, sketchId))
  }
  if (typeof value !== 'object' || value === null) {
    return false
  }
  return Object.entries(value).some(
    ([key, entry]) => (key === 'sketch' && entry === sketchId) || referencesSketch(entry, sketchId),
  )
}

/** Session visibility of a sketch, datum or origin item; without an entry, the opening rule of 3.10 applies. */
function sessionVisible(state: AppState, key: string, recipe: Document | null): boolean {
  const stored = state.view.sessionVisible[key]
  if (stored !== undefined) {
    return stored
  }
  if (key.startsWith('origin:')) {
    return selectOriginVisible(state, key)
  }
  const id = key.slice('feature:'.length)
  const features = recipe?.features ?? []
  const feature = features.find((candidate) => candidate.id === id)
  if (feature?.type !== 'sketch') {
    return true
  }
  return !features.some((other) => other.id !== id && referencesSketch(other, id))
}

function bodyVisible(body: Body): boolean {
  return body.visible !== false
}

// Picking rules.

function activeFieldKind(state: AppState): SelectionKind | null {
  const command = state.command
  if (command === null || command.activeField === null) {
    return null
  }
  const hint = state.catalog?.byType[command.featureType]?.manifest.fields.find(
    (candidate) => candidate.path === command.activeField,
  )
  return hint?.selection ?? null
}

/**
 * Whether the active field also takes faces: a point field places holes on a flat face, and a
 * multiple edge field takes a face as all of its edges (3.7).
 */
function activeFieldTakesFaces(state: AppState): boolean {
  const command = state.command
  const kind = activeFieldKind(state)
  if (command === null || (kind !== 'sketch_point' && kind !== 'edge')) {
    return false
  }
  if (kind === 'sketch_point') {
    return true
  }
  const field = commandSelectionFields(state).find(
    (candidate) => candidate.path === command.activeField,
  )
  return field?.multiple === true
}

function pickingFor(state: AppState): Picking {
  if (state.command !== null) {
    const field = activeFieldKind(state)
    const kinds = new Set<SelectionItemKind>(field === null ? [] : FIELD_ACCEPTS[field])
    if (activeFieldTakesFaces(state)) {
      kinds.add('face')
    }
    return { accepts: (kind) => kinds.has(kind), facesPickBodies: false }
  }
  if (state.context === 'sketch' && state.sketch !== null) {
    const kinds = state.sketch.sketchId === null ? PLANE_PICKING : ACTIVE_SKETCH_KINDS
    return { accepts: (kind) => kinds.has(kind), facesPickBodies: false }
  }
  // Lay on face takes faces only; whether one is flat is known once it is picked (3.6, 3.17).
  if (state.context === 'print' && state.print.pickingFace) {
    return { accepts: (kind) => kind === 'face', facesPickBodies: false }
  }
  const filter = state.view.filter
  const facesPickBodies = filter.body && !filter.face
  return {
    accepts: (kind) => {
      switch (kind) {
        case 'face':
          return filter.face || facesPickBodies
        case 'edge':
          return filter.edge
        case 'vertex':
          return filter.vertex
        case 'body':
          return filter.body
        default:
          return true
      }
    },
    facesPickBodies,
  }
}

/** A selection field of the active command with its value and the object that holds it. */
export type CommandSelectionField = {
  readonly path: string
  readonly accepts: SelectionKind
  readonly multiple: boolean
  readonly value: unknown
  /** The params object that holds the field, whose `body` or `sketch` qualifies its value. */
  readonly siblings: Readonly<Record<string, unknown>>
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** The selection fields of the active feature command, in manifest order. */
export function commandSelectionFields(state: AppState): readonly CommandSelectionField[] {
  const command = state.command
  const feature = command === null ? undefined : state.catalog?.byType[command.featureType]
  if (command === null || feature === undefined) {
    return []
  }
  const model = buildInspectorModel({ feature, params: command.params, errors: {} })
  return [...model.essential, ...model.more].flatMap((field) => {
    if (field.control.type !== 'selection') {
      return []
    }
    const lastDot = field.path.lastIndexOf('.')
    const parent =
      lastDot === -1 ? command.params : getParam(command.params, field.path.slice(0, lastDot))
    return [
      {
        path: field.path,
        accepts: field.control.accepts,
        multiple: field.control.multiple,
        value: field.value,
        siblings: isRecord(parent) ? parent : {},
      },
    ]
  })
}

/** The selection items whose state the viewport shows now: the command's field values, the sketch selection or the app selection. */
function shownSelection(state: AppState): readonly SelectionItem[] {
  if (state.command !== null) {
    return commandSelectionFields(state).flatMap((field) =>
      itemsFromValue(field.accepts, field.value, state, field.siblings),
    )
  }
  if (state.context === 'sketch' && state.sketch !== null) {
    return state.sketch.selection
  }
  return state.selection
}

/**
 * The keys drawn hovered: the pointer's hover target, and the items of the selection field whose
 * box the pointer is over (3.7).
 */
export function hoveredKeys(state: AppState): ReadonlySet<string> {
  const keys = new Set<string>()
  if (state.hover !== null) {
    keys.add(state.hover.key)
  }
  const path = state.command?.hoveredField ?? null
  const field =
    path === null
      ? undefined
      : commandSelectionFields(state).find((candidate) => candidate.path === path)
  if (field !== undefined) {
    for (const item of itemsFromValue(field.accepts, field.value, state, field.siblings)) {
      keys.add(item.key)
    }
  }
  return keys
}

type Highlight = {
  readonly hover: ReadonlySet<string>
  readonly selected: ReadonlySet<string>
}

function statesFor(
  count: number,
  keyOf: (index: number) => string,
  highlight: Highlight,
): Uint8Array {
  const states = new Uint8Array(count)
  for (let i = 0; i < count; i++) {
    const key = keyOf(i)
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

/** The section clip with every -0 written 0. */
function clipOf(
  plane: 'XY' | 'XZ' | 'YZ',
  offset: number,
  flipped: boolean,
): NonNullable<RenderFrame['section']> {
  const clip = sectionClip(plane, offset, flipped)
  return {
    normal: [clip.normal[0] + 0, clip.normal[1] + 0, clip.normal[2] + 0],
    offset: clip.offset + 0,
  }
}

const MILLIMETERS: Readonly<Record<Units, number>> = { mm: 1, cm: 10, m: 1000, in: 25.4 }

function isUnit(word: string): word is Units {
  return Object.hasOwn(MILLIMETERS, word)
}

/** Parses a plain number with an optional length unit word, in mm; null for anything else. */
function plainLength(text: string, units: Units): number | null {
  const match = /^\s*([+-]?(?:\d+(?:\.\d*)?|\.\d+))\s*(mm|cm|m|in)?\s*$/.exec(text)
  if (match === null) {
    return null
  }
  const [, number = '', word = ''] = match
  return Number(number) * MILLIMETERS[isUnit(word) ? word : units]
}

/** Side and center of drawn planes: max(50 mm, 1.2 × the visible diagonal) about the visible center (3.12). */
export function planeExtent(bounds: BoundingBox | null): { side: number; center: Vector3 } {
  if (bounds === null) {
    return { side: MINIMUM_PLANE_SIDE, center: [0, 0, 0] }
  }
  const diagonal = Math.hypot(
    bounds.max[0] - bounds.min[0],
    bounds.max[1] - bounds.min[1],
    bounds.max[2] - bounds.min[2],
  )
  return {
    side: Math.max(MINIMUM_PLANE_SIDE, PLANE_SIDE_FACTOR * diagonal),
    center: [
      (bounds.min[0] + bounds.max[0]) / 2,
      (bounds.min[1] + bounds.max[1]) / 2,
      (bounds.min[2] + bounds.max[2]) / 2,
    ],
  }
}

/** Whether a sketch, datum plane or origin item is shown this session (3.10). */
export function isSessionVisible(state: AppState, key: string): boolean {
  return sessionVisible(state, key, state.document?.recipe ?? null)
}

/** The 3D bounds of a visible sketch's entities. */
function sketchBounds(solution: SketchSolution): BoundingBox | null {
  const yAxis = yAxisOf(solution.placement)
  let bounds: BoundingBox | null = null
  for (const entity of solution.entities) {
    const radius = entity.type === 'circle' || entity.type === 'arc' ? entity.radius : 1
    const points = entityPoints(entity, Math.max(radius * 0.01, 1e-6)).map((point) =>
      onPlane(solution.placement, yAxis, point),
    )
    bounds = extend(bounds, points)
  }
  return bounds
}

function visibleSketches(state: AppState): readonly {
  readonly id: string
  readonly solution: SketchSolution
  readonly active: boolean
}[] {
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
    if (
      solution === undefined ||
      (!active && !sessionVisible(state, `feature:${feature.id}`, document.recipe))
    ) {
      return []
    }
    return [{ id: feature.id, solution, active }]
  })
}

function visibleDatums(
  state: AppState,
): readonly { readonly id: string; readonly placement: Placement }[] {
  const document = state.document
  if (document === null) {
    return []
  }
  return (document.recipe.features ?? []).flatMap((feature) => {
    const placement = document.datums[feature.id] ?? null
    if (
      feature.type !== 'datum_plane' ||
      placement === null ||
      !sessionVisible(state, `feature:${feature.id}`, document.recipe)
    ) {
      return []
    }
    return [{ id: feature.id, placement }]
  })
}

/** The 4 corners of a square plane of this side centered at the projection of `center` onto it. */
function squareCorners(
  placement: Placement,
  side: number,
  center: Vector3,
): [Point3, Point3, Point3, Point3] {
  const normal = normalize(placement.normal) ?? [0, 0, 1]
  const xAxis = normalize(placement.x_axis) ?? [1, 0, 0]
  const yAxis = cross(normal, xAxis)
  const offset = subtract(center, placement.origin)
  const middle = addScaled(center, normal, -dot(offset, normal))
  const half = side / 2
  return [
    addScaled(addScaled(middle, xAxis, -half), yAxis, -half),
    addScaled(addScaled(middle, xAxis, half), yAxis, -half),
    addScaled(addScaled(middle, xAxis, half), yAxis, half),
    addScaled(addScaled(middle, xAxis, -half), yAxis, half),
  ]
}

function originPlanesShown(state: AppState): boolean {
  if (state.context === 'sketch' && state.sketch?.sketchId === null) {
    return true
  }
  return activeFieldKind(state) === 'plane'
}

/**
 * The origin planes the scene draws: all three while a plane is picked for a new sketch or a plane
 * field is active, otherwise those shown this session (3.10, 3.12).
 */
export function drawnOriginPlanes(state: AppState): readonly OriginPlane[] {
  const shown = originPlanesShown(state)
  return ORIGIN_PLANES.filter((plane) => shown || sessionVisible(state, `origin:${plane}`, null))
}

/**
 * Where an origin plane's label goes (3.16.1): the corner of its drawn square in the plane's
 * positive first and second axis directions.
 */
export function originPlaneLabelAnchor(plane: OriginPlane, bounds: BoundingBox | null): Vector3 {
  const extent = planeExtent(bounds)
  const [, , corner] = squareCorners(ORIGIN_PLANE_PLACEMENTS[plane], extent.side, extent.center)
  return corner
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
  const millimetersPerPixel = visibleHeightAtTarget(camera) / Math.max(height, 1)
  const exact = SKETCH_TOLERANCE_PIXELS * millimetersPerPixel
  const step = Math.log(RETESSELLATION_FACTOR)
  return Math.exp(Math.floor(Math.log(exact) / step) * step)
}

function placementScale(entity: SolvedEntity): number {
  return entity.type === 'circle' || entity.type === 'arc' ? entity.radius * 0.01 : 1
}

/** Bounds of visible bodies, sketches (through `boundsOf`, which may cache) and datum planes. */
function modelBounds(
  state: AppState,
  boundsOf: (solution: SketchSolution) => BoundingBox | null,
): BoundingBox | null {
  const document = state.document
  if (document === null) {
    return null
  }
  let bounds: BoundingBox | null = null
  for (const body of document.recipe.bodies ?? []) {
    const mesh = document.meshes[body.id]
    if (bodyVisible(body) && mesh !== undefined) {
      bounds = extend(bounds, boxCorners(mesh.boundingBox))
    }
  }
  for (const sketch of visibleSketches(state)) {
    const box = boundsOf(sketch.solution)
    if (box !== null) {
      bounds = extend(bounds, boxCorners(box))
    }
  }
  for (const datum of visibleDatums(state)) {
    bounds = extend(bounds, [datum.placement.origin])
  }
  return bounds
}

/** The four sides of a square as line segments, two points each. */
function squareOutline(corners: readonly Point3[]): number[] {
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
  // Solutions are replaced, never changed, so their bounds and identities can be kept by object.
  const sketchBoxes = new WeakMap<SketchSolution, BoundingBox | null>()
  const solutionNumbers = new WeakMap<SketchSolution, number>()
  let nextSolutionNumber = 1

  function cachedSketchBounds(solution: SketchSolution): BoundingBox | null {
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
    const bounds = modelBounds(state, cachedSketchBounds)
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
    size: { width: number; height: number; pixelRatio: number },
  ): RenderFrame {
    lastState = state
    const document = state.document
    pruneDocuments(document?.documentHandle ?? null)
    const theme = resolvedTheme(state.settings)
    const scale = state.settings.uiScale
    const picking = pickingFor(state)
    const hoverKey = state.hover?.key ?? null
    const hoverKeys = hoveredKeys(state)
    const selection = shownSelection(state)
    const selectedKeys = new Set(selection.map((item) => item.key))
    const pending: PendingDraw[] = []
    const bounds = modelBounds(state, cachedSketchBounds)
    let sceneBounds = bounds

    // Bodies (3.12 passes 2 to 5).
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
          sceneBounds = extend(sceneBounds, boxCorners(model.data.boundingBox))
        }
        pending.push(
          ...bodyDraws(state, {
            body,
            model,
            preview,
            theme,
            scale,
            picking,
            hoverKey,
            hoverKeys,
            selectedBody: selectedBodies.has(body.id),
            field: fields[body.id] ?? null,
          }),
        )
        if (preview !== undefined) {
          sceneBounds = extend(sceneBounds, boxCorners(preview.data.boundingBox))
        }
      }
    }

    // The ground or sketch grid (3.12 pass 6), never picked.
    const gridDraws = drawGrid(state, bounds, camera, size.height, theme, scale)
    for (const corner of gridDraws.corners) {
      sceneBounds = extend(sceneBounds, [corner])
    }
    pending.push(...gridDraws.draws)

    // Datum and origin planes, origin axes (3.12 pass 6).
    const planes = planeExtent(bounds)
    const datumColor = colorOf(theme, 'datum')
    const planeKinds: {
      readonly key: string
      readonly item: SelectionItem
      readonly placement: Placement
    }[] = [
      ...visibleDatums(state).map((datum) => ({
        key: `feature:${datum.id}`,
        item: plainItem(`feature:${datum.id}`, 'datum_plane', datum.id, { feature: datum.id }),
        placement: datum.placement,
      })),
      ...drawnOriginPlanes(state).map((plane) => ({
        key: `origin:${plane}`,
        item: originItem(plane),
        placement: ORIGIN_PLANE_PLACEMENTS[plane],
      })),
    ]
    for (const plane of planeKinds) {
      const corners = squareCorners(plane.placement, planes.side, planes.center)
      sceneBounds = extend(sceneBounds, corners)
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
      uploadGenerated(outlineId, signature, () => ({
        kind: 'lines',
        positions: new Float32Array(squareOutline(corners)),
        elementIds: new Uint32Array(4),
        elementCount: 1,
      }))
      const label =
        plane.item.kind === 'datum_plane' && document !== null
          ? featureLabel(document.recipe, plane.item.feature ?? '', state.catalog)
          : plane.item.label
      const item = { ...plane.item, label }
      const highlight = { hover: hoverKeys, selected: selectedKeys }
      const states = statesFor(1, () => plane.key, highlight)
      pending.push({
        call: {
          ...overlayCall(datumColor, 'translucent', PLANE_FILL_OPACITY, 0, false, false),
          id: fillId,
          depthTest: true,
        },
        elementCount: 1,
        target: picking.accepts(item.kind) ? itemTarget([item], 'area') : null,
        states,
      })
      pending.push({
        call: {
          ...overlayCall(
            datumColor,
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
    pending.push(...originDraws(state, theme, scale, planes, picking, hoverKeys, selectedKeys))

    // Sketches (3.12 pass 7, 3.16.2).
    for (const sketch of visibleSketches(state)) {
      const box = cachedSketchBounds(sketch.solution)
      if (box !== null) {
        sceneBounds = extend(sceneBounds, boxCorners(box))
      }
      pending.push(
        ...sketchDraws(state, sketch, {
          theme,
          scale,
          camera,
          size,
          picking,
          hoverKeys,
          selectedKeys,
        }),
      )
    }

    // The hole centers a command placed on a face, drawn from the command before any preview (3.7).
    const helper = state.command?.helper ?? null
    const helperPoints = helperDraw(helper, theme, scale, picking, hoverKeys, selectedKeys)
    if (helperPoints === null) {
      remove('helper:points')
    } else {
      pending.push(helperPoints)
    }

    // Print bed (3.12 pass 8).
    const bed = bedOutline(state)
    if (bed === null) {
      remove('bed')
    } else {
      const bedPoints: Point3[] = []
      for (let i = 0; i + 2 < bed.length; i += 3) {
        bedPoints.push([bed[i] ?? 0, bed[i + 1] ?? 0, bed[i + 2] ?? 0])
      }
      sceneBounds = extend(sceneBounds, bedPoints)
      if (uploaded.get('bed')?.positions !== bed) {
        upload('bed', {
          kind: 'lines',
          positions: bed,
          elementIds: new Uint32Array(bed.length / 6),
          elementCount: 1,
        })
      }
      pending.push({
        call: {
          ...overlayCall(colorOf(theme, 'bed'), 'opaque', 1, BED_WIDTH * scale, false, false),
          id: 'bed',
          depthTest: true,
        },
        elementCount: 1,
        target: null,
        states: null,
      })
    }

    // Pick ids: consecutive from 1, each draw's range as long as its element count.
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

    const aspect = size.height > 0 && size.width > 0 ? size.width / size.height : 1
    const center: Vector3 =
      sceneBounds === null
        ? camera.target
        : [
            (sceneBounds.min[0] + sceneBounds.max[0]) / 2,
            (sceneBounds.min[1] + sceneBounds.max[1]) / 2,
            (sceneBounds.min[2] + sceneBounds.max[2]) / 2,
          ]
    const radius =
      sceneBounds === null
        ? Math.max(visibleHeightAtTarget(camera), 1)
        : Math.max(
            Math.hypot(
              sceneBounds.max[0] - sceneBounds.min[0],
              sceneBounds.max[1] - sceneBounds.min[1],
              sceneBounds.max[2] - sceneBounds.min[2],
            ) / 2,
            1,
          )
    const { near, far } = depthRange(camera, center, radius)
    const section = state.view.section
    const units = document?.recipe.units ?? 'mm'
    const sectionOffset =
      section === null
        ? 0
        : (sectionOffsets.get(section.offset) ?? plainLength(section.offset, units) ?? 0)
    return {
      view: viewMatrix(camera),
      projection: projectionMatrix(camera, aspect, near, far),
      eye: eye(camera),
      orthographic: camera.projection === 'orthographic',
      pixelRatio: size.pixelRatio,
      uiScale: scale,
      theme: viewportTheme(theme),
      draws,
      section: section === null ? null : clipOf(section.plane, sectionOffset, section.flipped),
      analysis: overhangAnalysis(state, theme),
    }
  }

  /** The grid's two draws, minor and major lines, and its square's corners; nothing when no grid is drawn. */
  function drawGrid(
    state: AppState,
    bounds: BoundingBox | null,
    camera: Camera,
    height: number,
    theme: ThemeName,
    scale: number,
  ): { readonly draws: PendingDraw[]; readonly corners: readonly Point3[] } {
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
        ...overlayCall(color, 'translucent', opacity, GRID_LINE_WIDTH * scale, false, false),
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

  function bodyDraws(state: AppState, input: BodyInput): PendingDraw[] {
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
    const draws: PendingDraw[] =
      model === undefined
        ? []
        : modelDraws(state, input, model, {
            base,
            shaded,
            lines,
            previewing: preview !== undefined,
          })
    if (preview !== undefined) {
      const previewIds = meshIds('preview', preview)
      draws.push({
        call: shaded(previewIds.faces, base),
        elementCount: preview.data.faceCount,
        target: null,
        states: null,
        scalars: null,
      })
      draws.push({
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
  ): PendingDraw[] {
    const { body, scale, picking, hoverKey, hoverKeys } = input
    const { base, shaded, lines, previewing } = style
    const locked = state.view.locked.includes(body.id)
    const mesh = model.data
    const ids = meshIds('model', model)
    const highlighted = highlights[body.id]
    const indices = (element: 'face' | 'edge' | 'vertex'): ReadonlySet<number> =>
      highlighted !== undefined && highlighted.meshId === model.meshId
        ? new Set(highlighted[element])
        : new Set()
    const bodyKey = `body:${body.id}`
    const bodyHovered = hoverKeys.has(bodyKey)
    const fieldHovered = fieldHover[body.id]
    const fieldIndices = (element: 'face' | 'edge' | 'vertex'): ReadonlySet<number> =>
      fieldHovered !== undefined && fieldHovered.meshId === model.meshId
        ? new Set(fieldHovered[element])
        : new Set()
    const meshKey = (element: string, index: number): string =>
      `mesh:${String(model.meshId)}:${element}:${String(index)}`
    // A hovered face that stands for its edges lights them up instead of itself (3.7).
    const redirect =
      hoverEdges !== null &&
      hoverEdges.body === body.id &&
      hoverEdges.meshId === model.meshId &&
      hoverEdges.faceKey === hoverKey
        ? new Set(hoverEdges.edges)
        : null
    const hoveredByField = {
      face: fieldIndices('face'),
      edge: fieldIndices('edge'),
      vertex: fieldIndices('vertex'),
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
      const selected = indices(element)
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
    return [
      {
        call: previewing
          ? { ...shaded(ids.faces, base), layer: 'translucent', opacity: 0, analysis: false }
          : shaded(ids.faces, base),
        elementCount: mesh.faceCount,
        target: pickable(faceKind) ? meshTarget('face', 'area') : null,
        states: elementStates('face', mesh.faceCount),
        scalars: previewing ? null : input.field,
      },
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
    ]
  }

  function originDraws(
    state: AppState,
    theme: ThemeName,
    scale: number,
    planes: { side: number; center: Vector3 },
    picking: Picking,
    hoverKeys: ReadonlySet<string>,
    selectedKeys: ReadonlySet<string>,
  ): PendingDraw[] {
    const sketchMode = state.context === 'sketch' && state.sketch?.sketchId !== null
    if (sketchMode) {
      remove('origin:axes')
      remove('origin:point')
      return []
    }
    const axisFieldActive = activeFieldKind(state) === 'axis'
    const axes = ORIGIN_AXES.filter(
      (axis) => axisFieldActive || sessionVisible(state, `origin:${axis.name}`, null),
    )
    const color = colorOf(theme, 'sketchReference')
    const highlight = { hover: hoverKeys, selected: selectedKeys }
    const draws: PendingDraw[] = []
    if (axes.length === 0) {
      remove('origin:axes')
    } else {
      const half = planes.side / 2
      const signature = JSON.stringify([axes.map((axis) => axis.name), half])
      uploadGenerated('origin:axes', signature, () => ({
        kind: 'lines',
        positions: new Float32Array(
          axes.flatMap((axis) => [
            ...axis.direction.map((component) => component * -half),
            ...axis.direction.map((component) => component * half),
          ]),
        ),
        elementIds: new Uint32Array(axes.map((_, index) => index)),
        elementCount: axes.length,
      }))
      draws.push({
        call: {
          ...overlayCall(color, 'translucent', 1, REFERENCE_CURVE_WIDTH * scale, true, true),
          id: 'origin:axes',
          pickSize: SKETCH_CURVE_PICK_WIDTH * scale,
        },
        elementCount: axes.length,
        target: picking.accepts('origin_axis')
          ? itemTarget(
              axes.map((axis) => originItem(axis.name)),
              'curve',
            )
          : null,
        states: statesFor(axes.length, (index) => `origin:${axes[index]?.name ?? ''}`, highlight),
      })
    }
    if (!sessionVisible(state, 'origin:O', null)) {
      remove('origin:point')
    } else {
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
        target: picking.accepts('origin_point') ? itemTarget([originItem('O')], 'point') : null,
        states: statesFor(1, () => 'origin:O', highlight),
      })
    }
    return draws
  }

  type SketchInput = {
    readonly theme: ThemeName
    readonly scale: number
    readonly camera: Camera
    readonly size: { readonly width: number; readonly height: number }
    readonly picking: Picking
    readonly hoverKeys: ReadonlySet<string>
    readonly selectedKeys: ReadonlySet<string>
  }

  function sketchDraws(
    state: AppState,
    sketch: { readonly id: string; readonly solution: SketchSolution; readonly active: boolean },
    input: SketchInput,
  ): PendingDraw[] {
    const { theme, scale, picking, hoverKeys, selectedKeys } = input
    const { solution } = sketch
    const tolerance = sketchTolerance(input.camera, input.size.height)
    const prefix = `sketch:${sketch.id}`
    const reference = colorOf(theme, 'sketchReference')
    const highlight = { hover: hoverKeys, selected: selectedKeys }
    // In sketch mode only the active sketch is pickable; elsewhere visible sketches are (3.6).
    const sketchMode = state.context === 'sketch'
    const pickableSketch = sketch.active || !sketchMode
    const curves = solution.entities.filter((entity) => entity.type !== 'point')
    const draws: PendingDraw[] = []
    const problemEntities = new Set(
      solution.profile_findings.flatMap((finding) =>
        'entities' in finding ? finding.entities : 'entity' in finding ? [finding.entity] : [],
      ),
    )
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
      uploadGenerated(id, signature, () => {
        const polylines = chosen.map((entity, element) => ({
          element,
          points: curvePolyline(entity, tolerance) ?? new Float32Array(),
        }))
        const { positions, elementIds } = segmentsOnPlane(solution.placement, polylines)
        return { kind: 'lines', positions, elementIds, elementCount: chosen.length }
      })
      const elementColors = new Float32Array(chosen.flatMap((entity) => [...entityColor(entity)]))
      const items = chosen.map((entity) => entityItem(sketch.id, entity))
      const states = statesFor(chosen.length, (index) => items[index]?.key ?? '', highlight)
      if (sketch.active) {
        chosen.forEach((entity, index) => {
          if (problemEntities.has(entity.id)) {
            states[index] = (states[index] ?? 0) | ELEMENT_STATE.problem
          }
        })
      }
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
      const yAxis = yAxisOf(solution.placement)
      const signature = `${String(solutionNumber(solution))}:${String(sketch.active)}`
      uploadGenerated(pointsId, signature, () => ({
        kind: 'points',
        positions: new Float32Array(
          solvedPoints.flatMap((point) => [...onPlane(solution.placement, yAxis, point.at)]),
        ),
        elementIds: new Uint32Array(solvedPoints.map((_, index) => index)),
        elementCount: solvedPoints.length,
      }))
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
        states: statesFor(solvedPoints.length, (index) => items[index]?.key ?? '', highlight),
      })
    }

    // Region fills from the solution's display buffers.
    const regionsId = `${prefix}:regions`
    const display = state.document?.sketches[sketch.id]?.display ?? null
    if (display === null || solution.regions.length === 0) {
      remove(regionsId)
    } else {
      const placement = solution.placement
      const yAxis = yAxisOf(placement)
      const signature = JSON.stringify(placement)
      const current = uploaded.get(regionsId)
      if (
        current === undefined ||
        signatures.get(regionsId) !== signature ||
        current.elementIds !== display.regionIds
      ) {
        const positions = new Float32Array((display.regionPositions.length / 2) * 3)
        for (let i = 0; 2 * i + 1 < display.regionPositions.length; i++) {
          const point = onPlane(placement, yAxis, [
            display.regionPositions[2 * i] ?? 0,
            display.regionPositions[2 * i + 1] ?? 0,
          ])
          positions.set(point, 3 * i)
        }
        upload(regionsId, {
          kind: 'fill',
          positions,
          indices: display.regionIndices,
          elementIds: display.regionIds,
          elementCount: solution.regions.length,
        })
        signatures.set(regionsId, signature)
      }
      const items = solution.regions.map((_, index) => regionItem(sketch.id, solution, index))
      draws.push({
        call: {
          ...overlayCall(
            colorOf(theme, 'sketchRegion'),
            'overlay',
            sketch.active ? REGION_OPACITY : 0,
            0,
            false,
            false,
          ),
          id: regionsId,
        },
        elementCount: solution.regions.length,
        target:
          pickableSketch && picking.accepts('sketch_region') ? itemTarget(items, 'area') : null,
        states: statesFor(solution.regions.length, (index) => items[index]?.key ?? '', highlight),
      })
    }

    // The active sketch's origin point and axes (3.16.2).
    const axesId = `${prefix}:axes`
    const originId = `${prefix}:origin`
    if (!sketch.active) {
      remove(axesId)
      remove(originId)
      return draws
    }
    const camera = input.camera
    const aspect = input.size.height > 0 ? input.size.width / input.size.height : 1
    const reach = SKETCH_AXIS_REACH * visibleHeightAtTarget(camera) * Math.max(aspect, 1)
    const yAxis = yAxisOf(solution.placement)
    const offset = subtract(camera.target, solution.placement.origin)
    const alongX = dot(offset, solution.placement.x_axis)
    const alongY = dot(offset, yAxis)
    // Quantized so the axes are uploaded again only after a large camera change.
    const quantum = Math.max(reach / 8, 1e-6)
    const centerX = Math.round(alongX / quantum) * quantum
    const centerY = Math.round(alongY / quantum) * quantum
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
      return { kind: 'lines', positions, elementIds: new Uint32Array([0, 1]), elementCount: 2 }
    })
    uploadGenerated(originId, JSON.stringify(solution.placement), () => ({
      kind: 'points',
      positions: new Float32Array(solution.placement.origin),
      elementIds: new Uint32Array([0]),
      elementCount: 1,
    }))
    const axisItems = [sketchOriginItem(sketch.id, 'x_axis'), sketchOriginItem(sketch.id, 'y_axis')]
    const sketchOrigin = sketchOriginItem(sketch.id, 'point')
    draws.push({
      call: {
        ...overlayCall(reference, 'overlay', 1, REFERENCE_CURVE_WIDTH * scale, true, true),
        id: axesId,
        pickSize: SKETCH_CURVE_PICK_WIDTH * scale,
      },
      elementCount: 2,
      target: picking.accepts('sketch_curve') ? itemTarget(axisItems, 'curve') : null,
      states: statesFor(2, (index) => axisItems[index]?.key ?? '', highlight),
    })
    draws.push({
      call: {
        ...overlayCall(reference, 'overlay', 1, ORIGIN_POINT_SIZE * scale, false, true),
        id: originId,
        pickSize: SKETCH_POINT_PICK_SIZE * scale,
      },
      elementCount: 1,
      target: picking.accepts('sketch_point') ? itemTarget([sketchOrigin], 'point') : null,
      states: statesFor(1, () => sketchOrigin.key, highlight),
    })
    return draws
  }

  /** The point entities of a command's helper sketch as pickable sketch points, or null without any. */
  function helperDraw(
    helper: HelperSketch | null,
    theme: ThemeName,
    scale: number,
    picking: Picking,
    hoverKeys: ReadonlySet<string>,
    selectedKeys: ReadonlySet<string>,
  ): PendingDraw | null {
    const plane = helper?.params.plane
    const entities = (helper?.params.entities ?? []).flatMap((entity) =>
      entity.type === 'point' ? [entity] : [],
    )
    if (helper === null || plane?.kind !== 'face' || entities.length === 0) {
      return null
    }
    const placement = plane.fallback
    const yAxis = yAxisOf(placement)
    const id = 'helper:points'
    uploadGenerated(id, JSON.stringify([placement, entities]), () => ({
      kind: 'points',
      positions: new Float32Array(
        entities.flatMap((entity) => [...onPlane(placement, yAxis, entity.position)]),
      ),
      elementIds: new Uint32Array(entities.map((_, index) => index)),
      elementCount: entities.length,
    }))
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
      states: statesFor(entities.length, (index) => items[index]?.key ?? '', {
        hover: hoverKeys,
        selected: selectedKeys,
      }),
    }
  }

  /** World points that stand for a selection item, for its screen rectangle. */
  function itemPoints(item: SelectionItem): Point3[] {
    const state = lastState
    if (item.kind === 'body' && item.body !== null) {
      const box =
        state?.document?.meshes[item.body]?.boundingBox ??
        Array.from(models.values()).find((entry) => entry.body === item.body)?.data.boundingBox
      return box === undefined ? [] : boxCorners(box)
    }
    if (item.element !== null) {
      return [item.element.centroid]
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
          return entity === undefined
            ? []
            : entityPoints(entity, Math.max(placementScale(entity), 1e-6)).map((point) =>
                onPlane(placement, yAxis, point),
              )
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

    screenRectangle(items, camera, size) {
      const aspect = size.height > 0 ? size.width / size.height : 1
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
          const x = ((position.x + 1) / 2) * size.width
          const y = ((1 - position.y) / 2) * size.height
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
