import type { SketchConstraint, SketchSolution } from '../generated/protocol'
import { framedArea, type ViewWindow } from '../viewport/camera'
import { glyphAnchor } from './annotations'
import type { DimensionGeometry } from './annotations'
import type { PreviewShape } from './drawing'
import {
  add,
  clipSegment,
  length,
  scale,
  segmentTouchesBox,
  subtract,
  tessellateArc,
  boxContains,
  type Box2,
  type Vector2,
} from './geometry'
import type { SolvedEntity } from './operands'

// The sketch overlay in canvas pixels (ui.md 3.16.2, 3.16.9): sketch geometry projected through the
// camera into SVG path data, the screen offsets of constraint glyphs, and box selection.

/** The canvas pixel (CSS pixels from the top left) of a sketch point, or null behind the eye. */
export type Projector = (point: Vector2) => Vector2 | null

/** Glyphs sit this many CSS pixels (times the UI scale) off their curve (3.16.2). */
const GLYPH_OFFSET_PIXELS = 14
/** Further glyphs on the same anchor stack this many CSS pixels apart along the curve. */
const GLYPH_STACK_PIXELS = 20

/** A number for SVG path data: two decimals are finer than a device pixel. */
function coordinate(value: number): string {
  return String(Math.round(value * 100) / 100)
}

/** Vertices of a tessellation, 2 floats per vertex, as points. */
function vertices(positions: Float32Array): Vector2[] {
  const points: Vector2[] = []
  for (let i = 0; 2 * i + 1 < positions.length; i += 1) {
    points.push([positions[2 * i] ?? 0, positions[2 * i + 1] ?? 0])
  }
  return points
}

/** A polyline through sketch points, projected; a point behind the eye breaks it into pieces. */
export function projectPolyline(points: readonly Vector2[], project: Projector): Vector2[][] {
  const pieces: Vector2[][] = []
  let current: Vector2[] = []
  for (const point of points) {
    const pixel = project(point)
    if (pixel === null) {
      if (current.length > 1) {
        pieces.push(current)
      }
      current = []
      continue
    }
    current.push(pixel)
  }
  if (current.length > 1) {
    pieces.push(current)
  }
  return pieces
}

/** SVG path data of open polylines in canvas pixels. */
export function polylinePath(polylines: readonly (readonly Vector2[])[]): string {
  return polylines
    .filter((polyline) => polyline.length > 1)
    .map((polyline) =>
      polyline
        .map(
          (point, index) =>
            `${index === 0 ? 'M' : 'L'}${coordinate(point[0])} ${coordinate(point[1])}`,
        )
        .join(' '),
    )
    .join(' ')
}

/** SVG path data of a circle of `radius` pixels around a canvas pixel. */
export function circlePath(center: Vector2, radius: number): string {
  const [x, y] = center
  const left = `${coordinate(x - radius)} ${coordinate(y)}`
  const diameter = coordinate(2 * radius)
  const size = coordinate(radius)
  return `M${left} a${size} ${size} 0 1 0 ${diameter} 0 a${size} ${size} 0 1 0 -${diameter} 0`
}

/** The sketch points along a circle or arc, a full turn when `end - start` is 2π or more. */
function arcPoints(
  center: Vector2,
  radius: number,
  start: number,
  end: number,
  toleranceMillimeters: number,
): Vector2[] {
  return vertices(tessellateArc(center, radius, start, end, toleranceMillimeters))
}

/** The sketch points along a rubber band shape. */
export function shapePoints(shape: PreviewShape, toleranceMillimeters: number): Vector2[] {
  switch (shape.kind) {
    case 'line':
      return [shape.start, shape.end]
    case 'circle':
      return arcPoints(shape.center, shape.radius, 0, 2 * Math.PI, toleranceMillimeters)
    case 'arc':
      return arcPoints(shape.center, shape.radius, shape.start, shape.end, toleranceMillimeters)
  }
}

/** The sketch points along an entity's curve; a point entity is its position alone. */
export function entityPoints(entity: SolvedEntity, toleranceMillimeters: number): Vector2[] {
  switch (entity.type) {
    case 'point':
      return [entity.position]
    case 'line':
      return [entity.start, entity.end]
    case 'circle':
      return arcPoints(entity.center, entity.radius, 0, 2 * Math.PI, toleranceMillimeters)
    case 'arc':
      return arcPoints(
        entity.center,
        entity.radius,
        entity.start_angle,
        entity.end_angle,
        toleranceMillimeters,
      )
  }
}

/** An arrowhead as a closed triangle: its tip and a base `size` pixels back toward `toward`. */
export function arrowheadPath(tip: Vector2, toward: Vector2, size: number): string {
  const along = subtract(toward, tip)
  const span = length(along)
  if (span === 0) {
    return ''
  }
  const unit = scale(along, 1 / span)
  const normal: Vector2 = [-unit[1], unit[0]]
  const base = add(tip, scale(unit, size))
  const left = add(base, scale(normal, size / 3))
  const right = add(base, scale(normal, -size / 3))
  return `M${coordinate(tip[0])} ${coordinate(tip[1])} L${coordinate(left[0])} ${coordinate(left[1])} L${coordinate(right[0])} ${coordinate(right[1])} Z`
}

/** A dimension's geometry in canvas pixels: line path data and filled arrowhead path data. */
export function dimensionPaths(
  geometry: DimensionGeometry,
  project: Projector,
  toleranceMillimeters: number,
  arrowPixels: number,
): { readonly lines: string; readonly arrows: string } {
  const polylines = [
    ...geometry.lines.flatMap(([start, end]) => projectPolyline([start, end], project)),
    ...geometry.arcs.flatMap((arc) =>
      projectPolyline(
        arcPoints(arc.center, arc.radius, arc.start, arc.end, toleranceMillimeters),
        project,
      ),
    ),
  ]
  const arrows = geometry.arrows.flatMap((arrow) => {
    const tip = project(arrow.tip)
    const toward = project(arrow.toward)
    return tip === null || toward === null ? [] : [arrowheadPath(tip, toward, arrowPixels)]
  })
  return { lines: polylinePath(polylines), arrows: arrows.filter((path) => path !== '').join(' ') }
}

/** The screen direction of a sketch direction at a sketch point, as a unit vector, or null. */
function screenDirection(
  project: Projector,
  at: Vector2,
  direction: Vector2,
  pixel: Vector2,
): Vector2 | null {
  const size = length(direction)
  if (size === 0) {
    return null
  }
  const ahead = project(add(at, scale(direction, 1 / size)))
  if (ahead === null) {
    return null
  }
  const along = subtract(ahead, pixel)
  const span = length(along)
  return span > 1e-9 ? scale(along, 1 / span) : null
}

/**
 * The canvas pixel at the center of each glyph (3.16.2): its anchor moved 14 pixels (times the UI
 * scale) off the curve on the side facing up the screen, further glyphs on the same anchor 20
 * pixels apart along it; glyphs at a point sit up and to the right of it. Null when the anchor is
 * not in view.
 */
export function glyphCenters(
  constraints: readonly SketchConstraint[],
  solution: SketchSolution,
  project: Projector,
  uiScale: number,
): ReadonlyMap<string, Vector2 | null> {
  const centers = new Map<string, Vector2 | null>()
  const stacked = new Map<string, number>()
  const offset = GLYPH_OFFSET_PIXELS * uiScale
  const stack = GLYPH_STACK_PIXELS * uiScale
  for (const constraint of constraints) {
    const anchor = glyphAnchor(constraint, solution)
    const pixel = anchor === null ? null : project(anchor.at)
    if (anchor === null || pixel === null) {
      centers.set(constraint.id, null)
      continue
    }
    const index = stacked.get(anchor.group) ?? 0
    stacked.set(anchor.group, index + 1)
    const along =
      anchor.tangent === null ? null : screenDirection(project, anchor.at, anchor.tangent, pixel)
    if (along === null) {
      const diagonal = offset / Math.SQRT2
      centers.set(constraint.id, [pixel[0] + diagonal + index * stack, pixel[1] - diagonal])
      continue
    }
    // Of the two normals, the one pointing up the screen (or right, for a vertical curve).
    const normal: Vector2 = [-along[1], along[0]]
    const facing =
      normal[1] < -1e-9 || (Math.abs(normal[1]) <= 1e-9 && normal[0] > 0)
        ? normal
        : scale(normal, -1)
    centers.set(constraint.id, add(add(pixel, scale(facing, offset)), scale(along, index * stack)))
  }
  return centers
}

/**
 * Where the draw inputs go (3.16.4): a corner `offset` pixels off the rubber band's midpoint, and
 * the sides of that corner they extend to. They take the quadrant facing away from the pointer, so
 * they never cover the point the next click goes to.
 */
export type DrawInputsPlacement = {
  readonly at: Vector2
  /** The inputs extend left of `at` (the pointer is right of the midpoint), else right. */
  readonly left: boolean
  /** The inputs extend above `at` (the pointer is below the midpoint), else below. */
  readonly above: boolean
}

/** The draw inputs' placement beside the rubber band's midpoint, away from the pointer. */
export function drawInputsPlacement(
  midpoint: Vector2,
  pointer: Vector2 | null,
  offset: number,
): DrawInputsPlacement {
  const left = pointer !== null && pointer[0] > midpoint[0]
  const above = pointer !== null && pointer[1] > midpoint[1]
  return {
    at: [midpoint[0] + (left ? -offset : offset), midpoint[1] + (above ? -offset : offset)],
    left,
    above,
  }
}

/** A rectangle on the canvas in CSS pixels. */
export type ScreenBox = {
  readonly left: number
  readonly top: number
  readonly right: number
  readonly bottom: number
}

/** The free area of a view window (3.2) as a screen box. */
export function freeAreaBox(view: ViewWindow): ScreenBox {
  const area = framedArea(view)
  return { left: area.x, top: area.y, right: area.x + area.width, bottom: area.y + area.height }
}

/** A box moved in by `inset` on every side. */
export function insetBox(box: ScreenBox, inset: number): ScreenBox {
  return {
    left: box.left + inset,
    top: box.top + inset,
    right: box.right - inset,
    bottom: box.bottom - inset,
  }
}

/** A position when it lies in the box, edges included, else null: anchors outside the free area hide their element (3.12). */
export function insideBox(position: Vector2 | null, box: ScreenBox): Vector2 | null {
  return position !== null &&
    position[0] >= box.left &&
    position[0] <= box.right &&
    position[1] >= box.top &&
    position[1] <= box.bottom
    ? position
    : null
}

/**
 * One axis of `keptInside`: the position moved so the box of `extent`, starting `offset` extents
 * from it, lies between `low` and `high`, or starts at `low` when it is longer than that.
 */
function keptAlong(
  position: number,
  extent: number,
  offset: number,
  low: number,
  high: number,
): number {
  const first = position + offset * extent
  const moved = Math.max(low, Math.min(first, high - extent))
  return position + moved - first
}

/**
 * Moves an element's position so its box stays inside `area` (3.12: draw inputs and the dimension
 * editor). `start` is where the box begins relative to the position on each axis, in its size: 0
 * extends right of or below it, -1 left of or above it, -0.5 centers it. A box larger than the area
 * keeps its top-left corner inside.
 */
export function keptInside(
  at: Vector2,
  size: { readonly width: number; readonly height: number },
  start: Vector2,
  area: ScreenBox,
): Vector2 {
  return [
    keptAlong(at[0], size.width, start[0], area.left, area.right),
    keptAlong(at[1], size.height, start[1], area.top, area.bottom),
  ]
}

/** Halvings that find where a line passes behind the eye, far below a pixel for any view. */
const EYE_PLANE_STEPS = 48

/**
 * The pixel of the last point in front of the eye between a distance in front of it (`seen`, at
 * `pixel`) and one behind it (`hidden`) along a line, found by halving the stretch between them.
 */
function lastInFront(
  at: (distance: number) => Vector2 | null,
  seen: number,
  pixel: Vector2,
  hidden: number,
): Vector2 {
  let front = seen
  let back = hidden
  let frontPixel = pixel
  for (let i = 0; i < EYE_PLANE_STEPS; i++) {
    const middle = (front + back) / 2
    const middlePixel = at(middle)
    if (middlePixel === null) {
      back = middle
    } else {
      front = middle
      frontPixel = middlePixel
    }
  }
  return frontPixel
}

/**
 * The canvas pixels of the stretch of an axis from `start` to `end` along `direction` (sketch
 * millimeters) that lies in front of the eye, or null when none does. The points of a line in
 * front of the eye form one stretch, so a stretch that passes behind it is cut where it does.
 */
function stretchInFront(
  project: Projector,
  direction: Vector2,
  start: number,
  end: number,
): readonly [Vector2, Vector2] | null {
  const at = (distance: number): Vector2 | null => project(scale(direction, distance))
  const first = at(start)
  const last = at(end)
  if (first === null) {
    return last === null ? null : [lastInFront(at, end, last, start), last]
  }
  return [first, last ?? lastInFront(at, start, first, end)]
}

/**
 * Where the label of a sketch axis goes (3.16.2): the point where the axis's drawn positive half
 * (from the origin, or the drawn `span`'s start when that is further, to the span's end along
 * `direction`) leaves `area`, or its end when it ends inside; null while it does not cross `area`.
 */
export function axisLabelPosition(
  project: Projector,
  direction: Vector2,
  span: readonly [number, number],
  area: ScreenBox,
): Vector2 | null {
  const start = Math.max(span[0], 0)
  if (!(span[1] > start)) {
    return null
  }
  const stretch = stretchInFront(project, direction, start, span[1])
  if (stretch === null) {
    return null
  }
  const [from, to] = stretch
  const box: Box2 = { min: [area.left, area.top], max: [area.right, area.bottom] }
  const inside = clipSegment(box, from, to)
  return inside === null ? null : add(from, scale(subtract(to, from), inside[1]))
}

/** The box spanned by two canvas pixels. */
export function screenBox(start: Vector2, end: Vector2): ScreenBox {
  return {
    left: Math.min(start[0], end[0]),
    top: Math.min(start[1], end[1]),
    right: Math.max(start[0], end[0]),
    bottom: Math.max(start[1], end[1]),
  }
}

/**
 * The entities a selection box takes (3.16.9): for a window selection those whose every point and
 * tessellation vertex lies inside, for a crossing selection those with any part inside.
 */
export function entitiesInBox(
  solution: SketchSolution,
  project: Projector,
  box: ScreenBox,
  mode: 'window' | 'crossing',
  toleranceMillimeters: number,
): readonly SolvedEntity[] {
  const area: Box2 = { min: [box.left, box.top], max: [box.right, box.bottom] }
  return solution.entities.filter((entity) => {
    const pixels = entityPoints(entity, toleranceMillimeters).map(project)
    if (mode === 'window') {
      return pixels.every((pixel) => pixel !== null && boxContains(area, pixel))
    }
    if (pixels.length === 1) {
      const [only] = pixels
      return only !== null && only !== undefined && boxContains(area, only)
    }
    return pixels.some((pixel, index) => {
      const next = pixels[index + 1]
      return (
        pixel !== null &&
        next !== null &&
        next !== undefined &&
        segmentTouchesBox(area, pixel, next)
      )
    })
  })
}
