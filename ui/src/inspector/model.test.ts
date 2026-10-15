import { describe, expect, it } from 'vitest'
import type { FeatureManifest } from '../generated/protocol'
import { FILLET_EDGES_COMMAND, readyState, testFeature } from '../layout/fixtures.testing'
import { INITIAL_STATE, type AppState, type CatalogFeature } from '../state/store'
import { catalogFeature } from '../test/fakes/catalog'
import { CUBE_REGION } from '../test/fakes/documents'
import {
  activeCommandFeature,
  activeCommandModel,
  buildInspectorModel,
  defaultParams,
  fieldsOf,
  getParam,
  isEmptyValue,
  setParam,
  siblingsOf,
  type InspectorField,
  type InspectorModel,
} from './model'

type JsonObject = Readonly<Record<string, unknown>>

const EXTRUDE = testFeature('extrude')
const PROFILE = { sketch: 'f-sk1', regions: [CUBE_REGION.reference] }
const DISTANCE_PARAMS = { extent: { kind: 'distance', expr: '30 mm' }, op: 'new' }
const NO_BODIES = {
  bodies: [],
  bodiesWithGeometry: [],
  lastTargetBody: null,
  document: null,
  featureStatus: {},
}

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
  it('U-31 puts profile, extent.kind, extent.expr, the inline reverse and op in essential', () => {
    expect(paths(extrudeModel(DISTANCE_PARAMS).essential)).toEqual([
      'profile',
      'extent.kind',
      'extent.expr',
      'reverse',
      'op',
    ])
  })

  it('U-31 puts body in more', () => {
    expect(paths(extrudeModel(DISTANCE_PARAMS).more)).toEqual(['body'])
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

  it('U-31 makes reverse a toggle attached to extent.expr', () => {
    const reverse = field(extrudeModel(DISTANCE_PARAMS), 'reverse')
    expect(reverse.control).toEqual({ type: 'toggle' })
    expect(reverse.attachedTo).toBe('extent.expr')
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
      'reverse',
      'extent.opposite_expr',
      'op',
    ])
    expect(field(model, 'extent.opposite_expr').control).toEqual({
      type: 'expression',
      unit: 'length',
    })
    expect(field(model, 'reverse').attachedTo).toBe('extent.expr')
  })

  it('U-31 leaves out extent.expr for through all and attaches reverse to extent.kind', () => {
    const model = extrudeModel({ extent: { kind: 'through_all' }, op: 'join' })
    expect(paths(model.essential)).toEqual(['profile', 'extent.kind', 'reverse', 'op'])
    expect(field(model, 'reverse').attachedTo).toBe('extent.kind')
  })

  it('U-31 shows no reverse for a symmetric extent, and keeps its value', () => {
    const model = extrudeModel({
      extent: { kind: 'symmetric', expr: '20 mm' },
      op: 'new',
      reverse: true,
    })
    expect([...paths(model.essential), ...paths(model.more)]).not.toContain('reverse')
  })

  it('U-31 shows no reverse for a full revolve', () => {
    const model = buildInspectorModel({
      feature: testFeature('revolve'),
      params: { extent: { kind: 'full' }, op: 'new' },
      errors: {},
    })
    expect([...paths(model.essential), ...paths(model.more)]).not.toContain('reverse')
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

  it('U-111 gives the extrude op the help of FIELD_HELP', () => {
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(field(model, 'op').help).toBe(
      'New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap.',
    )
  })

  it('U-111 gives a field without an entry help null even when its schema has a description', () => {
    // Reverse has the schema description "Extrude against the sketch normal.", which is never shown.
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(field(model, 'extent.expr').help).toBeNull()
    expect(field(model, 'reverse').help).toBe('Extrudes to the other side of the sketch.')
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

  it('forces More open for a required empty body in More beside op join', () => {
    const model = extrudeModel({ ...DISTANCE_PARAMS, op: 'join', profile: PROFILE })
    expect(model.forceMore).toBe(true)
    expect(extrudeModel({ ...DISTANCE_PARAMS, profile: PROFILE }).forceMore).toBe(false)
  })

  it('U-117 forces More open for a hole whose points are set and whose body is empty', () => {
    const hole = testFeature('hole')
    const points = { points: ['e-p1'], sketch: 'f-sk1' }
    expect(buildInspectorModel({ feature: hole, params: points, errors: {} }).forceMore).toBe(true)
    expect(
      buildInspectorModel({ feature: hole, params: { ...points, body: 'b-cube' }, errors: {} })
        .forceMore,
    ).toBe(false)
  })

  it('U-117 keeps More closed for a fillet with no edges and no body, which the first pick fills', () => {
    const model = buildInspectorModel({
      feature: testFeature('fillet'),
      params: { radius: '2 mm' },
      errors: {},
    })
    expect(model.forceMore).toBe(false)
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
          options: ['a', 'b', 'c', 'd', 'e'].map((value) => ({
            value,
            label: value.toUpperCase(),
          })),
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

/** The axis of the field at a path for these params. */
function axisOf(
  feature: CatalogFeature,
  params: Readonly<Record<string, unknown>>,
  path: string,
): InspectorField['axis'] {
  return field(buildInspectorModel({ feature, params, errors: {} }), path).axis
}

describe('axis fields (U-180)', () => {
  const MOVE_BODY = testFeature('move_body')
  const BOX = testFeature('box')

  it('U-180 gives Move body’s translation the world axes X, Y and Z', () => {
    const params = { body: 'b-cube', translation: ['0', '0', '0'] }
    expect(axisOf(MOVE_BODY, params, 'translation[0]')).toEqual({ letter: 'X', world: true })
    expect(axisOf(MOVE_BODY, params, 'translation[1]')).toEqual({ letter: 'Y', world: true })
    expect(axisOf(MOVE_BODY, params, 'translation[2]')).toEqual({ letter: 'Z', world: true })
  })

  it('U-180 gives a box’s position world axes on the origin XY plane', () => {
    const params = { position: ['0', '0', '0'], plane: { kind: 'origin', plane: 'XY' } }
    expect(axisOf(BOX, params, 'position[1]')).toEqual({ letter: 'Y', world: true })
  })

  it('U-180 gives a box’s position the axes of its own plane on another plane', () => {
    const onFront = { position: ['0', '0', '0'], plane: { kind: 'origin', plane: 'XZ' } }
    const onDatum = { position: ['0', '0', '0'], plane: { kind: 'datum', feature: 'f-dp1' } }
    expect(axisOf(BOX, onFront, 'position[1]')).toEqual({ letter: 'Y', world: false })
    expect(axisOf(BOX, onDatum, 'position[0]')).toEqual({ letter: 'X', world: false })
  })

  it('U-180 gives no axis to other length fields', () => {
    expect(field(extrudeModel(DISTANCE_PARAMS), 'extent.expr').axis).toBeNull()
    const sizes = buildInspectorModel({ feature: BOX, params: {}, errors: {} })
    for (const size of ['size[0]', 'size[1]', 'size[2]']) {
      expect(field(sizes, size).axis).toBeNull()
    }
    const offset = customFeature(
      { offset: { $ref: '#/$defs/Expr' } },
      ['offset'],
      [{ path: 'offset', label: 'Offset X', essential: true, unit: 'length' }],
    )
    expect(axisOf(offset, { offset: '5 mm' }, 'offset')).toBeNull()
  })

  it('gives no axis to an indexed field whose label names another axis or none', () => {
    const offsets = customFeature(
      { offsets: { type: 'array', items: { $ref: '#/$defs/Expr' } } },
      ['offsets'],
      [
        { path: 'offsets[0]', label: 'Offset Y', essential: true, unit: 'length' },
        { path: 'offsets[1]', label: 'Offset', essential: true, unit: 'length' },
        { path: 'offsets[2]', label: 'Turn Z', essential: true, unit: 'angle' },
      ],
    )
    const params = { offsets: ['1', '2', '3'] }
    expect(axisOf(offsets, params, 'offsets[0]')).toBeNull()
    expect(axisOf(offsets, params, 'offsets[1]')).toBeNull()
    expect(axisOf(offsets, params, 'offsets[2]')).toBeNull()
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
      document: null,
      featureStatus: {},
    })
    expect(params.op).toBe('join')
    expect(params.body).toBe('b-cube')
  })

  it('takes schema defaults without a start value, so a circular pattern spans 360 deg', () => {
    const params = defaultParams(testFeature('circular_pattern'), NO_BODIES)
    expect(params.angle).toBe('360 deg')
    expect(params.axis).toBeUndefined()
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
    expect(defaultParams(feature, NO_BODIES)).toMatchObject({
      angle: '90 deg',
      factor: '1',
      count: '2',
    })
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
    const params = setParam(
      testFeature('box'),
      { size: ['1', '2', '3'], op: 'new' },
      'size[1]',
      '20 mm',
    )
    expect(params.size).toEqual(['1', '20 mm', '3'])
  })

  it('creates intermediate objects', () => {
    const feature = customFeature(
      { rotation: { type: 'object', properties: { angle: { $ref: '#/$defs/Expr' } } } },
      [],
      [{ path: 'rotation.angle', label: 'Angle', unit: 'angle' }],
    )
    expect(setParam(feature, {}, 'rotation.angle', '30 deg')).toEqual({
      rotation: { angle: '30 deg' },
    })
  })

  it('reads values at dotted and indexed paths, and undefined where nothing is', () => {
    const params = { extent: { kind: 'distance', expr: '30 mm' }, size: ['1', '2', '3'] }
    expect(getParam(params, 'extent.expr')).toBe('30 mm')
    expect(getParam(params, 'size[2]')).toBe('3')
    expect(getParam(params, 'extent.opposite_expr')).toBeUndefined()
    expect(getParam(params, 'profile.sketch')).toBeUndefined()
  })
})

describe('the active command model', () => {
  const withFillet: AppState = {
    ...INITIAL_STATE,
    ...readyState(null, { command: FILLET_EDGES_COMMAND }),
  }

  it('lists the essential fields, then the More options fields', () => {
    const model = extrudeModel(DISTANCE_PARAMS)
    expect(fieldsOf(model)).toEqual([...model.essential, ...model.more])
  })

  it('is null without a command or without its catalog entry', () => {
    const idle: AppState = { ...INITIAL_STATE, ...readyState(null) }
    expect(activeCommandFeature(idle)).toBeUndefined()
    expect(activeCommandModel(idle)).toBeNull()
    const uncataloged: AppState = { ...withFillet, catalog: null }
    expect(activeCommandFeature(uncataloged)).toBeUndefined()
    expect(activeCommandModel(uncataloged)).toBeNull()
  })

  it("builds the command's model from its params and the given errors", () => {
    const errors = { radius: 'Radius needs a value.' }
    const active = activeCommandModel(withFillet, errors)
    const fillet = testFeature('fillet')
    expect(activeCommandFeature(withFillet)).toBe(withFillet.catalog?.byType['fillet'])
    expect(active?.command).toBe(FILLET_EDGES_COMMAND)
    expect(active?.feature).toBe(withFillet.catalog?.byType['fillet'])
    expect(active?.model).toEqual(
      buildInspectorModel({ feature: fillet, params: FILLET_EDGES_COMMAND.params, errors }),
    )
  })
})

describe('isEmptyValue', () => {
  it('is true for missing values, empty text and lists, and a profile without regions', () => {
    for (const value of [undefined, null, '', [], { sketch: 'f-sk1', regions: [] }]) {
      expect(isEmptyValue(value)).toBe(true)
    }
  })

  it('is false for anything else', () => {
    for (const value of [0, 'b-1', ['e-1'], { sketch: 'f-sk1', regions: ['r'] }, {}]) {
      expect(isEmptyValue(value)).toBe(false)
    }
  })
})

describe('siblingsOf', () => {
  const params = { extent: { kind: 'distance', expr: '10 mm' }, op: 'new', size: ['1 mm'] }

  it('is the params for a top-level field', () => {
    expect(siblingsOf(params, 'op')).toBe(params)
  })

  it('is the object holding a nested field', () => {
    expect(siblingsOf(params, 'extent.expr')).toBe(params.extent)
  })

  it('is empty when the parent is missing or no object', () => {
    expect(siblingsOf(params, 'axis.line')).toEqual({})
    expect(siblingsOf(params, 'op.kind')).toEqual({})
  })
})
