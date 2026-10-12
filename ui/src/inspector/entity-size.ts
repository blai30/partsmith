import { formatNumber } from '../expression/format'
import type { SketchConstraint, SketchParams, SketchSolution, Units } from '../generated/protocol'
import { dimensionFor } from '../sketch/dimensions'
import type { Vector2 } from '../sketch/geometry'
import { curveCircle, lineEnds } from '../sketch/operands'
import type { IdSource } from '../sketch/recipes'
import type { SelectionItem } from '../state/store'

// The size field of the selection inspector (ui.md 3.8): one selected line, circle or arc shows
// its length, diameter or radius as an expression, so a shape's size can be typed after selecting
// it. Its text is the driving dimension already on the entity, else the solved size.

/** The dimension type that sets an entity's size. */
type SizeType = 'distance' | 'diameter' | 'radius'

/** The size field of one selected entity. */
export type EntitySize = {
  readonly entity: string
  readonly label: 'Length' | 'Diameter' | 'Radius'
  readonly type: SizeType
  /** The field's text: the driving dimension's expression, else `formatNumber` of the solved size. */
  readonly text: string
  /** The driving dimension already on the entity, which a change replaces; null without one. */
  readonly dimension: SketchConstraint | null
  /** Where the entity is, which orders a new dimension's operands (any point of it will do). */
  readonly at: Vector2
}

function isDriving(constraint: SketchConstraint): boolean {
  return !('driving' in constraint) || constraint.driving !== false
}

/** A driving distance on the line, or on its two end points in either order. */
function lineDimension(
  constraints: readonly SketchConstraint[],
  line: string,
): SketchConstraint | null {
  const ends = new Set([`${line}.start`, `${line}.end`])
  return (
    constraints.find(
      (constraint) =>
        constraint.type === 'distance' &&
        isDriving(constraint) &&
        (('line' in constraint && !('point' in constraint) && constraint.line === line) ||
          ('points' in constraint &&
            constraint.points[0] !== constraint.points[1] &&
            ends.has(constraint.points[0]) &&
            ends.has(constraint.points[1]))),
    ) ?? null
  )
}

/** A driving diameter or radius on the circle or arc. */
function roundDimension(
  constraints: readonly SketchConstraint[],
  curve: string,
): SketchConstraint | null {
  return (
    constraints.find(
      (constraint) =>
        (constraint.type === 'diameter' || constraint.type === 'radius') &&
        isDriving(constraint) &&
        constraint.curve === curve,
    ) ?? null
  )
}

function expressionOf(constraint: SketchConstraint): string | null {
  return 'expr' in constraint ? constraint.expr : null
}

/** The size field of the sketch selection: one line, circle or arc; null for anything else. */
export function entitySize(
  items: readonly SelectionItem[],
  params: SketchParams,
  solution: SketchSolution,
  units: Units,
): EntitySize | null {
  const [only] = items
  const reference = only?.sketchItem
  if (items.length !== 1 || reference?.kind !== 'entity') {
    return null
  }
  const constraints = params.constraints ?? []
  const { entity, entityType } = reference
  if (entityType === 'line') {
    const ends = lineEnds(solution, entity)
    if (ends === null) {
      return null
    }
    const dimension = lineDimension(constraints, entity)
    const length = Math.hypot(ends.end[0] - ends.start[0], ends.end[1] - ends.start[1])
    return {
      entity,
      label: 'Length',
      type: 'distance',
      text:
        (dimension === null ? null : expressionOf(dimension)) ??
        formatNumber(length, 'length', units),
      dimension,
      at: ends.start,
    }
  }
  if (entityType === 'circle' || entityType === 'arc') {
    const circle = curveCircle(solution, entity)
    if (circle === null) {
      return null
    }
    const dimension = roundDimension(constraints, entity)
    // A stored dimension keeps its kind, so a circle with a radius dimension shows that radius.
    const type: SizeType =
      dimension?.type === 'diameter' || dimension?.type === 'radius'
        ? dimension.type
        : entityType === 'circle'
          ? 'diameter'
          : 'radius'
    return {
      entity,
      label: type === 'diameter' ? 'Diameter' : 'Radius',
      type,
      text:
        (dimension === null ? null : expressionOf(dimension)) ??
        formatNumber(type === 'diameter' ? 2 * circle.radius : circle.radius, 'length', units),
      dimension,
      at: circle.center,
    }
  }
  return null
}

/**
 * The dimension a typed size becomes (3.8): the driving dimension with the new expression, or a new
 * driving dimension (`distance` for a line, `diameter` for a circle, `radius` for an arc) whose
 * label sits at `defaultLabelPosition`, since the sketch stores no label positions. With
 * `reference`, a new dimension only measures.
 */
export function entitySizeDimension(
  size: EntitySize,
  item: SelectionItem,
  expression: string,
  solution: SketchSolution,
  ids: IdSource,
  options: { readonly reference?: boolean } = {},
): SketchConstraint {
  if (size.dimension !== null && 'expr' in size.dimension) {
    return { ...size.dimension, expr: expression }
  }
  return dimensionFor(
    size.type,
    [item],
    expression,
    options.reference !== true,
    size.at,
    ids,
    solution,
  )
}
