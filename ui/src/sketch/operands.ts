import type { PointRef, SketchSolution } from '../generated/protocol'
import type { SelectionItem } from '../state/store'
import { axisLine, isAxis, ORIGIN_POINT } from './refs'
import { arcSweep, closestPointOnArc, closestPointOnSegment, type Vector2 } from './geometry'

// Sketch selection items as constraint and dimension operands, and their solved geometry.

export type SolvedEntity = SketchSolution['entities'][number]

/** What a selection item is to a constraint or dimension; the origin counts as a point, axes as lines. */
export type Operand =
  | { readonly kind: 'point'; readonly point: PointRef; readonly owner: string }
  | { readonly kind: 'line'; readonly line: string; readonly axis: boolean }
  | { readonly kind: 'circle'; readonly curve: string }
  | { readonly kind: 'arc'; readonly curve: string }

/** The entity a point reference belongs to: the id before the role, or the reserved name itself. */
export function ownerOf(point: PointRef): string {
  const dot = point.indexOf('.')
  return dot === -1 ? point : point.slice(0, dot)
}

/** The operand a sketch selection item stands for, or null for constraints, regions and model items. */
export function operandOf(item: SelectionItem): Operand | null {
  const reference = item.sketchItem
  if (reference === null) {
    return null
  }
  switch (reference.kind) {
    case 'point':
      return { kind: 'point', point: reference.point, owner: ownerOf(reference.point) }
    case 'entity':
      switch (reference.entityType) {
        case 'point':
          return { kind: 'point', point: reference.entity, owner: reference.entity }
        case 'line':
          return { kind: 'line', line: reference.entity, axis: false }
        case 'circle':
          return { kind: 'circle', curve: reference.entity }
        case 'arc':
          return { kind: 'arc', curve: reference.entity }
      }
      return null
    case 'origin':
      return reference.which === 'point'
        ? { kind: 'point', point: ORIGIN_POINT, owner: ORIGIN_POINT }
        : { kind: 'line', line: reference.which, axis: true }
    case 'constraint':
    case 'region':
      return null
  }
}

/** The operands of a selection, or null when any item is not a sketch point or curve. */
export function operandsOf(selection: readonly SelectionItem[]): Operand[] | null {
  const operands: Operand[] = []
  for (const item of selection) {
    const operand = operandOf(item)
    if (operand === null) {
      return null
    }
    operands.push(operand)
  }
  return operands
}

/** The entity of a solution with this id. */
export function solvedEntity(solution: SketchSolution, id: string): SolvedEntity | undefined {
  return solution.entities.find((entity) => entity.id === id)
}

/** The solved position of a point reference; the origin is (0, 0). */
export function pointPosition(solution: SketchSolution, point: PointRef): Vector2 | null {
  if (point === ORIGIN_POINT) {
    return [0, 0]
  }
  const solved = solution.points.find((candidate) => candidate.point === point)
  if (solved !== undefined) {
    return solved.at
  }
  // A point entity's own reference is its id; solutions list it, but fall back to the entity.
  const entity = solvedEntity(solution, point)
  return entity?.type === 'point' ? entity.position : null
}

/** The start and end of a line or axis (an axis is its constant unit segment from the origin). */
export function lineEnds(
  solution: SketchSolution,
  line: string,
): { readonly start: Vector2; readonly end: Vector2 } | null {
  if (isAxis(line)) {
    return axisLine(line)
  }
  const entity = solvedEntity(solution, line)
  return entity?.type === 'line' ? { start: entity.start, end: entity.end } : null
}

/** A solved circle or arc. */
export type RoundEntity = Extract<SolvedEntity, { type: 'circle' | 'arc' }>

/** Center, radius and counterclockwise angles of a circle (a full turn) or an arc. */
export type CurveCircle = {
  readonly center: Vector2
  readonly radius: number
  readonly start: number
  readonly end: number
  readonly full: boolean
}

/** The circle of a circle or arc entity, with the angles it runs through. */
export function entityCircle(entity: RoundEntity): CurveCircle {
  if (entity.type === 'circle') {
    return { center: entity.center, radius: entity.radius, start: 0, end: 2 * Math.PI, full: true }
  }
  return {
    center: entity.center,
    radius: entity.radius,
    start: entity.start_angle,
    end: entity.start_angle + arcSweep(entity.start_angle, entity.end_angle),
    full: false,
  }
}

/** The circle of the solution's circle or arc with this id, or null when it is neither. */
export function curveCircle(solution: SketchSolution, curve: string): CurveCircle | null {
  const entity = solvedEntity(solution, curve)
  return entity?.type === 'circle' || entity?.type === 'arc' ? entityCircle(entity) : null
}

/** The closest point of an entity's curve to `cursor`, or null for point entities. */
export function closestOnEntity(entity: SolvedEntity, cursor: Vector2): Vector2 | null {
  switch (entity.type) {
    case 'line':
      return closestPointOnSegment(cursor, entity.start, entity.end)
    case 'circle':
    case 'arc': {
      const circle = entityCircle(entity)
      return closestPointOnArc(cursor, circle.center, circle.radius, circle.start, circle.end)
    }
    case 'point':
      return null
  }
}

/** The kind noun of an operand with its article, for messages: "a point", "an arc". */
export function operandNoun(operand: Operand): string {
  switch (operand.kind) {
    case 'point':
      return 'a point'
    case 'line':
      return 'a line'
    case 'circle':
      return 'a circle'
    case 'arc':
      return 'an arc'
  }
}
