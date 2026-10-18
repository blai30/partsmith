import type { BoundingBox, Placement } from '../generated/protocol'
import {
  eye,
  forward,
  normalizedPosition,
  pointAtDepth,
  windowAspect,
  windowCursor,
  windowPosition,
  type Camera,
  type ViewWindow,
} from '../viewport/camera'
import { addScaled, cross, dot, length, subtract, type Vector3 } from '../viewport/math'
import { distance, type Vector2 } from './geometry'

// Between sketch coordinates, model space and the canvas (ui.md 3.16.3): a sketch's placement maps
// (u, v) to model space (sketch.md 3.1), and the camera maps model space to canvas CSS pixels
// through the view window, in the free area's device coordinates (ui.md 2.11).

/** Pointer input is ignored when the view ray is within this of parallel to the sketch plane. */
const PARALLEL_RAY_TOLERANCE = 1e-6

/** The sketch +v direction in model space: normal × x axis (sketch.md 3.1). */
export function placementYAxis(placement: Placement): Vector3 {
  return cross(placement.normal, placement.x_axis)
}

/** The model-space point of sketch coordinates (u, v). */
export function sketchToWorld(placement: Placement, point: Vector2): Vector3 {
  const onX = addScaled(placement.origin, placement.x_axis, point[0])
  return addScaled(onX, placementYAxis(placement), point[1])
}

/** The sketch coordinates of a model-space point projected onto the sketch plane. */
export function worldToSketch(placement: Placement, world: Vector3): Vector2 {
  const offset = subtract(world, placement.origin)
  return [dot(offset, placement.x_axis), dot(offset, placementYAxis(placement))]
}

/** The model-space box around sketch points, or null without points. */
export function worldBounds(placement: Placement, points: readonly Vector2[]): BoundingBox | null {
  if (points.length === 0) {
    return null
  }
  const corners = points.map((point) => sketchToWorld(placement, point))
  const axis = (index: 0 | 1 | 2, pick: (...values: number[]) => number): number =>
    pick(...corners.map((corner) => corner[index]))
  return {
    min: [axis(0, Math.min), axis(1, Math.min), axis(2, Math.min)],
    max: [axis(0, Math.max), axis(1, Math.max), axis(2, Math.max)],
  }
}

/** Whether the view window has a canvas to map onto. */
function hasCanvas(view: ViewWindow): boolean {
  return view.width > 0 && view.height > 0
}

/**
 * The sketch point under a canvas pixel: where the camera ray through it meets the sketch plane.
 * Null when the ray is within 1e-6 of parallel to the plane or meets it behind the eye.
 */
export function pixelToSketch(
  camera: Camera,
  view: ViewWindow,
  placement: Placement,
  pixel: Vector2,
): Vector2 | null {
  if (!hasCanvas(view)) {
    return null
  }
  const aspect = windowAspect(view)
  const device = windowCursor(view, pixel[0], pixel[1])
  const perspective = camera.projection === 'perspective'
  const origin = perspective ? eye(camera) : pointAtDepth(camera, device, aspect, 0)
  const ray = perspective
    ? subtract(pointAtDepth(camera, device, aspect, 1), origin)
    : forward(camera)
  const rayLength = length(ray)
  const facing = dot(ray, placement.normal)
  if (!(rayLength > 0) || Math.abs(facing / rayLength) < PARALLEL_RAY_TOLERANCE) {
    return null
  }
  const travel = dot(subtract(placement.origin, origin), placement.normal) / facing
  if (perspective && !(travel > 0)) {
    return null
  }
  return worldToSketch(placement, addScaled(origin, ray, travel))
}

/** The canvas pixel of a sketch point, or null when it is behind a perspective eye. */
export function sketchToPixel(
  camera: Camera,
  view: ViewWindow,
  placement: Placement,
  point: Vector2,
): Vector2 | null {
  if (!hasCanvas(view)) {
    return null
  }
  const position = normalizedPosition(camera, sketchToWorld(placement, point), windowAspect(view))
  if (position === null) {
    return null
  }
  const pixel = windowPosition(view, position)
  return [pixel.x, pixel.y]
}

/**
 * Sketch millimeters per CSS pixel around a pixel: the mean of how far one pixel right and one
 * pixel down move on the sketch plane. Null where the plane cannot be reached.
 */
export function millimetersPerPixelAt(
  camera: Camera,
  view: ViewWindow,
  placement: Placement,
  pixel: Vector2,
): number | null {
  const here = pixelToSketch(camera, view, placement, pixel)
  const right = pixelToSketch(camera, view, placement, [pixel[0] + 1, pixel[1]])
  const below = pixelToSketch(camera, view, placement, [pixel[0], pixel[1] + 1])
  if (here === null || right === null || below === null) {
    return null
  }
  const across = distance(right, here)
  const down = distance(below, here)
  const mean = (across + down) / 2
  return mean > 0 && Number.isFinite(mean) ? mean : null
}
