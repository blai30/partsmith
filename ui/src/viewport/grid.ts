import type { Placement, Units } from '../generated/protocol'
import { selectDocumentUnits } from '../state/selectors'
import type { AppState } from '../state/store'
import { visibleHeightAtTarget, type Camera } from './camera'
import { boundsDiagonal, type Bounds, type Vector3 } from './math'
import { ORIGIN_PLANE_PLACEMENTS } from './planes'
import { onPlane, yAxisOf } from './sketch-drawables'

// The ground and sketch grid (ui.md 3.12): its spacing for the zoom, the plane it lies on, and its
// lines. It gives scale, so its spacing is mirrored in the viewport and the status bar.

/** Minor spacings in millimeters, smallest first: 1, 10, 100 and 1000 mm, or 0.1 to 100 in. */
const METRIC_STEPS = [1, 10, 100, 1000] as const
const INCH_STEPS = [2.54, 25.4, 254, 2540] as const

/** Minor lines are at least this many CSS pixels (times the UI scale) apart. */
const MINIMUM_GAP_PIXELS = 8

/** Every tenth line is a major line. */
const MAJOR_EVERY = 10

/** The grid square's side is at least this many millimeters. */
const MINIMUM_SIDE = 200

const GROUND: Placement = ORIGIN_PLANE_PLACEMENTS.XY

/** Minor grid spacing in millimeters for the scale at the camera target (2.11, 3.12). */
export function gridSpacing(millimetersPerPixel: number, units: Units): number {
  const steps = units === 'in' ? INCH_STEPS : METRIC_STEPS
  // The largest step when no step is far enough apart.
  return steps.find((step) => step / millimetersPerPixel >= MINIMUM_GAP_PIXELS) ?? steps[3]
}

/**
 * The plane the grid lies on: the XY plane in Model context and while a plane is picked for a new
 * sketch, the sketch plane while a sketch is edited (once its solution is known); null in Print and
 * while the grid is turned off (3.12).
 */
export function gridPlacement(state: AppState): Placement | null {
  if (!state.view.grid || state.context === 'print') {
    return null
  }
  const sketchId = state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null
  if (sketchId === null) {
    return GROUND
  }
  return state.document?.sketches[sketchId]?.solution.placement ?? null
}

/** The world size of one CSS pixel at the camera target, times the UI scale. */
export function millimetersPerPixelAtTarget(
  camera: Camera,
  height: number,
  uiScale: number,
): number {
  return (visibleHeightAtTarget(camera) / Math.max(height, 1)) * uiScale
}

/** The minor spacing of the grid drawn for the state and camera, or null when none is drawn. */
export function drawnGridSpacing(state: AppState, camera: Camera, height: number): number | null {
  if (gridPlacement(state) === null) {
    return null
  }
  return gridSpacing(
    millimetersPerPixelAtTarget(camera, height, state.settings.uiScale),
    selectDocumentUnits(state),
  )
}

/**
 * Half the side of the grid square: half of max(200 mm, 2 × the diagonal of the visible bounds),
 * rounded out to whole major cells (3.12).
 */
export function gridHalfSide(bounds: Bounds | null, spacing: number): number {
  const diagonal = bounds === null ? 0 : boundsDiagonal(bounds)
  const major = spacing * MAJOR_EVERY
  return Math.ceil(Math.max(MINIMUM_SIDE, 2 * diagonal) / 2 / major) * major
}

/** The world point at (x, y) in a placement's plane. */
function planePoint(placement: Placement, x: number, y: number): Vector3 {
  return onPlane(placement, yAxisOf(placement), [x, y])
}

/** The grid's line segments, two points each: the minor lines and every tenth (major) line. */
export type GridSegments = { readonly minor: Float32Array; readonly major: Float32Array }

/** The lines of a square grid of half side `half` on the placement, centered on its origin. */
export function gridSegments(placement: Placement, spacing: number, half: number): GridSegments {
  const count = Math.round(half / spacing)
  const minor: number[] = []
  const major: number[] = []
  for (let i = -count; i <= count; i++) {
    const offset = i * spacing
    const target = i % MAJOR_EVERY === 0 ? major : minor
    // One line along each axis through this offset.
    target.push(...planePoint(placement, -half, offset), ...planePoint(placement, half, offset))
    target.push(...planePoint(placement, offset, -half), ...planePoint(placement, offset, half))
  }
  return { minor: new Float32Array(minor), major: new Float32Array(major) }
}

/** The corners of the grid square, for the depth range. */
export function gridCorners(placement: Placement, half: number): readonly Vector3[] {
  return [
    planePoint(placement, -half, -half),
    planePoint(placement, half, -half),
    planePoint(placement, half, half),
    planePoint(placement, -half, half),
  ]
}
