import { describe, expect, it } from 'vitest'
import type { SketchConstraint, SketchEntity } from '../generated/protocol'
import { pointOnCircle, sequentialIds } from './fixtures.testing'
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

const DIGITS = 9

function expectPoint(actual: readonly number[] | undefined, expected: Vector2): void {
  expect(actual).toHaveLength(2)
  expect(actual?.[0]).toBeCloseTo(expected[0], DIGITS)
  expect(actual?.[1]).toBeCloseTo(expected[1], DIGITS)
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

/** A constraint without its generated id, for comparing operands. */
function withoutId(constraint: SketchConstraint): Record<string, unknown> {
  return Object.fromEntries(Object.entries(constraint).filter(([key]) => key !== 'id'))
}

/** Asserts that `constraints` are exactly `expected` (ids ignored), in any order. */
function expectConstraints(
  constraints: readonly SketchConstraint[],
  expected: readonly Record<string, unknown>[],
): void {
  expect(constraints).toHaveLength(expected.length)
  expect(constraints.map(withoutId)).toEqual(expect.arrayContaining([...expected]))
}

/** The constraints 3.16.4 lists for a rectangle with lines `l1` to `l4`. */
function rectangleConstraints(
  l1: string,
  l2: string,
  l3: string,
  l4: string,
): Record<string, unknown>[] {
  return [
    { type: 'coincident', points: [`${l1}.end`, `${l2}.start`] },
    { type: 'coincident', points: [`${l2}.end`, `${l3}.start`] },
    { type: 'coincident', points: [`${l3}.end`, `${l4}.start`] },
    { type: 'coincident', points: [`${l4}.end`, `${l1}.start`] },
    { type: 'horizontal', line: l1 },
    { type: 'horizontal', line: l3 },
    { type: 'vertical', line: l2 },
    { type: 'vertical', line: l4 },
  ]
}

describe('rectangleTwoPointRecipe', () => {
  function rectangle(construction: boolean): SketchAddition {
    return present(
      rectangleTwoPointRecipe({
        corner: [0, 0],
        opposite: [40, 20],
        construction,
        ids: sequentialIds(),
      }),
    )
  }

  it('U-50 returns 4 lines in the order and directions of 3.16.4', () => {
    const addition = rectangle(false)
    expect(addition.entities).toHaveLength(4)
    const [l1, l2, l3, l4] = addition.entities.map(asLine)
    expectPoint(l1?.start, [0, 0])
    expectPoint(l1?.end, [40, 0])
    expectPoint(l2?.start, [40, 0])
    expectPoint(l2?.end, [40, 20])
    expectPoint(l3?.start, [40, 20])
    expectPoint(l3?.end, [0, 20])
    expectPoint(l4?.start, [0, 20])
    expectPoint(l4?.end, [0, 0])
  })

  it('U-50 returns exactly 4 coincident, 2 horizontal and 2 vertical constraints with the listed operands', () => {
    const addition = rectangle(false)
    const [l1, l2, l3, l4] = addition.entities.map((entity) => entity.id)
    expectConstraints(
      addition.constraints,
      rectangleConstraints(l1 ?? '', l2 ?? '', l3 ?? '', l4 ?? ''),
    )
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
    const addition = rectangle(false)
    const entityIds = addition.entities.map((entity) => entity.id)
    const constraintIds = addition.constraints.map((constraint) => constraint.id)
    expect(entityIds.every((id) => id.startsWith('e-'))).toBe(true)
    expect(constraintIds.every((id) => id.startsWith('c-'))).toBe(true)
    expect(new Set([...entityIds, ...constraintIds]).size).toBe(12)
  })

  it('copies the construction flag to the 4 lines', () => {
    expect(rectangle(true).entities.map(isConstruction)).toEqual([true, true, true, true])
    expect(rectangle(false).entities.map(isConstruction)).toEqual([false, false, false, false])
  })
})

describe('rectangleCenterRecipe', () => {
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

  it('U-51 returns 4 lines from (-20, -10), one construction line and one construction point', () => {
    const { addition, rectangleLines } = centerRectangle()
    expect(addition.entities).toHaveLength(6)
    expect(rectangleLines).toHaveLength(4)
    const [l1, l2, l3, l4] = rectangleLines.map(asLine)
    expectPoint(l1?.start, [-20, -10])
    expectPoint(l1?.end, [20, -10])
    expectPoint(l2?.start, [20, -10])
    expectPoint(l2?.end, [20, 10])
    expectPoint(l3?.start, [20, 10])
    expectPoint(l3?.end, [-20, 10])
    expectPoint(l4?.start, [-20, 10])
    expectPoint(l4?.end, [-20, -10])
  })

  it('U-51 the construction line runs from (-20, -10) to (20, 10)', () => {
    const { diagonal } = centerRectangle()
    expectPoint(diagonal.start, [-20, -10])
    expectPoint(diagonal.end, [20, 10])
  })

  it('U-51 the construction point is at the center (0, 0)', () => {
    const { centerPoint } = centerRectangle()
    expectPoint(centerPoint.position, [0, 0])
  })

  it('U-51 returns 11 constraints: the rectangle 8, 2 coincident for the diagonal and 1 midpoint', () => {
    const { addition, rectangleLines, diagonal, centerPoint } = centerRectangle()
    const [l1, l2, l3, l4] = rectangleLines.map((entity) => entity.id)
    expectConstraints(addition.constraints, [
      ...rectangleConstraints(l1 ?? '', l2 ?? '', l3 ?? '', l4 ?? ''),
      { type: 'coincident', points: [`${diagonal.id}.start`, `${l1 ?? ''}.start`] },
      { type: 'coincident', points: [`${diagonal.id}.end`, `${l2 ?? ''}.end`] },
      { type: 'midpoint', point: centerPoint.id, line: diagonal.id },
    ])
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
    expectPoint(circle.center, [5, -3])
    expect(circle.radius).toBeCloseTo(7.5, DIGITS)
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
    expectPoint(asPointEntity(addition.entities[0]).position, [2, 3])
  })

  it('U-52 the point entity has the construction flag as given', () => {
    const regular = pointRecipe({ at: [2, 3], construction: false, ids: sequentialIds() })
    const construction = pointRecipe({ at: [2, 3], construction: true, ids: sequentialIds() })
    expect(regular.entities.map(isConstruction)).toEqual([false])
    expect(construction.entities.map(isConstruction)).toEqual([true])
  })
})

describe('arcThreePointRecipe', () => {
  function arcThrough(start: Vector2, end: Vector2, through: Vector2) {
    const addition = present(
      arcThreePointRecipe({ start, end, through, construction: false, ids: sequentialIds() }),
    )
    expect(addition.entities).toHaveLength(1)
    expect(addition.constraints).toHaveLength(0)
    return asArc(addition.entities[0])
  }

  function sweepOf(arc: { readonly start_angle: number; readonly end_angle: number }): number {
    const sweep = (arc.end_angle - arc.start_angle) % (2 * Math.PI)
    return sweep < 0 ? sweep + 2 * Math.PI : sweep
  }

  it('U-53 A (10, 0), B (-10, 0), T (0, 10) gives center (0, 0) and radius 10', () => {
    const arc = arcThrough([10, 0], [-10, 0], [0, 10])
    expectPoint(arc.center, [0, 0])
    expect(arc.radius).toBeCloseTo(10, DIGITS)
  })

  it('U-53 A (10, 0), B (-10, 0), T (0, 10) runs counterclockwise from (10, 0) to (-10, 0)', () => {
    const arc = arcThrough([10, 0], [-10, 0], [0, 10])
    expectPoint(pointOnCircle(arc.center, arc.radius, arc.start_angle), [10, 0])
    expectPoint(pointOnCircle(arc.center, arc.radius, arc.end_angle), [-10, 0])
    expect(sweepOf(arc)).toBeCloseTo(Math.PI, DIGITS)
    // Halfway along the counterclockwise sweep lies T.
    expectPoint(pointOnCircle(arc.center, arc.radius, arc.start_angle + sweepOf(arc) / 2), [0, 10])
  })

  it('U-53 with T (0, -10) the arc runs from (-10, 0) to (10, 0)', () => {
    const arc = arcThrough([10, 0], [-10, 0], [0, -10])
    expectPoint(arc.center, [0, 0])
    expect(arc.radius).toBeCloseTo(10, DIGITS)
    expectPoint(pointOnCircle(arc.center, arc.radius, arc.start_angle), [-10, 0])
    expectPoint(pointOnCircle(arc.center, arc.radius, arc.end_angle), [10, 0])
    expect(sweepOf(arc)).toBeCloseTo(Math.PI, DIGITS)
    expectPoint(pointOnCircle(arc.center, arc.radius, arc.start_angle + sweepOf(arc) / 2), [0, -10])
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
    expect(addition.entities).toHaveLength(1)
    expect(addition.constraints).toHaveLength(0)
    const line = asLine(addition.entities[0])
    expectPoint(line.start, [1, 2])
    expectPoint(line.end, [31, 2])
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
