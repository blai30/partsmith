import { formatNumber, formatPoint, numberText, roundToDecimals } from '../expression/format'
import type { Plane, Suggestion } from '../generated/protocol'
import { memoized } from '../state/selectors'
import type { AppState } from '../state/store'
import {
  add,
  boundsCorners,
  cross,
  dot,
  length,
  normalize,
  scale,
  subtract,
  type Bounds,
  type Vector3,
} from '../viewport/math'
import { findingAt } from './checks'

// The words of a print suggestion and the split planes the viewport draws (ui.md 3.17, print.md
// 3.12, ADR 0040).

/** A split plane whose normal is within this of a model axis reads as that axis (print.md 3.12). */
const AXIS_TOLERANCE = 1e-9
const AXIS_NAMES = ['x', 'y', 'z'] as const
/** Decimals of a tilted plane's normal components. */
const NORMAL_DECIMALS = 3
/** Points within this many mm of a plane lie on it, and points within it of each other are one. */
const OUTLINE_TOLERANCE = 1e-9

/** The words of one split plane: `x = 150 mm` along a model axis, else its origin and normal. */
function planeWords(plane: Plane): string {
  const axis = plane.normal.findIndex((component) => Math.abs(component) >= 1 - AXIS_TOLERANCE)
  const name = AXIS_NAMES[axis]
  if (name !== undefined) {
    return `${name} = ${formatNumber(plane.origin[axis] ?? 0, 'length', 'mm')} mm`
  }
  const normal = plane.normal.map((component) =>
    numberText(roundToDecimals(component, NORMAL_DECIMALS)),
  )
  return `the plane through (${formatPoint(plane.origin, 'mm')}) with normal (${normal.join(', ')})`
}

/**
 * A suggestion in the words of print.md 3.12, in millimeters whatever the document's unit: "Split
 * at x = 150 mm" or "Add a drain hole near (5, 5, 5 mm)". Null for a split without planes, which
 * has no words.
 */
export function suggestionText(suggestion: Suggestion): string | null {
  if (suggestion.kind === 'drain_hole') {
    return `Add a drain hole near (${formatPoint(suggestion.at, 'mm')})`
  }
  const words = suggestion.planes.map(planeWords)
  const last = words.at(-1)
  if (last === undefined) {
    return null
  }
  const others = words.slice(0, -1)
  return others.length === 0 ? `Split at ${last}` : `Split at ${others.join(', ')} and ${last}`
}

/** The 12 edges of a box, each as its two corners. */
function boxEdges(box: Bounds): (readonly [Vector3, Vector3])[] {
  const corners = boundsCorners(box)
  // `boundsCorners` gives x, y and z one bit of a corner's index each, so two corners share an
  // edge exactly when their indices differ in one bit.
  return corners.flatMap((corner, i) =>
    [1, 2, 4].flatMap((bit) => {
      const other = corners[i | bit]
      return (i & bit) === 0 && other !== undefined ? [[corner, other] as const] : []
    }),
  )
}

/** The points where an edge meets a plane: both ends when it lies in the plane. */
function edgeCut(
  [start, end]: readonly [Vector3, Vector3],
  origin: Vector3,
  normal: Vector3,
): Vector3[] {
  const startDistance = dot(normal, subtract(start, origin))
  const endDistance = dot(normal, subtract(end, origin))
  const startOn = Math.abs(startDistance) <= OUTLINE_TOLERANCE
  const endOn = Math.abs(endDistance) <= OUTLINE_TOLERANCE
  if (startOn || endOn) {
    return [...(startOn ? [start] : []), ...(endOn ? [end] : [])]
  }
  if (startDistance < 0 === endDistance < 0) {
    return []
  }
  const share = startDistance / (startDistance - endDistance)
  return [add(start, scale(subtract(end, start), share))]
}

/**
 * The polygon where a split plane cuts a box (3.17): the points of the box's edges that lie on the
 * plane, each once, in order around their centroid in the plane; null when there are fewer than 3
 * (a plane that misses the box or touches only an edge or a corner).
 */
export function splitPlaneOutline(plane: Plane, box: Bounds): Vector3[] | null {
  const normal = normalize(plane.normal)
  if (normal === null) {
    return null
  }
  const corners: Vector3[] = []
  for (const point of boxEdges(box).flatMap((edge) => edgeCut(edge, plane.origin, normal))) {
    if (corners.every((corner) => length(subtract(corner, point)) > OUTLINE_TOLERANCE)) {
      corners.push(point)
    }
  }
  if (corners.length < 3) {
    return null
  }
  const centroid = scale(
    corners.reduce((sum, corner) => add(sum, corner)),
    1 / corners.length,
  )
  // Two directions in the plane, from the model axis least along the normal.
  const across: Vector3 = Math.abs(normal[0]) < 0.9 ? [1, 0, 0] : [0, 1, 0]
  const first = normalize(cross(normal, across)) ?? across
  const second = cross(normal, first)
  const angle = (point: Vector3): number => {
    const offset = subtract(point, centroid)
    return Math.atan2(dot(offset, second), dot(offset, first))
  }
  return corners.toSorted((one, other) => angle(one) - angle(other))
}

/**
 * The outlines of the focused finding's split planes in its box (3.17): none unless the finding
 * has a `split` suggestion and a bounds location, and none for a plane that does not cut the box.
 */
export const focusedSplitOutlines: (state: AppState) => readonly (readonly Vector3[])[] = memoized(
  (state) => [state.print.report, state.print.focusedFinding] as const,
  (report, focused): readonly (readonly Vector3[])[] => {
    const finding = focused === null ? undefined : findingAt(report, focused)
    const suggestion = finding?.suggestion
    const location = finding?.location
    if (suggestion?.kind !== 'split' || location?.kind !== 'bounds') {
      return []
    }
    return suggestion.planes.flatMap((plane) => {
      const outline = splitPlaneOutline(plane, location)
      return outline === null ? [] : [outline]
    })
  },
)
