import type { Bed, BodyPrint } from '../generated/protocol'
import { memoized } from '../state/selectors'
import type { AppState } from '../state/store'
import { cross, dot, scale, subtract, type Vector3 } from '../viewport/math'
import { findPrinter } from './printer'

// The print bed outline drawn in Print context after a report (ui.md 3.12 pass 8, 3.17).

/** Straight segments that draw a round bed. */
export const CIRCLE_SEGMENTS = 128

/** Below this determinant a transform is taken to be singular. */
const SINGULAR_DETERMINANT = 1e-12

/** A column-major 4 × 4 matrix, as `BodyPrint.transform` carries it. */
export type Matrix = BodyPrint['transform']

/**
 * The inverse of an affine matrix (bottom row 0, 0, 0, 1) as a point mapping, or null when the
 * matrix cannot be inverted.
 */
export function inverseAffine(matrix: Matrix): ((point: Vector3) => Vector3) | null {
  const xAxis: Vector3 = [matrix[0], matrix[1], matrix[2]]
  const yAxis: Vector3 = [matrix[4], matrix[5], matrix[6]]
  const zAxis: Vector3 = [matrix[8], matrix[9], matrix[10]]
  const translation: Vector3 = [matrix[12], matrix[13], matrix[14]]
  // The inverse of the matrix with columns x, y and z has the rows y × z, z × x and x × y over its determinant.
  const determinant = dot(xAxis, cross(yAxis, zAxis))
  if (!Number.isFinite(determinant) || Math.abs(determinant) < SINGULAR_DETERMINANT) {
    return null
  }
  const firstRow = scale(cross(yAxis, zAxis), 1 / determinant)
  const secondRow = scale(cross(zAxis, xAxis), 1 / determinant)
  const thirdRow = scale(cross(xAxis, yAxis), 1 / determinant)
  return (point) => {
    const relative = subtract(point, translation)
    return [dot(firstRow, relative), dot(secondRow, relative), dot(thirdRow, relative)]
  }
}

/** The bed's outline at z = 0 in bed coordinates as a closed polygon: the rectangle [0, width] × [0, depth], or the circle around the origin. */
export function bedPolygon(bed: Bed): readonly Vector3[] {
  if (bed.kind === 'rectangle') {
    return [
      [0, 0, 0],
      [bed.width, 0, 0],
      [bed.width, bed.depth, 0],
      [0, bed.depth, 0],
    ]
  }
  const radius = bed.diameter / 2
  return Array.from({ length: CIRCLE_SEGMENTS }, (_, index): Vector3 => {
    const angle = (2 * Math.PI * index) / CIRCLE_SEGMENTS
    return [radius * Math.cos(angle), radius * Math.sin(angle), 0]
  })
}

/**
 * The outline of a bed in model coordinates as line segments (6 floats each: start, then end),
 * mapped by the inverse of an analyzed body's model-to-bed transform; null when that transform
 * cannot be inverted.
 */
export function bedOutlineSegments(bed: Bed, body: BodyPrint): Float32Array | null {
  const toModel = inverseAffine(body.transform)
  if (toModel === null) {
    return null
  }
  const corners = bedPolygon(bed).map(toModel)
  const segments = new Float32Array(corners.length * 6)
  corners.forEach((start, index) => {
    const end = corners[(index + 1) % corners.length] ?? start
    segments.set([...start, ...end], index * 6)
  })
  return segments
}

/**
 * The bed outline the viewport draws in Print context after a report: the bed of the report's
 * profile, mapped into model coordinates by the first analyzed body's transform. Null outside
 * Print, without a report or an analyzed body, and when the profile is unknown. The same array
 * comes back until the context, the report or the printers change.
 */
export const bedOutline: (state: AppState) => Float32Array | null = memoized(
  (state) => [state.context, state.print.report, state.printers] as const,
  (context, report, printers): Float32Array | null => {
    const [body] = report?.bodies ?? []
    if (context !== 'print' || report === null || body === undefined) {
      return null
    }
    const printer = findPrinter(printers, report.profile)
    return printer === undefined ? null : bedOutlineSegments(printer.bed, body)
  },
)
