import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../generated/protocol'
import { boxMesh } from '../layout/fixtures.testing'
import { CUBE_FRONT_FACE, CUBE_TOP_FRONT_EDGE } from '../test/fakes/documents'
import { elementExtentPoints } from './element-extent'
import type { Vector3 } from './math'

// The points the inspector places itself beside (3.7), on the 20 mm cube of cube.ptsm.

const CUBE_MESH = boxMesh([-10, -10, 0], [10, 10, 20])

/** The box spanned by points, as min and max corners. */
function spanOf(points: readonly Vector3[]): { min: number[]; max: number[] } {
  return {
    min: [0, 1, 2].map((axis) => Math.min(...points.map((point) => point[axis] ?? 0))),
    max: [0, 1, 2].map((axis) => Math.max(...points.map((point) => point[axis] ?? 0))),
  }
}

describe('elementExtentPoints (3.7)', () => {
  it('spans the whole of a flat face, not just its centroid', () => {
    const points = elementExtentPoints(CUBE_FRONT_FACE, CUBE_MESH)
    expect(spanOf(points)).toEqual({ min: [-10, -10, 0], max: [10, -10, 20] })
  })

  it('takes only the vertices that face the face’s way in its plane', () => {
    const points = elementExtentPoints(CUBE_FRONT_FACE, CUBE_MESH)
    // The side faces' vertices on the front edges lie in the plane too, but face elsewhere.
    expect(points.length).toBeGreaterThanOrEqual(3)
    expect(points.every((point) => point[1] === -10)).toBe(true)
    const back: ElementDescriptor = { ...CUBE_FRONT_FACE, centroid: [0, 10, 10], normal: [0, 1, 0] }
    expect(spanOf(elementExtentPoints(back, CUBE_MESH))).toEqual({
      min: [-10, 10, 0],
      max: [10, 10, 20],
    })
    // No triangle of the front plane faces backward: the centroid is all there is.
    const reversed: ElementDescriptor = { ...CUBE_FRONT_FACE, normal: [0, 1, 0] }
    expect(elementExtentPoints(reversed, CUBE_MESH)).toEqual([[0, -10, 10]])
  })

  it('spans a straight edge from end to end', () => {
    expect(elementExtentPoints(CUBE_TOP_FRONT_EDGE, CUBE_MESH)).toEqual([
      [-10, -10, 20],
      [10, -10, 20],
    ])
  })

  it('falls back to the centroid without a mesh, for a curved face and for a vertex', () => {
    expect(elementExtentPoints(CUBE_FRONT_FACE, null)).toEqual([[0, -10, 10]])
    const curved: ElementDescriptor = { ...CUBE_FRONT_FACE, surface: 'cylinder', normal: null }
    expect(elementExtentPoints(curved, CUBE_MESH)).toEqual([[0, -10, 10]])
    const vertex: ElementDescriptor = {
      ...CUBE_TOP_FRONT_EDGE,
      kind: 'vertex',
      centroid: [10, -10, 20],
    }
    expect(elementExtentPoints(vertex, CUBE_MESH)).toEqual([[10, -10, 20]])
  })
})
