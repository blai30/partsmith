import { describe, expect, it } from 'vitest'
import type { WorldAxis } from '../core/labels'
import type { FeatureRecord } from '../generated/protocol'
import { placementYAxis } from '../sketch/placement'
import type { Vector3 } from './math'
import {
  ORIGIN_PLANE_AXES,
  ORIGIN_PLANE_NORMAL_AXES,
  ORIGIN_PLANE_PLACEMENTS,
  ORIGIN_PLANES,
  sketchWorldAxes,
} from './planes'

// The world axes of the origin planes (ui.md 3.12, 3.16.2): a plane takes its normal's color, and
// a sketch's axes on an origin plane are world axes.

const UNIT_VECTORS: Readonly<Record<WorldAxis, Vector3>> = {
  X: [1, 0, 0],
  Y: [0, 1, 0],
  Z: [0, 0, 1],
}

/** Matches a vector by value, so a -0 from a cross product equals 0. */
function closeTo(vector: Vector3): unknown[] {
  return vector.map((component) => expect.closeTo(component, 12))
}

function sketchOn(plane: unknown): FeatureRecord {
  return { id: 'f-sk1', type: 'sketch', plane, entities: [], constraints: [] }
}

describe('origin plane axes', () => {
  it.each(ORIGIN_PLANES)(
    'the %s plane frame runs its sketch x and y axes along the named world axes',
    (plane) => {
      const placement = ORIGIN_PLANE_PLACEMENTS[plane]
      const [first, second] = ORIGIN_PLANE_AXES[plane]
      expect(placement.x_axis).toEqual(closeTo(UNIT_VECTORS[first]))
      expect(placementYAxis(placement)).toEqual(closeTo(UNIT_VECTORS[second]))
    },
  )

  it('names the axes of each plane in its own name', () => {
    expect(ORIGIN_PLANE_AXES).toEqual({ XY: ['X', 'Y'], XZ: ['X', 'Z'], YZ: ['Y', 'Z'] })
  })

  it.each(ORIGIN_PLANES)('the %s plane takes the axis along its normal', (plane) => {
    const normal = ORIGIN_PLANE_PLACEMENTS[plane].normal.map(Math.abs)
    expect(normal).toEqual(closeTo(UNIT_VECTORS[ORIGIN_PLANE_NORMAL_AXES[plane]]))
  })

  it('maps Top to Z, Front to Y and Right to X', () => {
    expect(ORIGIN_PLANE_NORMAL_AXES).toEqual({ XY: 'Z', XZ: 'Y', YZ: 'X' })
  })
})

describe('sketchWorldAxes', () => {
  it('gives the world axes of a sketch on an origin plane', () => {
    expect(sketchWorldAxes(sketchOn({ kind: 'origin', plane: 'XZ' }))).toEqual(['X', 'Z'])
    expect(sketchWorldAxes(sketchOn({ kind: 'origin', plane: 'YZ' }))).toEqual(['Y', 'Z'])
  })

  it('gives null for a sketch on a datum plane or a face', () => {
    expect(sketchWorldAxes(sketchOn({ kind: 'datum', feature: 'f-dp1' }))).toBeNull()
    expect(
      sketchWorldAxes(
        sketchOn({
          kind: 'face',
          body: 'b-cube',
          face: { name: 'top' },
          fallback: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
          fallback_frame: 'body',
        }),
      ),
    ).toBeNull()
  })

  it('gives null for a record without a valid plane', () => {
    expect(sketchWorldAxes(sketchOn(null))).toBeNull()
    expect(sketchWorldAxes(sketchOn({ kind: 'origin', plane: 'XW' }))).toBeNull()
  })
})
