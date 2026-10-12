import type { PointRef, SketchConstraint, SketchSolution } from '../generated/protocol'
import type { ConstraintType } from './constraints'
import {
  add,
  circleThroughThreePoints,
  distance,
  dot,
  length,
  midpoint,
  normalizeAngle,
  pointOnCircle,
  scale,
  subtract,
  unitAt,
  type Vector2,
} from './geometry'
import {
  infer,
  inferTangent,
  type DirectionInference,
  type InferenceResult,
  type SnapTarget,
} from './inference'
import { solvedEntity } from './operands'
import {
  arcAngles,
  arcThreePointRecipe,
  circleRecipe,
  lineRecipe,
  MAXIMUM_ARC_RADIUS,
  pointRecipe,
  rectangleCenterRecipe,
  rectangleTwoPointRecipe,
  type IdSource,
  type SketchAddition,
} from './recipes'
import { X_AXIS } from './refs'
import type { SketchChange } from './session'

// The drawing tools as a state machine (ui.md 3.16.4, 3.16.5): what the rubber band shows for the
// pointer and typed values, and the change each click or Enter commits.

export type DrawingTool =
  | 'line'
  | 'rectangle_2point'
  | 'rectangle_center'
  | 'circle'
  | 'arc_3point'
  | 'point'

/** The quantities of dimension as you draw, named by their draw inputs' accessible names. */
export type DrawFieldName = 'Length' | 'Angle' | 'Width' | 'Height' | 'Diameter' | 'Radius'

export type DrawField = { readonly name: DrawFieldName; readonly unit: 'length' | 'angle' }

/** A valid typed value: the expression text and its value in mm or rad. */
export type TypedValue = { readonly text: string; readonly value: number }

export type TypedValues = Readonly<Partial<Record<DrawFieldName, TypedValue>>>

/** A clicked point and what it snapped to. */
export type Placed = { readonly at: Vector2; readonly snap: SnapTarget | null }

export type DrawState =
  | { readonly stage: 'idle' }
  | {
      readonly stage: 'line'
      readonly start: Placed
      /** The end of the previous segment of the chain, which the new segment's start joins. */
      readonly previousEnd: PointRef | null
      /** The chain's first point, where a click closes the chain. */
      readonly chainStart: Vector2
    }
  | { readonly stage: 'second'; readonly first: Placed }
  | { readonly stage: 'third'; readonly first: Placed; readonly second: Placed }

export const IDLE: DrawState = { stage: 'idle' }

export type DrawInput = {
  readonly cursor: Vector2
  /** Sketch millimeters per CSS pixel at the cursor. */
  readonly millimetersPerPixel: number
  /** The UI scale; inference distances are CSS pixels times the UI scale (3.16.5). */
  readonly uiScale: number
  /** Whether inference is suppressed (Ctrl, or Cmd on macOS, held). */
  readonly suppressed: boolean
  readonly solution: SketchSolution
  readonly typed: TypedValues
}

/** Shapes of the rubber band in sketch coordinates. */
export type PreviewShape =
  | { readonly kind: 'line'; readonly start: Vector2; readonly end: Vector2 }
  | { readonly kind: 'circle'; readonly center: Vector2; readonly radius: number }
  | {
      readonly kind: 'arc'
      readonly center: Vector2
      readonly radius: number
      readonly start: number
      readonly end: number
    }

export type DrawPreview = {
  readonly shapes: readonly PreviewShape[]
  /** The snap target to highlight, or null. */
  readonly snap: SnapTarget | null
  /** One inference glyph per inferred constraint: snap first, then direction (3.16.5). */
  readonly glyphs: readonly ConstraintType[]
  /** The sketch hint for this state, or null. */
  readonly hint: string | null
  /** Where the draw inputs sit (the rubber band's midpoint), or null without draw inputs. */
  readonly inputsAt: Vector2 | null
  /** The rubber band's current quantities, shown in empty draw inputs. */
  readonly measures: Readonly<Partial<Record<DrawFieldName, number>>>
}

export type DrawCommit = { readonly label: string; readonly change: SketchChange }

export type DrawStep = { readonly state: DrawState; readonly commit: DrawCommit | null }

/** Rubber bands shorter than this many CSS pixels are ignored (3.16.4). */
const MINIMUM_SIZE_PIXELS = 2
/** Typed angles within this of a multiple of 90 degrees become horizontal or vertical. */
const ANGLE_TOLERANCE = 1e-9

export const ARC_COLLINEAR_HINT =
  'These three points are in a line. Move the last point off the line to make an arc.'

const LABELS: Readonly<Record<DrawingTool, string>> = {
  line: 'Line',
  rectangle_2point: 'Rectangle',
  rectangle_center: 'Rectangle',
  circle: 'Circle',
  arc_3point: 'Arc',
  point: 'Point',
}

/** The draw inputs of a tool in a state (3.16.4), empty when the shape is not rubber-banding. */
export function drawFields(tool: DrawingTool, state: DrawState): readonly DrawField[] {
  switch (tool) {
    case 'line':
      return state.stage === 'line'
        ? [
            { name: 'Length', unit: 'length' },
            { name: 'Angle', unit: 'angle' },
          ]
        : []
    case 'rectangle_2point':
    case 'rectangle_center':
      return state.stage === 'second'
        ? [
            { name: 'Width', unit: 'length' },
            { name: 'Height', unit: 'length' },
          ]
        : []
    case 'circle':
      return state.stage === 'second' ? [{ name: 'Diameter', unit: 'length' }] : []
    case 'arc_3point':
      return state.stage === 'third' ? [{ name: 'Radius', unit: 'length' }] : []
    case 'point':
      return []
  }
}

/** Inference at the cursor; `anchor` is the start of a line segment. */
function inferAt(
  input: DrawInput,
  tool: DrawingTool,
  anchor: { readonly at: Vector2; readonly point: PointRef | null } | null,
): InferenceResult {
  return infer({
    cursor: input.cursor,
    millimetersPerPixel: input.millimetersPerPixel * input.uiScale,
    solution: input.solution,
    anchor,
    tool,
    suppressed: input.suppressed,
  })
}

/** The point reference a snap names when it is a point, for chaining and tangency. */
function snappedPoint(snap: SnapTarget | null): PointRef | null {
  return snap?.kind === 'point' ? snap.point : null
}

/** The inferred constraint a snap adds at a new point (3.16.5). */
function snapConstraint(snap: SnapTarget, point: PointRef, ids: IdSource): SketchConstraint {
  switch (snap.kind) {
    case 'point':
      return { id: ids.next('c'), type: 'coincident', points: [point, snap.point] }
    case 'midpoint':
      return { id: ids.next('c'), type: 'midpoint', point, line: snap.line }
    case 'curve':
      return { id: ids.next('c'), type: 'point_on_object', point, curve: snap.entity }
  }
}

function snapGlyph(snap: SnapTarget | null): ConstraintType[] {
  switch (snap?.kind) {
    case 'point':
      return ['coincident']
    case 'midpoint':
      return ['midpoint']
    case 'curve':
      return ['point_on_object']
    case undefined:
      return []
  }
}

function directionGlyph(direction: DirectionInference | null): ConstraintType[] {
  return direction === null ? [] : [direction.kind]
}

/** The constraint a direction inference adds to a new line or arc. */
function directionConstraint(
  direction: DirectionInference,
  curve: string,
  curveIsLine: boolean,
  solution: SketchSolution,
  ids: IdSource,
): SketchConstraint {
  switch (direction.kind) {
    case 'horizontal':
      return { id: ids.next('c'), type: 'horizontal', line: curve }
    case 'vertical':
      return { id: ids.next('c'), type: 'vertical', line: curve }
    case 'tangent': {
      // A line goes first in a tangent, as the constraint tools write it.
      const otherIsLine = solvedEntity(solution, direction.entity)?.type === 'line'
      const curves: [string, string] =
        otherIsLine || !curveIsLine ? [direction.entity, curve] : [curve, direction.entity]
      return { id: ids.next('c'), type: 'tangent', curves }
    }
  }
}

/** Ids of the change's added constraints that are not inferred. */
function requestedOf(addition: SketchAddition, inferred: readonly SketchConstraint[]): string[] {
  const inferredIds = new Set(inferred.map((constraint) => constraint.id))
  return addition.constraints
    .map((constraint) => constraint.id)
    .filter((id) => !inferredIds.has(id))
}

/** A commit of a recipe's addition plus inferred and requested extras. */
function commitOf(
  tool: DrawingTool,
  addition: SketchAddition,
  inferred: readonly SketchConstraint[],
  dimensions: readonly SketchConstraint[],
): DrawCommit {
  const constraints = [...addition.constraints, ...inferred, ...dimensions]
  const all: SketchAddition = { entities: addition.entities, constraints }
  return {
    label: LABELS[tool],
    change: {
      add: all,
      requested: requestedOf(all, inferred),
      inferred: inferred.map((constraint) => constraint.id),
    },
  }
}

/** A driving dimension with a typed expression, or none when the value was not typed. */
function typedDimension(
  typed: TypedValue | undefined,
  build: (expr: string, id: string) => SketchConstraint,
  ids: IdSource,
): SketchConstraint[] {
  return typed === undefined ? [] : [build(typed.text, ids.next('c'))]
}

/** The sign of a coordinate difference, with zero counting as positive. */
function sign(value: number): 1 | -1 {
  return value < 0 ? -1 : 1
}

// Line.

type LineEnd = {
  readonly end: Vector2
  readonly snap: SnapTarget | null
  readonly direction: DirectionInference | null
}

/** The end of the segment being drawn from the cursor, inference and typed values (3.16.4). */
function lineEnd(state: Extract<DrawState, { stage: 'line' }>, input: DrawInput): LineEnd {
  const start = state.start.at
  const anchorPoint = state.previousEnd ?? snappedPoint(state.start.snap)
  const inferred = inferAt(input, 'line', { at: start, point: anchorPoint })
  const { Length: typedLength, Angle: typedAngle } = input.typed
  if (typedAngle !== undefined) {
    // A typed angle replaces direction inference; the length follows the cursor along it.
    const along = unitAt(typedAngle.value)
    const size = typedLength?.value ?? dot(subtract(input.cursor, start), along)
    return { end: add(start, scale(along, size)), snap: null, direction: null }
  }
  if (typedLength !== undefined) {
    const offset = subtract(inferred.point, start)
    const size = length(offset)
    const along = size === 0 ? ([1, 0] as Vector2) : scale(offset, 1 / size)
    return {
      end: add(start, scale(along, typedLength.value)),
      snap: null,
      direction: inferred.direction,
    }
  }
  return { end: inferred.point, snap: inferred.snap, direction: inferred.direction }
}

function linePreview(state: Extract<DrawState, { stage: 'line' }>, input: DrawInput): DrawPreview {
  const { end, snap, direction } = lineEnd(state, input)
  const offset = subtract(end, state.start.at)
  return {
    shapes: [{ kind: 'line', start: state.start.at, end }],
    snap,
    glyphs: [...snapGlyph(snap), ...directionGlyph(direction)],
    hint: null,
    inputsAt: midpoint(state.start.at, end),
    measures: {
      Length: length(offset),
      Angle: normalizeAngle(Math.atan2(offset[1], offset[0])),
    },
  }
}

/** The constraints a typed angle adds to a line (3.16.4). */
function angleConstraint(typed: TypedValue, line: string, id: string): SketchConstraint {
  const halfTurns = typed.value / Math.PI
  const remainder = halfTurns - Math.round(halfTurns)
  if (Math.abs(remainder) <= ANGLE_TOLERANCE) {
    return { id, type: 'horizontal', line }
  }
  const quarter = halfTurns - 0.5
  if (Math.abs(quarter - Math.round(quarter)) <= ANGLE_TOLERANCE) {
    return { id, type: 'vertical', line }
  }
  return { id, type: 'angle', lines: [X_AXIS, line], expr: typed.text }
}

function lineCommit(
  state: Extract<DrawState, { stage: 'line' }>,
  input: DrawInput,
  ids: IdSource,
  construction: boolean,
): DrawStep {
  const { end, snap, direction } = lineEnd(state, input)
  const start = state.start.at
  if (distance(start, end) < MINIMUM_SIZE_PIXELS * input.millimetersPerPixel) {
    return { state, commit: null }
  }
  const addition = lineRecipe({ start, end, construction, ids })
  const line = addition?.entities[0]
  if (addition === null || line === undefined) {
    return { state, commit: null }
  }
  const inferred: SketchConstraint[] = []
  if (state.previousEnd !== null) {
    inferred.push({
      id: ids.next('c'),
      type: 'coincident',
      points: [state.previousEnd, `${line.id}.start`],
    })
  } else if (state.start.snap !== null) {
    inferred.push(snapConstraint(state.start.snap, `${line.id}.start`, ids))
  }
  if (snap !== null) {
    inferred.push(snapConstraint(snap, `${line.id}.end`, ids))
  }
  if (direction !== null) {
    inferred.push(directionConstraint(direction, line.id, true, input.solution, ids))
  }
  const { Length: typedLength, Angle: typedAngle } = input.typed
  const dimensions = [
    ...typedDimension(
      typedLength,
      (expr, id): SketchConstraint => ({ id, type: 'distance', line: line.id, expr }),
      ids,
    ),
    ...(typedAngle === undefined ? [] : [angleConstraint(typedAngle, line.id, ids.next('c'))]),
  ]
  // A click on the chain's first point closes the chain (3.16.4).
  const closes =
    snap?.kind === 'point' &&
    distance(snap.at, state.chainStart) <= 1e-9 &&
    state.previousEnd !== null
  const next: DrawState = closes
    ? IDLE
    : {
        stage: 'line',
        start: { at: end, snap: null },
        previousEnd: `${line.id}.end`,
        chainStart: state.chainStart,
      }
  return { state: next, commit: commitOf('line', addition, inferred, dimensions) }
}

// Rectangles.

/** The second corner (2-point) or corner (center) from the cursor or typed width and height. */
function rectangleCorner(
  tool: 'rectangle_2point' | 'rectangle_center',
  first: Placed,
  input: DrawInput,
): { readonly corner: Vector2; readonly snap: SnapTarget | null } {
  const { Width: typedWidth, Height: typedHeight } = input.typed
  const inferred = inferAt(input, tool, null)
  if (typedWidth === undefined && typedHeight === undefined) {
    return { corner: inferred.point, snap: inferred.snap }
  }
  // Typed sizes take their sign from the cursor's quadrant; the center rectangle's are halved.
  const share = tool === 'rectangle_center' ? 0.5 : 1
  const offset = subtract(input.cursor, first.at)
  const x = typedWidth === undefined ? offset[0] : sign(offset[0]) * typedWidth.value * share
  const y = typedHeight === undefined ? offset[1] : sign(offset[1]) * typedHeight.value * share
  return { corner: add(first.at, [x, y]), snap: null }
}

/** The 2-point rectangle's corners for a first click and a corner. */
function rectangleCorners(
  tool: 'rectangle_2point' | 'rectangle_center',
  first: Vector2,
  corner: Vector2,
): { readonly from: Vector2; readonly to: Vector2 } {
  return tool === 'rectangle_center'
    ? { from: [2 * first[0] - corner[0], 2 * first[1] - corner[1]], to: corner }
    : { from: first, to: corner }
}

function rectangleShapes(from: Vector2, to: Vector2): PreviewShape[] {
  const corners: Vector2[] = [from, [to[0], from[1]], to, [from[0], to[1]]]
  return corners.map((start, index) => ({
    kind: 'line',
    start,
    end: corners[(index + 1) % corners.length] ?? from,
  }))
}

function rectanglePreview(
  tool: 'rectangle_2point' | 'rectangle_center',
  first: Placed,
  input: DrawInput,
): DrawPreview {
  const { corner, snap } = rectangleCorner(tool, first, input)
  const { from, to } = rectangleCorners(tool, first.at, corner)
  const shapes = rectangleShapes(from, to)
  if (tool === 'rectangle_center') {
    shapes.push({ kind: 'line', start: from, end: to })
  }
  return {
    shapes,
    snap,
    glyphs: snapGlyph(snap),
    hint: null,
    inputsAt: midpoint(from, to),
    measures: { Width: Math.abs(to[0] - from[0]), Height: Math.abs(to[1] - from[1]) },
  }
}

function rectangleCommit(
  tool: 'rectangle_2point' | 'rectangle_center',
  first: Placed,
  input: DrawInput,
  ids: IdSource,
  construction: boolean,
): DrawStep {
  const { corner, snap } = rectangleCorner(tool, first, input)
  const { from, to } = rectangleCorners(tool, first.at, corner)
  const minimum = MINIMUM_SIZE_PIXELS * input.millimetersPerPixel
  if (Math.abs(to[0] - from[0]) < minimum || Math.abs(to[1] - from[1]) < minimum) {
    return { state: { stage: 'second', first }, commit: null }
  }
  const addition =
    tool === 'rectangle_center'
      ? rectangleCenterRecipe({ center: first.at, corner, construction, ids })
      : rectangleTwoPointRecipe({ corner: first.at, opposite: corner, construction, ids })
  const [bottom, right, , left, , center] = addition?.entities ?? []
  if (addition === null || bottom === undefined || right === undefined || left === undefined) {
    return { state: { stage: 'second', first }, commit: null }
  }
  const inferred: SketchConstraint[] = []
  if (first.snap !== null) {
    // The first click is C1 (L1's start) or the center point Q.
    const firstPoint = tool === 'rectangle_center' ? center?.id : `${bottom.id}.start`
    if (firstPoint !== undefined) {
      inferred.push(snapConstraint(first.snap, firstPoint, ids))
    }
  }
  if (snap !== null) {
    inferred.push(snapConstraint(snap, `${right.id}.end`, ids))
  }
  const { Width: typedWidth, Height: typedHeight } = input.typed
  const dimensions = [
    ...typedDimension(
      typedWidth,
      (expr, id): SketchConstraint => ({ id, type: 'distance', line: bottom.id, expr }),
      ids,
    ),
    ...typedDimension(
      typedHeight,
      (expr, id): SketchConstraint => ({ id, type: 'distance', line: left.id, expr }),
      ids,
    ),
  ]
  return { state: IDLE, commit: commitOf(tool, addition, inferred, dimensions) }
}

// Circle.

function circleRadius(
  first: Placed,
  input: DrawInput,
): { readonly radius: number; readonly snap: SnapTarget | null } {
  const diameter = input.typed.Diameter
  if (diameter !== undefined) {
    return { radius: diameter.value / 2, snap: null }
  }
  const inferred = inferAt(input, 'circle', null)
  return { radius: distance(first.at, inferred.point), snap: inferred.snap }
}

function circlePreview(first: Placed, input: DrawInput): DrawPreview {
  const { radius, snap } = circleRadius(first, input)
  const toward = subtract(input.cursor, first.at)
  const direction = length(toward) === 0 ? 0 : Math.atan2(toward[1], toward[0])
  // Only a point snap on the rim adds a constraint (point on object).
  const rimSnap = snap?.kind === 'point' ? snap : null
  return {
    shapes: [{ kind: 'circle', center: first.at, radius }],
    snap: rimSnap,
    glyphs: rimSnap === null ? [] : ['point_on_object'],
    hint: null,
    inputsAt: midpoint(first.at, pointOnCircle(first.at, radius, direction)),
    measures: { Diameter: 2 * radius },
  }
}

function circleCommit(
  first: Placed,
  input: DrawInput,
  ids: IdSource,
  construction: boolean,
): DrawStep {
  const { radius, snap } = circleRadius(first, input)
  if (radius < MINIMUM_SIZE_PIXELS * input.millimetersPerPixel) {
    return { state: { stage: 'second', first }, commit: null }
  }
  const addition = circleRecipe({ center: first.at, radius, construction, ids })
  const circle = addition?.entities[0]
  if (addition === null || circle === undefined) {
    return { state: { stage: 'second', first }, commit: null }
  }
  const inferred: SketchConstraint[] = []
  if (first.snap !== null) {
    inferred.push(snapConstraint(first.snap, `${circle.id}.center`, ids))
  }
  if (snap?.kind === 'point') {
    inferred.push({
      id: ids.next('c'),
      type: 'point_on_object',
      point: snap.point,
      curve: circle.id,
    })
  }
  const dimensions = typedDimension(
    input.typed.Diameter,
    (expr, id): SketchConstraint => ({ id, type: 'diameter', curve: circle.id, expr }),
    ids,
  )
  return { state: IDLE, commit: commitOf('circle', addition, inferred, dimensions) }
}

// 3-point arc.

/** The third point: the cursor, or the arc's middle for a typed radius on the cursor's side. */
function arcThrough(first: Placed, second: Placed, input: DrawInput): Vector2 | null {
  const radius = input.typed.Radius
  if (radius === undefined) {
    return input.cursor
  }
  const chord = subtract(second.at, first.at)
  const half = length(chord) / 2
  if (!(radius.value >= half) || half === 0) {
    return null
  }
  const middle = midpoint(first.at, second.at)
  const normal: Vector2 = [-chord[1] / (2 * half), chord[0] / (2 * half)]
  // The arc bulges toward the cursor: its center lies on the other side of the chord.
  const side = dot(subtract(input.cursor, middle), normal) >= 0 ? 1 : -1
  const centerOffset = Math.sqrt(radius.value * radius.value - half * half)
  const center = add(middle, scale(normal, -side * centerOffset))
  return add(center, scale(normal, side * radius.value))
}

type ArcGeometry = {
  readonly center: Vector2
  readonly radius: number
  readonly startAngle: number
  readonly endAngle: number
  readonly reversed: boolean
}

function arcGeometry(first: Placed, second: Placed, through: Vector2 | null): ArcGeometry | null {
  if (through === null) {
    return null
  }
  const circle = circleThroughThreePoints(first.at, second.at, through)
  if (circle === null || circle.radius > MAXIMUM_ARC_RADIUS) {
    return null
  }
  const angles = arcAngles(circle.center, first.at, second.at, through)
  return { center: circle.center, radius: circle.radius, ...angles }
}

/** The tangent inference at A: the new arc leaves A along the tangent of the curve ending there. */
function arcTangent(
  first: Placed,
  arc: ArcGeometry,
  solution: SketchSolution,
): DirectionInference | null {
  const radial = subtract(first.at, arc.center)
  return inferTangent(solution, snappedPoint(first.snap), [-radial[1], radial[0]], true)
}

function arcPreview(state: DrawState, input: DrawInput): DrawPreview {
  if (state.stage === 'second') {
    const { point, snap } = inferAt(input, 'arc_3point', null)
    return {
      shapes: [{ kind: 'line', start: state.first.at, end: point }],
      snap,
      glyphs: snapGlyph(snap),
      hint: null,
      inputsAt: null,
      measures: {},
    }
  }
  if (state.stage !== 'third') {
    return idlePreview('arc_3point', input)
  }
  const arc = arcGeometry(state.first, state.second, arcThrough(state.first, state.second, input))
  if (arc === null) {
    return {
      shapes: [{ kind: 'line', start: state.first.at, end: state.second.at }],
      snap: null,
      glyphs: [],
      hint: ARC_COLLINEAR_HINT,
      inputsAt: midpoint(state.first.at, state.second.at),
      measures: {},
    }
  }
  const sweep = normalizeAngle(arc.endAngle - arc.startAngle)
  const tangent = arcTangent(state.first, arc, input.solution)
  return {
    shapes: [
      {
        kind: 'arc',
        center: arc.center,
        radius: arc.radius,
        start: arc.startAngle,
        end: arc.startAngle + sweep,
      },
    ],
    snap: null,
    glyphs: directionGlyph(tangent),
    hint: null,
    inputsAt: pointOnCircle(arc.center, arc.radius, arc.startAngle + sweep / 2),
    measures: { Radius: arc.radius },
  }
}

function arcCommit(
  state: Extract<DrawState, { stage: 'third' }>,
  input: DrawInput,
  ids: IdSource,
  construction: boolean,
): DrawStep {
  const through = arcThrough(state.first, state.second, input)
  const arcShape = arcGeometry(state.first, state.second, through)
  if (through === null || arcShape === null) {
    return { state, commit: null }
  }
  const addition = arcThreePointRecipe({
    start: state.first.at,
    end: state.second.at,
    through,
    construction,
    ids,
  })
  const arc = addition?.entities[0]
  if (addition === null || arc === undefined) {
    return { state, commit: null }
  }
  // Stored counterclockwise, so A is the arc's start unless it runs from B to A.
  const [atFirst, atSecond] = arcShape.reversed ? ['end', 'start'] : ['start', 'end']
  const inferred: SketchConstraint[] = []
  if (state.first.snap !== null) {
    inferred.push(snapConstraint(state.first.snap, `${arc.id}.${atFirst}`, ids))
  }
  if (state.second.snap !== null) {
    inferred.push(snapConstraint(state.second.snap, `${arc.id}.${atSecond}`, ids))
  }
  const tangent = arcTangent(state.first, arcShape, input.solution)
  if (tangent !== null) {
    inferred.push(directionConstraint(tangent, arc.id, false, input.solution, ids))
  }
  const dimensions = typedDimension(
    input.typed.Radius,
    (expr, id): SketchConstraint => ({ id, type: 'radius', curve: arc.id, expr }),
    ids,
  )
  return { state: IDLE, commit: commitOf('arc_3point', addition, inferred, dimensions) }
}

// Every tool.

/** Before the first click: the snap the click would take. */
function idlePreview(tool: DrawingTool, input: DrawInput): DrawPreview {
  const { snap } = inferAt(input, tool, null)
  return { shapes: [], snap, glyphs: snapGlyph(snap), hint: null, inputsAt: null, measures: {} }
}

/** The rubber band after the first click of a two-click tool. */
function secondPreview(
  tool: DrawingTool,
  state: Extract<DrawState, { stage: 'second' }>,
  input: DrawInput,
): DrawPreview {
  switch (tool) {
    case 'rectangle_2point':
    case 'rectangle_center':
      return rectanglePreview(tool, state.first, input)
    case 'circle':
      return circlePreview(state.first, input)
    case 'arc_3point':
      return arcPreview(state, input)
    case 'line':
    case 'point':
      return idlePreview(tool, input)
  }
}

/** What the rubber band shows for the pointer and typed values. */
export function drawPreview(tool: DrawingTool, state: DrawState, input: DrawInput): DrawPreview {
  switch (state.stage) {
    case 'idle':
      return idlePreview(tool, input)
    case 'line':
      return linePreview(state, input)
    case 'second':
      return secondPreview(tool, state, input)
    case 'third':
      return arcPreview(state, input)
  }
}

/** The first click: where the shape starts, with its snap. */
function firstClick(tool: DrawingTool, input: DrawInput): Placed {
  const { point, snap } = inferAt(input, tool, null)
  return { at: point, snap }
}

/**
 * A click with a drawing tool: the next state, and the change to commit when the click finishes a
 * shape. Typed values are used as Enter uses them.
 */
export function drawClick(
  tool: DrawingTool,
  state: DrawState,
  input: DrawInput,
  ids: IdSource,
  construction: boolean,
): DrawStep {
  switch (state.stage) {
    case 'idle': {
      const placed = firstClick(tool, input)
      if (tool === 'point') {
        const addition = pointRecipe({ at: placed.at, construction, ids })
        const point = addition.entities[0]
        const inferred =
          placed.snap === null || point === undefined
            ? []
            : [snapConstraint(placed.snap, point.id, ids)]
        return { state: IDLE, commit: commitOf('point', addition, inferred, []) }
      }
      if (tool === 'line') {
        return {
          state: { stage: 'line', start: placed, previousEnd: null, chainStart: placed.at },
          commit: null,
        }
      }
      return { state: { stage: 'second', first: placed }, commit: null }
    }
    case 'line':
      return lineCommit(state, input, ids, construction)
    case 'second':
      return secondClick(tool, state, input, ids, construction)
    case 'third':
      return arcCommit(state, input, ids, construction)
  }
}

/** The second click of a two-click tool, or the arc's second point. */
function secondClick(
  tool: DrawingTool,
  state: Extract<DrawState, { stage: 'second' }>,
  input: DrawInput,
  ids: IdSource,
  construction: boolean,
): DrawStep {
  switch (tool) {
    case 'rectangle_2point':
    case 'rectangle_center':
      return rectangleCommit(tool, state.first, input, ids, construction)
    case 'circle':
      return circleCommit(state.first, input, ids, construction)
    case 'arc_3point': {
      const second = firstClick(tool, input)
      if (distance(second.at, state.first.at) < MINIMUM_SIZE_PIXELS * input.millimetersPerPixel) {
        return { state, commit: null }
      }
      return { state: { stage: 'third', first: state.first, second }, commit: null }
    }
    case 'line':
    case 'point':
      return { state: IDLE, commit: null }
  }
}

/** Whether a shape is rubber-banding, so Esc, Enter and typed values apply to it. */
export function isDrawing(state: DrawState): boolean {
  return state.stage !== 'idle'
}
