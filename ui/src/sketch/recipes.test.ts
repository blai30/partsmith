import { describe, expect, it } from 'vitest'
import type { SketchConstraint, SketchEntity } from '../generated/protocol'
import { sequentialIds } from './fixtures.testing'
import { pointOnCircle } from './geometry'
import type { Vector2 } from './geometry'
import {
  arcThreePointRecipe,
  circleRecipe,
  lineRecipe,
  pointRecipe,
  rectangleCenterRecipe,
  rectangleTwoPointRecipe,
  type SketchAddition,
} from './recipes'

/** Coordinates rounded to 9 decimals, with -0 written 0, so computed points compare exactly. */
function rounded(point: readonly number[] | undefined): number[] {
  return (point ?? []).map((value) => Math.round(value * 1e9) / 1e9 + 0)
}

function present(addition: SketchAddition | null): SketchAddition {
  if (addition === null) {
    throw new Error('Expected the recipe to return an addition, but it returned null.')
  }
  return addition
}

function asLine(entity: SketchEntity | undefined) {
  if (entity?.type !== 'line') {
    throw new Error(`Expected a line entity, got ${JSON.stringify(entity)}.`)
  }
  return entity
}

function asPointEntity(entity: SketchEntity | undefined) {
  if (entity?.type !== 'point') {
    throw new Error(`Expected a point entity, got ${JSON.stringify(entity)}.`)
  }
  return entity
}

function asCircle(entity: SketchEntity | undefined) {
  if (entity?.type !== 'circle') {
    throw new Error(`Expected a circle entity, got ${JSON.stringify(entity)}.`)
  }
  return entity
}

function asArc(entity: SketchEntity | undefined) {
  if (entity?.type !== 'arc') {
    throw new Error(`Expected an arc entity, got ${JSON.stringify(entity)}.`)
  }
  return entity
}

function isConstruction(entity: SketchEntity): boolean {
  return entity.construction === true
}

/** The start and end of each line, rounded. */
function segments(entities: readonly SketchEntity[]): number[][][] {
  return entities.map(asLine).map((line) => [rounded(line.start), rounded(line.end)])
}

/** A constraint without its generated id, for comparing operands. */
function withoutId(constraint: SketchConstraint): Record<string, unknown> {
  return Object.fromEntries(Object.entries(constraint).filter(([key]) => key !== 'id'))
}

/** The constraints 3.16.4 lists for a rectangle whose lines L1 to L4 have these ids. */
function rectangleConstraints(lineIds: readonly string[]): Record<string, unknown>[] {
  const [first = '', second = '', third = '', fourth = ''] = lineIds
  return [
    { type: 'coincident', points: [`${first}.end`, `${second}.start`] },
    { type: 'coincident', points: [`${second}.end`, `${third}.start`] },
    { type: 'coincident', points: [`${third}.end`, `${fourth}.start`] },
    { type: 'coincident', points: [`${fourth}.end`, `${first}.start`] },
    { type: 'horizontal', line: first },
    { type: 'horizontal', line: third },
    { type: 'vertical', line: second },
    { type: 'vertical', line: fourth },
  ]
}

/** The 2-point rectangle from (0, 0) to (40, 20). */
function twoPointRectangle(construction: boolean): SketchAddition {
  return present(
    rectangleTwoPointRecipe({
      corner: [0, 0],
      opposite: [40, 20],
      construction,
      ids: sequentialIds(),
    }),
  )
}

/** The center rectangle with center (0, 0) and corner (20, 10), split into its parts. */
function centerRectangle() {
  const addition = present(
    rectangleCenterRecipe({
      center: [0, 0],
      corner: [20, 10],
      construction: false,
      ids: sequentialIds(),
    }),
  )
  const constructionEntities = addition.entities.filter(isConstruction)
  return {
    addition,
    rectangleLines: addition.entities.filter((entity) => !isConstruction(entity)),
    diagonal: asLine(constructionEntities.find((entity) => entity.type === 'line')),
    centerPoint: asPointEntity(constructionEntities.find((entity) => entity.type === 'point')),
  }
}

function arcThrough(start: Vector2, end: Vector2, through: Vector2) {
  const addition = present(
    arcThreePointRecipe({ start, end, through, construction: false, ids: sequentialIds() }),
  )
  return { addition, arc: asArc(addition.entities[0]) }
}

/** The counterclockwise sweep of an arc, in [0, 2π). */
function sweepOf(arc: { readonly start_angle: number; readonly end_angle: number }): number {
  const sweep = (arc.end_angle - arc.start_angle) % (2 * Math.PI)
  return sweep < 0 ? sweep + 2 * Math.PI : sweep
}

/** The start, middle and end points of an arc, rounded. */
function arcPoints(arc: ReturnType<typeof asArc>): number[][] {
  const at = (angle: number) => rounded(pointOnCircle(arc.center, arc.radius, angle))
  return [at(arc.start_angle), at(arc.start_angle + sweepOf(arc) / 2), at(arc.end_angle)]
}

describe('rectangleTwoPointRecipe', () => {
  it('U-50 returns 4 lines in the order and directions of 3.16.4', () => {
    expect(segments(twoPointRectangle(false).entities)).toEqual([
      [
        [0, 0],
        [40, 0],
      ],
      [
        [40, 0],
        [40, 20],
      ],
      [
        [40, 20],
        [0, 20],
      ],
      [
        [0, 20],
        [0, 0],
      ],
    ])
  })

  it('U-50 returns exactly 4 coincident, 2 horizontal and 2 vertical constraints with the listed operands', () => {
    const addition = twoPointRectangle(false)
    const expected = rectangleConstraints(addition.entities.map((entity) => entity.id))
    expect(addition.constraints).toHaveLength(8)
    expect(addition.constraints.map(withoutId)).toEqual(expect.arrayContaining(expected))
  })

  it('U-50 a zero width returns null', () => {
    expect(
      rectangleTwoPointRecipe({
        corner: [0, 0],
        opposite: [0, 20],
        construction: false,
        ids: sequentialIds(),
      }),
    ).toBeNull()
  })

  it('a zero height returns null', () => {
    expect(
      rectangleTwoPointRecipe({
        corner: [0, 0],
        opposite: [40, 0],
        construction: false,
        ids: sequentialIds(),
      }),
    ).toBeNull()
  })

  it('takes entity ids from the e prefix and constraint ids from the c prefix, all distinct', () => {
    const addition = twoPointRectangle(false)
    const entityIds = addition.entities.map((entity) => entity.id)
    const constraintIds = addition.constraints.map((constraint) => constraint.id)
    expect(entityIds.every((id) => id.startsWith('e-'))).toBe(true)
    expect(constraintIds.every((id) => id.startsWith('c-'))).toBe(true)
    expect(new Set([...entityIds, ...constraintIds]).size).toBe(12)
  })

  it('copies the construction flag to the 4 lines', () => {
    expect(twoPointRectangle(true).entities.map(isConstruction)).toEqual([true, true, true, true])
    expect(twoPointRectangle(false).entities.map(isConstruction)).toEqual([
      false,
      false,
      false,
      false,
    ])
  })
})

describe('rectangleCenterRecipe', () => {
  it('U-51 returns 4 lines from (-20, -10), one construction line and one construction point', () => {
    const { addition, rectangleLines } = centerRectangle()
    expect(addition.entities).toHaveLength(6)
    expect(segments(rectangleLines)).toEqual([
      [
        [-20, -10],
        [20, -10],
      ],
      [
        [20, -10],
        [20, 10],
      ],
      [
        [20, 10],
        [-20, 10],
      ],
      [
        [-20, 10],
        [-20, -10],
      ],
    ])
  })

  it('U-51 the construction line runs from (-20, -10) to (20, 10)', () => {
    const { diagonal } = centerRectangle()
    expect(segments([diagonal])).toEqual([
      [
        [-20, -10],
        [20, 10],
      ],
    ])
  })

  it('U-51 the construction point is at the center (0, 0)', () => {
    const { centerPoint } = centerRectangle()
    expect(rounded(centerPoint.position)).toEqual([0, 0])
  })

  it('U-51 returns 11 constraints: the rectangle 8, 2 coincident for the diagonal and 1 midpoint', () => {
    const { addition, rectangleLines, diagonal, centerPoint } = centerRectangle()
    const lineIds = rectangleLines.map((entity) => entity.id)
    const expected = [
      ...rectangleConstraints(lineIds),
      { type: 'coincident', points: [`${diagonal.id}.start`, `${lineIds[0] ?? ''}.start`] },
      { type: 'coincident', points: [`${diagonal.id}.end`, `${lineIds[1] ?? ''}.end`] },
      { type: 'midpoint', point: centerPoint.id, line: diagonal.id },
    ]
    expect(addition.constraints).toHaveLength(11)
    expect(addition.constraints.map(withoutId)).toEqual(expect.arrayContaining(expected))
  })
})

describe('circleRecipe', () => {
  it('U-52 returns one circle and no constraints', () => {
    const addition = present(
      circleRecipe({ center: [5, -3], radius: 7.5, construction: false, ids: sequentialIds() }),
    )
    expect(addition.entities).toHaveLength(1)
    expect(addition.constraints).toHaveLength(0)
    const circle = asCircle(addition.entities[0])
    expect(rounded(circle.center)).toEqual([5, -3])
    expect(circle.radius).toBeCloseTo(7.5, 9)
  })

  it('U-52 radius 0 returns null', () => {
    expect(
      circleRecipe({ center: [5, -3], radius: 0, construction: false, ids: sequentialIds() }),
    ).toBeNull()
  })

  it('copies the construction flag', () => {
    const addition = present(
      circleRecipe({ center: [0, 0], radius: 2, construction: true, ids: sequentialIds() }),
    )
    expect(addition.entities.map(isConstruction)).toEqual([true])
  })
})

describe('pointRecipe', () => {
  it('U-52 returns one point entity at the location and no constraints', () => {
    const addition = pointRecipe({ at: [2, 3], construction: false, ids: sequentialIds() })
    expect(addition.entities).toHaveLength(1)
    expect(addition.constraints).toHaveLength(0)
    expect(rounded(asPointEntity(addition.entities[0]).position)).toEqual([2, 3])
  })

  it('U-52 the point entity has the construction flag as given', () => {
    const regular = pointRecipe({ at: [2, 3], construction: false, ids: sequentialIds() })
    const construction = pointRecipe({ at: [2, 3], construction: true, ids: sequentialIds() })
    expect(regular.entities.map(isConstruction)).toEqual([false])
    expect(construction.entities.map(isConstruction)).toEqual([true])
  })
})

describe('arcThreePointRecipe', () => {
  it('U-53 A (10, 0), B (-10, 0), T (0, 10) gives center (0, 0) and radius 10', () => {
    const { addition, arc } = arcThrough([10, 0], [-10, 0], [0, 10])
    expect(addition.entities).toHaveLength(1)
    expect(addition.constraints).toHaveLength(0)
    expect(rounded(arc.center)).toEqual([0, 0])
    expect(arc.radius).toBeCloseTo(10, 9)
  })

  it('U-53 A (10, 0), B (-10, 0), T (0, 10) runs counterclockwise from (10, 0) through T to (-10, 0)', () => {
    const { arc } = arcThrough([10, 0], [-10, 0], [0, 10])
    expect(sweepOf(arc)).toBeCloseTo(Math.PI, 9)
    expect(arcPoints(arc)).toEqual([
      [10, 0],
      [0, 10],
      [-10, 0],
    ])
  })

  it('U-53 with T (0, -10) the arc runs counterclockwise from (-10, 0) through T to (10, 0)', () => {
    const { arc } = arcThrough([10, 0], [-10, 0], [0, -10])
    expect(rounded(arc.center)).toEqual([0, 0])
    expect(arc.radius).toBeCloseTo(10, 9)
    expect(sweepOf(arc)).toBeCloseTo(Math.PI, 9)
    expect(arcPoints(arc)).toEqual([
      [-10, 0],
      [0, -10],
      [10, 0],
    ])
  })

  it('U-53 collinear points return null', () => {
    expect(
      arcThreePointRecipe({
        start: [0, 0],
        end: [10, 0],
        through: [5, 0],
        construction: false,
        ids: sequentialIds(),
      }),
    ).toBeNull()
  })
})

describe('lineRecipe', () => {
  it('returns one line from start to end and no constraints', () => {
    const addition = present(
      lineRecipe({ start: [1, 2], end: [31, 2], construction: false, ids: sequentialIds() }),
    )
    expect(addition.constraints).toHaveLength(0)
    expect(segments(addition.entities)).toEqual([
      [
        [1, 2],
        [31, 2],
      ],
    ])
  })

  it('U-54 equal ends return null', () => {
    expect(
      lineRecipe({ start: [4, 4], end: [4, 4], construction: false, ids: sequentialIds() }),
    ).toBeNull()
  })

  it('U-54 the construction flag is copied', () => {
    const construction = present(
      lineRecipe({ start: [0, 0], end: [10, 0], construction: true, ids: sequentialIds() }),
    )
    const regular = present(
      lineRecipe({ start: [0, 0], end: [10, 0], construction: false, ids: sequentialIds() }),
    )
    expect(construction.entities.map(isConstruction)).toEqual([true])
    expect(regular.entities.map(isConstruction)).toEqual([false])
  })
})
