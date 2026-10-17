/** A point or direction in millimeters, components x, y, z. */
export type Vector3 = readonly [number, number, number]
/** A rotation as a unit quaternion, components x, y, z, w. */
export type Quaternion = readonly [number, number, number, number]
/** An axis-aligned box in millimeters. The protocol's `BoundingBox` (mesh.md) is assignable to it. */
export type Bounds = { readonly min: Vector3; readonly max: Vector3 }

export const IDENTITY_QUATERNION: Quaternion = [0, 0, 0, 1]

export function add(first: Vector3, second: Vector3): Vector3 {
  return [first[0] + second[0], first[1] + second[1], first[2] + second[2]]
}

export function subtract(first: Vector3, second: Vector3): Vector3 {
  return [first[0] - second[0], first[1] - second[1], first[2] - second[2]]
}

export function scale(vector: Vector3, factor: number): Vector3 {
  return [vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

/** `first + second × factor`. */
export function addScaled(first: Vector3, second: Vector3, factor: number): Vector3 {
  return [
    first[0] + second[0] * factor,
    first[1] + second[1] * factor,
    first[2] + second[2] * factor,
  ]
}

export function dot(first: Vector3, second: Vector3): number {
  return first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

export function cross(first: Vector3, second: Vector3): Vector3 {
  return [
    first[1] * second[2] - first[2] * second[1],
    first[2] * second[0] - first[0] * second[2],
    first[0] * second[1] - first[1] * second[0],
  ]
}

export function length(vector: Vector3): number {
  return Math.hypot(vector[0], vector[1], vector[2])
}

/** The unit vector along `vector`, or null when its length is below `epsilon`. */
export function normalize(vector: Vector3, epsilon = 1e-12): Vector3 | null {
  const vectorLength = length(vector)
  return vectorLength < epsilon ? null : scale(vector, 1 / vectorLength)
}

/** The angle between two non-zero vectors in radians, accurate near 0 and π. */
export function angleBetween(first: Vector3, second: Vector3): number {
  return Math.atan2(length(cross(first, second)), dot(first, second))
}

/** The point at `index` of a flat list of x, y, z coordinates, such as a ghost's (2.7). */
export function pointAt(buffer: Float32Array, index: number): Vector3 {
  return [buffer[index * 3] ?? 0, buffer[index * 3 + 1] ?? 0, buffer[index * 3 + 2] ?? 0]
}

/** The points of a flat list of x, y, z coordinates, in order; a trailing partial point is left out. */
export function pointsOf(buffer: Float32Array): Vector3[] {
  const points: Vector3[] = []
  for (let i = 0; (i + 1) * 3 <= buffer.length; i++) {
    points.push(pointAt(buffer, i))
  }
  return points
}

/** The center of a box. */
export function boundsCenter(bounds: Bounds): Vector3 {
  return scale(add(bounds.min, bounds.max), 0.5)
}

/** The length of a box's diagonal. */
export function boundsDiagonal(bounds: Bounds): number {
  return length(subtract(bounds.max, bounds.min))
}

/** The 8 corners of a box. */
export function boundsCorners(bounds: Bounds): Vector3[] {
  const corners: Vector3[] = []
  for (const x of [bounds.min[0], bounds.max[0]]) {
    for (const y of [bounds.min[1], bounds.max[1]]) {
      for (const z of [bounds.min[2], bounds.max[2]]) {
        corners.push([x, y, z])
      }
    }
  }
  return corners
}

/** A box's union with the finite ones of some points; null stays null when there are none. */
export function extendBounds(bounds: Bounds | null, points: Iterable<Vector3>): Bounds | null {
  let result = bounds
  for (const [x, y, z] of points) {
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
      continue
    }
    result =
      result === null
        ? { min: [x, y, z], max: [x, y, z] }
        : {
            min: [
              Math.min(result.min[0], x),
              Math.min(result.min[1], y),
              Math.min(result.min[2], z),
            ],
            max: [
              Math.max(result.max[0], x),
              Math.max(result.max[1], y),
              Math.max(result.max[2], z),
            ],
          }
  }
  return result
}

/** Rotation by `angle` radians about `axis` (right-handed). A zero axis gives the identity. */
export function quaternionFromAxisAngle(axis: Vector3, angle: number): Quaternion {
  const unit = normalize(axis)
  if (unit === null) {
    return IDENTITY_QUATERNION
  }
  const sine = Math.sin(angle / 2)
  return [unit[0] * sine, unit[1] * sine, unit[2] * sine, Math.cos(angle / 2)]
}

/** The rotation that applies `second` first, then `first`. */
export function multiplyQuaternions(first: Quaternion, second: Quaternion): Quaternion {
  const [firstX, firstY, firstZ, firstW] = first
  const [secondX, secondY, secondZ, secondW] = second
  return [
    firstW * secondX + firstX * secondW + firstY * secondZ - firstZ * secondY,
    firstW * secondY - firstX * secondZ + firstY * secondW + firstZ * secondX,
    firstW * secondZ + firstX * secondY - firstY * secondX + firstZ * secondW,
    firstW * secondW - firstX * secondX - firstY * secondY - firstZ * secondZ,
  ]
}

export function normalizeQuaternion(rotation: Quaternion): Quaternion {
  const norm = Math.hypot(rotation[0], rotation[1], rotation[2], rotation[3])
  if (norm === 0) {
    return IDENTITY_QUATERNION
  }
  return [rotation[0] / norm, rotation[1] / norm, rotation[2] / norm, rotation[3] / norm]
}

/** Rotates a vector by a unit quaternion. */
export function rotateVector(rotation: Quaternion, vector: Vector3): Vector3 {
  const [x, y, z, scalar] = rotation
  // With u the vector part: t = 2 (u × v), and the result is v + scalar t + u × t.
  const twiceCrossX = 2 * (y * vector[2] - z * vector[1])
  const twiceCrossY = 2 * (z * vector[0] - x * vector[2])
  const twiceCrossZ = 2 * (x * vector[1] - y * vector[0])
  return [
    vector[0] + scalar * twiceCrossX + (y * twiceCrossZ - z * twiceCrossY),
    vector[1] + scalar * twiceCrossY + (z * twiceCrossX - x * twiceCrossZ),
    vector[2] + scalar * twiceCrossZ + (x * twiceCrossY - y * twiceCrossX),
  ]
}

/**
 * The rotation whose matrix has the columns `xAxis`, `yAxis`, `zAxis`, which must be orthonormal and right-handed.
 */
export function quaternionFromBasis(xAxis: Vector3, yAxis: Vector3, zAxis: Vector3): Quaternion {
  // The matrix entries by column (axis) and row (component): `yAxisZ` is row z of column y.
  const [xAxisX, xAxisY, xAxisZ] = xAxis
  const [yAxisX, yAxisY, yAxisZ] = yAxis
  const [zAxisX, zAxisY, zAxisZ] = zAxis
  const trace = xAxisX + yAxisY + zAxisZ
  // Shepperd's method: divide by the largest of the four candidates for numerical stability.
  if (trace > 0) {
    const factor = 0.5 / Math.sqrt(trace + 1)
    return normalizeQuaternion([
      (yAxisZ - zAxisY) * factor,
      (zAxisX - xAxisZ) * factor,
      (xAxisY - yAxisX) * factor,
      0.25 / factor,
    ])
  }
  if (xAxisX > yAxisY && xAxisX > zAxisZ) {
    const factor = 2 * Math.sqrt(1 + xAxisX - yAxisY - zAxisZ)
    return normalizeQuaternion([
      0.25 * factor,
      (yAxisX + xAxisY) / factor,
      (zAxisX + xAxisZ) / factor,
      (yAxisZ - zAxisY) / factor,
    ])
  }
  if (yAxisY > zAxisZ) {
    const factor = 2 * Math.sqrt(1 + yAxisY - xAxisX - zAxisZ)
    return normalizeQuaternion([
      (yAxisX + xAxisY) / factor,
      0.25 * factor,
      (zAxisY + yAxisZ) / factor,
      (zAxisX - xAxisZ) / factor,
    ])
  }
  const factor = 2 * Math.sqrt(1 + zAxisZ - xAxisX - yAxisY)
  return normalizeQuaternion([
    (zAxisX + xAxisZ) / factor,
    (zAxisY + yAxisZ) / factor,
    0.25 * factor,
    (xAxisY - yAxisX) / factor,
  ])
}

/** Spherical linear interpolation along the shorter arc, `amount` from 0 (`from`) to 1 (`to`). */
export function slerp(from: Quaternion, to: Quaternion, amount: number): Quaternion {
  let cosine = from[0] * to[0] + from[1] * to[1] + from[2] * to[2] + from[3] * to[3]
  let target = to
  if (cosine < 0) {
    cosine = -cosine
    target = [-to[0], -to[1], -to[2], -to[3]]
  }
  if (cosine > 0.9995) {
    return normalizeQuaternion([
      from[0] + (target[0] - from[0]) * amount,
      from[1] + (target[1] - from[1]) * amount,
      from[2] + (target[2] - from[2]) * amount,
      from[3] + (target[3] - from[3]) * amount,
    ])
  }
  const angle = Math.acos(cosine)
  const sine = Math.sin(angle)
  const fromWeight = Math.sin((1 - amount) * angle) / sine
  const toWeight = Math.sin(amount * angle) / sine
  return normalizeQuaternion([
    from[0] * fromWeight + target[0] * toWeight,
    from[1] * fromWeight + target[1] * toWeight,
    from[2] * fromWeight + target[2] * toWeight,
    from[3] * fromWeight + target[3] * toWeight,
  ])
}
