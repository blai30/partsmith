import { describe, expect, it } from 'vitest'
import {
  CIRCULAR_EDGE,
  CORNER_VERTEX,
  cubeDocumentState,
  CYLINDRICAL_FACE,
  readyState,
} from '../layout/fixtures.testing'
import { createAppStore } from '../state/store'
import { CUBE_REGION, CUBE_TOP_FACE, CUBE_TOP_FRONT_EDGE } from '../test/fakes/documents'

const TOP_FACE = CUBE_TOP_FACE
const LINE_EDGE = CUBE_TOP_FRONT_EDGE
import {
  bodyItem,
  datumItem,
  elementItem,
  featureItem,
  originItem,
  parameterItem,
  regionItem,
  sketchLineItem,
  sketchPointItem,
} from '../test/fakes/selection'
import {
  acceptsTarget,
  bodyKey,
  bodySelectionItem,
  featureKey,
  featureSelectionItem,
  itemsFromValue,
  parameterKey,
  parameterSelectionItem,
  parseSelectionKey,
  selectionValue,
} from './selection-values'

const SECOND_REGION = {
  reference: { key: 'e-5', fp: 'c=30,30|a=78.539816' },
  area: 78.539816,
  centroid: [30, 30] as [number, number],
}

describe('selectionValue', () => {
  it('U-35 origin XY for a plane gives {kind: origin, plane: XY}', () => {
    expect(selectionValue('plane', [originItem('XY')], false)).toEqual({
      kind: 'origin',
      plane: 'XY',
    })
  })

  it('U-35 a flat face for a plane gives {kind: face, body, face: reference}', () => {
    expect(selectionValue('plane', [elementItem(TOP_FACE)], false)).toEqual({
      kind: 'face',
      body: 'b-cube',
      face: TOP_FACE.reference,
    })
  })

  it('a datum plane for a plane gives {kind: datum, feature}', () => {
    expect(selectionValue('plane', [datumItem('f-dp1', 'Datum plane 1')], false)).toEqual({
      kind: 'datum',
      feature: 'f-dp1',
    })
  })

  it('U-35 origin Z for an axis gives {kind: origin, axis: Z}', () => {
    expect(selectionValue('axis', [originItem('Z')], false)).toEqual({ kind: 'origin', axis: 'Z' })
  })

  it('U-35 a sketch line for an axis gives {kind: sketch_line, sketch, line}', () => {
    expect(selectionValue('axis', [sketchLineItem('f-sk1', 'e-1')], false)).toEqual({
      kind: 'sketch_line',
      sketch: 'f-sk1',
      line: 'e-1',
    })
  })

  it('a straight edge for an axis gives {kind: edge, body, edge: reference}', () => {
    expect(selectionValue('axis', [elementItem(LINE_EDGE)], false)).toEqual({
      kind: 'edge',
      body: 'b-cube',
      edge: LINE_EDGE.reference,
    })
  })

  it('a round face for an axis gives {kind: face, body, face: reference}', () => {
    expect(selectionValue('axis', [elementItem(CYLINDRICAL_FACE, 'Peg')], false)).toEqual({
      kind: 'face',
      body: 'b-peg',
      face: CYLINDRICAL_FACE.reference,
    })
  })

  it('U-35 two regions of f-sk1 give a ProfileRef with both regions in pick order', () => {
    const items = [regionItem('f-sk1', SECOND_REGION, 'Region 2'), regionItem('f-sk1', CUBE_REGION)]
    expect(selectionValue('sketch_region', items, true)).toEqual({
      sketch: 'f-sk1',
      regions: [SECOND_REGION.reference, CUBE_REGION.reference],
    })
  })

  it('U-35 edges with multiple give an array of references', () => {
    const second = { ...LINE_EDGE, name: 'edge(b)', reference: { q: 'edge(b)', fp: 'line|b' } }
    expect(selectionValue('edge', [elementItem(LINE_EDGE), elementItem(second)], true)).toEqual([
      LINE_EDGE.reference,
      second.reference,
    ])
  })

  it('one edge without multiple gives the reference itself', () => {
    expect(selectionValue('edge', [elementItem(LINE_EDGE)], false)).toEqual(LINE_EDGE.reference)
  })

  it('faces with multiple give an array of references', () => {
    expect(selectionValue('face', [elementItem(TOP_FACE)], true)).toEqual([TOP_FACE.reference])
  })

  it('a body row or an element of the body gives the body id; multiple gives ids', () => {
    expect(selectionValue('body', [bodyItem('b-cube', 'Cube')], false)).toBe('b-cube')
    expect(selectionValue('body', [elementItem(TOP_FACE)], false)).toBe('b-cube')
    expect(
      selectionValue('body', [bodyItem('b-cube', 'Cube'), bodyItem('b-peg', 'Peg')], true),
    ).toEqual(['b-cube', 'b-peg'])
  })

  it('a sketch feature or one of its curves gives the sketch id', () => {
    expect(selectionValue('sketch', [featureItem('f-sk1', 'Sketch 1')], false)).toBe('f-sk1')
    expect(selectionValue('sketch', [sketchLineItem('f-sk1', 'e-2')], false)).toBe('f-sk1')
  })

  it('points of one sketch give PointRef strings in pick order', () => {
    expect(
      selectionValue(
        'sketch_point',
        [sketchPointItem('f-sk2', 'e-p1'), sketchPointItem('f-sk2', 'e-1.start')],
        true,
      ),
    ).toEqual(['e-p1', 'e-1.start'])
  })

  it('gives null when there are no items', () => {
    expect(selectionValue('plane', [], false)).toBeNull()
    expect(selectionValue('sketch_region', [], true)).toBeNull()
  })
})

describe('acceptsTarget', () => {
  it('U-36 a plane field accepts a face whose surface is plane', () => {
    expect(acceptsTarget('plane', elementItem(TOP_FACE))).toBe(true)
  })

  it('U-36 a plane field refuses a cylindrical face', () => {
    expect(acceptsTarget('plane', elementItem(CYLINDRICAL_FACE, 'Peg'))).toBe(false)
  })

  it('U-36 an axis field accepts a line edge and refuses a circle edge', () => {
    expect(acceptsTarget('axis', elementItem(LINE_EDGE))).toBe(true)
    expect(acceptsTarget('axis', elementItem(CIRCULAR_EDGE, 'Peg'))).toBe(false)
  })

  it('U-36 an axis field accepts a cylindrical face', () => {
    expect(acceptsTarget('axis', elementItem(CYLINDRICAL_FACE, 'Peg'))).toBe(true)
  })

  it('an axis field refuses a flat face', () => {
    expect(acceptsTarget('axis', elementItem(TOP_FACE))).toBe(false)
  })

  it('plane and axis fields accept origin planes and origin axes respectively', () => {
    expect(acceptsTarget('plane', originItem('XY'))).toBe(true)
    expect(acceptsTarget('plane', originItem('Z'))).toBe(false)
    expect(acceptsTarget('axis', originItem('X'))).toBe(true)
    expect(acceptsTarget('axis', originItem('YZ'))).toBe(false)
    expect(acceptsTarget('plane', datumItem('f-dp1', 'Datum plane 1'))).toBe(true)
    expect(acceptsTarget('axis', sketchLineItem('f-sk1', 'e-1'))).toBe(true)
  })

  it('a body field accepts body rows and the faces, edges and vertices of a body', () => {
    expect(acceptsTarget('body', bodyItem('b-cube', 'Cube'))).toBe(true)
    expect(acceptsTarget('body', elementItem(TOP_FACE))).toBe(true)
    expect(acceptsTarget('body', elementItem(LINE_EDGE))).toBe(true)
    expect(acceptsTarget('body', elementItem(CORNER_VERTEX))).toBe(true)
    expect(acceptsTarget('body', originItem('XY'))).toBe(false)
  })

  it('face and edge fields accept only their own kind', () => {
    expect(acceptsTarget('face', elementItem(TOP_FACE))).toBe(true)
    expect(acceptsTarget('face', elementItem(LINE_EDGE))).toBe(false)
    expect(acceptsTarget('edge', elementItem(LINE_EDGE))).toBe(true)
    expect(acceptsTarget('edge', elementItem(TOP_FACE))).toBe(false)
  })

  it('region, point and sketch fields accept their sketch items', () => {
    expect(acceptsTarget('sketch_region', regionItem('f-sk1', CUBE_REGION))).toBe(true)
    expect(acceptsTarget('sketch_region', elementItem(TOP_FACE))).toBe(false)
    expect(acceptsTarget('sketch_point', sketchPointItem('f-sk1', 'e-1.start'))).toBe(true)
    expect(acceptsTarget('sketch_point', sketchLineItem('f-sk1', 'e-1'))).toBe(false)
    expect(acceptsTarget('sketch', sketchLineItem('f-sk1', 'e-1'))).toBe(true)
    expect(acceptsTarget('sketch', regionItem('f-sk1', CUBE_REGION))).toBe(true)
    expect(acceptsTarget('sketch', elementItem(TOP_FACE))).toBe(false)
  })
})

describe('itemsFromValue', () => {
  const state = createAppStore(readyState(cubeDocumentState())).getState()

  it('gives the origin plane item for an origin plane value', () => {
    const [item, ...rest] = itemsFromValue('plane', { kind: 'origin', plane: 'XY' }, state)
    expect(rest).toEqual([])
    expect(item).toMatchObject({ key: 'origin:XY', kind: 'origin_plane', origin: 'XY' })
  })

  it('names origin planes by view and coordinates, as field boxes show them (3.10)', () => {
    const labels = (['XY', 'XZ', 'YZ'] as const).map(
      (plane) => itemsFromValue('plane', { kind: 'origin', plane }, state)[0]?.label,
    )
    expect(labels).toEqual(['Top plane (XY)', 'Front plane (XZ)', 'Right plane (YZ)'])
  })

  it('names origin axes and the origin point plainly', () => {
    expect(itemsFromValue('axis', { kind: 'origin', axis: 'Z' }, state)[0]?.label).toBe('Z axis')
  })

  it('gives the body item, labeled with the body name, for a body id', () => {
    const [item] = itemsFromValue('body', 'b-cube', state)
    expect(item).toMatchObject({ key: 'body:b-cube', kind: 'body', label: 'Cube', body: 'b-cube' })
  })

  it('gives the sketch feature item, labeled with the feature label, for a sketch id', () => {
    const [item] = itemsFromValue('sketch', 'f-sk1', state)
    expect(item).toMatchObject({ key: 'feature:f-sk1', label: 'Sketch 1', feature: 'f-sk1' })
  })

  it('gives one region item per region of a ProfileRef', () => {
    const items = itemsFromValue(
      'sketch_region',
      { sketch: 'f-sk1', regions: [CUBE_REGION.reference] },
      state,
    )
    expect(items.map((item) => [item.key, item.kind, item.feature])).toEqual([
      ['region:f-sk1:e-1,e-2,e-3,e-4', 'sketch_region', 'f-sk1'],
    ])
  })

  it('gives no items for an empty value', () => {
    expect(itemsFromValue('plane', undefined, state)).toEqual([])
    expect(itemsFromValue('edge', [], state)).toEqual([])
  })
})

describe('selection items and keys (3.6)', () => {
  it('builds body, feature, datum plane and parameter items', () => {
    expect(bodySelectionItem('b1', 'Body 1')).toEqual(bodyItem('b1', 'Body 1'))
    expect(featureSelectionItem('f-sk1', 'Sketch 1')).toEqual(featureItem('f-sk1', 'Sketch 1'))
    expect(featureSelectionItem('f-dp1', 'Plane 1', 'datum_plane')).toEqual(
      datumItem('f-dp1', 'Plane 1'),
    )
    expect(parameterSelectionItem('p1', 'width')).toEqual(parameterItem('p1', 'width'))
  })

  it('keys bodies, features and parameters by kind and id', () => {
    expect(bodyKey('b1')).toBe('body:b1')
    expect(featureKey('f-sk1')).toBe('feature:f-sk1')
    expect(parameterKey('p1')).toBe('parameter:p1')
  })

  it('parses body, feature and parameter keys back into kind and id', () => {
    expect(parseSelectionKey('body:b1')).toEqual({ kind: 'body', id: 'b1' })
    expect(parseSelectionKey('feature:f-sk1')).toEqual({ kind: 'feature', id: 'f-sk1' })
    expect(parseSelectionKey('parameter:p1')).toEqual({ kind: 'parameter', id: 'p1' })
  })

  it('keeps the rest of the key as the id', () => {
    expect(parseSelectionKey('feature:a:b')).toEqual({ kind: 'feature', id: 'a:b' })
  })

  it('parses no other key', () => {
    expect(parseSelectionKey('origin:XY')).toBeNull()
    expect(parseSelectionKey(`element:b1:${CUBE_TOP_FACE.name}`)).toBeNull()
    expect(parseSelectionKey('body')).toBeNull()
  })
})
