import { describe, expect, it } from 'vitest'
import type { FeatureManifest } from '../generated/protocol'
import { testFeature } from '../layout/fixtures.testing'
import type { CatalogFeature } from '../state/store'
import { catalogFeature } from '../test/fakes/catalog'
import { CUBE_REGION } from '../test/fakes/documents'
import {
  buildInspectorModel,
  defaultParams,
  getParam,
  setParam,
  type InspectorField,
  type InspectorModel,
} from './model'

type JsonObject = Readonly<Record<string, unknown>>

const EXTRUDE = testFeature('extrude')
const PROFILE = { sketch: 'f-sk1', regions: [CUBE_REGION.reference] }
const DISTANCE_PARAMS = { extent: { kind: 'distance', expr: '30 mm' }, op: 'new' }
const NO_BODIES = { bodies: [], bodiesWithGeometry: [], lastTargetBody: null }

function extrudeModel(
  params: Readonly<Record<string, unknown>>,
  errors: Readonly<Record<string, string>> = {},
): InspectorModel {
  return buildInspectorModel({ feature: EXTRUDE, params, errors })
}

function paths(fields: readonly InspectorField[]): string[] {
  return fields.map((entry) => entry.path)
}

function field(model: InspectorModel, path: string): InspectorField {
  const found = [...model.essential, ...model.more].find((candidate) => candidate.path === path)
  if (found === undefined) {
    throw new Error(`No field ${path}`)
  }
  return found
}

/** A catalog feature of one made-up type, for schema shapes the built-in catalog lacks. */
function customFeature(
  properties: JsonObject,
  required: readonly string[],
  fields: FeatureManifest['fields'],
): CatalogFeature {
  return catalogFeature({
    type: 'custom',
    manifest: { label: 'Custom', category: 'create', icon: 'custom', version: 1, fields },
    params_schema: {
      title: 'Custom',
      type: 'object',
      properties,
      required,
      $defs: { Expr: { type: 'string' } },
    },
  })
}

describe('buildInspectorModel with the extrude entry', () => {
  it('U-31 puts profile, extent.kind, extent.expr and op in essential, in manifest order', () => {
    expect(paths(extrudeModel(DISTANCE_PARAMS).essential)).toEqual([
      'profile',
      'extent.kind',
      'extent.expr',
      'op',
    ])
  })

  it('U-31 puts reverse and body in more', () => {
    expect(paths(extrudeModel(DISTANCE_PARAMS).more)).toEqual(['reverse', 'body'])
  })

  it('U-31 leaves out extent.opposite_expr for a distance extent', () => {
    const model = extrudeModel(DISTANCE_PARAMS)
    expect([...paths(model.essential), ...paths(model.more)]).not.toContain('extent.opposite_expr')
  })

  it('U-31 makes profile a multiple selection of sketch regions', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'profile').control).toEqual({
      type: 'selection',
      accepts: 'sketch_region',
      multiple: true,
      minimum: 1,
    })
  })

  it('U-31 makes extent.kind a segmented choice with the 4 hint options in order', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'extent.kind').control).toEqual({
      type: 'choice',
      presentation: 'segmented',
      options: [
        { value: 'distance', label: 'Distance' },
        { value: 'symmetric', label: 'Symmetric' },
        { value: 'two_sides', label: 'Two sides' },
        { value: 'through_all', label: 'Through all' },
      ],
    })
  })

  it('U-31 makes extent.expr a length expression', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'extent.expr').control).toEqual({
      type: 'expression',
      unit: 'length',
    })
  })

  it('U-31 makes op a segmented choice', () => {
    const control = field(extrudeModel(DISTANCE_PARAMS), 'op').control
    expect(control).toEqual({
      type: 'choice',
      presentation: 'segmented',
      options: [
        { value: 'new', label: 'New body' },
        { value: 'join', label: 'Join' },
        { value: 'cut', label: 'Cut' },
        { value: 'intersect', label: 'Intersect' },
      ],
    })
  })

  it('U-31 makes reverse a switch', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'reverse').control).toEqual({ type: 'switch' })
  })

  it('U-31 makes body a single body selection', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'body').control).toEqual({
      type: 'selection',
      accepts: 'body',
      multiple: false,
      minimum: 1,
    })
  })

  it('U-31 shows extent.opposite_expr after extent.expr for two sides', () => {
    const model = extrudeModel({
      extent: { kind: 'two_sides', expr: '20 mm', opposite_expr: '10 mm' },
      op: 'new',
    })
    expect(paths(model.essential)).toEqual([
      'profile',
      'extent.kind',
      'extent.expr',
      'extent.opposite_expr',
      'op',
    ])
    expect(field(model, 'extent.opposite_expr').control).toEqual({ type: 'expression', unit: 'length' })
  })

  it('U-31 leaves out extent.expr for through all', () => {
    const model = extrudeModel({ extent: { kind: 'through_all' }, op: 'join' })
    expect(paths(model.essential)).toEqual(['profile', 'extent.kind', 'op'])
  })

  it('labels fields with the hint labels and gives each its current value', () => {
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(field(model, 'extent.expr').label).toBe('Distance')
    expect(field(model, 'extent.expr').value).toBe('30 mm')
    expect(field(model, 'extent.kind').value).toBe('distance')
    expect(field(model, 'op').label).toBe('Operation')
    expect(field(model, 'op').value).toBe('new')
    expect(field(model, 'profile').value).toBeUndefined()
  })

  it('takes help from the schema description at the path, else null', () => {
    // The op property is a bare $ref to BodyOp, whose definition has no description.
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(field(model, 'reverse').help).toBe('Extrude against the sketch normal.')
    expect(field(model, 'op').help).toBeNull()
  })

  it('marks fields required when every object on the path requires them', () => {
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(field(model, 'profile').required).toBe(true)
    expect(field(model, 'extent.kind').required).toBe(true)
    expect(field(model, 'extent.expr').required).toBe(true)
    expect(field(model, 'reverse').required).toBe(false)
  })

  it('marks essential fields as essential', () => {
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(model.essential.every((candidate) => candidate.essential)).toBe(true)
    expect(model.more.some((candidate) => candidate.essential)).toBe(false)
  })
})

describe('completeness and errors', () => {
  it('U-32 is incomplete while profile is missing', () => {
    expect(extrudeModel(DISTANCE_PARAMS).complete).toBe(false)
  })

  it('U-32 is complete once profile is set (the command creates the body for op new)', () => {
    expect(extrudeModel({ ...DISTANCE_PARAMS, profile: PROFILE }).complete).toBe(true)
  })

  it('is incomplete with an empty body when op is not new', () => {
    expect(extrudeModel({ ...DISTANCE_PARAMS, op: 'join', profile: PROFILE }).complete).toBe(false)
    expect(
      extrudeModel({ ...DISTANCE_PARAMS, op: 'join', profile: PROFILE, body: 'b-cube' }).complete,
    ).toBe(true)
  })

  it('U-32 forces More open when a field in more has an error', () => {
    const model = extrudeModel({ ...DISTANCE_PARAMS, profile: PROFILE }, { body: 'Pick a body.' })
    expect(model.forceMore).toBe(true)
    expect(field(model, 'body').error).toBe('Pick a body.')
  })

  it('does not force More open for an error in an essential field, which still makes it incomplete', () => {
    const model = extrudeModel(
      { ...DISTANCE_PARAMS, profile: PROFILE },
      { 'extent.expr': 'Distance needs a value.' },
    )
    expect(model.forceMore).toBe(false)
    expect(model.complete).toBe(false)
    expect(field(model, 'extent.expr').error).toBe('Distance needs a value.')
  })

  it('leaves error null for fields without one', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'op').error).toBeNull()
  })
})

describe('controls for other schema shapes', () => {
  it('uses a native select for more than 4 options', () => {
    const feature = customFeature(
      { style: { type: 'string', enum: ['a', 'b', 'c', 'd', 'e'] } },
      ['style'],
      [
        {
          path: 'style',
          label: 'Style',
          essential: true,
          options: ['a', 'b', 'c', 'd', 'e'].map((value) => ({ value, label: value.toUpperCase() })),
        },
      ],
    )
    const model = buildInspectorModel({ feature, params: { style: 'a' }, errors: {} })
    expect(field(model, 'style').control).toEqual({
      type: 'choice',
      presentation: 'select',
      options: ['a', 'b', 'c', 'd', 'e'].map((value) => ({ value, label: value.toUpperCase() })),
    })
  })

  it('maps boolean, string, integer and number schemas without hints to switch, text and number', () => {
    const feature = customFeature(
      {
        flag: { type: 'boolean' },
        name: { type: 'string' },
        copies: { type: 'integer' },
        ratio: { type: 'number' },
      },
      [],
      [
        { path: 'flag', label: 'Flag', essential: true },
        { path: 'name', label: 'Name', essential: true },
        { path: 'copies', label: 'Copies', essential: true },
        { path: 'ratio', label: 'Ratio', essential: true },
      ],
    )
    const model = buildInspectorModel({ feature, params: {}, errors: {} })
    expect(field(model, 'flag').control).toEqual({ type: 'switch' })
    expect(field(model, 'name').control).toEqual({ type: 'text' })
    expect(field(model, 'copies').control).toEqual({ type: 'number', integer: true })
    expect(field(model, 'ratio').control).toEqual({ type: 'number', integer: false })
  })

  it('maps a string schema titled Expr with a unit to an expression', () => {
    const feature = customFeature(
      { angle: { title: 'Expr', type: 'string' } },
      ['angle'],
      [{ path: 'angle', label: 'Angle', essential: true, unit: 'angle' }],
    )
    const model = buildInspectorModel({ feature, params: { angle: '45 deg' }, errors: {} })
    expect(field(model, 'angle').control).toEqual({ type: 'expression', unit: 'angle' })
  })

  it('shows an Option<T> as the inner control, not required', () => {
    const feature = customFeature(
      { tip: { anyOf: [{ $ref: '#/$defs/Expr' }, { type: 'null' }] } },
      [],
      [{ path: 'tip', label: 'Tip angle', unit: 'angle' }],
    )
    const model = buildInspectorModel({ feature, params: {}, errors: {} })
    expect(field(model, 'tip').control).toEqual({ type: 'expression', unit: 'angle' })
    expect(field(model, 'tip').required).toBe(false)
  })

  it('shows anything else as unsupported', () => {
    const model = buildInspectorModel({
      feature: testFeature('sketch'),
      params: { plane: { kind: 'origin', plane: 'XY' }, entities: [], constraints: [] },
      errors: {},
    })
    expect(field(model, 'entities').control).toEqual({ type: 'unsupported' })
    expect(field(model, 'plane').control).toEqual({
      type: 'selection',
      accepts: 'plane',
      multiple: false,
      minimum: 1,
    })
  })

  it('gives array item hints such as size[1] their own expression fields', () => {
    const model = buildInspectorModel({
      feature: testFeature('box'),
      params: { size: ['10 mm', '20 mm', '30 mm'], op: 'new' },
      errors: {},
    })
    expect(paths(model.essential)).toEqual(['size[0]', 'size[1]', 'size[2]', 'op'])
    expect(field(model, 'size[1]').value).toBe('20 mm')
    expect(field(model, 'size[1]').control).toEqual({ type: 'expression', unit: 'length' })
  })
})

describe('defaultParams', () => {
  it('U-33 gives extrude a distance extent of 10 mm and op new without bodies with geometry', () => {
    const params = defaultParams(EXTRUDE, NO_BODIES)
    expect(params.extent).toEqual({ kind: 'distance', expr: '10 mm' })
    expect(params.op).toBe('new')
    expect(params.body).toBeUndefined()
    expect(params.profile).toBeUndefined()
  })

  it('U-33 gives op join and the last target body when bodies have geometry', () => {
    const params = defaultParams(EXTRUDE, {
      bodies: [{ id: 'b-cube', name: 'Cube' }],
      bodiesWithGeometry: ['b-cube'],
      lastTargetBody: 'b-cube',
    })
    expect(params.op).toBe('join')
    expect(params.body).toBe('b-cube')
  })

  it('takes schema defaults, so a datum plane starts with offset "0"', () => {
    const params = defaultParams(testFeature('datum_plane'), NO_BODIES)
    expect(params.offset).toBe('0')
    expect(params.base).toBeUndefined()
  })

  it('gives each length of a box 10 mm and keeps the placement defaults', () => {
    const params = defaultParams(testFeature('box'), NO_BODIES)
    expect(params.size).toEqual(['10 mm', '10 mm', '10 mm'])
    expect(params.position).toEqual(['0', '0', '0'])
    expect(params.plane).toEqual({ kind: 'origin', plane: 'XY' })
    expect(params.op).toBe('new')
  })

  it('gives angle, scalar and count expressions 90 deg, 1 and 2', () => {
    const feature = customFeature(
      {
        angle: { $ref: '#/$defs/Expr' },
        factor: { $ref: '#/$defs/Expr' },
        count: { $ref: '#/$defs/Expr' },
      },
      ['angle', 'factor', 'count'],
      [
        { path: 'angle', label: 'Angle', essential: true, unit: 'angle' },
        { path: 'factor', label: 'Factor', essential: true, unit: 'scalar' },
        { path: 'count', label: 'Count', essential: true, unit: 'count' },
      ],
    )
    expect(defaultParams(feature, NO_BODIES)).toMatchObject({ angle: '90 deg', factor: '1', count: '2' })
  })

  it('gives a choice without default its first option', () => {
    const feature = customFeature(
      { style: { type: 'string', enum: ['round', 'square'] } },
      ['style'],
      [
        {
          path: 'style',
          label: 'Style',
          essential: true,
          options: [
            { value: 'round', label: 'Round' },
            { value: 'square', label: 'Square' },
          ],
        },
      ],
    )
    expect(defaultParams(feature, NO_BODIES).style).toBe('round')
  })
})

describe('setParam and getParam', () => {
  it('U-34 switching extent.kind to two_sides keeps expr and adds opposite_expr 10 mm', () => {
    const params = setParam(EXTRUDE, DISTANCE_PARAMS, 'extent.kind', 'two_sides')
    expect(params.extent).toEqual({ kind: 'two_sides', expr: '30 mm', opposite_expr: '10 mm' })
  })

  it('U-34 preserves the unknown key taper_angle through every setParam', () => {
    let params: Record<string, unknown> = { ...DISTANCE_PARAMS, taper_angle: '5 deg' }
    params = setParam(EXTRUDE, params, 'op', 'join')
    expect(params.taper_angle).toBe('5 deg')
    params = setParam(EXTRUDE, params, 'extent.expr', '12 mm')
    expect(params.taper_angle).toBe('5 deg')
    params = setParam(EXTRUDE, params, 'extent.kind', 'symmetric')
    expect(params.taper_angle).toBe('5 deg')
    params = setParam(EXTRUDE, params, 'profile', PROFILE)
    expect(params.taper_angle).toBe('5 deg')
  })

  it('switching distance to symmetric keeps expr', () => {
    const params = setParam(EXTRUDE, DISTANCE_PARAMS, 'extent.kind', 'symmetric')
    expect(params.extent).toEqual({ kind: 'symmetric', expr: '30 mm' })
  })

  it('switching to through all drops the distances the variant does not declare', () => {
    const params = setParam(
      EXTRUDE,
      { extent: { kind: 'two_sides', expr: '20 mm', opposite_expr: '5 mm' }, op: 'join' },
      'extent.kind',
      'through_all',
    )
    expect(params.extent).toEqual({ kind: 'through_all' })
  })

  it('writes a value at a nested path and leaves the input untouched', () => {
    const input = { ...DISTANCE_PARAMS }
    const params = setParam(EXTRUDE, input, 'extent.expr', '12 mm')
    expect(params.extent).toEqual({ kind: 'distance', expr: '12 mm' })
    expect(input.extent).toEqual({ kind: 'distance', expr: '30 mm' })
  })

  it('writes an array item', () => {
    const params = setParam(testFeature('box'), { size: ['1', '2', '3'], op: 'new' }, 'size[1]', '20 mm')
    expect(params.size).toEqual(['1', '20 mm', '3'])
  })

  it('creates intermediate objects', () => {
    const feature = customFeature(
      { rotation: { type: 'object', properties: { angle: { $ref: '#/$defs/Expr' } } } },
      [],
      [{ path: 'rotation.angle', label: 'Angle', unit: 'angle' }],
    )
    expect(setParam(feature, {}, 'rotation.angle', '30 deg')).toEqual({ rotation: { angle: '30 deg' } })
  })

  it('reads values at dotted and indexed paths, and undefined where nothing is', () => {
    const params = { extent: { kind: 'distance', expr: '30 mm' }, size: ['1', '2', '3'] }
    expect(getParam(params, 'extent.expr')).toBe('30 mm')
    expect(getParam(params, 'size[2]')).toBe('3')
    expect(getParam(params, 'extent.opposite_expr')).toBeUndefined()
    expect(getParam(params, 'profile.sketch')).toBeUndefined()
  })
})
