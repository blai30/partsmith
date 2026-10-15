import type { Placement, SketchSolution } from '../generated/protocol'
import { tessellateArc, type Vector2 } from '../sketch/geometry'
import type { SelectionItem, SketchItemReference } from '../state/store'
import type { Geometry } from './backend'
import { framedArea, visibleHeightAtTarget, type Camera, type ViewWindow } from './camera'
import { addScaled, cross, dot, extendBounds, subtract, type Bounds, type Vector3 } from './math'
import type { SelectionItemKind } from './picking'

// Sketch geometry as the scene draws it (ui.md 3.12 pass 7, 3.16.2): curves as polylines on the
// sketch plane, and the selection items of sketch entities, points, regions and origins.

export type SolvedEntity = SketchSolution['entities'][number]

/** A selection item with only its key, kind, label and the given fields set. */
export function plainItem(
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

/** The 3D point of sketch coordinates on a placement. */
export function onPlane(placement: Placement, yAxis: Vector3, point: Vector2): Vector3 {
  return addScaled(addScaled(placement.origin, placement.x_axis, point[0]), yAxis, point[1])
}

/** The sketch +v direction of a placement. */
export function yAxisOf(placement: Placement): Vector3 {
  return cross(placement.normal, placement.x_axis)
}

/** A sketch's axes reach this many visible heights (times the canvas's longer side in free-area heights, when longer) each way. */
const SKETCH_AXIS_REACH = 4

/**
 * Where the scene draws a sketch's axes (3.16.2): across the view, from `center - reach` to
 * `center + reach` along each in sketch millimeters, about the camera target's foot on the plane
 * rounded to a `quantum`, so they are uploaded again only after a large camera change.
 */
export type SketchAxisSpans = {
  readonly center: Vector2
  readonly reach: number
  readonly quantum: number
}

export function sketchAxisSpans(
  placement: Placement,
  camera: Camera,
  view: ViewWindow,
): SketchAxisSpans {
  // The visible height spans the free area; the axes cross the whole canvas around it (2.11).
  const across = Math.max(view.width, view.height) / Math.max(framedArea(view).height, 1)
  const reach = SKETCH_AXIS_REACH * visibleHeightAtTarget(camera) * Math.max(across, 1)
  const offset = subtract(camera.target, placement.origin)
  const quantum = Math.max(reach / 8, 1e-6)
  const quantized = (along: number): number => Math.round(along / quantum) * quantum
  return {
    center: [quantized(dot(offset, placement.x_axis)), quantized(dot(offset, yAxisOf(placement)))],
    reach,
    quantum,
  }
}

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
export function segmentsOnPlane(
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

/** Element ids 0 to `count - 1`, one per element in order. */
export function sequentialIds(count: number): Uint32Array {
  return Uint32Array.from({ length: count }, (_, index) => index)
}

/** The curves of entities as line geometry on a placement, one element per entity. */
export function curveLines(
  placement: Placement,
  entities: readonly SolvedEntity[],
  tolerance: number,
): Geometry {
  const polylines = entities.map((entity, element) => ({
    element,
    points: curvePolyline(entity, tolerance) ?? new Float32Array(),
  }))
  const { positions, elementIds } = segmentsOnPlane(placement, polylines)
  return { kind: 'lines', positions, elementIds, elementCount: entities.length }
}

/** Sketch points as point geometry on a placement, one element per point. */
export function pointsOnPlane(placement: Placement, points: readonly Vector2[]): Geometry {
  const yAxis = yAxisOf(placement)
  return {
    kind: 'points',
    positions: new Float32Array(points.flatMap((point) => [...onPlane(placement, yAxis, point)])),
    elementIds: sequentialIds(points.length),
    elementCount: points.length,
  }
}

/** The selection item of a sketch point reference. */
export function pointItem(sketch: string, point: string): SelectionItem {
  return plainItem(`point:${sketch}:${point}`, 'sketch_point', `Point ${point}`, {
    feature: sketch,
    sketchItem: { kind: 'point', sketch, point },
  })
}

/** The selection item of a sketch entity: a point entity by its point reference (its id), a curve by entity. */
export function entityItem(sketch: string, entity: SolvedEntity): SelectionItem {
  if (entity.type === 'point') {
    return pointItem(sketch, entity.id)
  }
  const noun = entity.type === 'line' ? 'Line' : entity.type === 'circle' ? 'Circle' : 'Arc'
  return plainItem(`sketch:${sketch}:${entity.id}`, 'sketch_curve', `${noun} ${entity.id}`, {
    feature: sketch,
    sketchItem: { kind: 'entity', sketch, entity: entity.id, entityType: entity.type },
  })
}

/** The selection item of a sketch's origin point or one of its axes. */
export function sketchOriginItem(
  sketch: string,
  which: 'point' | 'x_axis' | 'y_axis',
): SelectionItem {
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

/**
 * The chord tolerance of the points that bound an entity: 1 percent of a circle's or arc's radius;
 * lines and points have no chords.
 */
function boundsTolerance(entity: SolvedEntity): number {
  return Math.max(
    entity.type === 'circle' || entity.type === 'arc' ? entity.radius * 0.01 : 1,
    1e-6,
  )
}

/** The 3D points that bound an entity on its sketch's placement. */
export function entityWorldPoints(placement: Placement, entity: SolvedEntity): Vector3[] {
  const yAxis = yAxisOf(placement)
  return entityPoints(entity, boundsTolerance(entity)).map((point) =>
    onPlane(placement, yAxis, point),
  )
}

/** The 3D bounds of a visible sketch's entities. */
export function sketchBounds(solution: SketchSolution): Bounds | null {
  let bounds: Bounds | null = null
  for (const entity of solution.entities) {
    bounds = extendBounds(bounds, entityWorldPoints(solution.placement, entity))
  }
  return bounds
}
