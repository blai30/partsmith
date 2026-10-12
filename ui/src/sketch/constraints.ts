import type { SketchConstraint, SketchSolution, Units } from '../generated/protocol'
import { formatNumber } from '../expression/format'
import type { SelectionItem } from '../state/store'
import { operandsOf, ownerOf, solvedEntity, type Operand } from './operands'
import type { IdSource } from './recipes'
import { isReservedReference, ORIGIN_POINT } from './refs'

// The constraint tools (ui.md 3.16.6): which constraints a selection allows and the constraint
// records they make, in sketch.md's JSON.

export type ConstraintType =
  | 'coincident'
  | 'point_on_object'
  | 'horizontal'
  | 'vertical'
  | 'parallel'
  | 'perpendicular'
  | 'tangent'
  | 'collinear'
  | 'concentric'
  | 'equal'
  | 'symmetric'
  | 'midpoint'
  | 'fix'

export type DimensionType =
  | 'distance'
  | 'horizontal_distance'
  | 'vertical_distance'
  | 'radius'
  | 'diameter'
  | 'angle'

/** The constraint tools in the order of 3.16.6. */
export const CONSTRAINT_TYPES: readonly ConstraintType[] = [
  'coincident',
  'point_on_object',
  'horizontal',
  'vertical',
  'parallel',
  'perpendicular',
  'tangent',
  'collinear',
  'concentric',
  'equal',
  'symmetric',
  'midpoint',
  'fix',
]

export const DIMENSION_TYPES: readonly DimensionType[] = [
  'distance',
  'horizontal_distance',
  'vertical_distance',
  'radius',
  'diameter',
  'angle',
]

/** The title of each constraint type: its command title and accessible name. */
export const CONSTRAINT_TITLES: Readonly<Record<ConstraintType, string>> = {
  coincident: 'Coincident',
  point_on_object: 'Point on object',
  horizontal: 'Horizontal',
  vertical: 'Vertical',
  parallel: 'Parallel',
  perpendicular: 'Perpendicular',
  tangent: 'Tangent',
  collinear: 'Collinear',
  concentric: 'Concentric',
  equal: 'Equal',
  symmetric: 'Symmetric',
  midpoint: 'Midpoint',
  fix: 'Fix',
}

export const DIMENSION_TITLES: Readonly<Record<DimensionType, string>> = {
  distance: 'Distance',
  horizontal_distance: 'Horizontal distance',
  vertical_distance: 'Vertical distance',
  radius: 'Radius',
  diameter: 'Diameter',
  angle: 'Angle',
}

const REQUIREMENTS: Readonly<Record<ConstraintType, string>> = {
  coincident: 'Select two points, or a point and a curve.',
  point_on_object: 'Select a point and a curve.',
  horizontal: 'Select a line or two points.',
  vertical: 'Select a line or two points.',
  parallel: 'Select two lines.',
  perpendicular: 'Select two lines.',
  tangent: 'Select a line and a circle or arc, or two circles or arcs.',
  collinear: 'Select two lines.',
  concentric: 'Select two circles or arcs.',
  equal: 'Select two lines, or two circles or arcs.',
  symmetric: 'Select two points and a line.',
  midpoint: 'Select a point and a line.',
  fix: 'Select points or curves.',
}

export function isConstraintType(value: string): value is ConstraintType {
  return (CONSTRAINT_TYPES as readonly string[]).includes(value)
}

export function isDimensionType(value: string): value is DimensionType {
  return (DIMENSION_TYPES as readonly string[]).includes(value)
}

/** What a constraint tool asks for while it waits for picks (3.16.6). */
export function constraintRequirement(type: ConstraintType): string {
  return REQUIREMENTS[type]
}

type Point = Extract<Operand, { kind: 'point' }>
type Line = Extract<Operand, { kind: 'line' }>
type Curve = Extract<Operand, { kind: 'circle' | 'arc' }>

/** A selection's operands sorted by kind, in selection order within a kind. */
type Sorted = {
  readonly count: number
  readonly points: readonly Point[]
  readonly lines: readonly Line[]
  readonly round: readonly Curve[]
}

function sortOperands(operands: readonly Operand[]): Sorted {
  return {
    count: operands.length,
    points: operands.filter((operand): operand is Point => operand.kind === 'point'),
    lines: operands.filter((operand): operand is Line => operand.kind === 'line'),
    round: operands.filter(
      (operand): operand is Curve => operand.kind === 'circle' || operand.kind === 'arc',
    ),
  }
}

/** Exactly `points` points, `lines` lines and `round` circles or arcs. */
function shaped(sorted: Sorted, points: number, lines: number, round: number): boolean {
  return (
    sorted.points.length === points &&
    sorted.lines.length === lines &&
    sorted.round.length === round &&
    sorted.count === points + lines + round
  )
}

function curveId(operand: Line | Curve): string {
  return operand.kind === 'line' ? operand.line : operand.curve
}

/** The `type` constraint on these operands with this id, or null when they are not its operand set. */
function build(type: ConstraintType, sorted: Sorted, id: string): SketchConstraint | null {
  const [firstPoint, secondPoint] = sorted.points
  const [firstLine, secondLine] = sorted.lines
  const [firstRound] = sorted.round
  switch (type) {
    case 'coincident':
      return shaped(sorted, 2, 0, 0) &&
        firstPoint !== undefined &&
        secondPoint !== undefined &&
        firstPoint.owner !== secondPoint.owner
        ? { id, type, points: [firstPoint.point, secondPoint.point] }
        : null
    case 'point_on_object': {
      const curve = shaped(sorted, 1, 1, 0)
        ? firstLine
        : shaped(sorted, 1, 0, 1)
          ? firstRound
          : null
      return firstPoint !== undefined &&
        curve !== null &&
        curve !== undefined &&
        firstPoint.owner !== curveId(curve)
        ? { id, type, point: firstPoint.point, curve: curveId(curve) }
        : null
    }
    case 'horizontal':
    case 'vertical':
      if (shaped(sorted, 0, 1, 0) && firstLine !== undefined && !firstLine.axis) {
        return { id, type, line: firstLine.line }
      }
      return shaped(sorted, 2, 0, 0) &&
        firstPoint !== undefined &&
        secondPoint !== undefined &&
        firstPoint.point !== secondPoint.point
        ? { id, type, points: [firstPoint.point, secondPoint.point] }
        : null
    case 'parallel':
    case 'perpendicular':
    case 'collinear':
      return shaped(sorted, 0, 2, 0) &&
        firstLine !== undefined &&
        secondLine !== undefined &&
        firstLine.line !== secondLine.line
        ? { id, type, lines: [firstLine.line, secondLine.line] }
        : null
    case 'tangent':
      if (shaped(sorted, 0, 1, 1) && firstLine !== undefined && firstRound !== undefined) {
        // The line goes first, which is where sketch.md accepts an axis.
        return { id, type, curves: [firstLine.line, firstRound.curve] }
      }
      return withCurves(id, type, roundPair(sorted))
    case 'concentric':
      return withCurves(id, type, roundPair(sorted))
    case 'equal':
      if (
        shaped(sorted, 0, 2, 0) &&
        firstLine !== undefined &&
        secondLine !== undefined &&
        !firstLine.axis &&
        !secondLine.axis &&
        firstLine.line !== secondLine.line
      ) {
        return { id, type, curves: [firstLine.line, secondLine.line] }
      }
      return withCurves(id, type, roundPair(sorted))
    case 'symmetric':
      return shaped(sorted, 2, 1, 0) &&
        firstPoint !== undefined &&
        secondPoint !== undefined &&
        firstLine !== undefined &&
        firstPoint.point !== secondPoint.point &&
        firstPoint.owner !== firstLine.line &&
        secondPoint.owner !== firstLine.line
        ? { id, type, points: [firstPoint.point, secondPoint.point], line: firstLine.line }
        : null
    case 'midpoint':
      return shaped(sorted, 1, 1, 0) &&
        firstPoint !== undefined &&
        firstLine !== undefined &&
        !firstLine.axis &&
        firstPoint.owner !== firstLine.line
        ? { id, type, point: firstPoint.point, line: firstLine.line }
        : null
    case 'fix':
      return null
  }
}

/** The two circles or arcs of `sorted`, or null when the operands are not exactly two different ones. */
function roundPair(sorted: Sorted): [string, string] | null {
  const [first, second] = sorted.round
  return shaped(sorted, 0, 0, 2) &&
    first !== undefined &&
    second !== undefined &&
    first.curve !== second.curve
    ? [first.curve, second.curve]
    : null
}

function withCurves(
  id: string,
  type: 'tangent' | 'concentric' | 'equal',
  curves: [string, string] | null,
): SketchConstraint | null {
  return curves === null ? null : { id, type, curves }
}

/** The `fix` target of an operand: a point, or the whole entity by its bare id; null for the origin and axes. */
function fixTarget(operand: Operand): string | null {
  switch (operand.kind) {
    case 'point':
      return operand.point === ORIGIN_POINT ? null : operand.point
    case 'line':
      return operand.axis ? null : operand.line
    case 'circle':
    case 'arc':
      return operand.curve
  }
}

/** Whether every entity the operands name is in the solution (a stale selection allows nothing). */
function present(operands: readonly Operand[], solution: SketchSolution): boolean {
  return operands.every((operand) => {
    const reference =
      operand.kind === 'point'
        ? operand.owner
        : operand.kind === 'line'
          ? operand.line
          : operand.curve
    return isReservedReference(reference) || solvedEntity(solution, reference) !== undefined
  })
}

/** The fix targets of a selection, or null when it is not one or more fixable points or curves. */
function fixTargets(operands: readonly Operand[]): string[] | null {
  const targets = operands.flatMap((operand) => {
    const target = fixTarget(operand)
    return target === null ? [] : [target]
  })
  const distinct = new Set(targets)
  return targets.length > 0 &&
    targets.length === operands.length &&
    distinct.size === targets.length
    ? targets
    : null
}

function validOperands(
  selection: readonly SelectionItem[],
  solution: SketchSolution,
): Operand[] | null {
  const operands = operandsOf(selection)
  return operands !== null && present(operands, solution) ? operands : null
}

/**
 * Every constraint whose operand rule the selection satisfies exactly, in the order of 3.16.6; axes
 * count as lines and the origin as a point where sketch.md accepts them.
 */
export function applicableConstraints(
  selection: readonly SelectionItem[],
  solution: SketchSolution,
): readonly ConstraintType[] {
  const operands = validOperands(selection, solution)
  if (operands === null || operands.length === 0) {
    return []
  }
  const sorted = sortOperands(operands)
  return CONSTRAINT_TYPES.filter((type) =>
    type === 'fix' ? fixTargets(operands) !== null : build(type, sorted, '') !== null,
  )
}

/**
 * The constraints that apply `type` to the selection: one, or one `fix` per item; empty when the
 * selection is not a valid operand set. Coincident on a point and a curve makes point on object.
 */
export function constraintsFor(
  type: ConstraintType,
  selection: readonly SelectionItem[],
  solution: SketchSolution,
  ids: IdSource,
): readonly SketchConstraint[] {
  const operands = validOperands(selection, solution)
  if (operands === null || operands.length === 0) {
    return []
  }
  if (type === 'fix') {
    return (fixTargets(operands) ?? []).map((target): SketchConstraint => ({
      id: ids.next('c'),
      type: 'fix',
      target,
    }))
  }
  const sorted = sortOperands(operands)
  // Coincident on a point and a curve is point on object (3.16.6).
  const effective =
    type === 'coincident' && build(type, sorted, '') === null ? 'point_on_object' : type
  const constraint = build(effective, sorted, '')
  return constraint === null ? [] : [{ ...constraint, id: ids.next('c') }]
}

/**
 * The constraint that applies `type` to the selection, or null when the selection is not a valid
 * operand set (or, for `fix`, names more than one item: use `constraintsFor`).
 */
export function constraintFor(
  type: ConstraintType,
  selection: readonly SelectionItem[],
  solution: SketchSolution,
  ids: IdSource,
): SketchConstraint | null {
  const operands = validOperands(selection, solution)
  if (operands === null || (type === 'fix' && operands.length !== 1)) {
    return null
  }
  const [only] = constraintsFor(type, selection, solution, ids)
  return only ?? null
}

/** Whether a constraint is a dimension (it has an expression). */
export function isDimension(
  constraint: SketchConstraint,
): constraint is Extract<SketchConstraint, { expr: string }> {
  return 'expr' in constraint
}

/** Every point reference and entity id a constraint names, in field order. */
export function constraintReferences(constraint: SketchConstraint): readonly string[] {
  const references: string[] = []
  for (const key of ['points', 'point', 'target', 'curve', 'curves', 'line', 'lines'] as const) {
    const value: unknown = Reflect.get(constraint, key)
    if (typeof value === 'string') {
      references.push(value)
    } else if (Array.isArray(value)) {
      for (const item of value) {
        if (typeof item === 'string') {
          references.push(item)
        }
      }
    }
  }
  return references
}

/** The entities a constraint references, without reserved names and duplicates (sketch.md 2.5). */
export function referencedEntities(constraint: SketchConstraint): readonly string[] {
  const owners = constraintReferences(constraint)
    .map(ownerOf)
    .filter((owner) => !isReservedReference(owner))
  return [...new Set(owners)]
}

/**
 * The title of a stored constraint for messages and accessible names: the type title, and for a
 * dimension also its solved value as the label shows it ("Distance 40", "Angle 30°").
 */
export function constraintTitle(
  constraint: SketchConstraint,
  solution: SketchSolution | null,
  units: Units,
): string {
  if (!isDimension(constraint)) {
    return CONSTRAINT_TITLES[constraint.type]
  }
  const title = DIMENSION_TITLES[constraint.type]
  const value = solution?.constraints.find((state) => state.id === constraint.id)?.value
  if (value === undefined || value === null) {
    return `${title} ${constraint.expr}`
  }
  const text =
    constraint.type === 'angle'
      ? `${formatNumber(value, 'angle', units)}°`
      : formatNumber(value, 'length', units)
  return `${title} ${text}`
}

/** Titles joined as a sentence list: "A", "A and B", "A, B and C". */
export function joinTitles(titles: readonly string[]): string {
  if (titles.length <= 1) {
    return titles.join('')
  }
  return `${titles.slice(0, -1).join(', ')} and ${titles.at(-1) ?? ''}`
}
