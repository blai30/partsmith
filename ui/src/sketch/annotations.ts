import type { SketchConstraint, SketchSolution, Units } from '../generated/protocol'
import { isPlainNumber } from '../expression/step'
import {
  dimensionValueText,
  isDimension,
  type DimensionConstraint,
  type GlyphConstraint,
} from './constraints'
import { angleSweep, dimensionEnds } from './dimensions'
import {
  add,
  angleWithin,
  direction,
  distance,
  dot,
  length,
  lineIntersection,
  midpoint,
  pointOnCircle,
  scale,
  subtract,
  unitAt,
  type Vector2,
} from './geometry'
import {
  curveCircle,
  entityCircle,
  lineEnds,
  ownerOf,
  pointPosition,
  solvedEntity,
  solvedValue,
} from './operands'
import { isAxis, isReservedReference } from './refs'

// Where constraint glyphs and dimensions draw (ui.md 3.16.2), in sketch coordinates; the overlay
// projects them and adds the screen-space offsets.

/** Geometric constraints that draw no glyph: their shared point is drawn filled instead. */
const WITHOUT_GLYPH: ReadonlySet<SketchConstraint['type']> = new Set([
  'coincident',
  'point_on_object',
])

/** Where a constraint's glyph is anchored and which way its curve runs there. */
export type GlyphAnchor = {
  readonly at: Vector2
  /** The curve's direction at the anchor, or null when the glyph sits at a point. */
  readonly tangent: Vector2 | null
  /** Glyphs with the same group share the anchor and stack along the curve. */
  readonly group: string
}

/** Whether a constraint shows a glyph (geometric constraints except coincident and point on object). */
export function hasGlyph(constraint: SketchConstraint): constraint is GlyphConstraint {
  return !isDimension(constraint) && !WITHOUT_GLYPH.has(constraint.type)
}

/** The curves a constraint names, in field order: lines first for line fields, then curves. */
function constrainedCurves(constraint: SketchConstraint): readonly string[] {
  switch (constraint.type) {
    case 'parallel':
    case 'perpendicular':
    case 'collinear':
      return constraint.lines
    case 'tangent':
    case 'concentric':
    case 'equal':
      return constraint.curves
    case 'symmetric':
    case 'midpoint':
      return [constraint.line]
    case 'horizontal':
    case 'vertical':
      return 'line' in constraint ? [constraint.line] : []
    case 'fix':
      return [ownerOf(constraint.target)]
    default:
      return []
  }
}

/** The points a constraint names, in field order. */
function constrainedPoints(constraint: SketchConstraint): readonly string[] {
  switch (constraint.type) {
    case 'horizontal':
    case 'vertical':
    case 'symmetric':
      return 'points' in constraint ? constraint.points : []
    case 'midpoint':
      return [constraint.point]
    case 'fix':
      return [constraint.target]
    default:
      return []
  }
}

/**
 * The anchor of a constraint's glyph: the midpoint of the first constrained line, the point at 45
 * degrees on a circle (on an arc, there when the arc reaches it, else at its middle), or the
 * constrained point itself.
 */
export function glyphAnchor(
  constraint: SketchConstraint,
  solution: SketchSolution,
): GlyphAnchor | null {
  for (const curve of constrainedCurves(constraint)) {
    if (isReservedReference(curve)) {
      continue
    }
    const entity = solvedEntity(solution, curve)
    if (entity?.type === 'line') {
      return {
        at: midpoint(entity.start, entity.end),
        tangent: subtract(entity.end, entity.start),
        group: entity.id,
      }
    }
    if (entity?.type === 'circle' || entity?.type === 'arc') {
      const circle = entityCircle(entity)
      const eighth = Math.PI / 4
      const angle =
        circle.full || angleWithin(eighth, circle.start, circle.end - circle.start)
          ? eighth
          : (circle.start + circle.end) / 2
      return {
        at: pointOnCircle(circle.center, circle.radius, angle),
        tangent: unitAt(angle + Math.PI / 2),
        group: entity.id,
      }
    }
  }
  for (const point of [...constrainedPoints(constraint), ...constrainedCurves(constraint)]) {
    const at = pointPosition(solution, point)
    if (at !== null) {
      return { at, tangent: null, group: point }
    }
  }
  return null
}

/** The text of a dimension label (3.16.2): ⌀ and R prefixes, ° for angles, fx for formulas, and parentheses for reference dimensions. */
export function dimensionLabelText(
  constraint: DimensionConstraint,
  value: number | null,
  units: Units,
): string {
  const number =
    value === null ? constraint.expr : dimensionValueText(constraint.type, value, units)
  const prefix = constraint.type === 'diameter' ? '⌀' : constraint.type === 'radius' ? 'R' : ''
  const formula = isPlainNumber(constraint.expr) ? '' : 'fx '
  const text = `${formula}${prefix}${number}`
  return constraint.driving === false ? `(${text})` : text
}

/** Dimension geometry in sketch coordinates: lines, arcs and arrowheads (tip, and a point behind it). */
export type DimensionGeometry = {
  readonly lines: readonly (readonly [Vector2, Vector2])[]
  readonly arcs: readonly {
    readonly center: Vector2
    readonly radius: number
    readonly start: number
    readonly end: number
  }[]
  readonly arrows: readonly { readonly tip: Vector2; readonly toward: Vector2 }[]
}

const EMPTY: DimensionGeometry = { lines: [], arcs: [], arrows: [] }

/** Extension lines and an arrowed dimension line from `first` and `second` out to the label's line. */
function linearGeometry(
  first: Vector2,
  second: Vector2,
  labelAt: Vector2,
  along: Vector2,
): DimensionGeometry {
  const size = length(along)
  if (size === 0) {
    return EMPTY
  }
  const unit = scale(along, 1 / size)
  const normal: Vector2 = [-unit[1], unit[0]]
  // The dimension line runs along `along` through the label.
  const firstFoot = add(first, scale(normal, dot(subtract(labelAt, first), normal)))
  const secondFoot = add(second, scale(normal, dot(subtract(labelAt, second), normal)))
  return {
    lines: [
      [first, firstFoot],
      [second, secondFoot],
      [firstFoot, secondFoot],
    ],
    arcs: [],
    arrows: [
      { tip: firstFoot, toward: secondFoot },
      { tip: secondFoot, toward: firstFoot },
    ],
  }
}

/** The geometry of a dimension whose label sits at `labelAt` (3.16.2). */
export function dimensionGeometry(
  constraint: SketchConstraint,
  solution: SketchSolution,
  labelAt: Vector2,
): DimensionGeometry {
  switch (constraint.type) {
    case 'distance':
    case 'horizontal_distance':
    case 'vertical_distance': {
      const ends = dimensionEnds(constraint, solution)
      if (ends === null) {
        return EMPTY
      }
      const [first, second] = ends
      const along: Vector2 =
        constraint.type === 'horizontal_distance'
          ? [1, 0]
          : constraint.type === 'vertical_distance'
            ? [0, 1]
            : subtract(second, first)
      return linearGeometry(first, second, labelAt, along)
    }
    case 'radius':
    case 'diameter': {
      const circle = curveCircle(solution, constraint.curve)
      if (circle === null) {
        return EMPTY
      }
      const toward = subtract(labelAt, circle.center)
      const angle = length(toward) === 0 ? Math.PI / 4 : direction(toward)
      const near = pointOnCircle(circle.center, circle.radius, angle)
      // The dimension line reaches out to a label outside the circle.
      const outer = distance(labelAt, circle.center) > circle.radius ? labelAt : near
      if (constraint.type === 'radius') {
        return {
          lines: [[circle.center, outer]],
          arcs: [],
          arrows: [{ tip: near, toward: circle.center }],
        }
      }
      const far = pointOnCircle(circle.center, circle.radius, angle + Math.PI)
      return {
        lines: [[far, outer]],
        arcs: [],
        arrows: [
          { tip: near, toward: far },
          { tip: far, toward: near },
        ],
      }
    }
    case 'angle': {
      const [firstLine = '', secondLine = ''] = constraint.lines
      const first = lineEnds(solution, firstLine)
      const second = lineEnds(solution, secondLine)
      if (first === null || second === null) {
        return EMPTY
      }
      const vertex = lineIntersection(first.start, first.end, second.start, second.end)
      if (vertex === null) {
        return EMPTY
      }
      const radius = distance(labelAt, vertex)
      const firstAngle = direction(subtract(first.end, first.start))
      const sweep = angleSweep(first, second, solvedValue(constraint, solution))
      // Of the two sectors the value spans, draw the one that holds the label.
      const label = direction(subtract(labelAt, vertex))
      const start = angleWithin(label, firstAngle, sweep) ? firstAngle : firstAngle + Math.PI
      const lines: (readonly [Vector2, Vector2])[] = []
      // Axes have no ends, so their arms are drawn out to the arc.
      for (const [line, angle] of [
        [firstLine, start],
        [secondLine, start + sweep],
      ] as const) {
        if (isAxis(line)) {
          lines.push([vertex, pointOnCircle(vertex, radius, angle)])
        }
      }
      return {
        lines,
        arcs: [{ center: vertex, radius, start, end: start + sweep }],
        arrows: [
          {
            tip: pointOnCircle(vertex, radius, start),
            toward: pointOnCircle(vertex, radius, start + Math.min(sweep, 0.2)),
          },
          {
            tip: pointOnCircle(vertex, radius, start + sweep),
            toward: pointOnCircle(vertex, radius, start + sweep - Math.min(sweep, 0.2)),
          },
        ],
      }
    }
    default:
      return EMPTY
  }
}
