import {
  add,
  addScaled,
  angleBetween,
  boundsCenter,
  boundsCorners,
  cross,
  dot,
  multiplyQuaternions,
  normalize,
  normalizeQuaternion,
  quaternionFromAxisAngle,
  quaternionFromBasis,
  rotateVector,
  scale,
  slerp,
  subtract,
  type Bounds,
  type Quaternion,
  type Vector3,
} from './math'

export type { Bounds, Quaternion, Vector3 } from './math'

export type StandardView = 'top' | 'bottom' | 'front' | 'back' | 'left' | 'right' | 'iso'
export type Projection = 'perspective' | 'orthographic'

/** Lengths in mm, angles in radians (ui.md 2.11). */
export type Camera = {
  readonly target: Vector3
  /** Camera-to-world rotation. The camera looks along its local -Z; local +Y is up; local +X is right. */
  readonly rotation: Quaternion
  /** Eye distance from the target. */
  readonly distance: number
  readonly projection: Projection
  /** Vertical field of view, perspective only. */
  readonly verticalFieldOfView: number
  /** Visible height at the target, orthographic only. */
  readonly orthographicHeight: number
}

const DEGREE = Math.PI / 180

/** 30 degrees. */
export const DEFAULT_VERTICAL_FIELD_OF_VIEW = 30 * DEGREE
/** Orbit rate of 0.4 degrees per CSS pixel (3.13). */
export const ORBIT_RADIANS_PER_PIXEL = 0.4 * DEGREE
/** Bounds of `distance` and `orthographicHeight` under zoom. */
export const MINIMUM_ZOOM_EXTENT = 0.01
export const MAXIMUM_ZOOM_EXTENT = 1e7
/** Default `fitBounds` margin: content fills 80 percent of the limiting dimension. */
export const DEFAULT_FIT_MARGIN = 0.1

export const STANDARD_VIEWS: readonly StandardView[] = [
  'top',
  'bottom',
  'front',
  'back',
  'left',
  'right',
  'iso',
]

// Tolerance of classifyView, in radians.
const VIEW_TOLERANCE = 1e-6
// Extents below this are treated as an empty box by fitBounds.
const DEGENERATE_EXTENT = 1e-9
// Side of the cube fitBounds frames for an empty box.
const DEGENERATE_BOX_SIDE = 100
// The smallest perspective near depth (2.11).
const MINIMUM_NEAR = 0.01
// The smallest near-to-far span depthRange returns, so a point-sized scene still has a valid projection.
const MINIMUM_DEPTH_SPAN = 1e-3

const WORLD_UP: Vector3 = [0, 0, 1]

// Looking direction and up hint of each standard view (2.11). Iso looks from (1, -1, 1) with +Z projected as up.
const STANDARD_VIEW_AXES: Readonly<
  Record<StandardView, { readonly forward: Vector3; readonly upHint: Vector3 }>
> = {
  front: { forward: [0, 1, 0], upHint: WORLD_UP },
  back: { forward: [0, -1, 0], upHint: WORLD_UP },
  right: { forward: [-1, 0, 0], upHint: WORLD_UP },
  left: { forward: [1, 0, 0], upHint: WORLD_UP },
  top: { forward: [0, 0, -1], upHint: [0, 1, 0] },
  bottom: { forward: [0, 0, 1], upHint: [0, -1, 0] },
  iso: { forward: [-1, 1, -1], upHint: WORLD_UP },
}

// The component of `vector` perpendicular to the unit vector `axis`.
function perpendicularPart(vector: Vector3, axis: Vector3): Vector3 {
  return addScaled(vector, axis, -dot(vector, axis))
}

// The world axis least aligned with a unit vector, used when an up hint is parallel to the looking direction.
function leastAlignedAxis(unit: Vector3): Vector3 {
  const absolute = unit.map(Math.abs)
  const smallest = absolute.indexOf(Math.min(...absolute))
  return [smallest === 0 ? 1 : 0, smallest === 1 ? 1 : 0, smallest === 2 ? 1 : 0]
}

/**
 * The camera rotation that looks along `direction` with up taken from `upHint` projected onto the view plane. When the hint is parallel to the direction, the world axis least aligned with it is used instead.
 */
export function rotationLookingAlong(direction: Vector3, upHint: Vector3): Quaternion {
  const forwardAxis = normalize(direction)
  if (forwardAxis === null) {
    throw new RangeError('rotationLookingAlong needs a non-zero direction.')
  }
  const upAxis =
    normalize(perpendicularPart(upHint, forwardAxis), 1e-9) ??
    normalize(perpendicularPart(leastAlignedAxis(forwardAxis), forwardAxis)) ??
    WORLD_UP
  const rightAxis = cross(forwardAxis, upAxis)
  return quaternionFromBasis(rightAxis, upAxis, scale(forwardAxis, -1))
}

export function standardViewRotation(view: StandardView): Quaternion {
  const { forward: viewForward, upHint } = STANDARD_VIEW_AXES[view]
  return rotationLookingAlong(viewForward, upHint)
}

/** The iso view, target [0, 0, 0], distance 200, perspective, orthographicHeight 2 × 200 × tan(15°). */
export const DEFAULT_CAMERA: Camera = {
  target: [0, 0, 0],
  rotation: standardViewRotation('iso'),
  distance: 200,
  projection: 'perspective',
  verticalFieldOfView: DEFAULT_VERTICAL_FIELD_OF_VIEW,
  orthographicHeight: 2 * 200 * Math.tan(DEFAULT_VERTICAL_FIELD_OF_VIEW / 2),
}

/** The world direction the camera looks along. */
export function forward(camera: Camera): Vector3 {
  return rotateVector(camera.rotation, [0, 0, -1])
}

export function up(camera: Camera): Vector3 {
  return rotateVector(camera.rotation, [0, 1, 0])
}

export function right(camera: Camera): Vector3 {
  return rotateVector(camera.rotation, [1, 0, 0])
}

export function eye(camera: Camera): Vector3 {
  return addScaled(camera.target, forward(camera), -camera.distance)
}

/** Column-major world-to-view matrix: the eye at the origin looking along -Z. */
export function viewMatrix(camera: Camera): Float32Array {
  const rightAxis = right(camera)
  const upAxis = up(camera)
  const backAxis = scale(forward(camera), -1)
  const eyePosition = eye(camera)
  return new Float32Array([
    rightAxis[0],
    upAxis[0],
    backAxis[0],
    0,
    rightAxis[1],
    upAxis[1],
    backAxis[1],
    0,
    rightAxis[2],
    upAxis[2],
    backAxis[2],
    0,
    -dot(rightAxis, eyePosition),
    -dot(upAxis, eyePosition),
    -dot(backAxis, eyePosition),
    1,
  ])
}

/** Column-major projection to clip space; `near` and `far` are view depths along the forward axis. */
export function projectionMatrix(
  camera: Camera,
  aspect: number,
  near: number,
  far: number,
): Float32Array {
  const depthScale = 1 / (near - far)
  if (camera.projection === 'perspective') {
    const focal = 1 / Math.tan(camera.verticalFieldOfView / 2)
    return new Float32Array([
      focal / aspect,
      0,
      0,
      0,
      0,
      focal,
      0,
      0,
      0,
      0,
      (far + near) * depthScale,
      -1,
      0,
      0,
      2 * far * near * depthScale,
      0,
    ])
  }
  const halfHeight = camera.orthographicHeight / 2
  const halfWidth = halfHeight * aspect
  return new Float32Array([
    1 / halfWidth,
    0,
    0,
    0,
    0,
    1 / halfHeight,
    0,
    0,
    0,
    0,
    2 * depthScale,
    0,
    0,
    0,
    (far + near) * depthScale,
    1,
  ])
}

/** Near and far view depths that cover the scene's bounding sphere with 10 percent slack (2.11). */
export function depthRange(
  camera: Camera,
  sceneCenter: Vector3,
  sceneRadius: number,
): { near: number; far: number } {
  const centerDepth = dot(subtract(sceneCenter, eye(camera)), forward(camera))
  const reach = 1.1 * sceneRadius
  const far = centerDepth + reach
  const near =
    camera.projection === 'perspective'
      ? Math.max(centerDepth - reach, 0.001 * far, MINIMUM_NEAR)
      : centerDepth - reach
  return { near, far: Math.max(far, near + MINIMUM_DEPTH_SPAN) }
}

export function visibleHeightAtTarget(camera: Camera): number {
  return camera.projection === 'perspective'
    ? 2 * camera.distance * Math.tan(camera.verticalFieldOfView / 2)
    : camera.orthographicHeight
}

/** The standard view whose forward and up vectors are both within 1e-6 rad of the camera's, else 'custom'. */
export function classifyView(camera: Camera): StandardView | 'custom' {
  const cameraForward = forward(camera)
  const cameraUp = up(camera)
  for (const view of STANDARD_VIEWS) {
    const viewCamera = { ...camera, rotation: standardViewRotation(view) }
    if (
      angleBetween(cameraForward, forward(viewCamera)) <= VIEW_TOLERANCE &&
      angleBetween(cameraUp, up(viewCamera)) <= VIEW_TOLERANCE
    ) {
      return view
    }
  }
  return 'custom'
}

/**
 * Turns the camera about `pivot`: horizontal motion about the world +Z axis by -deltaXPixels × 0.4°, then vertical motion about the camera's right axis by -deltaYPixels × 0.4°, with no elevation clamp. The pivot keeps its place on screen.
 */
export function orbit(
  camera: Camera,
  deltaXPixels: number,
  deltaYPixels: number,
  pivot: Vector3,
): Camera {
  const horizontal = quaternionFromAxisAngle(WORLD_UP, -deltaXPixels * ORBIT_RADIANS_PER_PIXEL)
  const turnedRight = rotateVector(horizontal, right(camera))
  const vertical = quaternionFromAxisAngle(turnedRight, -deltaYPixels * ORBIT_RADIANS_PER_PIXEL)
  const turn = multiplyQuaternions(vertical, horizontal)
  return {
    ...camera,
    rotation: normalizeQuaternion(multiplyQuaternions(turn, camera.rotation)),
    target: add(pivot, rotateVector(turn, subtract(camera.target, pivot))),
  }
}

/** Moves the target so the point at the target depth under the cursor follows the cursor exactly. */
export function pan(
  camera: Camera,
  deltaXPixels: number,
  deltaYPixels: number,
  viewportHeightPixels: number,
): Camera {
  if (!(viewportHeightPixels > 0)) {
    return camera
  }
  const millimetersPerPixel = visibleHeightAtTarget(camera) / viewportHeightPixels
  const moved = addScaled(camera.target, right(camera), -deltaXPixels * millimetersPerPixel)
  return { ...camera, target: addScaled(moved, up(camera), deltaYPixels * millimetersPerPixel) }
}

function clampZoomExtent(value: number): number {
  return Math.min(Math.max(value, MINIMUM_ZOOM_EXTENT), MAXIMUM_ZOOM_EXTENT)
}

/**
 * Zooms by `factor` (above 1 zooms out) about the cursor, given in normalized device coordinates (each -1 to 1): the world point at the target depth under the cursor stays under it.
 */
export function zoomAt(
  camera: Camera,
  factor: number,
  cursorNormalized: readonly [number, number],
  aspect: number,
): Camera {
  const halfHeight = visibleHeightAtTarget(camera) / 2
  const offset = add(
    scale(right(camera), cursorNormalized[0] * halfHeight * aspect),
    scale(up(camera), cursorNormalized[1] * halfHeight),
  )
  if (camera.projection === 'perspective') {
    const distance = clampZoomExtent(camera.distance * factor)
    const target = addScaled(camera.target, offset, 1 - distance / camera.distance)
    return { ...camera, distance, target }
  }
  const orthographicHeight = clampZoomExtent(camera.orthographicHeight * factor)
  const target = addScaled(camera.target, offset, 1 - orthographicHeight / camera.orthographicHeight)
  return { ...camera, orthographicHeight, target }
}

/**
 * Frames `bounds` keeping the rotation (2.11): content fills `1 - 2 × margin` of the limiting dimension. An empty or degenerate box is framed as a 100 mm cube around its center.
 */
export function fitBounds(
  camera: Camera,
  bounds: Bounds,
  aspect: number,
  margin = DEFAULT_FIT_MARGIN,
): Camera {
  if (!(margin >= 0 && margin < 0.5)) {
    throw new RangeError(`fitBounds needs a margin from 0 to below 0.5, not ${String(margin)}.`)
  }
  const center = boundsCenter(bounds)
  const size = subtract(bounds.max, bounds.min)
  const framed: Bounds = size.every((extent) => Math.abs(extent) < DEGENERATE_EXTENT)
    ? {
        min: addScaled(center, [1, 1, 1], -DEGENERATE_BOX_SIDE / 2),
        max: addScaled(center, [1, 1, 1], DEGENERATE_BOX_SIDE / 2),
      }
    : bounds
  const axes = [right(camera), up(camera), forward(camera)] as const
  const minimum = [Infinity, Infinity, Infinity]
  const maximum = [-Infinity, -Infinity, -Infinity]
  for (const corner of boundsCorners(framed)) {
    axes.forEach((axis, index) => {
      const coordinate = dot(corner, axis)
      minimum[index] = Math.min(minimum[index] ?? Infinity, coordinate)
      maximum[index] = Math.max(maximum[index] ?? -Infinity, coordinate)
    })
  }
  const extent = (index: number): number => (maximum[index] ?? 0) - (minimum[index] ?? 0)
  const visibleHeight = Math.max(extent(1), extent(0) / aspect) / (1 - 2 * margin)
  if (camera.projection === 'orthographic') {
    return { ...camera, target: center, orthographicHeight: visibleHeight }
  }
  const distance =
    visibleHeight / (2 * Math.tan(camera.verticalFieldOfView / 2)) + extent(2) / 2
  return { ...camera, target: center, distance }
}

/** Switches the projection keeping the apparent size at the target. */
export function withProjection(camera: Camera, projection: Projection): Camera {
  if (camera.projection === projection) {
    return camera
  }
  const tangent = Math.tan(camera.verticalFieldOfView / 2)
  return projection === 'orthographic'
    ? { ...camera, projection, orthographicHeight: 2 * camera.distance * tangent }
    : { ...camera, projection, distance: camera.orthographicHeight / (2 * tangent) }
}

/** The zoom factor of a wheel event: 1.0015 per pixel, or 1.01 per pixel for a trackpad pinch (ctrlKey). */
export function wheelZoomFactor(
  event: { deltaY: number; deltaMode: number; ctrlKey: boolean },
  viewportHeightPixels: number,
): number {
  const pixelsPerUnit =
    event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? viewportHeightPixels : 1
  return (event.ctrlKey ? 1.01 : 1.0015) ** (event.deltaY * pixelsPerUnit)
}

/** The world point at view depth `depth` (along the forward axis) under a cursor in normalized device coordinates. */
export function pointAtDepth(
  camera: Camera,
  cursorNormalized: readonly [number, number],
  aspect: number,
  depth: number,
): Vector3 {
  const halfHeight =
    camera.projection === 'perspective'
      ? depth * Math.tan(camera.verticalFieldOfView / 2)
      : camera.orthographicHeight / 2
  const onAxis = addScaled(eye(camera), forward(camera), depth)
  return add(
    onAxis,
    add(
      scale(right(camera), cursorNormalized[0] * halfHeight * aspect),
      scale(up(camera), cursorNormalized[1] * halfHeight),
    ),
  )
}

/** A world point in normalized device coordinates with its view depth, or null when it is not in front of a perspective eye. */
export function normalizedPosition(
  camera: Camera,
  point: Vector3,
  aspect: number,
): { readonly x: number; readonly y: number; readonly depth: number } | null {
  const relative = subtract(point, eye(camera))
  const depth = dot(relative, forward(camera))
  if (camera.projection === 'perspective' && depth <= 0) {
    return null
  }
  const halfHeight =
    camera.projection === 'perspective'
      ? depth * Math.tan(camera.verticalFieldOfView / 2)
      : camera.orthographicHeight / 2
  return {
    x: dot(relative, right(camera)) / (halfHeight * aspect),
    y: dot(relative, up(camera)) / halfHeight,
    depth,
  }
}

/** Cubic ease-out of camera animations (2.11), for `amount` from 0 to 1. */
export function easeOutCubic(amount: number): number {
  return 1 - (1 - amount) ** 3
}

/**
 * The camera `amount` of the way from `from` to `to` (0 to 1): the rotation turns along the shorter arc, lengths change geometrically and the projection is `to`'s.
 */
export function interpolateCameras(from: Camera, to: Camera, amount: number): Camera {
  if (amount >= 1) {
    return to
  }
  const geometric = (start: number, end: number): number =>
    start > 0 && end > 0 ? start * (end / start) ** amount : start + (end - start) * amount
  return {
    ...to,
    target: addScaled(from.target, subtract(to.target, from.target), amount),
    rotation: slerp(from.rotation, to.rotation, amount),
    distance: geometric(from.distance, to.distance),
    orthographicHeight: geometric(from.orthographicHeight, to.orthographicHeight),
  }
}
