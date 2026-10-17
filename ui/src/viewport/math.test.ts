import { describe, expect, it } from 'vitest'
import {
  addScaled,
  angleBetween,
  boundsCenter,
  boundsCorners,
  boundsDiagonal,
  cross,
  extendBounds,
  IDENTITY_QUATERNION,
  length,
  multiplyQuaternions,
  normalize,
  normalizeQuaternion,
  pointAt,
  pointsOf,
  quaternionFromAxisAngle,
  quaternionFromBasis,
  rotateVector,
  slerp,
  type Quaternion,
  type Vector3,
} from './math'

function closeTo(values: readonly number[]): unknown[] {
  return values.map((value) => expect.closeTo(value, 12))
}

// The same rotation as `rotation` in the hemisphere of `reference`: q and -q are the same rotation.
function alignedWith(rotation: Quaternion, reference: Quaternion): number[] {
  const sign = Math.sign(
    rotation[0] * reference[0] +
      rotation[1] * reference[1] +
      rotation[2] * reference[2] +
      rotation[3] * reference[3],
  )
  return rotation.map((value) => value * sign)
}

const DEGREE = Math.PI / 180

describe('vectors', () => {
  it('addScaled adds a multiple of the second vector', () => {
    expect(addScaled([1, 2, 3], [1, 0, -1], 2)).toEqual([3, 2, 1])
  })

  it('cross follows the right-hand rule', () => {
    expect(cross([1, 0, 0], [0, 1, 0])).toEqual([0, 0, 1])
    expect(cross([0, 1, 0], [1, 0, 0])).toEqual([0, 0, -1])
  })

  it('length is the Euclidean norm', () => {
    expect(length([2, 3, 6])).toBe(7)
  })

  it('normalize gives a unit vector, or null for a zero vector', () => {
    expect(normalize([3, 0, 4]) ?? []).toEqual(closeTo([0.6, 0, 0.8]))
    expect(normalize([0, 0, 0])).toBeNull()
    expect(normalize([1e-10, 0, 0], 1e-9)).toBeNull()
  })

  it('angleBetween is accurate near 0 and π', () => {
    expect(angleBetween([1, 0, 0], [0, 1, 0])).toBeCloseTo(Math.PI / 2, 15)
    expect(angleBetween([1, 0, 0], [1, 1e-9, 0])).toBeCloseTo(1e-9, 20)
    expect(angleBetween([1, 0, 0], [-1, 1e-9, 0])).toBeCloseTo(Math.PI - 1e-9, 15)
  })
})

describe('points of a flat coordinate list', () => {
  const buffer = Float32Array.from([1, 2, 3, 4, 5, 6, 7])

  it('reads the point at an index', () => {
    expect(pointAt(buffer, 0)).toEqual([1, 2, 3])
    expect(pointAt(buffer, 1)).toEqual([4, 5, 6])
  })

  it('lists the whole points in order and leaves out a trailing partial one', () => {
    expect(pointsOf(buffer)).toEqual([
      [1, 2, 3],
      [4, 5, 6],
    ])
    expect(pointsOf(new Float32Array(0))).toEqual([])
  })
})

describe('bounds', () => {
  const bounds = { min: [-1, 0, 2] as Vector3, max: [3, 4, 6] as Vector3 }

  it('boundsCenter is the middle of the box', () => {
    expect(boundsCenter(bounds)).toEqual([1, 2, 4])
  })

  it('boundsDiagonal is the length from the minimum to the maximum corner', () => {
    expect(boundsDiagonal(bounds)).toBe(Math.hypot(4, 4, 4))
  })

  it('boundsCorners lists the 8 distinct corners', () => {
    const corners = boundsCorners(bounds)
    expect(corners).toHaveLength(8)
    expect(new Set(corners.map((corner) => corner.join(','))).size).toBe(8)
    expect(corners).toContainEqual([-1, 0, 2])
    expect(corners).toContainEqual([3, 4, 6])
    expect(corners).toContainEqual([3, 0, 6])
  })

  it('extendBounds starts a box at the first point when there is none', () => {
    expect(extendBounds(null, [[1, 2, 3]])).toEqual({ min: [1, 2, 3], max: [1, 2, 3] })
  })

  it('extendBounds grows a box to cover every point', () => {
    expect(
      extendBounds(bounds, [
        [-2, 5, 4],
        [0, -1, 7],
      ]),
    ).toEqual({
      min: [-2, -1, 2],
      max: [3, 5, 7],
    })
  })

  it('extendBounds skips points with a coordinate that is not finite', () => {
    expect(
      extendBounds(null, [
        [Number.NaN, 0, 0],
        [0, Infinity, 0],
      ]),
    ).toBeNull()
    expect(extendBounds(bounds, [[Number.NaN, 9, 9]])).toBe(bounds)
  })
})

describe('quaternions', () => {
  it('quaternionFromAxisAngle turns right-handed about the axis', () => {
    const quarterTurn = quaternionFromAxisAngle([0, 0, 2], 90 * DEGREE)
    expect(rotateVector(quarterTurn, [1, 0, 0])).toEqual(closeTo([0, 1, 0]))
    expect(Math.hypot(...quarterTurn)).toBeCloseTo(1, 15)
  })

  it('quaternionFromAxisAngle with a zero axis is the identity', () => {
    expect(quaternionFromAxisAngle([0, 0, 0], 1)).toBe(IDENTITY_QUATERNION)
  })

  it('multiplyQuaternions applies the second rotation first', () => {
    const aboutZ = quaternionFromAxisAngle([0, 0, 1], 90 * DEGREE)
    const aboutX = quaternionFromAxisAngle([1, 0, 0], 90 * DEGREE)
    // X goes to Y about Z, then Y goes to Z about X.
    expect(rotateVector(multiplyQuaternions(aboutX, aboutZ), [1, 0, 0])).toEqual(closeTo([0, 0, 1]))
  })

  it('normalizeQuaternion scales to unit length and maps zero to the identity', () => {
    expect(normalizeQuaternion([0, 0, 0, 2])).toEqual([0, 0, 0, 1])
    expect(normalizeQuaternion([0, 0, 0, 0])).toBe(IDENTITY_QUATERNION)
  })

  it('quaternionFromBasis inverts rotateVector for every branch of the trace test', () => {
    const rotations: readonly Quaternion[] = [
      quaternionFromAxisAngle([0, 0, 1], 30 * DEGREE),
      quaternionFromAxisAngle([1, 0, 0], 180 * DEGREE),
      quaternionFromAxisAngle([0, 1, 0], 170 * DEGREE),
      quaternionFromAxisAngle([0, 0, 1], 175 * DEGREE),
      quaternionFromAxisAngle([1, -1, 1], 200 * DEGREE),
    ]
    for (const rotation of rotations) {
      const rebuilt = quaternionFromBasis(
        rotateVector(rotation, [1, 0, 0]),
        rotateVector(rotation, [0, 1, 0]),
        rotateVector(rotation, [0, 0, 1]),
      )
      expect(alignedWith(rebuilt, rotation)).toEqual(closeTo(rotation))
    }
  })

  it('slerp moves along the shorter arc at constant angular speed', () => {
    const from = IDENTITY_QUATERNION
    const to = quaternionFromAxisAngle([0, 0, 1], 90 * DEGREE)
    const halfway = quaternionFromAxisAngle([0, 0, 1], 45 * DEGREE)
    expect(alignedWith(slerp(from, to, 0.5), halfway)).toEqual(closeTo(halfway))
    expect(alignedWith(slerp(from, to, 0), from)).toEqual(closeTo(from))
    expect(alignedWith(slerp(from, to, 1), to)).toEqual(closeTo(to))
    // The negated quaternion is the same rotation: the path still turns +45 degrees, not -135.
    const negated: Quaternion = [-to[0], -to[1], -to[2], -to[3]]
    expect(alignedWith(slerp(from, negated, 0.5), halfway)).toEqual(closeTo(halfway))
  })

  it('slerp between nearly equal rotations stays a unit quaternion', () => {
    const from = quaternionFromAxisAngle([0, 0, 1], 10 * DEGREE)
    const to = quaternionFromAxisAngle([0, 0, 1], 10.001 * DEGREE)
    const halfway = quaternionFromAxisAngle([0, 0, 1], 10.0005 * DEGREE)
    const middle = slerp(from, to, 0.5)
    expect(Math.hypot(...middle)).toBeCloseTo(1, 15)
    expect(alignedWith(middle, halfway)).toEqual(closeTo(halfway))
  })
})
