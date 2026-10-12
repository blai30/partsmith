import type { SketchConstraint, SketchEntity } from '../generated/protocol'
import {
  circleThroughThreePoints,
  direction,
  normalizeAngle,
  subtract,
  type Vector2,
} from './geometry'

// What each drawing tool adds to a sketch (ui.md 3.16.4): entities and constraints as sketch.md's
// JSON. Recipes are pure; snaps and inference add their constraints separately (3.16.5).

/** Hands out fresh entity ('e') and constraint ('c') ids. */
export type IdSource = { next(prefix: 'e' | 'c'): string }

/** Entities and constraints a tool adds to the sketch in one commit. */
export type SketchAddition = {
  readonly entities: readonly SketchEntity[]
  readonly constraints: readonly SketchConstraint[]
}

/** Above this radius (mm) three points count as in a line for the 3-point arc (3.16.4). */
export const MAXIMUM_ARC_RADIUS = 1e6

/** sketch.md omits `construction` when false. */
function constructionFlag(construction: boolean): { construction?: true } {
  return construction ? { construction: true } : {}
}

function line(
  id: string,
  start: Vector2,
  end: Vector2,
  construction: boolean,
): Extract<SketchEntity, { type: 'line' }> {
  return {
    id,
    type: 'line',
    start: [start[0], start[1]],
    end: [end[0], end[1]],
    ...constructionFlag(construction),
  }
}

function coincident(ids: IdSource, first: string, second: string): SketchConstraint {
  return { id: ids.next('c'), type: 'coincident', points: [first, second] }
}

/** A line from `start` to `end`; null when the ends are equal. */
export function lineRecipe(input: {
  start: Vector2
  end: Vector2
  construction: boolean
  ids: IdSource
}): SketchAddition | null {
  if (input.start[0] === input.end[0] && input.start[1] === input.end[1]) {
    return null
  }
  return {
    entities: [line(input.ids.next('e'), input.start, input.end, input.construction)],
    constraints: [],
  }
}

/**
 * The rectangle with corners `corner` (C1) and `opposite` (C2): lines L1 C1 to (C2.x, C1.y), L2 to
 * C2, L3 to (C1.x, C2.y) and L4 back to C1, joined end to start by coincident constraints, with L1
 * and L3 horizontal and L2 and L4 vertical. Null when the width or height is zero.
 */
export function rectangleTwoPointRecipe(input: {
  corner: Vector2
  opposite: Vector2
  construction: boolean
  ids: IdSource
}): SketchAddition | null {
  const { corner, opposite, construction, ids } = input
  if (corner[0] === opposite[0] || corner[1] === opposite[1]) {
    return null
  }
  const lowerRight: Vector2 = [opposite[0], corner[1]]
  const upperLeft: Vector2 = [corner[0], opposite[1]]
  const first = line(ids.next('e'), corner, lowerRight, construction)
  const second = line(ids.next('e'), lowerRight, opposite, construction)
  const third = line(ids.next('e'), opposite, upperLeft, construction)
  const fourth = line(ids.next('e'), upperLeft, corner, construction)
  const constraints: SketchConstraint[] = [
    coincident(ids, `${first.id}.end`, `${second.id}.start`),
    coincident(ids, `${second.id}.end`, `${third.id}.start`),
    coincident(ids, `${third.id}.end`, `${fourth.id}.start`),
    coincident(ids, `${fourth.id}.end`, `${first.id}.start`),
    { id: ids.next('c'), type: 'horizontal', line: first.id },
    { id: ids.next('c'), type: 'horizontal', line: third.id },
    { id: ids.next('c'), type: 'vertical', line: second.id },
    { id: ids.next('c'), type: 'vertical', line: fourth.id },
  ]
  return { entities: [first, second, third, fourth], constraints }
}

/**
 * The rectangle centered on `center` with a corner at `corner`: the 2-point rectangle from
 * 2 × center - corner to corner, plus a construction diagonal D from L1's start to L2's end and a
 * construction point at the center that is D's midpoint. Null when the width or height is zero.
 */
export function rectangleCenterRecipe(input: {
  center: Vector2
  corner: Vector2
  construction: boolean
  ids: IdSource
}): SketchAddition | null {
  const { center, corner, construction, ids } = input
  const opposite: Vector2 = [2 * center[0] - corner[0], 2 * center[1] - corner[1]]
  const rectangle = rectangleTwoPointRecipe({ corner: opposite, opposite: corner, construction, ids })
  const [first, second] = rectangle?.entities ?? []
  if (rectangle === null || first === undefined || second === undefined) {
    return null
  }
  const diagonal = line(ids.next('e'), opposite, corner, true)
  const centerPoint: SketchEntity = {
    id: ids.next('e'),
    type: 'point',
    position: [center[0], center[1]],
    construction: true,
  }
  return {
    entities: [...rectangle.entities, diagonal, centerPoint],
    constraints: [
      ...rectangle.constraints,
      coincident(ids, `${diagonal.id}.start`, `${first.id}.start`),
      coincident(ids, `${diagonal.id}.end`, `${second.id}.end`),
      { id: ids.next('c'), type: 'midpoint', point: centerPoint.id, line: diagonal.id },
    ],
  }
}

/** A circle; null when the radius is not positive. */
export function circleRecipe(input: {
  center: Vector2
  radius: number
  construction: boolean
  ids: IdSource
}): SketchAddition | null {
  if (!(input.radius > 0)) {
    return null
  }
  return {
    entities: [
      {
        id: input.ids.next('e'),
        type: 'circle',
        center: [input.center[0], input.center[1]],
        radius: input.radius,
        ...constructionFlag(input.construction),
      },
    ],
    constraints: [],
  }
}

/** The counterclockwise angles of the arc through `start`, `through` and `end` on (center), start first. */
export function arcAngles(
  center: Vector2,
  start: Vector2,
  end: Vector2,
  through: Vector2,
): { readonly startAngle: number; readonly endAngle: number; readonly reversed: boolean } {
  const startAngle = normalizeAngle(direction(subtract(start, center)))
  const endAngle = normalizeAngle(direction(subtract(end, center)))
  const throughAngle = normalizeAngle(direction(subtract(through, center)))
  // Arcs are stored counterclockwise: when `through` lies on the clockwise path from start to
  // end, the arc runs from end to start (3.16.4).
  const counterclockwise =
    normalizeAngle(throughAngle - startAngle) < normalizeAngle(endAngle - startAngle)
  return counterclockwise
    ? { startAngle, endAngle, reversed: false }
    : { startAngle: endAngle, endAngle: startAngle, reversed: true }
}

/**
 * The arc from `start` (A) to `end` (B) through `through` (T), stored counterclockwise; null when
 * the three points are in a line or the radius is above 1e6 mm.
 */
export function arcThreePointRecipe(input: {
  start: Vector2
  end: Vector2
  through: Vector2
  construction: boolean
  ids: IdSource
}): SketchAddition | null {
  const circle = circleThroughThreePoints(input.start, input.end, input.through)
  if (circle === null || circle.radius > MAXIMUM_ARC_RADIUS) {
    return null
  }
  const angles = arcAngles(circle.center, input.start, input.end, input.through)
  return {
    entities: [
      {
        id: input.ids.next('e'),
        type: 'arc',
        center: [circle.center[0], circle.center[1]],
        radius: circle.radius,
        start_angle: angles.startAngle,
        end_angle: angles.endAngle,
        ...constructionFlag(input.construction),
      },
    ],
    constraints: [],
  }
}

/** A point entity. */
export function pointRecipe(input: {
  at: Vector2
  construction: boolean
  ids: IdSource
}): SketchAddition {
  return {
    entities: [
      {
        id: input.ids.next('e'),
        type: 'point',
        position: [input.at[0], input.at[1]],
        ...constructionFlag(input.construction),
      },
    ],
    constraints: [],
  }
}
