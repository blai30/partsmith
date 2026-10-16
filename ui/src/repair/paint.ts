import type { GhostKind, Repair } from '../generated/protocol'
import type { AppState } from '../state/store'
import { framedArea } from '../viewport/camera'
import type { RenderedFrame } from '../viewport/controller'
import type { Vector3 } from '../viewport/math'
import { placeCircle, visiblePosition } from '../viewport/overlay'
import { ghostSegments, selectRepairMode } from './repair'

// Repair mode's ghosts and rings in a rendered frame (ui.md 3.7, 3.12): the overlay draws them
// in canvas pixels from their model-space points, so a ghost shows through the part without a
// depth test, where the element often is now.

/** The radius in CSS pixels of a ring of a `points` ghost: 8 pixels across (3.7). */
const POINT_RADIUS = 4

/** The ghost buffer an element repair names in the latest recompute, or undefined. */
export function ghostBuffer(state: AppState, repair: Repair): Float32Array | undefined {
  const ghost = repair.kind === 'element' ? (repair.ghost ?? null) : null
  return ghost === null || ghost.frame !== 'world'
    ? undefined
    : state.document?.ghosts[ghost.positions]
}

/** Outline segments by ghost buffer, so a frame does not find a face's boundary again. */
const outlines = new WeakMap<Float32Array, Float32Array>()

function outlineOf(kind: GhostKind, positions: Float32Array): Float32Array {
  const cached = outlines.get(positions)
  if (cached !== undefined) {
    return cached
  }
  const segments = ghostSegments(kind, positions)
  outlines.set(positions, segments)
  return segments
}

/** A ring of radius `radius` around a canvas point, as SVG path data. */
function ring(point: { readonly x: number; readonly y: number }, radius: number): string {
  const size = String(radius)
  const across = String(radius * 2)
  return `M ${String(point.x - radius)} ${String(point.y)} a ${size} ${size} 0 1 0 ${across} 0 a ${size} ${size} 0 1 0 -${across} 0`
}

/** The path data of a ghost in a frame: its segments, or a ring per point for `points`. */
function ghostPath(kind: GhostKind, positions: Float32Array, frame: RenderedFrame): string {
  const at = (data: Float32Array, index: number) =>
    frame.project([data[index] ?? 0, data[index + 1] ?? 0, data[index + 2] ?? 0])
  const parts: string[] = []
  if (kind === 'points') {
    for (let i = 0; i + 3 <= positions.length; i += 3) {
      const point = at(positions, i)
      if (point !== null) {
        parts.push(ring(point, POINT_RADIUS))
      }
    }
    return parts.join(' ')
  }
  const segments = outlineOf(kind, positions)
  for (let i = 0; i + 6 <= segments.length; i += 6) {
    const start = at(segments, i)
    const end = at(segments, i + 3)
    if (start !== null && end !== null) {
      parts.push(`M ${String(start.x)} ${String(start.y)} L ${String(end.x)} ${String(end.y)}`)
    }
  }
  return parts.join(' ')
}

/**
 * Draws repair mode's ghosts and rings for a rendered frame (3.12): each ghost's path from its
 * segments' projections, and each ring at its centroid's projection, hidden behind the eye or
 * outside the free area. The candidate markers are anchored labels the controller places.
 */
export function paintRepairOverlay(
  overlay: HTMLElement,
  frame: RenderedFrame,
  state: AppState,
): void {
  const mode = selectRepairMode(state)
  if (mode === null) {
    return
  }
  for (const path of overlay.querySelectorAll<SVGPathElement>('[data-testid="repair-ghost"]')) {
    const repair = mode.repairs[Number(path.dataset['repair'])]
    const positions = repair === undefined ? undefined : ghostBuffer(state, repair)
    const kind = repair?.kind === 'element' ? repair.ghost?.kind : undefined
    path.setAttribute(
      'd',
      positions === undefined || kind === undefined ? '' : ghostPath(kind, positions, frame),
    )
  }
  const area = framedArea(frame.view)
  for (const marker of overlay.querySelectorAll<SVGCircleElement>(
    '[data-testid="repair-marker"]',
  )) {
    const point: Vector3 = [
      Number(marker.dataset['x']),
      Number(marker.dataset['y']),
      Number(marker.dataset['z']),
    ]
    placeCircle(marker, visiblePosition(frame.project(point), area))
  }
}
