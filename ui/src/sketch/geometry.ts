// Plane geometry in sketch coordinates (millimeters, angles in radians counterclockwise from +u),
// as sketch.md 3.2 defines entities: arcs run counterclockwise from their start angle to their end
// angle.

export type Vector2 = readonly [number, number]

/** A line segment, or the two points that give a line its direction. */
export type Segment = { readonly start: Vector2; readonly end: Vector2 }

const FULL_TURN = 2 * Math.PI
/** Determinants below this fraction of the squared extent count as zero (three points in a line). */
const COLLINEAR_TOLERANCE = 1e-12
/** The most segments one tessellated arc gets, whatever its size on screen. */
const MAXIMUM_ARC_SEGMENTS = 4096

export function add(first: Vector2, second: Vector2): Vector2 {
  return [first[0] + second[0], first[1] + second[1]]
}

export function subtract(first: Vector2, second: Vector2): Vector2 {
  return [first[0] - second[0], first[1] - second[1]]
}

export function scale(vector: Vector2, factor: number): Vector2 {
  return [vector[0] * factor, vector[1] * factor]
}

/** `point` reflected through `center`: 2 × center - point. */
export function reflect(point: Vector2, center: Vector2): Vector2 {
  return [2 * center[0] - point[0], 2 * center[1] - point[1]]
}

export function dot(first: Vector2, second: Vector2): number {
  return first[0] * second[0] + first[1] * second[1]
}

/** The z component of the 3D cross product: positive when `second` is counterclockwise from `first`. */
export function cross(first: Vector2, second: Vector2): number {
  return first[0] * second[1] - first[1] * second[0]
}

/** The angle between two directions taken as undirected lines, in [0, π/2]. */
export function undirectedAngle(first: Vector2, second: Vector2): number {
  return Math.atan2(Math.abs(cross(first, second)), Math.abs(dot(first, second)))
}

export function length(vector: Vector2): number {
  return Math.hypot(vector[0], vector[1])
}

export function distance(first: Vector2, second: Vector2): number {
  return Math.hypot(first[0] - second[0], first[1] - second[1])
}

export function midpoint(first: Vector2, second: Vector2): Vector2 {
  return [(first[0] + second[0]) / 2, (first[1] + second[1]) / 2]
}

/** The direction of a vector in radians, in (-π, π]. */
export function direction(vector: Vector2): number {
  return Math.atan2(vector[1], vector[0])
}

/** The unit vector at `angle` radians. */
export function unitAt(angle: number): Vector2 {
  return [Math.cos(angle), Math.sin(angle)]
}

/** The point at `angle` radians on the circle (center, radius). */
export function pointOnCircle(center: Vector2, radius: number, angle: number): Vector2 {
  return [center[0] + radius * Math.cos(angle), center[1] + radius * Math.sin(angle)]
}

/** An angle mapped into [0, 2π), as sketch.md's `rem_euclid`. */
export function normalizeAngle(angle: number): number {
  const remainder = angle % FULL_TURN
  const normalized = remainder < 0 ? remainder + FULL_TURN : remainder
  // A remainder just below zero can round up to exactly 2π.
  return normalized >= FULL_TURN ? 0 : normalized
}

/** The counterclockwise sweep of an arc in [0, 2π) (sketch.md `arc_sweep`). */
export function arcSweep(startAngle: number, endAngle: number): number {
  return normalizeAngle(endAngle - startAngle)
}

/**
 * The counterclockwise sweep from `start` to `end` for drawing and distances: an `end - start` of
 * 2π or more is a full circle, anything else is taken modulo 2π.
 */
function drawnSweep(start: number, end: number): number {
  return end - start >= FULL_TURN ? FULL_TURN : arcSweep(start, end)
}

/** Whether `angle` lies on the counterclockwise sweep of `sweep` radians that starts at `start`. */
export function angleWithin(angle: number, start: number, sweep: number): boolean {
  return sweep >= FULL_TURN || normalizeAngle(angle - start) <= sweep
}

/** The circle through three points, or null when they are in a line or two of them coincide. */
export function circleThroughThreePoints(
  first: Vector2,
  second: Vector2,
  third: Vector2,
): { center: Vector2; radius: number } | null {
  const toSecond = subtract(second, first)
  const toThird = subtract(third, first)
  const determinant = 2 * cross(toSecond, toThird)
  const extent = Math.max(length(toSecond), length(toThird), distance(second, third))
  if (!(Math.abs(determinant) > COLLINEAR_TOLERANCE * extent * extent)) {
    return null
  }
  const secondSquared = dot(toSecond, toSecond)
  const thirdSquared = dot(toThird, toThird)
  const offset: Vector2 = [
    (toThird[1] * secondSquared - toSecond[1] * thirdSquared) / determinant,
    (toSecond[0] * thirdSquared - toThird[0] * secondSquared) / determinant,
  ]
  return { center: add(first, offset), radius: length(offset) }
}

/**
 * Vertices of the arc of (center, radius) running counterclockwise from `start` to `end`, 2 floats
 * per vertex, from the start point to the end point inclusive. An `end - start` of 2π or more is the
 * full circle. Each segment's chord deviates from the arc by at most `toleranceMillimeters`.
 */
export function tessellateArc(
  center: Vector2,
  radius: number,
  start: number,
  end: number,
  toleranceMillimeters: number,
): Float32Array {
  const sweep = drawnSweep(start, end)
  const segments = arcSegments(radius, sweep, toleranceMillimeters)
  const positions = new Float32Array(2 * (segments + 1))
  for (let i = 0; i <= segments; i += 1) {
    const angle = i === segments ? start + sweep : start + (sweep * i) / segments
    const [x, y] = pointOnCircle(center, radius, angle)
    positions[2 * i] = x
    positions[2 * i + 1] = y
  }
  return positions
}

/** Segments for a sweep so each chord stays within the tolerance of the arc. */
function arcSegments(radius: number, sweep: number, tolerance: number): number {
  if (!(radius > 0) || !(sweep > 0)) {
    return 1
  }
  const ratio = Math.min(Math.max(tolerance / radius, 0), 2)
  const segmentAngle = 2 * Math.acos(1 - ratio)
  if (!(segmentAngle > 0)) {
    return MAXIMUM_ARC_SEGMENTS
  }
  return Math.min(Math.max(Math.ceil(sweep / segmentAngle), 1), MAXIMUM_ARC_SEGMENTS)
}

/** The point of the segment from `start` to `end` closest to `point`. */
export function closestPointOnSegment(point: Vector2, start: Vector2, end: Vector2): Vector2 {
  const along = subtract(end, start)
  const lengthSquared = dot(along, along)
  if (lengthSquared === 0) {
    return start
  }
  const amount = Math.min(Math.max(dot(subtract(point, start), along) / lengthSquared, 0), 1)
  return add(start, scale(along, amount))
}

/** The point of the infinite line through `start` and `end` closest to `point`. */
export function closestPointOnLine(point: Vector2, start: Vector2, end: Vector2): Vector2 {
  const along = subtract(end, start)
  const lengthSquared = dot(along, along)
  if (lengthSquared === 0) {
    return start
  }
  return add(start, scale(along, dot(subtract(point, start), along) / lengthSquared))
}

export function distancePointToSegment(point: Vector2, start: Vector2, end: Vector2): number {
  return distance(point, closestPointOnSegment(point, start, end))
}

/** Signed distance from `point` to the infinite line through `start` and `end`, positive on its left. */
export function signedDistanceToLine(point: Vector2, start: Vector2, end: Vector2): number {
  const along = subtract(end, start)
  return cross(along, subtract(point, start)) / Math.max(length(along), 1e-12)
}

/**
 * The point of the arc of (center, radius) running counterclockwise from `start` to `end` that is
 * closest to `point`; 2π or more from start to end is the full circle.
 */
export function closestPointOnArc(
  point: Vector2,
  center: Vector2,
  radius: number,
  start: number,
  end: number,
): Vector2 {
  const offset = subtract(point, center)
  const sweep = drawnSweep(start, end)
  if (length(offset) === 0) {
    return pointOnCircle(center, radius, start)
  }
  const angle = direction(offset)
  if (angleWithin(angle, start, sweep)) {
    return pointOnCircle(center, radius, angle)
  }
  const startPoint = pointOnCircle(center, radius, start)
  const endPoint = pointOnCircle(center, radius, start + sweep)
  return distance(point, startPoint) <= distance(point, endPoint) ? startPoint : endPoint
}

export function distancePointToArc(
  point: Vector2,
  center: Vector2,
  radius: number,
  start: number,
  end: number,
): number {
  return distance(point, closestPointOnArc(point, center, radius, start, end))
}

/** The intersection of the infinite lines through the two pairs of points, or null when they are parallel. */
export function lineIntersection(
  firstStart: Vector2,
  firstEnd: Vector2,
  secondStart: Vector2,
  secondEnd: Vector2,
): Vector2 | null {
  const first = subtract(firstEnd, firstStart)
  const second = subtract(secondEnd, secondStart)
  const denominator = cross(first, second)
  if (Math.abs(denominator) <= 1e-12 * length(first) * length(second)) {
    return null
  }
  const amount = cross(subtract(secondStart, firstStart), second) / denominator
  return add(firstStart, scale(first, amount))
}

/** An axis-aligned rectangle in sketch coordinates. */
export type Box2 = { readonly min: Vector2; readonly max: Vector2 }

export function boxContains(box: Box2, point: Vector2): boolean {
  return (
    point[0] >= box.min[0] &&
    point[0] <= box.max[0] &&
    point[1] >= box.min[1] &&
    point[1] <= box.max[1]
  )
}

/** Whether the segment from `start` to `end` has any part inside the box. */
export function segmentTouchesBox(box: Box2, start: Vector2, end: Vector2): boolean {
  if (boxContains(box, start) || boxContains(box, end)) {
    return true
  }
  // Liang-Barsky clipping: the segment enters the box when the clipped interval is not empty.
  let enter = 0
  let leave = 1
  const delta = subtract(end, start)
  const bounds: readonly (readonly [number, number])[] = [
    [-delta[0], start[0] - box.min[0]],
    [delta[0], box.max[0] - start[0]],
    [-delta[1], start[1] - box.min[1]],
    [delta[1], box.max[1] - start[1]],
  ]
  for (const [rate, room] of bounds) {
    if (rate === 0) {
      if (room < 0) {
        return false
      }
      continue
    }
    const amount = room / rate
    if (rate < 0) {
      enter = Math.max(enter, amount)
    } else {
      leave = Math.min(leave, amount)
    }
    if (enter > leave) {
      return false
    }
  }
  return true
}
