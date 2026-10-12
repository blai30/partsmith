import { describe, expect, it } from 'vitest'
import type { BodyPrint } from '../generated/protocol'
import type { Vector3 } from '../viewport/math'
import {
  CIRCLE_SEGMENTS,
  bedOutline,
  bedOutlineSegments,
  bedPolygon,
  inverseAffine,
  type Matrix,
} from './bed'
import { CUBE, bodyPrint, printReport, printState, stateWith } from './fixtures.testing'

// Model to bed: a quarter turn about +Z, then a move by (10, 20, 5).
const TURNED: Matrix = [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 10, 20, 5, 1]

/** Matches a point within 1e-9 on every coordinate. */
function closeTo(expected: Vector3): unknown[] {
  return expected.map((value) => expect.closeTo(value, 9))
}

/** The segments as [start, end] point pairs. */
function segmentsOf(values: Float32Array): [Vector3, Vector3][] {
  const segments: [Vector3, Vector3][] = []
  for (let i = 0; i < values.length; i += 6) {
    const [startX = 0, startY = 0, startZ = 0, endX = 0, endY = 0, endZ = 0] = values.slice(
      i,
      i + 6,
    )
    segments.push([
      [startX, startY, startZ],
      [endX, endY, endZ],
    ])
  }
  return segments
}

function analyzed(transform: Matrix): BodyPrint {
  return { ...bodyPrint({ body: CUBE.id, meshId: 1 }), transform }
}

describe('inverseAffine', () => {
  it('undoes a move', () => {
    const toModel = inverseAffine([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 128, 128, 0, 1])
    expect(toModel?.([128, 128, 0])).toEqual(closeTo([0, 0, 0]))
    expect(toModel?.([0, 0, 10])).toEqual(closeTo([-128, -128, 10]))
  })

  it('undoes a turn followed by a move', () => {
    const toModel = inverseAffine(TURNED)
    // The model point (1, 0, 0) turns to (0, 1, 0) and moves to (10, 21, 5).
    expect(toModel?.([10, 21, 5])).toEqual(closeTo([1, 0, 0]))
    expect(toModel?.([10, 20, 5])).toEqual(closeTo([0, 0, 0]))
  })

  it('is null for a matrix that flattens space', () => {
    expect(inverseAffine([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])).toBeNull()
  })
})

describe('bedPolygon', () => {
  it('outlines a rectangular bed from the origin', () => {
    expect(bedPolygon({ kind: 'rectangle', width: 250, depth: 210, height: 220 })).toEqual([
      [0, 0, 0],
      [250, 0, 0],
      [250, 210, 0],
      [0, 210, 0],
    ])
  })

  it('outlines a round bed around the origin', () => {
    const points = bedPolygon({ kind: 'circle', diameter: 170, height: 240 })
    expect(points).toHaveLength(CIRCLE_SEGMENTS)
    expect(points[0]).toEqual(closeTo([85, 0, 0]))
    for (const [x, y, z] of points) {
      expect(Math.hypot(x, y)).toBeCloseTo(85, 9)
      expect(z).toBe(0)
    }
  })
})

describe('bedOutlineSegments', () => {
  it('maps the bed into model coordinates, closing the outline', () => {
    const bed = { kind: 'rectangle', width: 256, depth: 256, height: 256 } as const
    const body = analyzed([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 128, 128, 0, 1])
    expect(segmentsOf(bedOutlineSegments(bed, body) ?? new Float32Array())).toEqual([
      [
        [-128, -128, 0],
        [128, -128, 0],
      ],
      [
        [128, -128, 0],
        [128, 128, 0],
      ],
      [
        [128, 128, 0],
        [-128, 128, 0],
      ],
      [
        [-128, 128, 0],
        [-128, -128, 0],
      ],
    ])
  })

  it('turns the bed back with the body', () => {
    const bed = { kind: 'rectangle', width: 40, depth: 20, height: 100 } as const
    const [first] = segmentsOf(bedOutlineSegments(bed, analyzed(TURNED)) ?? new Float32Array())
    // The bed origin (0, 0, 0) and corner (40, 0, 0) in model coordinates.
    expect(first?.[0]).toEqual(closeTo([-20, 10, -5]))
    expect(first?.[1]).toEqual(closeTo([-20, -30, -5]))
  })

  it('draws one segment per side of a round bed', () => {
    const bed = { kind: 'circle', diameter: 170, height: 240 } as const
    const body = analyzed([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
    expect(bedOutlineSegments(bed, body)).toHaveLength(CIRCLE_SEGMENTS * 6)
  })
})

describe('bedOutline', () => {
  const REPORT = printReport([bodyPrint({ body: CUBE.id, meshId: 1 })], 'generic-resin')

  it("outlines the bed of the report's profile in Print context", () => {
    const outline = bedOutline(
      stateWith({ context: 'print', print: printState({ report: REPORT, reportRevision: 3 }) }),
    )
    // generic-resin has a 130 × 80 bed; the fixture body sits at (128, 128) on it.
    const [first] = segmentsOf(outline ?? new Float32Array())
    expect(first).toEqual([
      [-128, -128, 0],
      [2, -128, 0],
    ])
  })

  it('returns the same outline while the context, the report and the printers stay', () => {
    const state = stateWith({
      context: 'print',
      print: printState({ report: REPORT, reportRevision: 3 }),
    })
    const outline = bedOutline(state)
    expect(bedOutline({ ...state, selection: [] })).toBe(outline)
  })

  it('is null outside Print', () => {
    expect(
      bedOutline(stateWith({ context: 'model', print: printState({ report: REPORT }) })),
    ).toBeNull()
  })

  it('is null without a report or an analyzed body', () => {
    expect(bedOutline(stateWith({ context: 'print', print: printState() }))).toBeNull()
    expect(
      bedOutline(stateWith({ context: 'print', print: printState({ report: printReport([]) }) })),
    ).toBeNull()
  })

  it('is null when the report names a profile that is not installed', () => {
    const report = printReport([bodyPrint({ body: CUBE.id, meshId: 1 })], 'retired-printer')
    expect(bedOutline(stateWith({ context: 'print', print: printState({ report }) }))).toBeNull()
  })
})
