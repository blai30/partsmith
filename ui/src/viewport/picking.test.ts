import { describe, expect, it } from 'vitest'
import type { SelectionItem } from '../state/store'
import {
  candidateKind,
  choosePick,
  decodePickId,
  encodePickId,
  type PickCandidate,
} from './picking'

function candidate(
  key: string,
  kind: PickCandidate['kind'],
  pickClass: PickCandidate['pickClass'],
  distance: number,
): PickCandidate {
  return { key, kind, pickClass, distance }
}

function acceptAll(): boolean {
  return true
}

function acceptAllButVertices(kind: PickCandidate['kind']): boolean {
  return kind !== 'vertex'
}

const RADIUS = 4

describe('pick id encoding', () => {
  it.each([1, 255, 256, 65536, 16777216, 4294967295])(
    'U-49 encodePickId and decodePickId round-trip %s',
    (id) => {
      const [red, green, blue, alpha] = encodePickId(id)
      expect(decodePickId(red, green, blue, alpha)).toBe(id)
    },
  )

  it('U-49 encodePickId puts bits 0-7 in red, 8-15 in green, 16-23 in blue and 24-31 in alpha', () => {
    expect(encodePickId(0x04030201)).toEqual([1, 2, 3, 4])
    expect(encodePickId(256)).toEqual([0, 1, 0, 0])
    expect(encodePickId(16777216)).toEqual([0, 0, 0, 1])
  })

  it('encodePickId gives bytes from 0 to 255', () => {
    expect(encodePickId(4294967295)).toEqual([255, 255, 255, 255])
  })

  it('encodePickId(0), nothing, is all zeros', () => {
    expect(encodePickId(0)).toEqual([0, 0, 0, 0])
  })

  it('decodePickId reads the alpha byte as the high byte of an unsigned id', () => {
    expect(decodePickId(255, 255, 255, 255)).toBe(4294967295)
    expect(decodePickId(0, 0, 0, 128)).toBe(2147483648)
  })
})

describe('choosePick', () => {
  const vertex = candidate('mesh:1:vertex:0', 'vertex', 'point', 3)
  const edge = candidate('mesh:1:edge:2', 'edge', 'curve', 1)
  const face = candidate('mesh:1:face:5', 'face', 'area', 0)

  it('U-49 prefers a vertex at 3 pixels over an edge at 1 pixel over a face at 0 pixels', () => {
    expect(choosePick([face, edge, vertex], acceptAll, RADIUS)).toEqual(vertex)
  })

  it('U-49 prefers an edge at 1 pixel over a face at 0 pixels', () => {
    expect(choosePick([face, edge], acceptAll, RADIUS)).toEqual(edge)
  })

  it('U-49 picks the edge when vertices are not accepted', () => {
    expect(choosePick([face, edge, vertex], acceptAllButVertices, RADIUS)).toEqual(edge)
  })

  it('U-49 an edge at 5 pixels with radius 4 loses to the face', () => {
    const farEdge = candidate('mesh:1:edge:3', 'edge', 'curve', 5)
    expect(choosePick([farEdge, face], acceptAll, 4)).toEqual(face)
  })

  it('U-49 a face at 1 pixel is ignored', () => {
    const nearFace = candidate('mesh:1:face:4', 'face', 'area', 1)
    expect(choosePick([nearFace], acceptAll, RADIUS)).toBeNull()
  })

  it.each([
    ['sketch curve first', true],
    ['origin axis first', false],
  ])(
    'U-49 a sketch curve and an origin axis both at 0 pixels give the sketch curve (%s)',
    (_order, curveFirst) => {
      const sketchCurve = candidate('sketch:f-sk1:e-1', 'sketch_curve', 'curve', 0)
      const originAxis = candidate('origin:X', 'origin_axis', 'curve', 0)
      const candidates = curveFirst ? [sketchCurve, originAxis] : [originAxis, sketchCurve]
      expect(choosePick(candidates, acceptAll, RADIUS)).toEqual(sketchCurve)
    },
  )

  it('returns null without candidates', () => {
    expect(choosePick([], acceptAll, RADIUS)).toBeNull()
  })

  it('returns null when no candidate is accepted', () => {
    expect(choosePick([face, edge, vertex], () => false, RADIUS)).toBeNull()
  })

  it('picks the nearest candidate within a class', () => {
    const nearEdge = candidate('mesh:1:edge:7', 'edge', 'curve', 1.5)
    const fartherEdge = candidate('mesh:1:edge:8', 'edge', 'curve', 2.5)
    expect(choosePick([fartherEdge, nearEdge], acceptAll, RADIUS)).toEqual(nearEdge)
  })

  it('prefers a curve anywhere within the radius over a face at 0 pixels', () => {
    const edgeNearRadius = candidate('mesh:1:edge:9', 'edge', 'curve', 3.5)
    expect(choosePick([face, edgeNearRadius], acceptAll, RADIUS)).toEqual(edgeNearRadius)
  })

  it('ignores a point outside the radius', () => {
    const farVertex = candidate('mesh:1:vertex:1', 'vertex', 'point', 5)
    const nearEdge = candidate('mesh:1:edge:4', 'edge', 'curve', 2)
    expect(choosePick([farVertex, nearEdge], acceptAll, RADIUS)).toEqual(nearEdge)
  })

  it('uses the radius it is given (4 CSS pixels times the UI scale)', () => {
    const vertexAtSix = candidate('mesh:1:vertex:6', 'vertex', 'point', 6)
    expect(choosePick([face, vertexAtSix], acceptAll, 8)).toEqual(vertexAtSix)
    expect(choosePick([face, vertexAtSix], acceptAll, 4)).toEqual(face)
  })

  it('prefers a sketch point over the origin point at the same distance', () => {
    const originPoint = candidate('origin:O', 'origin_point', 'point', 0)
    const sketchPoint = candidate('point:f-sk1:origin', 'sketch_point', 'point', 0)
    expect(choosePick([originPoint, sketchPoint], acceptAll, RADIUS)).toEqual(sketchPoint)
  })

  it('prefers a sketch curve over the axis of its sketch at the same distance, whatever their order (3.6)', () => {
    const axis = candidate('sketch:f-sk1:x_axis', 'origin_axis', 'curve', 1)
    const line = candidate('sketch:f-sk1:e-1', 'sketch_curve', 'curve', 1)
    expect(choosePick([axis, line], acceptAll, RADIUS)).toEqual(line)
  })

  it('prefers a face over an origin plane at distance 0', () => {
    const originPlane = candidate('origin:XY', 'origin_plane', 'area', 0)
    expect(choosePick([originPlane, face], acceptAll, RADIUS)).toEqual(face)
  })

  it('prefers a datum plane over an origin plane at distance 0', () => {
    const originPlane = candidate('origin:XZ', 'origin_plane', 'area', 0)
    const datumPlane = candidate('feature:f-dat1', 'datum_plane', 'area', 0)
    expect(choosePick([originPlane, datumPlane], acceptAll, RADIUS)).toEqual(datumPlane)
  })
})

function sketchItem(
  key: string,
  kind: SelectionItem['kind'],
  reference: SelectionItem['sketchItem'],
): SelectionItem {
  return {
    key,
    kind,
    label: key,
    body: null,
    feature: 'f-sk1',
    element: null,
    sketchItem: reference,
    origin: null,
  }
}

describe('candidateKind (3.6)', () => {
  it('counts the origin point and axes of the active sketch as origin items', () => {
    const origin = sketchItem('point:f-sk1:origin', 'sketch_point', {
      kind: 'origin',
      sketch: 'f-sk1',
      which: 'point',
    })
    const axis = sketchItem('sketch:f-sk1:y_axis', 'sketch_curve', {
      kind: 'origin',
      sketch: 'f-sk1',
      which: 'y_axis',
    })
    expect(candidateKind(origin)).toBe('origin_point')
    expect(candidateKind(axis)).toBe('origin_axis')
  })

  it('keeps the kind of every other item', () => {
    const line = sketchItem('sketch:f-sk1:e-1', 'sketch_curve', {
      kind: 'entity',
      sketch: 'f-sk1',
      entity: 'e-1',
      entityType: 'line',
    })
    expect(candidateKind(line)).toBe('sketch_curve')
  })
})
