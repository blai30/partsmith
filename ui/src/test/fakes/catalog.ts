import type { CatalogEntry, FeatureManifest } from '../../generated/protocol'
import type { Catalog, CatalogFeature } from '../../state/store'

// The test feature catalog: every built-in feature type of features.md 3.13 with its manifest exactly
// as its feature spec lists it, and a params schema shaped the way schemars 1 emits it (an object
// schema titled with the manifest label, `$ref`s into its own `$defs`, defaults where the params
// struct has them).

type Schema = { readonly [key: string]: unknown }
type FieldHint = FeatureManifest['fields'][number]
type HintDetails = Omit<FieldHint, 'path' | 'label'>

function hint(path: string, label: string, details: HintDetails = {}): FieldHint {
  return { path, label, ...details }
}

function options(
  ...pairs: readonly (readonly [string, string])[]
): NonNullable<HintDetails['options']> {
  return pairs.map(([value, label]) => ({ value, label }))
}

const BODY_OPS = options(
  ['new', 'New body'],
  ['join', 'Join'],
  ['cut', 'Cut'],
  ['intersect', 'Intersect'],
)
const COPY_OPS = options(['join', 'Join'], ['new', 'New body'])

const EXPR: Schema = { $ref: '#/$defs/Expr' }
const ID: Schema = { $ref: '#/$defs/Id' }
const REFERENCE: Schema = { $ref: '#/$defs/Reference' }
const PLANE_REF: Schema = { $ref: '#/$defs/PlaneRef' }
const AXIS_REF: Schema = { $ref: '#/$defs/AxisRef' }

function optional(schema: Schema): Schema {
  return { anyOf: [schema, { type: 'null' }] }
}

function array(items: Schema): Schema {
  return { type: 'array', items }
}

function triple(items: Schema): Schema {
  return { type: 'array', items, minItems: 3, maxItems: 3 }
}

function flag(description: string): Schema {
  return { type: 'boolean', default: false, description }
}

function constant(value: string, description: string): Schema {
  return { type: 'string', const: value, description }
}

function variant(
  kind: string,
  properties: Readonly<Record<string, Schema>>,
  description: string,
): Schema {
  return {
    type: 'object',
    description,
    properties: { kind: { type: 'string', const: kind }, ...properties },
    required: ['kind', ...Object.keys(properties)],
  }
}

const DEFINITIONS: Readonly<Record<string, Schema>> = {
  Expr: { type: 'string', description: 'Expression source text (expressions.md).' },
  Id: { type: 'string', pattern: '^[A-Za-z0-9_-]{1,64}$' },
  Reference: {
    type: 'object',
    description: 'A semantic face, edge or vertex query plus its fingerprint (naming.md).',
    properties: { q: { type: 'string' }, fp: { type: 'string' } },
    required: ['q', 'fp'],
  },
  RegionRef: {
    type: 'object',
    properties: { key: { type: 'string' }, fp: { type: 'string' } },
    required: ['key', 'fp'],
  },
  ProfileRef: {
    type: 'object',
    description: 'Closed regions of one sketch used as a profile.',
    properties: { sketch: ID, regions: array({ $ref: '#/$defs/RegionRef' }) },
    required: ['sketch', 'regions'],
  },
  OriginPlane: { type: 'string', enum: ['XY', 'XZ', 'YZ'] },
  OriginAxis: { type: 'string', enum: ['X', 'Y', 'Z'] },
  PlaneRef: {
    description: 'A plane given by an origin plane, a datum plane, or a flat face of a body.',
    oneOf: [
      variant(
        'origin',
        { plane: { $ref: '#/$defs/OriginPlane' } },
        'One of the three origin planes.',
      ),
      variant('datum', { feature: ID }, 'A datum plane feature.'),
      variant('face', { body: ID, face: REFERENCE }, 'A flat face of a body.'),
    ],
  },
  AxisRef: {
    description:
      'An axis given by an origin axis, a sketch line, a straight edge, or a round face.',
    oneOf: [
      variant('origin', { axis: { $ref: '#/$defs/OriginAxis' } }, 'One of the three origin axes.'),
      variant('sketch_line', { sketch: ID, line: ID }, 'A line entity of a sketch.'),
      variant('edge', { body: ID, edge: REFERENCE }, 'A straight edge of a body.'),
      variant('face', { body: ID, face: REFERENCE }, 'The axis of a round face of a body.'),
    ],
  },
  BodyOp: {
    oneOf: [
      constant('new', 'Start the body.'),
      constant('join', 'Add the tool to the body.'),
      constant('cut', 'Remove the tool from the body.'),
      constant('intersect', 'Keep only the overlap of body and tool.'),
    ],
  },
  CopyOp: {
    oneOf: [
      constant('join', 'Fuse the copies into the source body.'),
      constant('new', 'Put the copies into the empty target body.'),
    ],
  },
  Placement: {
    type: 'object',
    properties: {
      origin: triple({ type: 'number' }),
      x_axis: triple({ type: 'number' }),
      normal: triple({ type: 'number' }),
    },
    required: ['origin', 'x_axis', 'normal'],
  },
  SketchPlane: {
    oneOf: [
      variant(
        'origin',
        { plane: { $ref: '#/$defs/OriginPlane' } },
        'One of the three origin planes.',
      ),
      variant('datum', { feature: ID }, 'The plane produced by a datum feature.'),
      variant(
        'face',
        { body: ID, face: REFERENCE, fallback: { $ref: '#/$defs/Placement' } },
        'A planar face of a body, plus the placement used when it cannot be resolved.',
      ),
    ],
  },
  SketchEntity: { type: 'object', description: 'One sketch entity (sketch.md 2.3).' },
  SketchConstraint: { type: 'object', description: 'One sketch constraint (sketch.md 2.5).' },
}

type ParamsShape = {
  readonly title: string
  readonly description: string
  readonly properties: Readonly<Record<string, Schema>>
  readonly required: readonly string[]
}

function paramsSchema(shape: ParamsShape): Schema {
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: shape.title,
    description: shape.description,
    type: 'object',
    properties: shape.properties,
    required: shape.required,
    $defs: DEFINITIONS,
  }
}

function entry(type: string, manifest: FeatureManifest, schema: ParamsShape): CatalogEntry {
  return { type, manifest, params_schema: paramsSchema(schema) }
}

const PRIMITIVE_PLACEMENT_HINTS: readonly FieldHint[] = [
  hint('plane', 'Plane', { selection: 'plane' }),
  hint('position[0]', 'Position X', { unit: 'length' }),
  hint('position[1]', 'Position Y', { unit: 'length' }),
  hint('position[2]', 'Position Z', { unit: 'length' }),
]

const PRIMITIVE_PLACEMENT_PROPERTIES: Readonly<Record<string, Schema>> = {
  plane: { ...PLANE_REF, default: { kind: 'origin', plane: 'XY' } },
  position: { ...triple(EXPR), default: ['0', '0', '0'] },
}

function primitive(
  type: string,
  label: string,
  description: string,
  sizeHints: readonly FieldHint[],
  sizeProperties: Readonly<Record<string, Schema>>,
): CatalogEntry {
  return entry(
    type,
    {
      label,
      category: 'create',
      icon: type,
      version: 1,
      fields: [
        ...sizeHints,
        hint('op', 'Operation', { essential: true, options: BODY_OPS }),
        ...PRIMITIVE_PLACEMENT_HINTS,
        hint('body', 'Body', { selection: 'body' }),
      ],
    },
    {
      title: label,
      description,
      properties: {
        body: ID,
        ...sizeProperties,
        ...PRIMITIVE_PLACEMENT_PROPERTIES,
        op: { $ref: '#/$defs/BodyOp' },
      },
      required: ['body', ...Object.keys(sizeProperties), 'op'],
    },
  )
}

const LENGTH = { essential: true, unit: 'length' } as const

export const BOOLEAN_ENTRY = entry(
  'boolean',
  {
    label: 'Boolean',
    category: 'modify',
    icon: 'boolean',
    version: 1,
    fields: [
      hint('target', 'Target body', { essential: true, selection: 'body' }),
      hint('tools', 'Tool bodies', { essential: true, selection: 'body' }),
      hint('op', 'Operation', {
        essential: true,
        options: options(['join', 'Join'], ['cut', 'Cut'], ['intersect', 'Intersect']),
      }),
      hint('keep_tools', 'Keep tools', { essential: true }),
    ],
  },
  {
    title: 'Boolean',
    description: 'Joins, cuts or intersects a target body with tool bodies.',
    properties: {
      target: ID,
      tools: array(ID),
      op: { type: 'string', enum: ['join', 'cut', 'intersect'] },
      keep_tools: flag('Leave the tools unchanged instead of consuming them.'),
    },
    required: ['target', 'tools', 'op'],
  },
)

export const BOX_ENTRY = primitive(
  'box',
  'Box',
  'A rectangular block.',
  [
    hint('size[0]', 'Length', LENGTH),
    hint('size[1]', 'Width', LENGTH),
    hint('size[2]', 'Height', LENGTH),
  ],
  { size: triple(EXPR) },
)

export const CHAMFER_ENTRY = entry(
  'chamfer',
  {
    label: 'Chamfer',
    category: 'modify',
    icon: 'chamfer',
    version: 1,
    fields: [
      hint('edges', 'Edges', { essential: true, selection: 'edge' }),
      hint('distance.kind', 'Type', {
        essential: true,
        options: options(['equal', 'Equal distance'], ['two_distances', 'Two distances']),
      }),
      hint('distance.expr', 'Distance', LENGTH),
      hint('distance.second_expr', 'Second distance', LENGTH),
      hint('distance.flip', 'Flip'),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Chamfer',
    description: 'Bevels edges of a body.',
    properties: {
      body: ID,
      edges: array(REFERENCE),
      distance: {
        oneOf: [
          variant('equal', { expr: EXPR }, '`expr` on both faces.'),
          {
            type: 'object',
            description:
              '`expr` on the first face and `second_expr` on the second face of each edge.',
            properties: {
              kind: { type: 'string', const: 'two_distances' },
              expr: EXPR,
              second_expr: EXPR,
              flip: flag('Swaps the two distances.'),
            },
            required: ['kind', 'expr', 'second_expr'],
          },
        ],
      },
    },
    required: ['body', 'edges', 'distance'],
  },
)

export const CIRCULAR_PATTERN_ENTRY = entry(
  'circular_pattern',
  {
    label: 'Circular pattern',
    category: 'pattern',
    icon: 'circular-pattern',
    version: 1,
    fields: [
      hint('body', 'Body', { essential: true, selection: 'body' }),
      hint('axis', 'Axis', { essential: true, selection: 'axis' }),
      hint('count', 'Count', { essential: true, unit: 'count' }),
      hint('angle', 'Total angle', { essential: true, unit: 'angle' }),
      hint('reverse', 'Reverse'),
      hint('op', 'Operation', { options: COPY_OPS }),
      hint('target', 'Target body', { selection: 'body' }),
    ],
  },
  {
    title: 'Circular pattern',
    description: 'Copies a body around an axis.',
    properties: {
      body: ID,
      axis: AXIS_REF,
      count: EXPR,
      angle: { ...EXPR, default: '360 deg' },
      reverse: flag('Turn in the negative sense about the axis direction.'),
      op: { $ref: '#/$defs/CopyOp', default: 'join' },
      target: optional(ID),
    },
    required: ['body', 'axis', 'count'],
  },
)

export const CONE_ENTRY = primitive(
  'cone',
  'Cone',
  'A right circular cone or frustum.',
  [
    hint('bottom_radius', 'Bottom radius', LENGTH),
    hint('top_radius', 'Top radius', LENGTH),
    hint('height', 'Height', LENGTH),
  ],
  { bottom_radius: EXPR, top_radius: EXPR, height: EXPR },
)

export const CYLINDER_ENTRY = primitive(
  'cylinder',
  'Cylinder',
  'A right circular cylinder.',
  [hint('radius', 'Radius', LENGTH), hint('height', 'Height', LENGTH)],
  { radius: EXPR, height: EXPR },
)

export const DATUM_PLANE_ENTRY = entry(
  'datum_plane',
  {
    label: 'Datum plane',
    category: 'construct',
    icon: 'datum-plane',
    version: 1,
    fields: [
      hint('base', 'Base plane', { essential: true, selection: 'plane' }),
      hint('offset', 'Offset', LENGTH),
      hint('flip', 'Flip'),
    ],
  },
  {
    title: 'Datum plane',
    description: 'A construction plane offset from another plane.',
    properties: {
      base: PLANE_REF,
      offset: { ...EXPR, default: '0' },
      flip: flag('Reverse the normal (the x-axis stays).'),
    },
    required: ['base'],
  },
)

export const EXTRUDE_ENTRY = entry(
  'extrude',
  {
    label: 'Extrude',
    category: 'create',
    icon: 'extrude',
    version: 1,
    fields: [
      hint('profile', 'Profile', { essential: true, selection: 'sketch_region' }),
      hint('extent.kind', 'Extent', {
        essential: true,
        options: options(
          ['distance', 'Distance'],
          ['symmetric', 'Symmetric'],
          ['two_sides', 'Two sides'],
          ['through_all', 'Through all'],
        ),
      }),
      hint('extent.expr', 'Distance', LENGTH),
      hint('extent.opposite_expr', 'Opposite distance', LENGTH),
      hint('op', 'Operation', { essential: true, options: BODY_OPS }),
      hint('reverse', 'Reverse'),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Extrude',
    description:
      'Extrudes closed sketch regions along the sketch normal and combines the result with a body.',
    properties: {
      body: ID,
      profile: { $ref: '#/$defs/ProfileRef' },
      extent: {
        description:
          'How far the extrusion goes, measured along its direction from the sketch plane.',
        oneOf: [
          variant(
            'distance',
            { expr: EXPR },
            'From the sketch plane to `expr` along the direction.',
          ),
          variant(
            'symmetric',
            { expr: EXPR },
            'Total length `expr`, centered on the sketch plane.',
          ),
          variant(
            'two_sides',
            { expr: EXPR, opposite_expr: EXPR },
            '`expr` along the direction and `opposite_expr` against it.',
          ),
          variant('through_all', {}, 'Through the whole target body along the direction.'),
        ],
      },
      reverse: flag('Extrude against the sketch normal.'),
      op: { $ref: '#/$defs/BodyOp' },
    },
    required: ['body', 'profile', 'extent', 'op'],
  },
)

export const FILLET_ENTRY = entry(
  'fillet',
  {
    label: 'Fillet',
    category: 'modify',
    icon: 'fillet',
    version: 1,
    fields: [
      hint('edges', 'Edges', { essential: true, selection: 'edge' }),
      hint('radius', 'Radius', LENGTH),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Fillet',
    description: 'Rounds edges of a body with a constant radius.',
    properties: { body: ID, edges: array(REFERENCE), radius: EXPR },
    required: ['body', 'edges', 'radius'],
  },
)

export const HOLE_ENTRY = entry(
  'hole',
  {
    label: 'Hole',
    category: 'create',
    icon: 'hole',
    version: 1,
    fields: [
      hint('points', 'Points', { essential: true, selection: 'sketch_point' }),
      hint('diameter', 'Diameter', LENGTH),
      hint('hole_type.kind', 'Type', {
        essential: true,
        options: options(
          ['simple', 'Simple'],
          ['counterbore', 'Counterbore'],
          ['countersink', 'Countersink'],
        ),
      }),
      hint('hole_type.diameter', 'Head diameter', LENGTH),
      hint('hole_type.depth', 'Counterbore depth', LENGTH),
      hint('hole_type.angle', 'Countersink angle', { essential: true, unit: 'angle' }),
      hint('extent.kind', 'Extent', {
        essential: true,
        options: options(['distance', 'Depth'], ['through_all', 'Through all']),
      }),
      hint('extent.expr', 'Depth', LENGTH),
      hint('tip_angle', 'Tip angle', { unit: 'angle' }),
      hint('reverse', 'Reverse'),
      hint('sketch', 'Sketch', { selection: 'sketch' }),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Hole',
    description: 'Cuts round holes at sketch points.',
    properties: {
      body: ID,
      sketch: ID,
      points: array({ type: 'string', pattern: '^[A-Za-z0-9_-]{1,64}(\\.(start|end|center))?$' }),
      diameter: EXPR,
      hole_type: {
        default: { kind: 'simple' },
        oneOf: [
          variant('simple', {}, 'A plain cylinder.'),
          variant('counterbore', { diameter: EXPR, depth: EXPR }, 'A wider cylinder at the top.'),
          variant('countersink', { diameter: EXPR, angle: EXPR }, 'A cone at the top.'),
        ],
      },
      extent: {
        oneOf: [
          variant('distance', { expr: EXPR }, 'To the shoulder at depth `expr`.'),
          variant('through_all', {}, 'Through the whole body.'),
        ],
      },
      tip_angle: optional(EXPR),
      reverse: flag('Drill along the sketch normal instead of against it.'),
    },
    required: ['body', 'sketch', 'points', 'diameter', 'extent'],
  },
)

export const IMPORT_ENTRY = entry(
  'import',
  {
    label: 'Import',
    category: 'create',
    icon: 'import',
    version: 1,
    fields: [
      hint('asset', 'File', { essential: true }),
      hint('op', 'Operation', { essential: true, options: BODY_OPS }),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Import',
    description: 'Puts the solids of a STEP asset into a body.',
    properties: { body: ID, asset: { type: 'string' }, op: { $ref: '#/$defs/BodyOp' } },
    required: ['body', 'asset', 'op'],
  },
)

const LINEAR_PATTERN_DIRECTION: Schema = {
  type: 'object',
  description: 'One direction of a linear pattern.',
  properties: {
    axis: AXIS_REF,
    count: EXPR,
    spacing: EXPR,
    reverse: flag('Go against the axis direction.'),
  },
  required: ['axis', 'count', 'spacing'],
}

export const LINEAR_PATTERN_ENTRY = entry(
  'linear_pattern',
  {
    label: 'Linear pattern',
    category: 'pattern',
    icon: 'linear-pattern',
    version: 1,
    fields: [
      hint('body', 'Body', { essential: true, selection: 'body' }),
      hint('direction.axis', 'Direction', { essential: true, selection: 'axis' }),
      hint('direction.count', 'Count', { essential: true, unit: 'count' }),
      hint('direction.spacing', 'Spacing', LENGTH),
      hint('direction.reverse', 'Reverse'),
      hint('second_direction.axis', 'Second direction', { selection: 'axis' }),
      hint('second_direction.count', 'Second count', { unit: 'count' }),
      hint('second_direction.spacing', 'Second spacing', { unit: 'length' }),
      hint('second_direction.reverse', 'Second reverse'),
      hint('op', 'Operation', { options: COPY_OPS }),
      hint('target', 'Target body', { selection: 'body' }),
    ],
  },
  {
    title: 'Linear pattern',
    description: 'Copies a body along one or two directions.',
    properties: {
      body: ID,
      direction: LINEAR_PATTERN_DIRECTION,
      second_direction: optional(LINEAR_PATTERN_DIRECTION),
      op: { $ref: '#/$defs/CopyOp', default: 'join' },
      target: optional(ID),
    },
    required: ['body', 'direction'],
  },
)

export const MIRROR_ENTRY = entry(
  'mirror',
  {
    label: 'Mirror',
    category: 'pattern',
    icon: 'mirror',
    version: 1,
    fields: [
      hint('body', 'Body', { essential: true, selection: 'body' }),
      hint('plane', 'Mirror plane', { essential: true, selection: 'plane' }),
      hint('op', 'Operation', { essential: true, options: COPY_OPS }),
      hint('target', 'Target body', { essential: true, selection: 'body' }),
    ],
  },
  {
    title: 'Mirror',
    description: 'Reflects a body across a plane.',
    properties: {
      body: ID,
      plane: PLANE_REF,
      op: { $ref: '#/$defs/CopyOp', default: 'join' },
      target: optional(ID),
    },
    required: ['body', 'plane'],
  },
)

export const MOVE_BODY_ENTRY = entry(
  'move_body',
  {
    label: 'Move body',
    category: 'modify',
    icon: 'move-body',
    version: 1,
    fields: [
      hint('body', 'Body', { essential: true, selection: 'body' }),
      hint('translation[0]', 'Move X', LENGTH),
      hint('translation[1]', 'Move Y', LENGTH),
      hint('translation[2]', 'Move Z', LENGTH),
      hint('rotation.axis', 'Rotation axis', { selection: 'axis' }),
      hint('rotation.angle', 'Rotation angle', { unit: 'angle' }),
    ],
  },
  {
    title: 'Move body',
    description: 'Rotates and then translates a body.',
    properties: {
      body: ID,
      translation: { ...triple(EXPR), default: ['0', '0', '0'] },
      rotation: optional({
        type: 'object',
        description: 'A rotation about an axis.',
        properties: { axis: AXIS_REF, angle: EXPR },
        required: ['axis', 'angle'],
      }),
    },
    required: ['body'],
  },
)

export const REVOLVE_ENTRY = entry(
  'revolve',
  {
    label: 'Revolve',
    category: 'create',
    icon: 'revolve',
    version: 1,
    fields: [
      hint('profile', 'Profile', { essential: true, selection: 'sketch_region' }),
      hint('axis', 'Axis', { essential: true, selection: 'axis' }),
      hint('extent.kind', 'Extent', {
        essential: true,
        options: options(['angle', 'Angle'], ['full', 'Full']),
      }),
      hint('extent.expr', 'Angle', { essential: true, unit: 'angle' }),
      hint('op', 'Operation', { essential: true, options: BODY_OPS }),
      hint('reverse', 'Reverse'),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Revolve',
    description:
      'Revolves closed sketch regions around an axis and combines the result with a body.',
    properties: {
      body: ID,
      profile: { $ref: '#/$defs/ProfileRef' },
      axis: AXIS_REF,
      extent: {
        oneOf: [
          variant('angle', { expr: EXPR }, 'Turn by `expr`.'),
          variant('full', {}, 'A full turn.'),
        ],
      },
      reverse: flag('Turn in the negative sense about the axis direction.'),
      op: { $ref: '#/$defs/BodyOp' },
    },
    required: ['body', 'profile', 'axis', 'extent', 'op'],
  },
)

export const SHELL_ENTRY = entry(
  'shell',
  {
    label: 'Shell',
    category: 'modify',
    icon: 'shell',
    version: 1,
    fields: [
      hint('remove_faces', 'Faces to remove', { essential: true, selection: 'face' }),
      hint('thickness', 'Thickness', LENGTH),
      hint('body', 'Body', { selection: 'body' }),
    ],
  },
  {
    title: 'Shell',
    description: 'Hollows a body inward to a wall thickness.',
    properties: {
      body: ID,
      thickness: EXPR,
      remove_faces: { ...array(REFERENCE), default: [] },
    },
    required: ['body', 'thickness'],
  },
)

export const SKETCH_ENTRY = entry(
  'sketch',
  {
    label: 'Sketch',
    category: 'sketch',
    icon: 'sketch',
    version: 1,
    fields: [
      hint('plane', 'Plane', { essential: true, selection: 'plane' }),
      hint('entities', 'Entities'),
      hint('constraints', 'Constraints'),
      hint('constraints[*].expr', 'Dimension'),
    ],
  },
  {
    title: 'Sketch',
    description: 'A 2D sketch placed on a plane.',
    properties: {
      plane: { $ref: '#/$defs/SketchPlane' },
      entities: { ...array({ $ref: '#/$defs/SketchEntity' }), default: [] },
      constraints: { ...array({ $ref: '#/$defs/SketchConstraint' }), default: [] },
    },
    required: ['plane'],
  },
)

export const SPHERE_ENTRY = primitive(
  'sphere',
  'Sphere',
  'A sphere.',
  [hint('radius', 'Radius', LENGTH)],
  { radius: EXPR },
)

export const SPLIT_BODY_ENTRY = entry(
  'split_body',
  {
    label: 'Split body',
    category: 'modify',
    icon: 'split-body',
    version: 1,
    fields: [
      hint('body', 'Body', { essential: true, selection: 'body' }),
      hint('plane', 'Split plane', { essential: true, selection: 'plane' }),
      hint('target', 'Target body', { essential: true, selection: 'body' }),
      hint('flip', 'Flip', { essential: true }),
    ],
  },
  {
    title: 'Split body',
    description: 'Cuts a body in two with a plane.',
    properties: {
      body: ID,
      plane: PLANE_REF,
      flip: flag('Keep the back part instead of the front part.'),
      target: optional(ID),
    },
    required: ['body', 'plane'],
  },
)

export const TORUS_ENTRY = primitive(
  'torus',
  'Torus',
  'A ring torus.',
  [hint('major_radius', 'Ring radius', LENGTH), hint('minor_radius', 'Tube radius', LENGTH)],
  { major_radius: EXPR, minor_radius: EXPR },
)

/** The `feature_catalog` result of the test core: every built-in type, sorted by type as the protocol sends them. */
export const TEST_CATALOG_ENTRIES: readonly CatalogEntry[] = [
  BOOLEAN_ENTRY,
  BOX_ENTRY,
  CHAMFER_ENTRY,
  CIRCULAR_PATTERN_ENTRY,
  CONE_ENTRY,
  CYLINDER_ENTRY,
  DATUM_PLANE_ENTRY,
  EXTRUDE_ENTRY,
  FILLET_ENTRY,
  HOLE_ENTRY,
  IMPORT_ENTRY,
  LINEAR_PATTERN_ENTRY,
  MIRROR_ENTRY,
  MOVE_BODY_ENTRY,
  REVOLVE_ENTRY,
  SHELL_ENTRY,
  SKETCH_ENTRY,
  SPHERE_ENTRY,
  SPLIT_BODY_ENTRY,
  TORUS_ENTRY,
]

/** A catalog entry in the store's form (ui.md 2.8). */
export function catalogFeature(catalogEntry: CatalogEntry): CatalogFeature {
  return {
    type: catalogEntry.type,
    manifest: catalogEntry.manifest,
    paramsSchema: paramsSchemaOf(catalogEntry),
  }
}

function paramsSchemaOf(catalogEntry: CatalogEntry): Schema {
  const schema: unknown = catalogEntry.params_schema
  if (typeof schema !== 'object' || schema === null || Array.isArray(schema)) {
    throw new Error(`The test catalog entry ${catalogEntry.type} has no object params schema.`)
  }
  return Object.fromEntries(Object.entries(schema))
}

/** Builds the store's `Catalog` from protocol catalog entries, in their order. */
export function catalogOf(entries: readonly CatalogEntry[]): Catalog {
  const features = entries.map(catalogFeature)
  return {
    features,
    byType: Object.fromEntries(features.map((feature) => [feature.type, feature])),
  }
}

/** The test catalog in the store's form. */
export const TEST_CATALOG: Catalog = catalogOf(TEST_CATALOG_ENTRIES)
