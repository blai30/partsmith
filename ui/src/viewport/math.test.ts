import { describe, expect, it } from 'vitest'
import {
  addScaled,
  angleBetween,
  boundsCenter,
  boundsCorners,
  cross,
  IDENTITY_QUATERNION,
  length,
  multiplyMatrices,
  multiplyQuaternions,
  normalize,
  normalizeQuaternion,
  quaternionFromAxisAngle,
  quaternionFromBasis,
  rotateVector,
  slerp,
  transformPoint,
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

describe('bounds', () => {
  const bounds = { min: [-1, 0, 2] as Vector3, max: [3, 4, 6] as Vector3 }

  it('boundsCenter is the middle of the box', () => {
    expect(boundsCenter(bounds)).toEqual([1, 2, 4])
  })

  it('boundsCorners lists the 8 distinct corners', () => {
    const corners = boundsCorners(bounds)
    expect(corners).toHaveLength(8)
    expect(new Set(corners.map((corner) => corner.join(','))).size).toBe(8)
    expect(corners).toContainEqual([-1, 0, 2])
    expect(corners).toContainEqual([3, 4, 6])
    expect(corners).toContainEqual([3, 0, 6])
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

describe('matrices', () => {
  // Column-major: a translation by (1, 2, 3) after scaling x by 2.
  const transform = new Float32Array([2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1])

  it('transformPoint applies a column-major matrix to a point with w 1', () => {
    expect(transformPoint(transform, [1, 1, 1])).toEqual([3, 3, 4, 1])
  })

  it('multiplyMatrices applies the second matrix first', () => {
    const translateX = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 0, 0, 1])
    const product = multiplyMatrices(transform, translateX)
    expect(transformPoint(product, [0, 0, 0])).toEqual([11, 2, 3, 1])
  })
})
