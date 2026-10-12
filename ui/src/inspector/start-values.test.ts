import { describe, expect, it } from 'vitest'
import type { Document, FeatureStatus } from '../generated/protocol'
import { testFeature } from '../layout/fixtures.testing'
import { CUBE_DOCUMENT, CUBE_REGION, featuresOf } from '../test/fakes/documents'
import { buildInspectorModel, defaultParams, setParam, type DefaultsContext } from './model'
import { START_VALUES, startValue } from './start-values'

// Starting values that work on printed parts and the reuse of the document's last values (ui.md
// 2.12), and what "More options" says it hides (3.7).

const NEW_DOCUMENT: DefaultsContext = {
  bodies: [],
  bodiesWithGeometry: [],
  lastTargetBody: null,
  document: null,
  featureStatus: {},
}

const CUBE: DefaultsContext = {
  bodies: [{ id: 'b-cube', name: 'Cube' }],
  bodiesWithGeometry: ['b-cube'],
  lastTargetBody: 'b-cube',
  document: CUBE_DOCUMENT,
  featureStatus: {},
}

function startsOf(type: string, context: DefaultsContext = NEW_DOCUMENT) {
  return defaultParams(testFeature(type), context)
}

/** cube.ptsm with fillets appended in timeline order. */
function withFillets(...radii: readonly string[]): Document {
  return {
    ...CUBE_DOCUMENT,
    features: [
      ...featuresOf(CUBE_DOCUMENT),
      ...radii.map((radius, index) => ({
        id: `f-fil${String(index + 1)}`,
        type: 'fillet',
        body: 'b-cube',
        edges: [{ q: `edge-${String(index)}`, fp: 'line' }],
        radius,
      })),
    ],
  }
}

function errorStatus(id: string): FeatureStatus {
  return { id, status: 'error', messages: [], repairs: [] }
}

describe('start values (U-103)', () => {
  it('U-103 starts dress-up and hole sizes that work on printed parts', () => {
    expect(startsOf('fillet').radius).toBe('1 mm')
    expect(startsOf('chamfer').distance).toEqual({ kind: 'equal', expr: '1 mm' })
    expect(startsOf('shell').thickness).toBe('2 mm')
    expect(startsOf('hole').diameter).toBe('3.2 mm')
    expect(startsOf('hole').extent).toEqual({ kind: 'through_all' })
    expect(startsOf('revolve').extent).toEqual({ kind: 'full' })
    expect(startsOf('datum_plane').offset).toBe('10 mm')
  })

  it('U-103 starts extrude as U-33 does', () => {
    expect(startsOf('extrude').extent).toEqual({ kind: 'distance', expr: '10 mm' })
    expect(startsOf('extrude').op).toBe('new')
  })

  it('U-103 reuses the latest fillet radius, but not its edges or body', () => {
    const params = startsOf('fillet', { ...CUBE, document: withFillets('2 mm', 'edge_r') })
    expect(params.radius).toBe('edge_r')
    expect(params.edges).toBeUndefined()
    expect(params.body).toBeUndefined()
  })

  it('U-103 skips a latest fillet in error for the one before it', () => {
    const params = startsOf('fillet', {
      ...CUBE,
      document: withFillets('edge_r', '50 mm'),
      featureStatus: { 'f-fil2': errorStatus('f-fil2') },
    })
    expect(params.radius).toBe('edge_r')
  })

  it('reuses a value inside a union only from a record with the same variant', () => {
    const document: Document = {
      ...CUBE_DOCUMENT,
      features: [
        ...featuresOf(CUBE_DOCUMENT),
        {
          id: 'f-ch1',
          type: 'chamfer',
          body: 'b-cube',
          edges: [],
          distance: { kind: 'two_distances', expr: '3 mm', second_expr: '1 mm' },
        },
      ],
    }
    expect(startsOf('chamfer', { ...CUBE, document }).distance).toEqual({
      kind: 'equal',
      expr: '1 mm',
    })
  })

  it('U-103 starts a fillet body empty and a hole body at the last target body', () => {
    expect(startsOf('fillet', CUBE).body).toBeUndefined()
    expect(startsOf('hole', CUBE).body).toBe('b-cube')
  })

  it('U-103 starts a counterbore head at twice Diameter with the same unit word', () => {
    const hole = testFeature('hole')
    const params = setParam(hole, { diameter: '3.2 mm' }, 'hole_type.kind', 'counterbore')
    expect(params.hole_type).toEqual({ kind: 'counterbore', diameter: '6.4 mm', depth: '3 mm' })
    const named = setParam(hole, { diameter: 'screw' }, 'hole_type.kind', 'counterbore')
    expect(named.hole_type).toMatchObject({ diameter: '2 * (screw)' })
    const countersink = setParam(hole, { diameter: '3.2 mm' }, 'hole_type.kind', 'countersink')
    expect(countersink.hole_type).toEqual({
      kind: 'countersink',
      diameter: '6.4 mm',
      angle: '90 deg',
    })
  })

  it('keeps a plain number without a unit word plain when doubling it', () => {
    expect(startValue('hole', 'hole_type.diameter', { diameter: '2.5' })).toBe('5')
    expect(startValue('hole', 'hole_type.diameter', { diameter: '3mm' })).toBe('6mm')
  })

  it('gives every start value an explicit unit', () => {
    for (const values of Object.values(START_VALUES)) {
      for (const [path, value] of Object.entries(values)) {
        expect(path.endsWith('.kind') || /(mm|deg)$/.test(value)).toBe(true)
      }
    }
  })
})

/** The More fields of a new feature of `type` that differ from where they start. */
function changedMore(type: string, params: Readonly<Record<string, unknown>>) {
  const feature = testFeature(type)
  return buildInspectorModel({
    feature,
    params,
    errors: {},
    startParams: defaultParams(feature, NEW_DOCUMENT),
  }).changedMore
}

describe('More options shows what it hides (U-117)', () => {
  it('U-117 names a box position changed out of sight', () => {
    const start = startsOf('box')
    const position = Array.isArray(start.position) ? start.position : []
    expect(changedMore('box', { ...start, position: ['20 mm', ...position.slice(1)] })).toEqual([
      'Position X',
    ])
  })

  it('U-117 never counts a body the op rule filled', () => {
    expect(changedMore('extrude', { ...startsOf('extrude'), op: 'join', body: 'b-cube' })).toEqual(
      [],
    )
  })

  it('U-117 names a hole tip angle', () => {
    expect(changedMore('hole', { ...startsOf('hole'), tip_angle: '118 deg' })).toEqual([
      'Tip angle',
    ])
  })

  it('compares a field of another variant with the start of that variant', () => {
    const chamfer = testFeature('chamfer')
    const params = setParam(chamfer, startsOf('chamfer'), 'distance.kind', 'two_distances')
    expect(changedMore('chamfer', params)).toEqual([])
    expect(changedMore('chamfer', setParam(chamfer, params, 'distance.flip', true))).toEqual([
      'Flip',
    ])
  })

  it('U-117 keeps More closed for a fillet with no edges and no body', () => {
    const model = buildInspectorModel({ feature: testFeature('fillet'), params: {}, errors: {} })
    expect(model.forceMore).toBe(false)
  })
})

describe('body-creating targets in the model (2.12)', () => {
  it('U-105 shows a mirror target only while op is new', () => {
    const mirror = testFeature('mirror')
    const paths = (op: string) => {
      const model = buildInspectorModel({ feature: mirror, params: { op }, errors: {} })
      return [...model.essential, ...model.more].map((field) => field.path)
    }
    expect(paths('join')).not.toContain('target')
    expect(paths('new')).toContain('target')
  })

  it('needs the mirror source body with op new, which no rule fills', () => {
    const mirror = testFeature('mirror')
    const plane = { kind: 'origin', plane: 'XZ' }
    expect(
      buildInspectorModel({ feature: mirror, params: { op: 'new', plane }, errors: {} }).complete,
    ).toBe(false)
    expect(
      buildInspectorModel({
        feature: mirror,
        params: { op: 'new', plane, body: 'b-cube' },
        errors: {},
      }).complete,
    ).toBe(true)
  })

  it('places a profile with a region in the extrude model as before', () => {
    const model = buildInspectorModel({
      feature: testFeature('extrude'),
      params: {
        ...startsOf('extrude'),
        profile: { sketch: 'f-sk1', regions: [CUBE_REGION.reference] },
      },
      errors: {},
    })
    expect(model.complete).toBe(true)
  })
})
