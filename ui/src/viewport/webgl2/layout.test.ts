import { describe, expect, it } from 'vitest'
import {
  ELEMENT_TEXTURE_WIDTH,
  LINE_ATTRIBUTES,
  LINE_CORNERS,
  POINT_ATTRIBUTES,
  POINT_CORNERS,
  SURFACE_ATTRIBUTES,
  elementTextureSize,
  geometryLengthProblem,
  lineStartDistances,
  packElementColors,
  packElementStates,
} from './layout'

describe('attribute locations', () => {
  it('give each attribute of a program its own location', () => {
    for (const attributes of [SURFACE_ATTRIBUTES, LINE_ATTRIBUTES, POINT_ATTRIBUTES]) {
      const locations = Object.values(attributes)
      expect(new Set(locations).size).toBe(locations.length)
    }
  })

  it('share the element id location across programs', () => {
    expect(LINE_ATTRIBUTES.elementId).toBe(SURFACE_ATTRIBUTES.elementId)
    expect(POINT_ATTRIBUTES.elementId).toBe(SURFACE_ATTRIBUTES.elementId)
  })
})

describe('quad corners', () => {
  it('run a segment quad from the start (x 0) to the end (x 1) on both sides', () => {
    expect(Array.from(LINE_CORNERS)).toEqual([0, -1, 0, 1, 1, -1, 1, 1])
  })

  it('cover the point square as a triangle strip', () => {
    expect(Array.from(POINT_CORNERS)).toEqual([-1, -1, 1, -1, -1, 1, 1, 1])
  })
})

describe('elementTextureSize', () => {
  it('has one row for empty geometry', () => {
    expect(elementTextureSize(0)).toEqual({ width: ELEMENT_TEXTURE_WIDTH, height: 1 })
  })

  it('fills rows of 1024 texels', () => {
    expect(elementTextureSize(1024)).toEqual({ width: 1024, height: 1 })
    expect(elementTextureSize(1025)).toEqual({ width: 1024, height: 2 })
  })
})

describe('packElementStates', () => {
  it('pads the states to the texture size', () => {
    const packed = packElementStates(new Uint8Array([1, 2, 4]), 3)
    expect(packed).toHaveLength(ELEMENT_TEXTURE_WIDTH)
    expect(Array.from(packed.subarray(0, 4))).toEqual([1, 2, 4, 0])
  })

  it('ignores states beyond the element count', () => {
    const packed = packElementStates(new Uint8Array([1, 2, 4]), 2)
    expect(Array.from(packed.subarray(0, 3))).toEqual([1, 2, 0])
  })
})

describe('packElementColors', () => {
  it('expands 3 floats per element to RGBA with alpha 1', () => {
    const packed = packElementColors(new Float32Array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6]), 2)
    expect(packed).toHaveLength(ELEMENT_TEXTURE_WIDTH * 4)
    expect(Array.from(packed.subarray(0, 9))).toEqual([
      Math.fround(0.1),
      Math.fround(0.2),
      Math.fround(0.3),
      1,
      Math.fround(0.4),
      Math.fround(0.5),
      Math.fround(0.6),
      1,
      0,
    ])
  })

  it('leaves elements without colors at zero', () => {
    const packed = packElementColors(new Float32Array([1, 1, 1]), 2)
    expect(Array.from(packed.subarray(4, 8))).toEqual([0, 0, 0, 0])
  })
})

describe('lineStartDistances', () => {
  it('continues the distance along a polyline', () => {
    const positions = new Float32Array([0, 0, 0, 3, 4, 0, 3, 4, 0, 3, 4, 2])
    expect(Array.from(lineStartDistances(positions))).toEqual([0, 5])
  })

  it('starts again at 0 where a segment does not continue the previous one', () => {
    const positions = new Float32Array([0, 0, 0, 3, 4, 0, 10, 0, 0, 10, 0, 1, 10, 0, 1, 10, 0, 3])
    expect(Array.from(lineStartDistances(positions))).toEqual([0, 0, 1])
  })

  it('is empty without segments', () => {
    expect(lineStartDistances(new Float32Array(0))).toHaveLength(0)
  })
})

describe('geometryLengthProblem', () => {
  const positions = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0])

  it('accepts triangles with a normal and an element id per vertex', () => {
    expect(
      geometryLengthProblem({
        kind: 'triangles',
        positions,
        normals: new Float32Array(9),
        indices: new Uint32Array([0, 1, 2]),
        elementIds: new Uint32Array(3),
      }),
    ).toBeNull()
  })

  it('rejects triangles without normals for every vertex', () => {
    expect(
      geometryLengthProblem({
        kind: 'triangles',
        positions,
        normals: new Float32Array(6),
        indices: new Uint32Array([0, 1, 2]),
        elementIds: new Uint32Array(3),
      }),
    ).toBe('triangles need one normal per vertex')
  })

  it('rejects fills whose indices are not whole triangles', () => {
    expect(
      geometryLengthProblem({
        kind: 'fill',
        positions,
        indices: new Uint32Array([0, 1]),
        elementIds: new Uint32Array(3),
      }),
    ).toBe('fill need 3 indices per triangle')
  })

  it('rejects points without an element id per point', () => {
    expect(
      geometryLengthProblem({ kind: 'points', positions, elementIds: new Uint32Array(2) }),
    ).toBe('points need 3 position floats and one element id per vertex')
  })

  it('accepts lines with one element id per segment and rejects others', () => {
    const segment = new Float32Array([0, 0, 0, 1, 0, 0])
    expect(
      geometryLengthProblem({ kind: 'lines', positions: segment, elementIds: new Uint32Array(1) }),
    ).toBeNull()
    expect(
      geometryLengthProblem({ kind: 'lines', positions: segment, elementIds: new Uint32Array(2) }),
    ).toBe('lines need 6 position floats and one element id per segment')
  })
})
