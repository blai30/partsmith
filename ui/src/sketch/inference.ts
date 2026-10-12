import type { PointRef, SketchSolution } from '../generated/protocol'
import {
  closestPointOnArc,
  closestPointOnLine,
  closestPointOnSegment,
  cross,
  distance,
  dot,
  length,
  midpoint,
  subtract,
  type Vector2,
} from './geometry'
import { curveCircle, ownerOf, solvedEntity } from './operands'
import { ORIGIN_POINT, X_AXIS, Y_AXIS } from './refs'
import type { SketchToolId } from './session'

// Auto-constraint inference while drawing (ui.md 3.16.5): snaps to points, midpoints and curves,
// and the direction of a line segment (horizontal, vertical, tangent).

export type SnapTarget =
  | { readonly kind: 'point'; readonly point: PointRef; readonly at: Vector2 }
  | { readonly kind: 'midpoint'; readonly line: string; readonly at: Vector2 }
  | { readonly kind: 'curve'; readonly entity: string; readonly at: Vector2 }

export type DirectionInference =
  | { readonly kind: 'horizontal' }
  | { readonly kind: 'vertical' }
  | { readonly kind: 'tangent'; readonly entity: string }

export type InferenceInput = {
  readonly cursor: Vector2
  /** Sketch millimeters per CSS pixel at the cursor. */
  readonly millimetersPerPixel: number
  readonly solution: SketchSolution
  /** Start of the segment being drawn, for direction inference. */
  readonly anchor: { readonly at: Vector2; readonly point: PointRef | null } | null
  /** Tool being drawn, which decides which direction inferences apply. */
  readonly tool: SketchToolId
  readonly suppressed: boolean
}

export type InferenceResult = {
  readonly point: Vector2
  readonly snap: SnapTarget | null
  readonly direction: DirectionInference | null
}

/** Points and line midpoints snap within this many pixels of the cursor. */
export const POINT_SNAP_PIXELS = 8
/** Curves, including the origin axes, snap within this many pixels. */
export const CURVE_SNAP_PIXELS = 6
/** A segment within this angle of an axis direction (modulo 180 degrees) is horizontal or vertical. */
const ALIGNMENT_TOLERANCE = (3 * Math.PI) / 180
/** A segment within this angle of a curve's tangent at its start is tangent. */
const TANGENT_TOLERANCE = (5 * Math.PI) / 180

type Candidate<Target extends SnapTarget> = { readonly target: Target; readonly distance: number }

/** The nearest candidate within reach, the first at equal distance. */
function nearest<Target extends SnapTarget>(
  candidates: readonly Candidate<Target>[],
  reach: number,
): Target | null {
  let best: Candidate<Target> | null = null
  for (const candidate of candidates) {
    if (candidate.distance <= reach && (best === null || candidate.distance < best.distance)) {
      best = candidate
    }
  }
  return best?.target ?? null
}

function pointCandidates(cursor: Vector2, solution: SketchSolution): Candidate<SnapTarget>[] {
  const points = [{ point: ORIGIN_POINT, at: [0, 0] as Vector2 }, ...solution.points]
  return points.map((solved) => ({
    target: { kind: 'point', point: solved.point, at: solved.at },
    distance: distance(cursor, solved.at),
  }))
}

function midpointCandidates(cursor: Vector2, solution: SketchSolution): Candidate<SnapTarget>[] {
  return solution.entities.flatMap((entity) => {
    if (entity.type !== 'line') {
      return []
    }
    const at = midpoint(entity.start, entity.end)
    return [{ target: { kind: 'midpoint', line: entity.id, at }, distance: distance(cursor, at) }]
  })
}

function curveCandidates(cursor: Vector2, solution: SketchSolution): Candidate<SnapTarget>[] {
  const onCurve = (entity: string, at: Vector2): Candidate<SnapTarget> => ({
    target: { kind: 'curve', entity, at },
    distance: distance(cursor, at),
  })
  const axes = [
    onCurve(X_AXIS, closestPointOnLine(cursor, [0, 0], [1, 0])),
    onCurve(Y_AXIS, closestPointOnLine(cursor, [0, 0], [0, 1])),
  ]
  const curves = solution.entities.flatMap((entity) => {
    switch (entity.type) {
      case 'line':
        return [onCurve(entity.id, closestPointOnSegment(cursor, entity.start, entity.end))]
      case 'circle':
      case 'arc': {
        const circle = curveCircle(solution, entity.id)
        return circle === null
          ? []
          : [
              onCurve(
                entity.id,
                closestPointOnArc(cursor, circle.center, circle.radius, circle.start, circle.end),
              ),
            ]
      }
      case 'point':
        return []
    }
  })
  return [...curves, ...axes]
}

/** The snap target under the cursor: points first, then line midpoints, then curves (3.16.5). */
export function snapTarget(
  cursor: Vector2,
  millimetersPerPixel: number,
  solution: SketchSolution,
): SnapTarget | null {
  const pointReach = POINT_SNAP_PIXELS * millimetersPerPixel
  return (
    nearest(pointCandidates(cursor, solution), pointReach) ??
    nearest(midpointCandidates(cursor, solution), pointReach) ??
    nearest(curveCandidates(cursor, solution), CURVE_SNAP_PIXELS * millimetersPerPixel)
  )
}

/** The angle between two directions taken as undirected lines, in [0, π/2]. */
function lineAngle(first: Vector2, second: Vector2): number {
  return Math.atan2(Math.abs(cross(first, second)), Math.abs(dot(first, second)))
}

/**
 * The tangent direction at the point of a line or arc (an undirected line direction), or null when
 * the point is not a line's or arc's end.
 */
function tangentAt(
  solution: SketchSolution,
  point: PointRef,
  kinds: ReadonlySet<'line' | 'arc'>,
): { readonly entity: string; readonly tangent: Vector2 } | null {
  const owner = ownerOf(point)
  const role = point.slice(owner.length + 1)
  if (role !== 'start' && role !== 'end') {
    return null
  }
  const entity = solvedEntity(solution, owner)
  if (entity?.type === 'line' && kinds.has('line')) {
    return { entity: entity.id, tangent: subtract(entity.end, entity.start) }
  }
  if (entity?.type === 'arc' && kinds.has('arc')) {
    const angle = role === 'start' ? entity.start_angle : entity.end_angle
    return { entity: entity.id, tangent: [-Math.sin(angle), Math.cos(angle)] }
  }
  return null
}

const ARCS: ReadonlySet<'line' | 'arc'> = new Set(['arc'])
const LINES_AND_ARCS: ReadonlySet<'line' | 'arc'> = new Set(['line', 'arc'])

/**
 * Tangent inference for a curve that leaves `point` along `leaving`: tangent to the arc (for a new
 * line) or to the line or arc (for a new arc, `fromLines`) that ends there, within 5 degrees.
 */
export function inferTangent(
  solution: SketchSolution,
  point: PointRef | null,
  leaving: Vector2,
  fromLines: boolean,
): DirectionInference | null {
  if (point === null || length(leaving) === 0) {
    return null
  }
  const end = tangentAt(solution, point, fromLines ? LINES_AND_ARCS : ARCS)
  if (end === null || length(end.tangent) === 0) {
    return null
  }
  return lineAngle(leaving, end.tangent) <= TANGENT_TOLERANCE
    ? { kind: 'tangent', entity: end.entity }
    : null
}

/** Horizontal or vertical when a segment is within 3 degrees of an axis, modulo 180 degrees. */
function alignment(segment: Vector2): DirectionInference | null {
  if (length(segment) === 0) {
    return null
  }
  if (lineAngle(segment, [1, 0]) <= ALIGNMENT_TOLERANCE) {
    return { kind: 'horizontal' }
  }
  if (lineAngle(segment, [0, 1]) <= ALIGNMENT_TOLERANCE) {
    return { kind: 'vertical' }
  }
  return null
}

/**
 * Snaps the cursor and infers the direction of a line segment being drawn (3.16.5). Suppressed
 * input returns the raw cursor with no inference.
 */
export function infer(input: InferenceInput): InferenceResult {
  const { cursor, anchor, solution } = input
  if (input.suppressed) {
    return { point: cursor, snap: null, direction: null }
  }
  const snap = snapTarget(cursor, input.millimetersPerPixel, solution)
  const point = snap?.at ?? cursor
  if (input.tool !== 'line' || anchor === null) {
    return { point, snap, direction: null }
  }
  const segment = subtract(point, anchor.at)
  const tangent = inferTangent(solution, anchor.point, segment, false)
  if (tangent !== null) {
    return { point, snap, direction: tangent }
  }
  const aligned = alignment(segment)
  if (aligned === null || snap !== null) {
    return { point, snap, direction: aligned }
  }
  // Without a snap, the end moves exactly onto the horizontal or vertical through the start.
  const onAxis: Vector2 =
    aligned.kind === 'horizontal' ? [point[0], anchor.at[1]] : [anchor.at[0], point[1]]
  return { point: onAxis, snap, direction: aligned }
}
