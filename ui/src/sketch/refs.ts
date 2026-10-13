import type { PointRef } from '../generated/protocol'
import type { Segment } from './geometry'

// sketch.md 3.16: the reserved references to the sketch's own origin and axes. They are fixed
// constants that constraints accept wherever they take a point or a line; they are never stored as
// entities, and entity and constraint ids may not use these names.

/** The sketch origin point, 'origin'. */
export const ORIGIN_POINT: PointRef = 'origin'
/** The sketch x axis, 'x_axis'. */
export const X_AXIS = 'x_axis'
/** The sketch y axis, 'y_axis'. */
export const Y_AXIS = 'y_axis'

/** A reserved axis name. */
export type AxisName = typeof X_AXIS | typeof Y_AXIS

/** Whether a reference is one of the reserved names rather than an entity of the sketch. */
export function isReservedReference(reference: string): boolean {
  return reference === ORIGIN_POINT || reference === X_AXIS || reference === Y_AXIS
}

/** Whether a line reference names one of the sketch axes. */
export function isAxis(reference: string): reference is AxisName {
  return reference === X_AXIS || reference === Y_AXIS
}

/** The constant geometry of an axis (sketch.md 3.16): the line through (0, 0) along +u or +v. */
export function axisLine(axis: AxisName): Segment {
  return axis === X_AXIS ? { start: [0, 0], end: [1, 0] } : { start: [0, 0], end: [0, 1] }
}
