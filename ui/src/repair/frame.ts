import { listedElementsOrNone } from '../app/actions/element-lists'
import type { App } from '../app/createApp'
import type { BoundingBox, MeshElement } from '../generated/protocol'
import { DEFAULT_FIT_MARGIN } from '../viewport/camera'
import {
  boundsCenter,
  extendBounds,
  pointsOf,
  scale,
  subtract,
  type Bounds,
  type Vector3,
} from '../viewport/math'
import { sketchBounds } from '../viewport/sketch-drawables'
import {
  ghostBuffer,
  regionCandidatePlaces,
  type RepairCandidateView,
  type RepairMode,
} from './repair'

// Where repair mode's candidate markers go and what it frames (ui.md 3.7).

/** The share of the body's size on each axis that repair mode frames at least (3.7). */
const LEAST_REPAIR_SHARE = 1 / 3

/**
 * The centroid of each element in its body's model-layer mesh (3.7): the `centroid` of the
 * `list_elements` descriptor at its display id; null for none, for an element of a mesh the body
 * no longer shows, or when the list cannot be had.
 */
export function elementCentroids(
  app: App,
  elements: readonly (MeshElement | null)[],
): Promise<readonly (Vector3 | null)[]> {
  return Promise.all(
    elements.map(async (element) => {
      if (element === null) {
        return null
      }
      const descriptors = await listedElementsOrNone(
        app,
        element.body,
        element.kind,
        element.mesh_id,
      )
      return descriptors[element.index]?.centroid ?? null
    }),
  )
}

/** Where the markers of element candidates go: their elements' centroids, null for none (3.7). */
export function candidatePoints(
  app: App,
  candidates: readonly RepairCandidateView[],
): Promise<readonly (Vector3 | null)[]> {
  return elementCentroids(
    app,
    candidates.map((candidate) => candidate.element),
  )
}

/** The points repair mode frames, and the box of which it frames at least a third on each axis (3.7). */
type RepairFrame = {
  readonly points: readonly Vector3[]
  readonly around: Bounds | null
}

/** The points that have a place, in order. */
function placed(points: readonly (Vector3 | null)[]): Vector3[] {
  return points.filter((point): point is Vector3 => point !== null)
}

/**
 * What repair mode frames (3.7): for an element repair the ghost's points, else the stored
 * centroid, with the candidates' markers, in the repair's body; for a region repair the
 * candidates' numbers in their sketch's drawing.
 */
async function repairFrame(app: App, mode: RepairMode): Promise<RepairFrame> {
  const { repair, candidates } = mode.view
  const state = app.store.getState()
  if (repair.kind === 'region') {
    const solution = state.document?.sketches[repair.sketch]?.solution
    return solution === undefined
      ? { points: [], around: null }
      : {
          points: placed(regionCandidatePlaces(repair, solution).anchors),
          around: sketchBounds(solution),
        }
  }
  const buffer = ghostBuffer(state, repair)
  const ghost = buffer === undefined ? [] : pointsOf(buffer)
  const lost: readonly Vector3[] =
    ghost.length > 0
      ? ghost
      : repair.centroid === null || repair.centroid === undefined
        ? []
        : [repair.centroid]
  const around = state.document?.meshes[repair.body]?.boundingBox ?? null
  const markers = await candidatePoints(app, candidates)
  return { points: [...lost, ...placed(markers)], around }
}

/** A box grown about its center to at least `least` on each axis. */
function grownTo(bounds: Bounds, least: Vector3): BoundingBox {
  const center = boundsCenter(bounds)
  const half = (axis: 0 | 1 | 2) => Math.max(bounds.max[axis] - bounds.min[axis], least[axis]) / 2
  const along = (axis: 0 | 1 | 2, sign: number) => center[axis] + sign * half(axis)
  return {
    min: [along(0, -1), along(1, -1), along(2, -1)],
    max: [along(0, 1), along(1, 1), along(2, 1)],
  }
}

/** Frames what repair mode shows, grown about its center to at least a third of what is around it on each axis (3.7). */
export async function frameRepair(app: App, mode: RepairMode): Promise<void> {
  const { points, around } = await repairFrame(app, mode)
  const bounds = extendBounds(null, points)
  const viewport = app.viewport()
  if (bounds === null || viewport === null) {
    return
  }
  const least =
    around === null
      ? ([0, 0, 0] as const)
      : scale(subtract(around.max, around.min), LEAST_REPAIR_SHARE)
  viewport.zoomToBounds(grownTo(bounds, least), DEFAULT_FIT_MARGIN)
}
