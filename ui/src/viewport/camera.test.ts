import { describe, expect, it } from 'vitest'
import {
  classifyView,
  DEFAULT_CAMERA,
  DEFAULT_VERTICAL_FIELD_OF_VIEW,
  depthRange,
  eye,
  fitBounds,
  forward,
  orbit,
  pan,
  projectionMatrix,
  rotationLookingAlong,
  standardViewRotation,
  up,
  viewMatrix,
  visibleHeightAtTarget,
  wheelZoomFactor,
  withProjection,
  zoomAt,
  type Camera,
  type Quaternion,
  type StandardView,
  type Vector3,
} from './camera'

type BoundingBox = Parameters<typeof fitBounds>[1]

const DEGREE = Math.PI / 180
const TAN_15_DEGREES = Math.tan(15 * DEGREE)
const SQRT_3 = Math.sqrt(3)
const SQRT_6 = Math.sqrt(6)

// Vector and quaternion helpers in double precision, independent of the module under test.

function add(first: Vector3, second: Vector3): Vector3 {
  return [first[0] + second[0], first[1] + second[1], first[2] + second[2]]
}

function subtract(first: Vector3, second: Vector3): Vector3 {
  return [first[0] - second[0], first[1] - second[1], first[2] - second[2]]
}

function scale(vector: Vector3, factor: number): Vector3 {
  return [vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

function dot(first: Vector3, second: Vector3): number {
  return first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

function cross(first: Vector3, second: Vector3): Vector3 {
  return [
    first[1] * second[2] - first[2] * second[1],
    first[2] * second[0] - first[0] * second[2],
    first[0] * second[1] - first[1] * second[0],
  ]
}

function normalize(vector: Vector3): Vector3 {
  return scale(vector, 1 / Math.hypot(vector[0], vector[1], vector[2]))
}

function multiplyQuaternions(first: Quaternion, second: Quaternion): Quaternion {
  const [firstX, firstY, firstZ, firstW] = first
  const [secondX, secondY, secondZ, secondW] = second
  return [
    firstW * secondX + firstX * secondW + firstY * secondZ - firstZ * secondY,
    firstW * secondY - firstX * secondZ + firstY * secondW + firstZ * secondX,
    firstW * secondZ + firstX * secondY - firstY * secondX + firstZ * secondW,
    firstW * secondW - firstX * secondX - firstY * secondY - firstZ * secondZ,
  ]
}

function rotate(rotation: Quaternion, vector: Vector3): Vector3 {
  const conjugate: Quaternion = [-rotation[0], -rotation[1], -rotation[2], rotation[3]]
  const rotated = multiplyQuaternions(
    multiplyQuaternions(rotation, [vector[0], vector[1], vector[2], 0]),
    conjugate,
  )
  return [rotated[0], rotated[1], rotated[2]]
}

function axisAngle(axis: Vector3, angle: number): Quaternion {
  const unit = normalize(axis)
  const sine = Math.sin(angle / 2)
  return [unit[0] * sine, unit[1] * sine, unit[2] * sine, Math.cos(angle / 2)]
}

// The camera's world axes from its rotation: it looks along local -Z, local +Y is up, local +X is right.
function basis(camera: Camera): {
  readonly right: Vector3
  readonly up: Vector3
  readonly forward: Vector3
} {
  return {
    right: rotate(camera.rotation, [1, 0, 0]),
    up: rotate(camera.rotation, [0, 1, 0]),
    forward: rotate(camera.rotation, [0, 0, -1]),
  }
}

function eyePosition(camera: Camera): Vector3 {
  return subtract(camera.target, scale(basis(camera).forward, camera.distance))
}

// Camera-space coordinates of a world point: right, up, and depth along forward from the eye.
function cameraSpace(camera: Camera, point: Vector3): Vector3 {
  const axes = basis(camera)
  const relative = subtract(point, eyePosition(camera))
  return [dot(relative, axes.right), dot(relative, axes.up), dot(relative, axes.forward)]
}

function halfHeightAtDepth(camera: Camera, depth: number): number {
  return camera.projection === 'perspective'
    ? depth * Math.tan(camera.verticalFieldOfView / 2)
    : camera.orthographicHeight / 2
}

function normalizedDeviceCoordinates(
  camera: Camera,
  point: Vector3,
  aspect: number,
): readonly [number, number] {
  const [x, y, depth] = cameraSpace(camera, point)
  const halfHeight = halfHeightAtDepth(camera, depth)
  return [x / (halfHeight * aspect), y / halfHeight]
}

// The world point at the target depth under a cursor given in normalized device coordinates.
function pointUnderCursor(
  camera: Camera,
  cursor: readonly [number, number],
  aspect: number,
): Vector3 {
  const axes = basis(camera)
  const halfHeight = halfHeightAtDepth(camera, camera.distance)
  return add(
    camera.target,
    add(scale(axes.right, cursor[0] * halfHeight * aspect), scale(axes.up, cursor[1] * halfHeight)),
  )
}

// Applies a column-major 4 x 4 matrix to a point with w = 1.
function transformPoint(
  matrix: Float32Array,
  point: Vector3,
): readonly [number, number, number, number] {
  const [x, y, z] = point
  const at = (index: number): number => matrix[index] ?? Number.NaN
  return [
    at(0) * x + at(4) * y + at(8) * z + at(12),
    at(1) * x + at(5) * y + at(9) * z + at(13),
    at(2) * x + at(6) * y + at(10) * z + at(14),
    at(3) * x + at(7) * y + at(11) * z + at(15),
  ]
}

function projectToNormalized(matrix: Float32Array, point: Vector3): Vector3 {
  const [x, y, z, w] = transformPoint(matrix, point)
  return [x / w, y / w, z / w]
}

// Forward and up of a camera, from its rotation, for comparing orientations whatever the quaternion's sign.
function orientation(camera: Camera): readonly number[] {
  const axes = basis(camera)
  return [...axes.forward, ...axes.up]
}

function closeToVector(vector: readonly number[], digits = 9): unknown[] {
  return vector.map((component) => expect.closeTo(component, digits))
}

function cameraWith(overrides: Partial<Camera>): Camera {
  return { ...DEFAULT_CAMERA, ...overrides }
}

function box(min: Vector3, max: Vector3): BoundingBox {
  return { min: [min[0], min[1], min[2]], max: [max[0], max[1], max[2]] }
}

const STANDARD_VIEWS: readonly {
  readonly view: StandardView
  readonly forward: Vector3
  readonly up: Vector3
}[] = [
  { view: 'front', forward: [0, 1, 0], up: [0, 0, 1] },
  { view: 'back', forward: [0, -1, 0], up: [0, 0, 1] },
  { view: 'right', forward: [-1, 0, 0], up: [0, 0, 1] },
  { view: 'left', forward: [1, 0, 0], up: [0, 0, 1] },
  { view: 'top', forward: [0, 0, -1], up: [0, 1, 0] },
  { view: 'bottom', forward: [0, 0, 1], up: [0, -1, 0] },
  {
    view: 'iso',
    forward: [-1 / SQRT_3, 1 / SQRT_3, -1 / SQRT_3],
    up: [-1 / SQRT_6, 1 / SQRT_6, 2 / SQRT_6],
  },
]

const TURN_AXES = ['right', 'up', 'forward'] as const

const PERTURBED_VIEWS = STANDARD_VIEWS.flatMap(({ view, forward: viewForward, up: viewUp }) =>
  TURN_AXES.map((axis) => ({ view, forward: viewForward, up: viewUp, axis })),
)

describe('defaults', () => {
  it('DEFAULT_VERTICAL_FIELD_OF_VIEW is 30 degrees', () => {
    expect(DEFAULT_VERTICAL_FIELD_OF_VIEW).toBeCloseTo(Math.PI / 6, 12)
  })

  it('DEFAULT_CAMERA is the perspective iso view of the origin from 200 mm', () => {
    expect(DEFAULT_CAMERA.target).toEqual(closeToVector([0, 0, 0]))
    expect(DEFAULT_CAMERA.distance).toBe(200)
    expect(DEFAULT_CAMERA.projection).toBe('perspective')
    expect(DEFAULT_CAMERA.verticalFieldOfView).toBeCloseTo(Math.PI / 6, 12)
    expect(classifyView(DEFAULT_CAMERA)).toBe('iso')
  })

  it('DEFAULT_CAMERA has orthographicHeight 2 x 200 x tan(15 degrees)', () => {
    expect(DEFAULT_CAMERA.orthographicHeight).toBeCloseTo(2 * 200 * TAN_15_DEGREES, 9)
  })
})

describe('standard views', () => {
  it.each(STANDARD_VIEWS)(
    'U-44 standardViewRotation($view) gives the forward and up vectors of 2.11',
    ({ view, forward: expectedForward, up: expectedUp }) => {
      const camera = cameraWith({ rotation: standardViewRotation(view) })
      expect(forward(camera)).toEqual(closeToVector(expectedForward))
      expect(up(camera)).toEqual(closeToVector(expectedUp))
    },
  )

  it.each(STANDARD_VIEWS)(
    'standardViewRotation($view) turns local -Z to the forward vector and local +Y to the up vector',
    ({ view, forward: expectedForward, up: expectedUp }) => {
      const rotation = standardViewRotation(view)
      expect(rotate(rotation, [0, 0, -1])).toEqual(closeToVector(expectedForward))
      expect(rotate(rotation, [0, 1, 0])).toEqual(closeToVector(expectedUp))
    },
  )

  it.each(STANDARD_VIEWS)('standardViewRotation($view) is a unit quaternion', ({ view }) => {
    const [x, y, z, w] = standardViewRotation(view)
    expect(Math.hypot(x, y, z, w)).toBeCloseTo(1, 12)
  })

  it.each(['front', 'top', 'bottom'] as const)('+X points right in the %s view', (view) => {
    const camera = cameraWith({ rotation: standardViewRotation(view) })
    expect(cross(forward(camera), up(camera))).toEqual(closeToVector([1, 0, 0]))
  })

  it.each(STANDARD_VIEWS)('U-44 classifyView returns $view for the $view rotation', ({ view }) => {
    expect(classifyView(cameraWith({ rotation: standardViewRotation(view) }))).toBe(view)
  })

  it.each(PERTURBED_VIEWS)(
    'U-44 classifyView is custom after turning the $view view 1e-3 rad about its $axis axis',
    ({ view, forward: viewForward, up: viewUp, axis }) => {
      const axes = { right: cross(viewForward, viewUp), up: viewUp, forward: viewForward }
      const rotation = multiplyQuaternions(axisAngle(axes[axis], 1e-3), standardViewRotation(view))
      expect(classifyView(cameraWith({ rotation }))).toBe('custom')
    },
  )

  it.each(STANDARD_VIEWS)(
    'classifyView still returns $view after turning it 1e-8 rad, inside the 1e-6 rad tolerance',
    ({ view, forward: viewForward, up: viewUp }) => {
      const rotation = multiplyQuaternions(
        axisAngle(cross(viewForward, viewUp), 1e-8),
        standardViewRotation(view),
      )
      expect(classifyView(cameraWith({ rotation }))).toBe(view)
    },
  )

  it('classifyView depends only on the rotation', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('front'),
      target: [10, 20, 30],
      distance: 55,
      projection: 'orthographic',
      orthographicHeight: 12,
    })
    expect(classifyView(camera)).toBe('front')
  })
})

describe('camera basis and matrices', () => {
  it('forward and up follow the rotation: local -Z and local +Y in world space', () => {
    const rotation = axisAngle([1, 2, 3], 0.7)
    const camera = cameraWith({ rotation })
    expect(forward(camera)).toEqual(closeToVector(rotate(rotation, [0, 0, -1])))
    expect(up(camera)).toEqual(closeToVector(rotate(rotation, [0, 1, 0])))
  })

  it('eye is the target minus forward times distance', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('front'),
      target: [1, 2, 3],
      distance: 50,
    })
    expect(eye(camera)).toEqual(closeToVector([1, -48, 3]))
  })

  it('visibleHeightAtTarget is 2 x distance x tan(verticalFieldOfView / 2) in perspective', () => {
    const camera = cameraWith({ distance: 120, verticalFieldOfView: 40 * DEGREE })
    expect(visibleHeightAtTarget(camera)).toBeCloseTo(2 * 120 * Math.tan(20 * DEGREE), 9)
  })

  it('visibleHeightAtTarget is orthographicHeight in orthographic', () => {
    const camera = cameraWith({
      projection: 'orthographic',
      orthographicHeight: 37.5,
      distance: 120,
    })
    expect(visibleHeightAtTarget(camera)).toBeCloseTo(37.5, 9)
  })

  it('viewMatrix is a column-major world-to-view matrix: the eye at the origin, the target on -Z', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('front'),
      target: [1, 2, 3],
      distance: 50,
    })
    const matrix = viewMatrix(camera)
    expect(matrix).toBeInstanceOf(Float32Array)
    expect(matrix).toHaveLength(16)
    expect(transformPoint(matrix, [1, -48, 3])).toEqual(closeToVector([0, 0, 0, 1], 3))
    expect(transformPoint(matrix, [1, 2, 3])).toEqual(closeToVector([0, 0, -50, 1], 3))
  })

  it('viewMatrix maps the camera right axis to view +X and the up axis to view +Y', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('front'),
      target: [1, 2, 3],
      distance: 50,
    })
    const matrix = viewMatrix(camera)
    expect(transformPoint(matrix, [11, 2, 3])).toEqual(closeToVector([10, 0, -50, 1], 3))
    expect(transformPoint(matrix, [1, 2, 13])).toEqual(closeToVector([0, 10, -50, 1], 3))
  })

  it('projectionMatrix in perspective maps the frustum corner at any depth to (1, 1) and near and far to -1 and 1', () => {
    const camera = cameraWith({ verticalFieldOfView: 30 * DEGREE })
    const matrix = projectionMatrix(camera, 1.5, 10, 1000)
    expect(matrix).toBeInstanceOf(Float32Array)
    expect(matrix).toHaveLength(16)
    const corner = projectToNormalized(matrix, [
      TAN_15_DEGREES * 1.5 * 100,
      TAN_15_DEGREES * 100,
      -100,
    ])
    expect([corner[0], corner[1]]).toEqual(closeToVector([1, 1], 4))
    expect(projectToNormalized(matrix, [0, 0, -10])[2]).toBeCloseTo(-1, 4)
    expect(projectToNormalized(matrix, [0, 0, -1000])[2]).toBeCloseTo(1, 4)
  })

  it('projectionMatrix in orthographic maps half the orthographic height to 1 and near and far to -1 and 1', () => {
    const camera = cameraWith({ projection: 'orthographic', orthographicHeight: 80 })
    const matrix = projectionMatrix(camera, 2, -50, 150)
    const corner = projectToNormalized(matrix, [80, 40, -20])
    const oppositeCorner = projectToNormalized(matrix, [-80, -40, -20])
    expect([corner[0], corner[1]]).toEqual(closeToVector([1, 1], 4))
    expect([oppositeCorner[0], oppositeCorner[1]]).toEqual(closeToVector([-1, -1], 4))
    expect(projectToNormalized(matrix, [0, 0, 50])[2]).toBeCloseTo(-1, 4)
    expect(projectToNormalized(matrix, [0, 0, -150])[2]).toBeCloseTo(1, 4)
  })
})

describe('depthRange', () => {
  const front = cameraWith({
    rotation: standardViewRotation('front'),
    target: [0, 0, 0],
    distance: 200,
  })

  it('covers the bounding sphere with 10 percent slack in perspective', () => {
    const range = depthRange(front, [0, 0, 0], 10)
    expect(range.near).toBeCloseTo(189, 9)
    expect(range.far).toBeCloseTo(211, 9)
  })

  it('measures the sphere center along the forward axis, ignoring its sideways offset', () => {
    const range = depthRange(front, [50, 30, -20], 10)
    expect(range.near).toBeCloseTo(219, 9)
    expect(range.far).toBeCloseTo(241, 9)
  })

  it('keeps near at 0.001 x far in perspective when the eye is inside the sphere', () => {
    const range = depthRange(front, [0, 0, 0], 300)
    expect(range.near).toBeCloseTo(0.53, 9)
    expect(range.far).toBeCloseTo(530, 9)
  })

  it('keeps near at least 0.01 in perspective', () => {
    const range = depthRange(front, [0, -198, 0], 2)
    expect(range.near).toBeCloseTo(0.01, 9)
    expect(range.far).toBeCloseTo(4.2, 9)
  })

  it('lets near be negative in orthographic', () => {
    const range = depthRange({ ...front, projection: 'orthographic' }, [0, 0, 0], 300)
    expect(range.near).toBeCloseTo(-130, 9)
    expect(range.far).toBeCloseTo(530, 9)
  })
})

describe('rotationLookingAlong', () => {
  it('looks along the direction with up from the hint: straight down with +Y up is the top view', () => {
    const camera = cameraWith({ rotation: rotationLookingAlong([0, 0, -1], [0, 1, 0]) })
    expect(classifyView(camera)).toBe('top')
  })

  it('normalizes the direction and projects the up hint onto the view plane', () => {
    const camera = cameraWith({ rotation: rotationLookingAlong([0, 5, 0], [0, 2, 3]) })
    expect(forward(camera)).toEqual(closeToVector([0, 1, 0]))
    expect(up(camera)).toEqual(closeToVector([0, 0, 1]))
    expect(classifyView(camera)).toBe('front')
  })

  it('looking from the front right top corner with up hint +Z is the iso view', () => {
    const camera = cameraWith({ rotation: rotationLookingAlong([-1, 1, -1], [0, 0, 1]) })
    expect(classifyView(camera)).toBe('iso')
  })

  it('looking from the front top edge has up along the projection of +Z', () => {
    const camera = cameraWith({ rotation: rotationLookingAlong([0, 1, -1], [0, 0, 1]) })
    expect(forward(camera)).toEqual(closeToVector([0, 1 / Math.SQRT2, -1 / Math.SQRT2]))
    expect(up(camera)).toEqual(closeToVector([0, 1 / Math.SQRT2, 1 / Math.SQRT2]))
    expect(classifyView(camera)).toBe('custom')
  })

  it('returns a unit quaternion', () => {
    const [x, y, z, w] = rotationLookingAlong([3, -4, 2], [0, 0, 1])
    expect(Math.hypot(x, y, z, w)).toBeCloseTo(1, 12)
  })
})

describe('fitBounds', () => {
  const cube = box([-10, -10, 0], [10, 10, 20])
  const frontOrthographic = cameraWith({
    rotation: standardViewRotation('front'),
    projection: 'orthographic',
  })

  it('U-45 fitBounds orthographic in the front view with aspect 2 gives orthographicHeight 25 and target (0, 0, 10)', () => {
    const fitted = fitBounds(frontOrthographic, cube, 2)
    expect(fitted.orthographicHeight).toBeCloseTo(25, 9)
    expect(fitted.target).toEqual(closeToVector([0, 0, 10]))
  })

  it('U-45 fitBounds orthographic in the front view with aspect 0.5 gives orthographicHeight 50', () => {
    const fitted = fitBounds(frontOrthographic, cube, 0.5)
    expect(fitted.orthographicHeight).toBeCloseTo(50, 9)
    expect(fitted.target).toEqual(closeToVector([0, 0, 10]))
  })

  it('U-46 fitBounds perspective in the top view of (0, 0, 0) to (40, 20, 0) with aspect 1.6 gives distance 31.25 / (2 tan 15 degrees)', () => {
    const camera = cameraWith({ rotation: standardViewRotation('top'), projection: 'perspective' })
    const fitted = fitBounds(camera, box([0, 0, 0], [40, 20, 0]), 1.6)
    expect(fitted.distance).toBeCloseTo(31.25 / (2 * TAN_15_DEGREES), 9)
    expect(fitted.target).toEqual(closeToVector([20, 10, 0]))
  })

  it('adds half the camera-space depth extent to the perspective distance', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('front'),
      projection: 'perspective',
    })
    const fitted = fitBounds(camera, cube, 1)
    expect(fitted.distance).toBeCloseTo(25 / (2 * TAN_15_DEGREES) + 10, 9)
  })

  it('keeps the rotation and the projection', () => {
    const camera = cameraWith({ rotation: standardViewRotation('iso'), projection: 'orthographic' })
    const fitted = fitBounds(camera, cube, 1.5)
    expect(orientation(fitted)).toEqual(closeToVector(orientation(camera)))
    expect(fitted.projection).toBe('orthographic')
    expect(classifyView(fitted)).toBe('iso')
  })

  it('uses the margin argument: margin 0 fills the limiting dimension', () => {
    expect(fitBounds(frontOrthographic, cube, 2, 0).orthographicHeight).toBeCloseTo(20, 9)
    expect(fitBounds(frontOrthographic, cube, 2, 0.25).orthographicHeight).toBeCloseTo(40, 9)
  })

  it('treats a degenerate box as a 100 mm cube around its center', () => {
    const fitted = fitBounds(frontOrthographic, box([5, 5, 5], [5, 5, 5]), 1)
    expect(fitted.orthographicHeight).toBeCloseTo(125, 9)
    expect(fitted.target).toEqual(closeToVector([5, 5, 5]))
  })
})

describe('withProjection', () => {
  it('switching to orthographic sets orthographicHeight to 2 x distance x tan(verticalFieldOfView / 2)', () => {
    const camera = cameraWith({ distance: 150, verticalFieldOfView: 30 * DEGREE })
    const orthographic = withProjection(camera, 'orthographic')
    expect(orthographic.projection).toBe('orthographic')
    expect(orthographic.orthographicHeight).toBeCloseTo(2 * 150 * TAN_15_DEGREES, 9)
    expect(orientation(orthographic)).toEqual(closeToVector(orientation(camera)))
    expect(orthographic.target).toEqual(closeToVector(camera.target))
  })

  it('switching to perspective sets distance to orthographicHeight / (2 tan(verticalFieldOfView / 2))', () => {
    const camera = cameraWith({
      projection: 'orthographic',
      orthographicHeight: 50,
      verticalFieldOfView: 30 * DEGREE,
    })
    const perspective = withProjection(camera, 'perspective')
    expect(perspective.projection).toBe('perspective')
    expect(perspective.distance).toBeCloseTo(50 / (2 * TAN_15_DEGREES), 9)
  })

  it('U-48 withProjection round trip keeps visibleHeightAtTarget', () => {
    const camera = cameraWith({ distance: 180, rotation: standardViewRotation('right') })
    const orthographic = withProjection(camera, 'orthographic')
    const perspective = withProjection(orthographic, 'perspective')
    expect(visibleHeightAtTarget(orthographic)).toBeCloseTo(visibleHeightAtTarget(camera), 9)
    expect(visibleHeightAtTarget(perspective)).toBeCloseTo(visibleHeightAtTarget(camera), 9)
    expect(perspective.distance).toBeCloseTo(180, 9)
  })
})

describe('orbit', () => {
  const front = cameraWith({
    rotation: standardViewRotation('front'),
    target: [0, 0, 0],
    distance: 200,
  })
  const iso = cameraWith({
    rotation: standardViewRotation('iso'),
    target: [5, -3, 2],
    distance: 150,
  })

  it('U-48 orbit(camera, 100, 0, target) rotates the front view about world Z by -40 degrees', () => {
    const orbited = orbit(front, 100, 0, front.target)
    expect(forward(orbited)).toEqual(
      closeToVector([Math.sin(40 * DEGREE), Math.cos(40 * DEGREE), 0]),
    )
    expect(up(orbited)).toEqual(closeToVector([0, 0, 1]))
    expect(orbited.target).toEqual(closeToVector([0, 0, 0]))
    expect(orbited.distance).toBeCloseTo(200, 9)
  })

  it('U-48 orbit(camera, 100, 0, target) rotates any view about world Z by -40 degrees', () => {
    const turn = axisAngle([0, 0, 1], -40 * DEGREE)
    const orbited = orbit(iso, 100, 0, iso.target)
    expect(forward(orbited)).toEqual(closeToVector(rotate(turn, forward(iso))))
    expect(up(orbited)).toEqual(closeToVector(rotate(turn, up(iso))))
    expect(orbited.target).toEqual(closeToVector(iso.target))
  })

  it('vertical motion rotates about the camera right axis by -deltaYPixels x 0.4 degrees', () => {
    const turn = axisAngle([1, 0, 0], -40 * DEGREE)
    const orbited = orbit(front, 0, 100, front.target)
    expect(forward(orbited)).toEqual(closeToVector(rotate(turn, [0, 1, 0])))
    expect(up(orbited)).toEqual(closeToVector(rotate(turn, [0, 0, 1])))
  })

  it('applies the vertical rotation about the right axis after the horizontal rotation', () => {
    const horizontal = axisAngle([0, 0, 1], -20 * DEGREE)
    const vertical = axisAngle([1, 0, 0], -10 * DEGREE)
    const orbited = orbit(front, 50, 25, front.target)
    expect(forward(orbited)).toEqual(closeToVector(rotate(horizontal, rotate(vertical, [0, 1, 0]))))
    expect(up(orbited)).toEqual(closeToVector(rotate(horizontal, rotate(vertical, [0, 0, 1]))))
  })

  it('has no elevation clamp: a long vertical drag turns the view upside down', () => {
    const orbited = orbit(front, 0, 300, front.target)
    expect(up(orbited)[2]).toBeLessThan(0)
  })

  it('keeps the pivot fixed on screen: its camera-space position does not change', () => {
    const pivot: Vector3 = [10, 5, -4]
    const orbited = orbit(iso, 37, -23, pivot)
    expect(cameraSpace(orbited, pivot)).toEqual(closeToVector(cameraSpace(iso, pivot)))
    expect(orbited.distance).toBeCloseTo(iso.distance, 9)
  })

  it('moves the target when the pivot is not the target', () => {
    const orbited = orbit(front, 100, 0, [50, 0, 0])
    expect(orbited.target).not.toEqual(closeToVector(front.target))
  })
})

describe('pan', () => {
  it('U-48 pan moves the target by exactly the pixel delta times millimeters per pixel (perspective)', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('iso'),
      target: [5, -3, 2],
      distance: 200,
    })
    const millimetersPerPixel = (2 * 200 * TAN_15_DEGREES) / 600
    const axes = basis(camera)
    const panned = pan(camera, 30, 20, 600)
    const expected = add(
      camera.target,
      add(scale(axes.right, -30 * millimetersPerPixel), scale(axes.up, 20 * millimetersPerPixel)),
    )
    expect(panned.target).toEqual(closeToVector(expected))
  })

  it('U-48 pan moves the target by exactly the pixel delta times millimeters per pixel (orthographic)', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('front'),
      target: [0, 0, 0],
      projection: 'orthographic',
      orthographicHeight: 90,
    })
    const panned = pan(camera, -10, 5, 450)
    expect(panned.target).toEqual(closeToVector([2, 0, 1]))
  })

  it('keeps the point at the target depth under the cursor under the cursor', () => {
    const camera = cameraWith({
      rotation: standardViewRotation('iso'),
      target: [5, -3, 2],
      distance: 200,
    })
    const aspect = 1.5
    const cursor: readonly [number, number] = [0.2, -0.1]
    const grabbed = pointUnderCursor(camera, cursor, aspect)
    // 60 pixels right and 30 pixels down in a 600 pixel tall viewport, in normalized device coordinates.
    const moved: readonly [number, number] = [
      cursor[0] + (2 * 60) / (600 * aspect),
      cursor[1] - (2 * 30) / 600,
    ]
    const panned = pan(camera, 60, 30, 600)
    expect(normalizedDeviceCoordinates(panned, grabbed, aspect)).toEqual(closeToVector(moved))
  })

  it('keeps the rotation, distance and projection', () => {
    const camera = cameraWith({ rotation: standardViewRotation('iso'), distance: 200 })
    const panned = pan(camera, 30, 20, 600)
    expect(orientation(panned)).toEqual(closeToVector(orientation(camera)))
    expect(panned.distance).toBeCloseTo(200, 9)
    expect(panned.projection).toBe('perspective')
  })
})

describe('wheelZoomFactor', () => {
  it('U-48 a pixel delta of 100 gives 1.0015^100', () => {
    expect(wheelZoomFactor({ deltaY: 100, deltaMode: 0, ctrlKey: false }, 600)).toBeCloseTo(
      1.0015 ** 100,
      12,
    )
  })

  it('U-48 deltaMode 1 counts 16 pixels per line: deltaY 3 gives 1.0015^48', () => {
    expect(wheelZoomFactor({ deltaY: 3, deltaMode: 1, ctrlKey: false }, 600)).toBeCloseTo(
      1.0015 ** 48,
      12,
    )
  })

  it('U-48 a trackpad pinch (ctrlKey) of 100 pixels gives 1.01^100', () => {
    expect(wheelZoomFactor({ deltaY: 100, deltaMode: 0, ctrlKey: true }, 600)).toBeCloseTo(
      1.01 ** 100,
      12,
    )
  })

  it('deltaMode 2 counts one viewport height per page', () => {
    expect(wheelZoomFactor({ deltaY: 1, deltaMode: 2, ctrlKey: false }, 600)).toBeCloseTo(
      1.0015 ** 600,
      9,
    )
  })

  it('wheel forward (negative deltaY) zooms in by the inverse factor', () => {
    expect(wheelZoomFactor({ deltaY: -100, deltaMode: 0, ctrlKey: false }, 600)).toBeCloseTo(
      1.0015 ** -100,
      12,
    )
  })
})

describe('zoomAt', () => {
  const aspect = 1.5
  const cursor: readonly [number, number] = [0.4, -0.3]
  const perspective = cameraWith({
    rotation: standardViewRotation('iso'),
    target: [5, -3, 2],
    distance: 200,
  })
  const orthographic = cameraWith({
    rotation: standardViewRotation('iso'),
    target: [5, -3, 2],
    distance: 200,
    projection: 'orthographic',
    orthographicHeight: 80,
  })

  it.each([0.8, 1.25])(
    'U-47 zoomAt by %s keeps the world point under the cursor at the same NDC position in perspective',
    (factor) => {
      const grabbed = pointUnderCursor(perspective, cursor, aspect)
      const zoomed = zoomAt(perspective, factor, cursor, aspect)
      expect(zoomed.distance).toBeCloseTo(200 * factor, 9)
      expect(normalizedDeviceCoordinates(zoomed, grabbed, aspect)).toEqual(closeToVector(cursor))
    },
  )

  it.each([0.8, 1.25])(
    'U-47 zoomAt by %s keeps the world point under the cursor at the same NDC position in orthographic',
    (factor) => {
      const grabbed = pointUnderCursor(orthographic, cursor, aspect)
      const zoomed = zoomAt(orthographic, factor, cursor, aspect)
      expect(zoomed.orthographicHeight).toBeCloseTo(80 * factor, 9)
      expect(normalizedDeviceCoordinates(zoomed, grabbed, aspect)).toEqual(closeToVector(cursor))
    },
  )

  it('zoomAt with the cursor at the center keeps the target and the rotation', () => {
    const zoomed = zoomAt(perspective, 0.5, [0, 0], aspect)
    expect(zoomed.target).toEqual(closeToVector(perspective.target))
    expect(orientation(zoomed)).toEqual(closeToVector(orientation(perspective)))
  })

  it('U-47 zoomAt clamps distance to at least 0.01', () => {
    expect(zoomAt(cameraWith({ distance: 0.015 }), 0.5, [0, 0], aspect).distance).toBeCloseTo(
      0.01,
      12,
    )
  })

  it('U-47 zoomAt clamps distance to at most 1e7', () => {
    expect(zoomAt(cameraWith({ distance: 8e6 }), 2, [0, 0], aspect).distance).toBeCloseTo(1e7, 3)
  })

  it('U-47 zoomAt clamps orthographicHeight to at least 0.01', () => {
    const camera = cameraWith({ projection: 'orthographic', orthographicHeight: 0.015 })
    expect(zoomAt(camera, 0.5, [0, 0], aspect).orthographicHeight).toBeCloseTo(0.01, 12)
  })

  it('U-47 zoomAt clamps orthographicHeight to at most 1e7', () => {
    const camera = cameraWith({ projection: 'orthographic', orthographicHeight: 8e6 })
    expect(zoomAt(camera, 2, [0, 0], aspect).orthographicHeight).toBeCloseTo(1e7, 3)
  })
})
