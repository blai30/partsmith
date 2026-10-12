import { describe, expect, it } from 'vitest'
import type { SketchEntity } from '../generated/protocol'
import { arcEntity, circleEntity, lineEntity, pointEntity, solutionFor } from './fixtures.testing'
import type { Vector2 } from './geometry'
import { infer, type InferenceInput, type InferenceResult } from './inference'
import { ORIGIN_POINT, X_AXIS } from './refs'

const DEGREE = Math.PI / 180
const DIGITS = 9

// 0.1 mm per CSS pixel: snaps to points and midpoints reach 0.8 mm, snaps to curves 0.6 mm.
const MILLIMETERS_PER_PIXEL = 0.1

/** An input for the point tool, with nothing being drawn. */
function snapInput(entities: readonly SketchEntity[], cursor: Vector2): InferenceInput {
  return {
    cursor,
    millimetersPerPixel: MILLIMETERS_PER_PIXEL,
    solution: solutionFor(entities),
    anchor: null,
    tool: 'point',
    suppressed: false,
  }
}

/** An input for the line tool, drawing from `start` toward `cursor`. */
function lineInput(
  entities: readonly SketchEntity[],
  start: { readonly at: Vector2; readonly point: string | null },
  cursor: Vector2,
): InferenceInput {
  return {
    cursor,
    millimetersPerPixel: MILLIMETERS_PER_PIXEL,
    solution: solutionFor(entities),
    anchor: start,
    tool: 'line',
    suppressed: false,
  }
}

/** The point `length` mm from `start` in the direction `degrees` counterclockwise from +X. */
function along(start: Vector2, degrees: number, length: number): Vector2 {
  return [
    start[0] + length * Math.cos(degrees * DEGREE),
    start[1] + length * Math.sin(degrees * DEGREE),
  ]
}

function expectPoint(actual: Vector2 | undefined, expected: Vector2): void {
  expect(actual?.[0]).toBeCloseTo(expected[0], DIGITS)
  expect(actual?.[1]).toBeCloseTo(expected[1], DIGITS)
}

function expectRawCursor(result: InferenceResult, cursor: Vector2): void {
  expectPoint(result.point, cursor)
  expect(result.snap).toBeNull()
  expect(result.direction).toBeNull()
}

describe('infer: snaps', () => {
  const lonePoint = pointEntity('e-7', [20, 20])

  it('U-56 a point 0.7 mm away snaps to it', () => {
    const result = infer(snapInput([lonePoint], [20.7, 20]))
    expect(result.snap).toMatchObject({ kind: 'point', point: 'e-7' })
    expectPoint(result.snap?.at, [20, 20])
    expectPoint(result.point, [20, 20])
  })

  it('U-56 a point 0.9 mm away does not snap', () => {
    expectRawCursor(infer(snapInput([lonePoint], [20.9, 20])), [20.9, 20])
  })

  it('U-56 a line midpoint 0.5 mm away snaps to the midpoint', () => {
    const line = lineEntity('e-1', [30, 30], [40, 30])
    const result = infer(snapInput([line], [35, 30.5]))
    expect(result.snap).toMatchObject({ kind: 'midpoint', line: 'e-1' })
    expectPoint(result.snap?.at, [35, 30])
    expectPoint(result.point, [35, 30])
  })

  it('U-56 a curve 0.5 mm away snaps to its closest point', () => {
    const circle = circleEntity('e-2', [60, 60], 5)
    const result = infer(snapInput([circle], [65.5, 60]))
    expect(result.snap).toMatchObject({ kind: 'curve', entity: 'e-2' })
    expectPoint(result.snap?.at, [65, 60])
    expectPoint(result.point, [65, 60])
  })

  it('U-56 a point and a midpoint both in range pick the point, even when the midpoint is nearer', () => {
    const line = lineEntity('e-1', [30, 30], [40, 30])
    const point = pointEntity('e-8', [35.5, 30.3])
    // The cursor is 0.22 mm from the midpoint (35, 30) and 0.36 mm from the point.
    const result = infer(snapInput([line, point], [35.2, 30.1]))
    expect(result.snap).toMatchObject({ kind: 'point', point: 'e-8' })
    expectPoint(result.point, [35.5, 30.3])
  })

  it('the nearest point wins when two points are in range (3.16.5)', () => {
    const near = pointEntity('e-3', [20.2, 20])
    const far = pointEntity('e-4', [19.5, 20])
    const result = infer(snapInput([far, near], [20.1, 20]))
    expect(result.snap).toMatchObject({ kind: 'point', point: 'e-3' })
  })

  it('the origin point snaps as ORIGIN_POINT (3.16.5)', () => {
    const result = infer(snapInput([], [0.3, 0.2]))
    expect(result.snap).toMatchObject({ kind: 'point', point: ORIGIN_POINT })
    expectPoint(result.point, [0, 0])
  })

  it('an origin axis snaps as a curve named by X_AXIS (3.16.5)', () => {
    const result = infer(snapInput([], [50, 0.4]))
    expect(result.snap).toMatchObject({ kind: 'curve', entity: X_AXIS })
    expectPoint(result.point, [50, 0])
  })
})

describe('infer: direction while drawing a line', () => {
  const start: Vector2 = [100, 100]
  const freeStart = { at: start, point: null }

  it('U-56 a segment at 2.9 degrees is horizontal and its end moves onto the horizontal through the start', () => {
    const cursor = along(start, 2.9, 20)
    const result = infer(lineInput([], freeStart, cursor))
    expect(result.direction).toEqual({ kind: 'horizontal' })
    expect(result.snap).toBeNull()
    expectPoint(result.point, [cursor[0], 100])
  })

  it('U-56 a segment at 3.1 degrees infers nothing', () => {
    const cursor = along(start, 3.1, 20)
    expectRawCursor(infer(lineInput([], freeStart, cursor)), cursor)
  })

  it('U-56 a segment at 88 degrees is vertical and its end moves onto the vertical through the start', () => {
    const cursor = along(start, 88, 20)
    const result = infer(lineInput([], freeStart, cursor))
    expect(result.direction).toEqual({ kind: 'vertical' })
    expectPoint(result.point, [100, cursor[1]])
  })

  it('a segment at 182 degrees is horizontal too, since the angle counts modulo 180 (3.16.5)', () => {
    const cursor = along(start, 182, 20)
    const result = infer(lineInput([], freeStart, cursor))
    expect(result.direction).toEqual({ kind: 'horizontal' })
    expectPoint(result.point, [cursor[0], 100])
  })

  it('U-56 a segment from an arc end within 5 degrees of its tangent is tangent, not horizontal', () => {
    // The arc runs counterclockwise from (210, 190) to (200, 200), where its direction is -X (180 degrees).
    const arc = arcEntity('e-5', [200, 190], 10, 0, Math.PI / 2)
    const arcEnd: Vector2 = [200, 200]
    // At 178 degrees the segment is 2 degrees from the tangent and 2 degrees from horizontal.
    const result = infer(lineInput([arc], { at: arcEnd, point: 'e-5.end' }, along(arcEnd, 178, 20)))
    expect(result.direction).toEqual({ kind: 'tangent', entity: 'e-5' })
  })

  it('U-56 suppressed returns the raw point and no inference', () => {
    const lonePoint = pointEntity('e-7', [120, 101])
    // Without suppression this cursor would snap to e-7 and the segment (1.4 degrees) would be horizontal.
    const cursor: Vector2 = [120.5, 100.5]
    const input: InferenceInput = { ...lineInput([lonePoint], freeStart, cursor), suppressed: true }
    expectRawCursor(infer(input), cursor)
  })
})
