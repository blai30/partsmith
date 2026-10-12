import type { SketchConstraint, SketchSolution } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { SelectionItem } from '../state/store'
import type { DimensionType } from './constraints'
import {
  add,
  closestPointOnLine,
  cross,
  direction,
  distance,
  dot,
  length,
  lineIntersection,
  midpoint,
  normalizeAngle,
  pointOnCircle,
  scale,
  signedDistanceToLine,
  subtract,
  unitAt,
  type Vector2,
} from './geometry'
import {
  curveCircle,
  lineEnds,
  operandNoun,
  operandsOf,
  pointPosition,
  solvedEntity,
  type Operand,
} from './operands'
import type { IdSource } from './recipes'

// The dimension tool (ui.md 3.16.7): which dimension a selection and label position make, its
// measured value, its constraint record, and where its label goes.

/** Coordinate differences at or below this (mm) make two points axis-aligned. */
const ALIGNED_TOLERANCE = 1e-9
/** Lines within this angle (rad) of each other are parallel. */
const PARALLEL_TOLERANCE = 1e-6
/** How far a default label sits from its geometry, in CSS pixels. */
const LABEL_OFFSET_PIXELS = 28

type Point = Extract<Operand, { kind: 'point' }>
type Line = Extract<Operand, { kind: 'line' }>

/** How a selection is dimensioned, with its solved geometry. */
type Measured =
  | {
      readonly kind: 'points'
      readonly first: Point
      readonly second: Point
      readonly at: readonly [Vector2, Vector2]
    }
  | { readonly kind: 'line'; readonly line: Line; readonly at: readonly [Vector2, Vector2] }
  | {
      readonly kind: 'point_line'
      readonly point: Point
      readonly line: Line
      readonly at: Vector2
      readonly ends: { readonly start: Vector2; readonly end: Vector2 }
    }
  | {
      readonly kind: 'lines'
      readonly first: Line
      readonly second: Line
      readonly firstEnds: { readonly start: Vector2; readonly end: Vector2 }
      readonly secondEnds: { readonly start: Vector2; readonly end: Vector2 }
    }
  | {
      readonly kind: 'round'
      readonly curve: string
      readonly circle: boolean
      readonly center: Vector2
      readonly radius: number
    }
  | { readonly kind: 'unsupported'; readonly message: string }

function unsupported(operands: readonly Operand[]): Measured {
  const [first, second] = operands
  if (first === undefined) {
    return { kind: 'unsupported', message: 'Select a point or a curve to dimension.' }
  }
  if (second === undefined) {
    return { kind: 'unsupported', message: 'Select a second point or a line to dimension a point.' }
  }
  return {
    kind: 'unsupported',
    message: MESSAGES.cannot_dimension(operandNoun(first), operandNoun(second)),
  }
}

/** Reads the selection as one of the dimensionable shapes of 3.16.7. */
function measuredOf(selection: readonly SelectionItem[], solution: SketchSolution): Measured {
  const operands = operandsOf(selection) ?? []
  const [first, second] = operands
  if (operands.length === 1 && first !== undefined) {
    if (first.kind === 'line' && !first.axis) {
      const ends = lineEnds(solution, first.line)
      return ends === null
        ? unsupported(operands)
        : { kind: 'line', line: first, at: [ends.start, ends.end] }
    }
    if (first.kind === 'circle' || first.kind === 'arc') {
      const circle = curveCircle(solution, first.curve)
      return circle === null
        ? unsupported(operands)
        : {
            kind: 'round',
            curve: first.curve,
            circle: first.kind === 'circle',
            center: circle.center,
            radius: circle.radius,
          }
    }
    return unsupported(operands)
  }
  if (operands.length !== 2 || first === undefined || second === undefined) {
    return unsupported(operands)
  }
  if (first.kind === 'point' && second.kind === 'point') {
    const firstAt = pointPosition(solution, first.point)
    const secondAt = pointPosition(solution, second.point)
    return firstAt === null || secondAt === null || first.point === second.point
      ? unsupported(operands)
      : { kind: 'points', first, second, at: [firstAt, secondAt] }
  }
  const point = first.kind === 'point' ? first : second.kind === 'point' ? second : null
  const line = first.kind === 'line' ? first : second.kind === 'line' ? second : null
  if (point !== null && line !== null) {
    const at = pointPosition(solution, point.point)
    const ends = lineEnds(solution, line.line)
    return at === null || ends === null || point.owner === line.line
      ? unsupported(operands)
      : { kind: 'point_line', point, line, at, ends }
  }
  if (first.kind === 'line' && second.kind === 'line' && first.line !== second.line) {
    const firstEnds = lineEnds(solution, first.line)
    const secondEnds = lineEnds(solution, second.line)
    return firstEnds === null || secondEnds === null
      ? unsupported(operands)
      : { kind: 'lines', first, second, firstEnds, secondEnds }
  }
  return unsupported(operands)
}

function parallel(
  first: { readonly start: Vector2; readonly end: Vector2 },
  second: { readonly start: Vector2; readonly end: Vector2 },
): boolean {
  const firstDirection = subtract(first.end, first.start)
  const secondDirection = subtract(second.end, second.start)
  const angle = Math.atan2(
    Math.abs(cross(firstDirection, secondDirection)),
    Math.abs(dot(firstDirection, secondDirection)),
  )
  return angle <= PARALLEL_TOLERANCE
}

function within(value: number, first: number, second: number): boolean {
  return value >= Math.min(first, second) && value <= Math.max(first, second)
}

/** The kind of dimension two points get for a label position (3.16.7). */
function pointsType(at: readonly [Vector2, Vector2], labelAt: Vector2): DimensionType {
  const [first, second] = at
  const aligned =
    Math.abs(first[0] - second[0]) <= ALIGNED_TOLERANCE ||
    Math.abs(first[1] - second[1]) <= ALIGNED_TOLERANCE
  if (aligned) {
    return 'distance'
  }
  const insideX = within(labelAt[0], first[0], second[0])
  const insideY = within(labelAt[1], first[1], second[1])
  if (insideX && !insideY) {
    return 'horizontal_distance'
  }
  if (insideY && !insideX) {
    return 'vertical_distance'
  }
  return 'distance'
}

/**
 * The dimension a selection gets with its label at `labelAt` (3.16.7), or why it cannot be
 * dimensioned.
 */
export function dimensionTypeFor(
  selection: readonly SelectionItem[],
  labelAt: Vector2,
  solution: SketchSolution,
): DimensionType | { readonly unsupported: string } {
  const measured = measuredOf(selection, solution)
  switch (measured.kind) {
    case 'points':
    case 'line':
      return pointsType(measured.at, labelAt)
    case 'point_line':
      return 'distance'
    case 'lines':
      return parallel(measured.firstEnds, measured.secondEnds) ? 'distance' : 'angle'
    case 'round':
      return measured.circle ? 'diameter' : 'radius'
    case 'unsupported':
      return { unsupported: measured.message }
  }
}

/**
 * The angle between two lines that the sector containing `labelAt` spans, with the line rays that
 * bound it: the sector runs counterclockwise from `from` to `from + value`.
 */
function labeledSector(
  first: { readonly start: Vector2; readonly end: Vector2 },
  second: { readonly start: Vector2; readonly end: Vector2 },
  labelAt: Vector2,
): {
  readonly vertex: Vector2
  readonly from: number
  readonly value: number
  readonly fromFirst: boolean
} | null {
  const vertex = lineIntersection(first.start, first.end, second.start, second.end)
  if (vertex === null) {
    return null
  }
  const firstAngle = direction(subtract(first.end, first.start))
  const secondAngle = direction(subtract(second.end, second.start))
  const rays = [
    { angle: normalizeAngle(firstAngle), first: true },
    { angle: normalizeAngle(firstAngle + Math.PI), first: true },
    { angle: normalizeAngle(secondAngle), first: false },
    { angle: normalizeAngle(secondAngle + Math.PI), first: false },
  ]
  const label = normalizeAngle(direction(subtract(labelAt, vertex)))
  // The sector is bounded by the last ray at or clockwise of the label and the next ray after it.
  let start = rays[0]
  let startGap = Number.POSITIVE_INFINITY
  for (const ray of rays) {
    const gap = normalizeAngle(label - ray.angle)
    if (gap < startGap) {
      startGap = gap
      start = ray
    }
  }
  if (start === undefined) {
    return null
  }
  const other = rays.filter((ray) => ray.first !== start.first)
  let value = Number.POSITIVE_INFINITY
  for (const ray of other) {
    value = Math.min(value, normalizeAngle(ray.angle - start.angle))
  }
  return { vertex, from: start.angle, value, fromFirst: start.first }
}

/** The value a dimension of `type` measures on the selection now: mm, or rad for angles. */
export function measuredValue(
  type: DimensionType,
  selection: readonly SelectionItem[],
  labelAt: Vector2,
  solution: SketchSolution,
): number {
  const measured = measuredOf(selection, solution)
  switch (measured.kind) {
    case 'points':
    case 'line': {
      const [first, second] = measured.at
      if (type === 'horizontal_distance') {
        return Math.abs(second[0] - first[0])
      }
      if (type === 'vertical_distance') {
        return Math.abs(second[1] - first[1])
      }
      return distance(first, second)
    }
    case 'point_line':
      return Math.abs(signedDistanceToLine(measured.at, measured.ends.start, measured.ends.end))
    case 'lines': {
      if (type === 'angle') {
        return labeledSector(measured.firstEnds, measured.secondEnds, labelAt)?.value ?? 0
      }
      return Math.abs(
        signedDistanceToLine(
          measured.firstEnds.start,
          measured.secondEnds.start,
          measured.secondEnds.end,
        ),
      )
    }
    case 'round':
      return type === 'diameter' ? 2 * measured.radius : measured.radius
    case 'unsupported':
      return Number.NaN
  }
}

/** The expression fields of a dimension: `driving` is written only when false (sketch.md 3.3). */
function valueFields(expression: string, driving: boolean): { expr: string; driving?: false } {
  return driving ? { expr: expression } : { expr: expression, driving: false }
}

/** A horizontal or vertical distance between two points. */
function signedDistance(
  type: 'horizontal_distance' | 'vertical_distance',
  id: string,
  points: [string, string],
  fields: { expr: string; driving?: false },
): SketchConstraint {
  // Each branch narrows `type` to one literal, so the record is one member of the union.
  return type === 'horizontal_distance'
    ? { id, type, points, ...fields }
    : { id, type, points, ...fields }
}

/** The constraint of `type` on these operands, or null when they cannot take it. */
function buildDimension(
  type: DimensionType,
  operands: readonly Operand[],
  id: string,
  fields: { expr: string; driving?: false },
  order: { readonly swapLines: boolean; readonly swapPoints: boolean },
): SketchConstraint | null {
  const [first, second] = operands
  if (first === undefined) {
    return null
  }
  if (second === undefined) {
    switch (first.kind) {
      case 'line': {
        if (first.axis) {
          return null
        }
        if (type === 'distance') {
          return { id, type, line: first.line, ...fields }
        }
        if (type === 'horizontal_distance' || type === 'vertical_distance') {
          const ends: [string, string] = [`${first.line}.start`, `${first.line}.end`]
          return signedDistance(type, id, order.swapPoints ? [ends[1], ends[0]] : ends, fields)
        }
        return null
      }
      case 'circle':
      case 'arc':
        return type === 'radius' || type === 'diameter'
          ? { id, type, curve: first.curve, ...fields }
          : null
      case 'point':
        return null
    }
  }
  if (operands.length !== 2) {
    return null
  }
  if (first.kind === 'point' && second.kind === 'point') {
    const points: [string, string] = order.swapPoints
      ? [second.point, first.point]
      : [first.point, second.point]
    if (type === 'distance') {
      return { id, type, points, ...fields }
    }
    return type === 'horizontal_distance' || type === 'vertical_distance'
      ? signedDistance(type, id, points, fields)
      : null
  }
  const point = first.kind === 'point' ? first : second.kind === 'point' ? second : null
  const line = first.kind === 'line' ? first : second.kind === 'line' ? second : null
  if (point !== null && line !== null) {
    return type === 'distance' ? { id, type, point: point.point, line: line.line, ...fields } : null
  }
  if (first.kind === 'line' && second.kind === 'line') {
    if (type === 'angle') {
      const lines: [string, string] = order.swapLines
        ? [second.line, first.line]
        : [first.line, second.line]
      return { id, type, lines, ...fields }
    }
    // Two parallel lines: the distance from a point of one to the other.
    if (type === 'distance' && !first.axis) {
      return { id, type, point: `${first.line}.start`, line: second.line, ...fields }
    }
    if (type === 'distance' && !second.axis) {
      return { id, type, point: `${second.line}.start`, line: first.line, ...fields }
    }
  }
  return null
}

/**
 * How to order a new dimension's operands so it means what the user sees: horizontal and vertical
 * distances read positive (sketch.md 2.5 makes them signed), and an angle measures the sector that
 * contains the label (it runs counterclockwise from its first line to its second).
 */
function operandOrder(
  type: DimensionType,
  selection: readonly SelectionItem[],
  labelAt: Vector2,
  solution: SketchSolution | undefined,
): { readonly swapLines: boolean; readonly swapPoints: boolean } {
  const measured = solution === undefined ? null : measuredOf(selection, solution)
  if (measured === null) {
    return { swapLines: false, swapPoints: false }
  }
  if (type === 'angle' && measured.kind === 'lines') {
    const sector = labeledSector(measured.firstEnds, measured.secondEnds, labelAt)
    return { swapLines: sector !== null && !sector.fromFirst, swapPoints: false }
  }
  if (
    (type === 'horizontal_distance' || type === 'vertical_distance') &&
    (measured.kind === 'points' || measured.kind === 'line')
  ) {
    const axis = type === 'horizontal_distance' ? 0 : 1
    const [first, second] = measured.at
    return { swapLines: false, swapPoints: second[axis] < first[axis] }
  }
  return { swapLines: false, swapPoints: false }
}

/**
 * The dimension constraint of `type` on the selection with `expression` as its value. With the
 * solution, operands are ordered so a horizontal or vertical distance is positive and an angle
 * measures the sector that contains `labelAt`. Throws when the selection cannot take the type, which
 * `dimensionTypeFor` rules out.
 */
export function dimensionFor(
  type: DimensionType,
  selection: readonly SelectionItem[],
  expression: string,
  driving: boolean,
  labelAt: Vector2,
  ids: IdSource,
  solution?: SketchSolution,
): SketchConstraint {
  const operands = operandsOf(selection) ?? []
  const order = operandOrder(type, selection, labelAt, solution)
  const built = buildDimension(type, operands, '', valueFields(expression, driving), order)
  if (built === null) {
    throw new Error(`The selection cannot take a ${type.replaceAll('_', ' ')} dimension.`)
  }
  return { ...built, id: ids.next('c') }
}

/** The sketch's centroid of points, which default labels move away from. */
function centroid(solution: SketchSolution): Vector2 {
  if (solution.points.length === 0) {
    return [0, 0]
  }
  const sum = solution.points.reduce<Vector2>((total, point) => add(total, point.at), [0, 0])
  return scale(sum, 1 / solution.points.length)
}

/** `base` moved by `offset` mm along the normal of `along` that points away from `away`. */
function awayFrom(base: Vector2, along: Vector2, away: Vector2, offset: number): Vector2 {
  const size = length(along)
  if (size === 0) {
    return add(base, [0, offset])
  }
  const normal: Vector2 = [-along[1] / size, along[0] / size]
  const side = dot(subtract(base, away), normal) >= 0 ? 1 : -1
  return add(base, scale(normal, side * offset))
}

function referencedPoint(solution: SketchSolution, reference: string): Vector2 | null {
  return pointPosition(solution, reference)
}

/**
 * Where a dimension's label goes when the sketch stores none (sketch.md stores no label positions):
 * beside its geometry, on the side away from the sketch's center, in sketch coordinates.
 */
export function defaultLabelPosition(
  constraint: SketchConstraint,
  solution: SketchSolution,
  millimetersPerPixel: number,
): Vector2 {
  const offset = LABEL_OFFSET_PIXELS * millimetersPerPixel
  const center = centroid(solution)
  switch (constraint.type) {
    case 'distance':
    case 'horizontal_distance':
    case 'vertical_distance': {
      const ends = dimensionEnds(constraint, solution)
      if (ends === null) {
        return center
      }
      const [first, second] = ends
      const middle = midpoint(first, second)
      if (constraint.type === 'horizontal_distance') {
        const high = Math.max(first[1], second[1])
        const low = Math.min(first[1], second[1])
        return middle[1] >= center[1] ? [middle[0], high + offset] : [middle[0], low - offset]
      }
      if (constraint.type === 'vertical_distance') {
        const right = Math.max(first[0], second[0])
        const left = Math.min(first[0], second[0])
        return middle[0] >= center[0] ? [right + offset, middle[1]] : [left - offset, middle[1]]
      }
      return awayFrom(middle, subtract(second, first), center, offset)
    }
    case 'radius':
    case 'diameter': {
      const circle = curveCircle(solution, constraint.curve)
      if (circle === null) {
        return center
      }
      const angle = circle.full ? Math.PI / 4 : (circle.start + circle.end) / 2
      return pointOnCircle(circle.center, circle.radius + offset, angle)
    }
    case 'angle': {
      const [firstLine = '', secondLine = ''] = constraint.lines
      const first = lineEnds(solution, firstLine)
      const second = lineEnds(solution, secondLine)
      if (first === null || second === null) {
        return center
      }
      const vertex = lineIntersection(first.start, first.end, second.start, second.end)
      if (vertex === null) {
        return midpoint(midpoint(first.start, first.end), midpoint(second.start, second.end))
      }
      const value = solution.constraints.find((state) => state.id === constraint.id)?.value
      const sweep =
        value ??
        normalizeAngle(
          direction(subtract(second.end, second.start)) -
            direction(subtract(first.end, first.start)),
        ) % Math.PI
      // The sector starts on the ray toward the first line's middle, so the label sits by the lines.
      const toward = subtract(midpoint(first.start, first.end), vertex)
      const from =
        length(toward) === 0 ? direction(subtract(first.end, first.start)) : direction(toward)
      const reach = Math.min(length(toward), distance(midpoint(second.start, second.end), vertex))
      const radius = Math.min(3 * offset, Math.max(offset, 0.75 * reach))
      return add(vertex, scale(unitAt(from + sweep / 2), radius))
    }
    default:
      return center
  }
}

/** The two measured points of a distance-like dimension (a point and its foot for point to line). */
export function dimensionEnds(
  constraint: SketchConstraint,
  solution: SketchSolution,
): readonly [Vector2, Vector2] | null {
  if ('points' in constraint) {
    const first = referencedPoint(solution, constraint.points[0])
    const second = referencedPoint(solution, constraint.points[1])
    return first === null || second === null ? null : [first, second]
  }
  if (
    constraint.type !== 'distance' &&
    constraint.type !== 'horizontal_distance' &&
    constraint.type !== 'vertical_distance'
  ) {
    return null
  }
  if ('point' in constraint) {
    const at = referencedPoint(solution, constraint.point)
    const ends = lineEnds(solution, constraint.line)
    return at === null || ends === null ? null : [at, closestPointOnLine(at, ends.start, ends.end)]
  }
  const entity = solvedEntity(solution, constraint.line)
  return entity?.type === 'line' ? [entity.start, entity.end] : null
}
