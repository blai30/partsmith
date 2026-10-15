import { describe, expect, it } from 'vitest'
import {
  circleThroughThreePoints,
  clipSegment,
  distancePointToArc,
  distancePointToSegment,
  segmentTouchesBox,
  tessellateArc,
  type Vector2,
} from './geometry'

// Float32 keeps about 7 significant digits, so tessellated coordinates of a 10 mm circle are
// compared to 4 decimal places.
const FLOAT32_DIGITS = 4

/** The (x, y) vertices of a tessellation, which stores 2 floats per vertex. */
function verticesOf(positions: Float32Array): Vector2[] {
  const vertices: Vector2[] = []
  for (let i = 0; i + 1 < positions.length; i += 2) {
    vertices.push([positions[i] ?? Number.NaN, positions[i + 1] ?? Number.NaN])
  }
  return vertices
}

function maximumSegmentAngle(radius: number, toleranceMillimeters: number): number {
  return 2 * Math.acos(1 - toleranceMillimeters / radius)
}

describe('circleThroughThreePoints', () => {
  it('U-55 of (0, 0), (2, 0) and (0, 2) is center (1, 1) with radius √2', () => {
    const circle = circleThroughThreePoints([0, 0], [2, 0], [0, 2])
    expect(circle).not.toBeNull()
    expect(circle?.center[0]).toBeCloseTo(1, 12)
    expect(circle?.center[1]).toBeCloseTo(1, 12)
    expect(circle?.radius).toBeCloseTo(Math.SQRT2, 12)
  })

  it('returns null for three points in a line (3.16.4)', () => {
    expect(circleThroughThreePoints([0, 0], [5, 0], [10, 0])).toBeNull()
  })

  it('returns null when two of the points are the same', () => {
    expect(circleThroughThreePoints([1, 1], [1, 1], [4, 5])).toBeNull()
  })
})

describe('tessellateArc', () => {
  it('U-55 a full circle of radius 10 with tolerance 0.01 has ceil(2π / (2 acos(1 - 0.001))) segments', () => {
    const segments = Math.ceil((2 * Math.PI) / (2 * Math.acos(1 - 0.001)))
    const positions = tessellateArc([0, 0], 10, 0, 2 * Math.PI, 0.01)
    expect(positions).toBeInstanceOf(Float32Array)
    expect(positions.length).toBe(2 * (segments + 1))
  })

  it('a full circle starts and ends at the start angle and keeps every vertex on the circle', () => {
    const vertices = verticesOf(tessellateArc([3, -2], 10, 0, 2 * Math.PI, 0.01))
    const first = vertices[0]
    const last = vertices.at(-1)
    expect(first?.[0]).toBeCloseTo(13, FLOAT32_DIGITS)
    expect(first?.[1]).toBeCloseTo(-2, FLOAT32_DIGITS)
    expect(last?.[0]).toBeCloseTo(13, FLOAT32_DIGITS)
    expect(last?.[1]).toBeCloseTo(-2, FLOAT32_DIGITS)
    for (const [x, y] of vertices) {
      expect(Math.hypot(x - 3, y + 2)).toBeCloseTo(10, FLOAT32_DIGITS)
    }
  })

  it('a quarter arc runs counterclockwise from start to end with the same chord tolerance', () => {
    const segments = Math.ceil(Math.PI / 2 / maximumSegmentAngle(10, 0.01))
    const vertices = verticesOf(tessellateArc([0, 0], 10, 0, Math.PI / 2, 0.01))
    expect(vertices).toHaveLength(segments + 1)
    expect(vertices[0]?.[0]).toBeCloseTo(10, FLOAT32_DIGITS)
    expect(vertices[0]?.[1]).toBeCloseTo(0, FLOAT32_DIGITS)
    expect(vertices.at(-1)?.[0]).toBeCloseTo(0, FLOAT32_DIGITS)
    expect(vertices.at(-1)?.[1]).toBeCloseTo(10, FLOAT32_DIGITS)
    // Counterclockwise: the second vertex is above the x axis.
    expect(vertices[1]?.[1]).toBeGreaterThan(0)
  })
})

describe('distancePointToSegment', () => {
  it('is the perpendicular distance when the foot lies inside the segment', () => {
    expect(distancePointToSegment([5, 3], [0, 0], [10, 0])).toBeCloseTo(3, 12)
  })

  it('is the distance to the nearer end when the foot lies beyond an end', () => {
    expect(distancePointToSegment([13, 4], [0, 0], [10, 0])).toBeCloseTo(5, 12)
    expect(distancePointToSegment([-3, -4], [0, 0], [10, 0])).toBeCloseTo(5, 12)
  })

  it('is the distance to the point for a segment of zero length', () => {
    expect(distancePointToSegment([3, 4], [0, 0], [0, 0])).toBeCloseTo(5, 12)
  })
})

describe('distancePointToArc', () => {
  it('is the distance to the circle when the point lies inside the arc angles', () => {
    expect(distancePointToArc([0, 12], [0, 0], 10, 0, Math.PI)).toBeCloseTo(2, 12)
    expect(distancePointToArc([0, 7], [0, 0], 10, 0, Math.PI)).toBeCloseTo(3, 12)
  })

  it('is the distance to the nearer end when the point lies outside the arc angles', () => {
    // The arc runs counterclockwise from (10, 0) to (0, 10); (10, -5) is below its start.
    expect(distancePointToArc([10, -5], [0, 0], 10, 0, Math.PI / 2)).toBeCloseTo(5, 12)
  })

  it('follows the counterclockwise direction across angle 0', () => {
    // From 270 degrees to 90 degrees counterclockwise covers the right half, so (12, 0) is 2 away.
    expect(distancePointToArc([12, 0], [0, 0], 10, (3 * Math.PI) / 2, Math.PI / 2)).toBeCloseTo(
      2,
      12,
    )
    // The left half is not part of that arc: (-12, 0) is nearest to the ends (0, ±10).
    expect(distancePointToArc([-12, 0], [0, 0], 10, (3 * Math.PI) / 2, Math.PI / 2)).toBeCloseTo(
      Math.hypot(12, 10),
      12,
    )
  })
})

describe('clipSegment', () => {
  const BOX = { min: [0, 0], max: [10, 10] } as const

  it('gives the whole segment when it lies inside', () => {
    expect(clipSegment(BOX, [2, 2], [8, 4])).toEqual([0, 1])
  })

  it('gives the parameters where a crossing segment enters and leaves', () => {
    expect(clipSegment(BOX, [-10, 5], [20, 5])).toEqual([
      expect.closeTo(1 / 3),
      expect.closeTo(2 / 3),
    ])
    expect(clipSegment(BOX, [5, 5], [5, 25])).toEqual([0, expect.closeTo(0.25)])
  })

  it('gives null for a segment beside the box, and a degenerate segment outside it', () => {
    expect(clipSegment(BOX, [-5, -5], [-1, 20])).toBeNull()
    expect(clipSegment(BOX, [11, 5], [11, 5])).toBeNull()
  })

  it('agrees with segmentTouchesBox', () => {
    expect(segmentTouchesBox(BOX, [-10, 5], [20, 5])).toBe(true)
    expect(segmentTouchesBox(BOX, [-5, -5], [-1, 20])).toBe(false)
  })
})
