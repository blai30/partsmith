import type { OriginPlane, WorldAxis } from '../core/labels'
import type { FeatureRecord, Placement } from '../generated/protocol'
import type { AppState } from '../state/store'
import { isSessionVisible } from '../state/visibility'
import {
  addScaled,
  boundsCenter,
  boundsDiagonal,
  cross,
  dot,
  normalize,
  subtract,
  type Bounds,
  type Vector3,
} from './math'
import { activeFieldKind } from './picking-rules'

// The drawn datum and origin planes (ui.md 3.10, 3.12, 3.16.1): squares sized to the visible
// bounds, and which origin planes are drawn.

/** The smallest side of a drawn datum or origin plane, in mm. */
const MINIMUM_PLANE_SIDE = 50
const PLANE_SIDE_FACTOR = 1.2

/** The frame of each origin plane: its sketch origin, x axis and normal (architecture.md). */
export const ORIGIN_PLANE_PLACEMENTS: Readonly<Record<OriginPlane, Placement>> = {
  XY: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  XZ: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  YZ: { origin: [0, 0, 0], x_axis: [0, 1, 0], normal: [1, 0, 0] },
}

/** The origin planes in naming order. */
export const ORIGIN_PLANES: readonly OriginPlane[] = ['XY', 'XZ', 'YZ']

/** The world axes along each origin plane's sketch x and y axes, as its frame above runs them. */
export const ORIGIN_PLANE_AXES: Readonly<Record<OriginPlane, readonly [WorldAxis, WorldAxis]>> = {
  XY: ['X', 'Y'],
  XZ: ['X', 'Z'],
  YZ: ['Y', 'Z'],
}

/** The world axis along each origin plane's normal, whose color the plane is drawn in (3.12). */
export const ORIGIN_PLANE_NORMAL_AXES: Readonly<Record<OriginPlane, WorldAxis>> = {
  XY: 'Z',
  XZ: 'Y',
  YZ: 'X',
}

/**
 * The world axes along a sketch's x and y axes while it lies on an origin plane, or null on a datum
 * plane or a face, whose axes are not world axes (3.16.2).
 */
export function sketchWorldAxes(record: FeatureRecord): readonly [WorldAxis, WorldAxis] | null {
  const plane: unknown = record['plane']
  if (typeof plane !== 'object' || plane === null || !('kind' in plane) || !('plane' in plane)) {
    return null
  }
  const origin = ORIGIN_PLANES.find((candidate) => candidate === plane.plane)
  return plane.kind === 'origin' && origin !== undefined ? ORIGIN_PLANE_AXES[origin] : null
}

/** Side and center of drawn planes: max(50 mm, 1.2 × the visible diagonal) about the visible center (3.12). */
export function planeExtent(bounds: Bounds | null): { side: number; center: Vector3 } {
  if (bounds === null) {
    return { side: MINIMUM_PLANE_SIDE, center: [0, 0, 0] }
  }
  return {
    side: Math.max(MINIMUM_PLANE_SIDE, PLANE_SIDE_FACTOR * boundsDiagonal(bounds)),
    center: boundsCenter(bounds),
  }
}

/** The 4 corners of a square plane of this side centered at the projection of `center` onto it. */
export function squareCorners(
  placement: Placement,
  side: number,
  center: Vector3,
): [Vector3, Vector3, Vector3, Vector3] {
  const normal = normalize(placement.normal) ?? [0, 0, 1]
  const xAxis = normalize(placement.x_axis) ?? [1, 0, 0]
  const yAxis = cross(normal, xAxis)
  const offset = subtract(center, placement.origin)
  const middle = addScaled(center, normal, -dot(offset, normal))
  const half = side / 2
  return [
    addScaled(addScaled(middle, xAxis, -half), yAxis, -half),
    addScaled(addScaled(middle, xAxis, half), yAxis, -half),
    addScaled(addScaled(middle, xAxis, half), yAxis, half),
    addScaled(addScaled(middle, xAxis, -half), yAxis, half),
  ]
}

function originPlanesShown(state: AppState): boolean {
  if (state.context === 'sketch' && state.sketch?.sketchId === null) {
    return true
  }
  return activeFieldKind(state) === 'plane'
}

/**
 * The origin planes the scene draws: all three while a plane is picked for a new sketch or a plane
 * field is active, otherwise those shown this session (3.10, 3.12).
 */
export function drawnOriginPlanes(state: AppState): readonly OriginPlane[] {
  const shown = originPlanesShown(state)
  return ORIGIN_PLANES.filter((plane) => shown || isSessionVisible(state, `origin:${plane}`))
}

/**
 * Where an origin plane's label goes (3.16.1): the corner of its drawn square in the plane's
 * positive first and second axis directions.
 */
export function originPlaneLabelAnchor(plane: OriginPlane, bounds: Bounds | null): Vector3 {
  const extent = planeExtent(bounds)
  const [, , corner] = squareCorners(ORIGIN_PLANE_PLACEMENTS[plane], extent.side, extent.center)
  return corner
}
