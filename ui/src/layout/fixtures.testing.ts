// Test support for the ui-model slice: the test catalog (the manifests of features.md 3.13 and
// the feature specs, with params schemas shaped as schemars 1 emits them), the documents of the
// e2e fixtures (ui.md 6.2) the unit tests need, their store states, meshes, element descriptors
// and selection items. Nothing here is part of the app.

import type { BodyMeshData, CoreEvent } from '../core/events'
import type {
  BoundingBox,
  CatalogEntry,
  Document,
  ElementDescriptor,
  FeatureManifest,
  FeatureStatus,
  FieldHint,
  PrinterProfile,
  RegionInfo,
  SketchSolution,
} from '../generated/protocol'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type {
  AppState,
  Catalog,
  CatalogFeature,
  DocumentState,
  MeshSummary,
  OriginName,
  SelectionItem,
} from '../state/store'

export type JsonObject = Readonly<Record<string, unknown>>

// Shared schema definitions, as schemars emits them into each params schema's $defs.

const ID_DEFINITION = {
  description: 'A permanent id: 1 to 64 ASCII letters, digits, "-" or "_".',
  type: 'string',
  pattern: '^[A-Za-z0-9_-]{1,64}$',
}

const EXPR_DEFINITION = {
  description: 'An expression source text, such as "10 mm" or "2 * wall".',
  type: 'string',
}

const REFERENCE_DEFINITION = {
  description: 'A stored face, edge or vertex reference: a semantic query and a fingerprint.',
  type: 'object',
  properties: { q: { type: 'string' }, fp: { type: 'string' } },
  required: ['q', 'fp'],
}

const REGION_REF_DEFINITION = {
  description: 'A stored reference to a region of a sketch.',
  type: 'object',
  properties: { key: { type: 'string' }, fp: { type: 'string' } },
  required: ['key', 'fp'],
}

const PROFILE_REF_DEFINITION = {
  description: 'Closed regions of one sketch used as a profile.',
  type: 'object',
  properties: {
    sketch: { description: 'The sketch feature.', $ref: '#/$defs/Id' },
    regions: {
      description: 'The regions, at least one, in pick order.',
      type: 'array',
      items: { $ref: '#/$defs/RegionRef' },
    },
  },
  required: ['sketch', 'regions'],
}

const BODY_OP_DEFINITION = {
  description: 'How a solid feature combines its tool shape with its target body.',
  oneOf: [
    { description: 'Start the body. The body must be empty at this point.', type: 'string', const: 'new' },
    { description: 'Add the tool to the body.', type: 'string', const: 'join' },
    { description: 'Remove the tool from the body.', type: 'string', const: 'cut' },
    { description: 'Keep only the overlap of body and tool.', type: 'string', const: 'intersect' },
  ],
}

const ORIGIN_PLANE_DEFINITION = { type: 'string', enum: ['XY', 'XZ', 'YZ'] }

const PLANE_REF_DEFINITION = {
  description: 'A plane given by an origin plane, a datum plane, or a flat face of a body.',
  oneOf: [
    {
      description: 'One of the three origin planes.',
      type: 'object',
      properties: {
        kind: { type: 'string', const: 'origin' },
        plane: { $ref: '#/$defs/OriginPlane' },
      },
      required: ['kind', 'plane'],
    },
    {
      description: 'A datum plane feature.',
      type: 'object',
      properties: { kind: { type: 'string', const: 'datum' }, feature: { $ref: '#/$defs/Id' } },
      required: ['kind', 'feature'],
    },
    {
      description: 'A flat face of a body, resolved at this point in the timeline.',
      type: 'object',
      properties: {
        kind: { type: 'string', const: 'face' },
        body: { $ref: '#/$defs/Id' },
        face: { $ref: '#/$defs/Reference' },
      },
      required: ['kind', 'body', 'face'],
    },
  ],
}

function paramsSchema(
  title: string,
  description: string,
  properties: JsonObject,
  required: readonly string[],
  definitions: JsonObject,
): JsonObject {
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title,
    description,
    type: 'object',
    properties,
    required,
    $defs: definitions,
  }
}

/** The extrude params schema (features/extrude.md 2). */
export const EXTRUDE_SCHEMA: JsonObject = paramsSchema(
  'Extrude',
  'Params of an extrude feature.',
  {
    body: { description: 'Target body.', $ref: '#/$defs/Id' },
    profile: { description: 'Closed regions of one sketch.', $ref: '#/$defs/ProfileRef' },
    extent: { description: 'How far the extrusion goes.', $ref: '#/$defs/ExtrudeExtent' },
    reverse: {
      description: 'Extrude against the sketch normal. Default false, omitted when false.',
      type: 'boolean',
      default: false,
    },
    op: { description: 'How the extrusion combines with the body. Required.', $ref: '#/$defs/BodyOp' },
  },
  ['body', 'profile', 'extent', 'op'],
  {
    Id: ID_DEFINITION,
    Expr: EXPR_DEFINITION,
    RegionRef: REGION_REF_DEFINITION,
    ProfileRef: PROFILE_REF_DEFINITION,
    BodyOp: BODY_OP_DEFINITION,
    ExtrudeExtent: {
      description: 'How far an extrusion goes, measured along its direction from the sketch plane.',
      oneOf: [
        {
          description: 'From the sketch plane to `expr` along the direction. Length, greater than 0.',
          type: 'object',
          properties: {
            kind: { type: 'string', const: 'distance' },
            expr: { $ref: '#/$defs/Expr' },
          },
          required: ['kind', 'expr'],
        },
        {
          description: 'Total length `expr`, centered on the sketch plane. Length, greater than 0.',
          type: 'object',
          properties: {
            kind: { type: 'string', const: 'symmetric' },
            expr: { $ref: '#/$defs/Expr' },
          },
          required: ['kind', 'expr'],
        },
        {
          description: '`expr` along the direction and `opposite_expr` against it.',
          type: 'object',
          properties: {
            kind: { type: 'string', const: 'two_sides' },
            expr: { $ref: '#/$defs/Expr' },
            opposite_expr: { $ref: '#/$defs/Expr' },
          },
          required: ['kind', 'expr', 'opposite_expr'],
        },
        {
          description: 'From the sketch plane through the whole target body along the direction.',
          type: 'object',
          properties: { kind: { type: 'string', const: 'through_all' } },
          required: ['kind'],
        },
      ],
    },
  },
)

/** The fillet params schema (features/fillet.md 2). */
export const FILLET_SCHEMA: JsonObject = paramsSchema(
  'Fillet',
  'Params of a fillet feature.',
  {
    body: { description: 'The body whose edges are rounded.', $ref: '#/$defs/Id' },
    edges: {
      description: 'Edges to round, at least one.',
      type: 'array',
      items: { $ref: '#/$defs/Reference' },
    },
    radius: { description: 'Fillet radius. Length, greater than 0.', $ref: '#/$defs/Expr' },
  },
  ['body', 'edges', 'radius'],
  { Id: ID_DEFINITION, Expr: EXPR_DEFINITION, Reference: REFERENCE_DEFINITION },
)

/** The datum plane params schema (features/datum_plane.md 2). */
export const DATUM_PLANE_SCHEMA: JsonObject = paramsSchema(
  'Datum plane',
  'Params of a datum plane feature.',
  {
    base: { description: 'The plane to offset from.', $ref: '#/$defs/PlaneRef' },
    offset: {
      description: 'Distance along the base normal. Length, any sign.',
      $ref: '#/$defs/Expr',
      default: '0',
    },
    flip: {
      description: 'Reverse the normal (the x-axis stays).',
      type: 'boolean',
      default: false,
    },
  },
  ['base'],
  {
    Id: ID_DEFINITION,
    Expr: EXPR_DEFINITION,
    Reference: REFERENCE_DEFINITION,
    OriginPlane: ORIGIN_PLANE_DEFINITION,
    PlaneRef: PLANE_REF_DEFINITION,
  },
)

/** The box params schema (features/box.md and features.md 3.14). */
export const BOX_SCHEMA: JsonObject = paramsSchema(
  'Box',
  'Params of a box feature.',
  {
    body: { description: 'Target body.', $ref: '#/$defs/Id' },
    size: {
      description: 'Length, width and height. Lengths, greater than 0.',
      type: 'array',
      items: { $ref: '#/$defs/Expr' },
      minItems: 3,
      maxItems: 3,
    },
    plane: {
      description: 'The plane the primitive stands on.',
      $ref: '#/$defs/PlaneRef',
      default: { kind: 'origin', plane: 'XY' },
    },
    position: {
      description: "Offset of the primitive's origin in the plane frame.",
      type: 'array',
      items: { $ref: '#/$defs/Expr' },
      minItems: 3,
      maxItems: 3,
      default: ['0', '0', '0'],
    },
    op: { description: 'How the box combines with the body.', $ref: '#/$defs/BodyOp' },
  },
  ['body', 'size', 'op'],
  {
    Id: ID_DEFINITION,
    Expr: EXPR_DEFINITION,
    Reference: REFERENCE_DEFINITION,
    OriginPlane: ORIGIN_PLANE_DEFINITION,
    PlaneRef: PLANE_REF_DEFINITION,
    BodyOp: BODY_OP_DEFINITION,
  },
)

/** The import params schema (features/import.md 2). */
export const IMPORT_SCHEMA: JsonObject = paramsSchema(
  'Import',
  'Params of an import feature.',
  {
    body: { description: 'Target body.', $ref: '#/$defs/Id' },
    asset: { description: 'The STEP asset.', type: 'string' },
    op: { description: 'How the imported solids combine with the body.', $ref: '#/$defs/BodyOp' },
  },
  ['body', 'asset', 'op'],
  { Id: ID_DEFINITION, BodyOp: BODY_OP_DEFINITION },
)

/** The sketch params schema (sketch.md 2.2); entities and constraints are not edited in the inspector. */
export const SKETCH_SCHEMA: JsonObject = paramsSchema(
  'Sketch',
  'Params of a sketch feature.',
  {
    plane: { description: 'Where the sketch lies in model space.', $ref: '#/$defs/SketchPlane' },
    entities: { description: 'Geometry in creation order.', type: 'array', items: { type: 'object' } },
    constraints: {
      description: 'Constraints in creation order.',
      type: 'array',
      items: { type: 'object' },
    },
  },
  ['plane'],
  {
    Id: ID_DEFINITION,
    Reference: REFERENCE_DEFINITION,
    OriginPlane: ORIGIN_PLANE_DEFINITION,
    SketchPlane: {
      oneOf: [
        {
          type: 'object',
          properties: {
            kind: { type: 'string', const: 'origin' },
            plane: { $ref: '#/$defs/OriginPlane' },
          },
          required: ['kind', 'plane'],
        },
        {
          type: 'object',
          properties: { kind: { type: 'string', const: 'datum' }, feature: { $ref: '#/$defs/Id' } },
          required: ['kind', 'feature'],
        },
        {
          type: 'object',
          properties: {
            kind: { type: 'string', const: 'face' },
            body: { $ref: '#/$defs/Id' },
            face: { $ref: '#/$defs/Reference' },
            fallback: { type: 'object' },
          },
          required: ['kind', 'body', 'face', 'fallback'],
        },
      ],
    },
  },
)

// Types the unit tests never open in the inspector get an open object schema.
function openSchema(label: string): JsonObject {
  return paramsSchema(label, `Params of a ${label.toLowerCase()} feature.`, {}, [], {})
}

type HintExtras = Omit<FieldHint, 'path' | 'label'>

function hint(path: string, label: string, extras: HintExtras = {}): FieldHint {
  return { path, label, ...extras }
}

const BODY_OPS = [
  { value: 'new', label: 'New body' },
  { value: 'join', label: 'Join' },
  { value: 'cut', label: 'Cut' },
  { value: 'intersect', label: 'Intersect' },
]

const COPY_OPS = [
  { value: 'join', label: 'Join' },
  { value: 'new', label: 'New body' },
]

// The primitives' shared placement hints (features.md 3.14).
const PLACEMENT_HINTS: readonly FieldHint[] = [
  hint('plane', 'Plane', { selection: 'plane' }),
  hint('position[0]', 'Position X', { unit: 'length' }),
  hint('position[1]', 'Position Y', { unit: 'length' }),
  hint('position[2]', 'Position Z', { unit: 'length' }),
]

function manifest(
  label: string,
  category: FeatureManifest['category'],
  icon: string,
  fields: readonly FieldHint[],
): FeatureManifest {
  return { label, category, icon, version: 1, fields: [...fields] }
}

export const EXTRUDE_MANIFEST: FeatureManifest = manifest('Extrude', 'create', 'extrude', [
  hint('profile', 'Profile', { essential: true, selection: 'sketch_region' }),
  hint('extent.kind', 'Extent', {
    essential: true,
    options: [
      { value: 'distance', label: 'Distance' },
      { value: 'symmetric', label: 'Symmetric' },
      { value: 'two_sides', label: 'Two sides' },
      { value: 'through_all', label: 'Through all' },
    ],
  }),
  hint('extent.expr', 'Distance', { essential: true, unit: 'length' }),
  hint('extent.opposite_expr', 'Opposite distance', { essential: true, unit: 'length' }),
  hint('op', 'Operation', { essential: true, options: BODY_OPS }),
  hint('reverse', 'Reverse'),
  hint('body', 'Body', { selection: 'body' }),
])

export const FILLET_MANIFEST: FeatureManifest = manifest('Fillet', 'modify', 'fillet', [
  hint('edges', 'Edges', { essential: true, selection: 'edge' }),
  hint('radius', 'Radius', { essential: true, unit: 'length' }),
  hint('body', 'Body', { selection: 'body' }),
])

const MANIFESTS: readonly FeatureManifest[] = [
  manifest('Boolean', 'modify', 'boolean', [
    hint('target', 'Target body', { essential: true, selection: 'body' }),
    hint('tools', 'Tool bodies', { essential: true, selection: 'body' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS.slice(1) }),
    hint('keep_tools', 'Keep tools', { essential: true }),
  ]),
  manifest('Box', 'create', 'box', [
    hint('size[0]', 'Length', { essential: true, unit: 'length' }),
    hint('size[1]', 'Width', { essential: true, unit: 'length' }),
    hint('size[2]', 'Height', { essential: true, unit: 'length' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    ...PLACEMENT_HINTS,
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Chamfer', 'modify', 'chamfer', [
    hint('edges', 'Edges', { essential: true, selection: 'edge' }),
    hint('distance.kind', 'Type', {
      essential: true,
      options: [
        { value: 'equal', label: 'Equal distance' },
        { value: 'two_distances', label: 'Two distances' },
      ],
    }),
    hint('distance.expr', 'Distance', { essential: true, unit: 'length' }),
    hint('distance.second_expr', 'Second distance', { essential: true, unit: 'length' }),
    hint('distance.flip', 'Flip'),
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Circular pattern', 'pattern', 'circular-pattern', [
    hint('body', 'Body', { essential: true, selection: 'body' }),
    hint('axis', 'Axis', { essential: true, selection: 'axis' }),
    hint('count', 'Count', { essential: true, unit: 'count' }),
    hint('angle', 'Total angle', { essential: true, unit: 'angle' }),
    hint('reverse', 'Reverse'),
    hint('op', 'Operation', { options: COPY_OPS }),
    hint('target', 'Target body', { selection: 'body' }),
  ]),
  manifest('Cone', 'create', 'cone', [
    hint('bottom_radius', 'Bottom radius', { essential: true, unit: 'length' }),
    hint('top_radius', 'Top radius', { essential: true, unit: 'length' }),
    hint('height', 'Height', { essential: true, unit: 'length' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    ...PLACEMENT_HINTS,
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Cylinder', 'create', 'cylinder', [
    hint('radius', 'Radius', { essential: true, unit: 'length' }),
    hint('height', 'Height', { essential: true, unit: 'length' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    ...PLACEMENT_HINTS,
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Datum plane', 'construct', 'datum-plane', [
    hint('base', 'Base plane', { essential: true, selection: 'plane' }),
    hint('offset', 'Offset', { essential: true, unit: 'length' }),
    hint('flip', 'Flip'),
  ]),
  EXTRUDE_MANIFEST,
  FILLET_MANIFEST,
  manifest('Hole', 'create', 'hole', [
    hint('points', 'Points', { essential: true, selection: 'sketch_point' }),
    hint('diameter', 'Diameter', { essential: true, unit: 'length' }),
    hint('hole_type.kind', 'Type', {
      essential: true,
      options: [
        { value: 'simple', label: 'Simple' },
        { value: 'counterbore', label: 'Counterbore' },
        { value: 'countersink', label: 'Countersink' },
      ],
    }),
    hint('hole_type.diameter', 'Head diameter', { essential: true, unit: 'length' }),
    hint('hole_type.depth', 'Counterbore depth', { essential: true, unit: 'length' }),
    hint('hole_type.angle', 'Countersink angle', { essential: true, unit: 'angle' }),
    hint('extent.kind', 'Extent', {
      essential: true,
      options: [
        { value: 'distance', label: 'Depth' },
        { value: 'through_all', label: 'Through all' },
      ],
    }),
    hint('extent.expr', 'Depth', { essential: true, unit: 'length' }),
    hint('tip_angle', 'Tip angle', { unit: 'angle' }),
    hint('reverse', 'Reverse'),
    hint('sketch', 'Sketch', { selection: 'sketch' }),
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Import', 'create', 'import', [
    hint('asset', 'File', { essential: true }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Linear pattern', 'pattern', 'linear-pattern', [
    hint('body', 'Body', { essential: true, selection: 'body' }),
    hint('direction.axis', 'Direction', { essential: true, selection: 'axis' }),
    hint('direction.count', 'Count', { essential: true, unit: 'count' }),
    hint('direction.spacing', 'Spacing', { essential: true, unit: 'length' }),
    hint('direction.reverse', 'Reverse'),
    hint('second_direction.axis', 'Second direction', { selection: 'axis' }),
    hint('second_direction.count', 'Second count', { unit: 'count' }),
    hint('second_direction.spacing', 'Second spacing', { unit: 'length' }),
    hint('second_direction.reverse', 'Second reverse'),
    hint('op', 'Operation', { options: COPY_OPS }),
    hint('target', 'Target body', { selection: 'body' }),
  ]),
  manifest('Mirror', 'pattern', 'mirror', [
    hint('body', 'Body', { essential: true, selection: 'body' }),
    hint('plane', 'Mirror plane', { essential: true, selection: 'plane' }),
    hint('op', 'Operation', { essential: true, options: COPY_OPS }),
    hint('target', 'Target body', { essential: true, selection: 'body' }),
  ]),
  manifest('Move body', 'modify', 'move-body', [
    hint('body', 'Body', { essential: true, selection: 'body' }),
    hint('translation[0]', 'Move X', { essential: true, unit: 'length' }),
    hint('translation[1]', 'Move Y', { essential: true, unit: 'length' }),
    hint('translation[2]', 'Move Z', { essential: true, unit: 'length' }),
    hint('rotation.axis', 'Rotation axis', { selection: 'axis' }),
    hint('rotation.angle', 'Rotation angle', { unit: 'angle' }),
  ]),
  manifest('Revolve', 'create', 'revolve', [
    hint('profile', 'Profile', { essential: true, selection: 'sketch_region' }),
    hint('axis', 'Axis', { essential: true, selection: 'axis' }),
    hint('extent.kind', 'Extent', {
      essential: true,
      options: [
        { value: 'angle', label: 'Angle' },
        { value: 'full', label: 'Full' },
      ],
    }),
    hint('extent.expr', 'Angle', { essential: true, unit: 'angle' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    hint('reverse', 'Reverse'),
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Shell', 'modify', 'shell', [
    hint('remove_faces', 'Faces to remove', { essential: true, selection: 'face' }),
    hint('thickness', 'Thickness', { essential: true, unit: 'length' }),
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Sketch', 'sketch', 'sketch', [
    hint('plane', 'Plane', { essential: true, selection: 'plane' }),
    hint('entities', 'Entities'),
    hint('constraints', 'Constraints'),
    hint('constraints[*].expr', 'Dimension'),
  ]),
  manifest('Sphere', 'create', 'sphere', [
    hint('radius', 'Radius', { essential: true, unit: 'length' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    ...PLACEMENT_HINTS,
    hint('body', 'Body', { selection: 'body' }),
  ]),
  manifest('Split body', 'modify', 'split-body', [
    hint('body', 'Body', { essential: true, selection: 'body' }),
    hint('plane', 'Split plane', { essential: true, selection: 'plane' }),
    hint('target', 'Target body', { essential: true, selection: 'body' }),
    hint('flip', 'Flip', { essential: true }),
  ]),
  manifest('Torus', 'create', 'torus', [
    hint('major_radius', 'Ring radius', { essential: true, unit: 'length' }),
    hint('minor_radius', 'Tube radius', { essential: true, unit: 'length' }),
    hint('op', 'Operation', { essential: true, options: BODY_OPS }),
    ...PLACEMENT_HINTS,
    hint('body', 'Body', { selection: 'body' }),
  ]),
]

/** Feature type by manifest label (features.md 3.13). */
const TYPE_BY_LABEL: Readonly<Record<string, string>> = {
  Boolean: 'boolean',
  Box: 'box',
  Chamfer: 'chamfer',
  'Circular pattern': 'circular_pattern',
  Cone: 'cone',
  Cylinder: 'cylinder',
  'Datum plane': 'datum_plane',
  Extrude: 'extrude',
  Fillet: 'fillet',
  Hole: 'hole',
  Import: 'import',
  'Linear pattern': 'linear_pattern',
  Mirror: 'mirror',
  'Move body': 'move_body',
  Revolve: 'revolve',
  Shell: 'shell',
  Sketch: 'sketch',
  Sphere: 'sphere',
  'Split body': 'split_body',
  Torus: 'torus',
}

const SCHEMAS: Readonly<Record<string, JsonObject>> = {
  extrude: EXTRUDE_SCHEMA,
  fillet: FILLET_SCHEMA,
  datum_plane: DATUM_PLANE_SCHEMA,
  box: BOX_SCHEMA,
  import: IMPORT_SCHEMA,
  sketch: SKETCH_SCHEMA,
}

/** The test catalog as `feature_catalog` returns it: every built-in type, sorted by type. */
export const TEST_CATALOG_ENTRIES: readonly CatalogEntry[] = MANIFESTS.map((entry) => {
  const type = TYPE_BY_LABEL[entry.label] ?? entry.label
  return { type, manifest: entry, params_schema: SCHEMAS[type] ?? openSchema(entry.label) }
}).toSorted((first, second) => (first.type < second.type ? -1 : 1))

/** Every catalog type, sorted. */
export const CATALOG_TYPES: readonly string[] = TEST_CATALOG_ENTRIES.map((entry) => entry.type)

export function catalogFeature(entry: CatalogEntry): CatalogFeature {
  return {
    type: entry.type,
    manifest: entry.manifest,
    paramsSchema: entry.params_schema as JsonObject,
  }
}

/** The store's catalog for a list of entries. */
export function catalogOf(entries: readonly CatalogEntry[]): Catalog {
  const features = entries.map(catalogFeature)
  return {
    features,
    byType: Object.fromEntries(features.map((feature) => [feature.type, feature])),
  }
}

export const TEST_CATALOG: Catalog = catalogOf(TEST_CATALOG_ENTRIES)

/** The catalog feature of one type of the test catalog. */
export function testFeature(type: string): CatalogFeature {
  const feature = TEST_CATALOG.byType[type]
  if (feature === undefined) {
    throw new Error(`The test catalog has no type ${type}.`)
  }
  return feature
}

/** The default FDM profile (protocol.md 2.5 command 3). */
export const GENERIC_FDM: PrinterProfile = {
  id: 'generic-fdm',
  name: 'Generic FDM 0.4 mm',
  technology: 'fdm',
  bed: { kind: 'rectangle', width: 256, depth: 256, height: 256 },
  nozzle_diameter: 0.4,
  layer_height: 0.2,
  max_overhang_angle: Math.PI / 4,
  max_bridge_length: 10,
  fit: { press: 0.05, snug: 0.1, slip: 0.2, loose: 0.4 },
} as PrinterProfile

// Documents of the e2e fixtures (ui.md 6.2).

const XY_PLACEMENT = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] } as const

/** A closed rectangle of four lines from corner (x0, y0) to (x1, y1), as the 2-point rectangle tool draws it. */
function rectangleEntities(x0: number, y0: number, x1: number, y1: number): JsonObject[] {
  return [
    { id: 'e-1', type: 'line', start: [x0, y0], end: [x1, y0] },
    { id: 'e-2', type: 'line', start: [x1, y0], end: [x1, y1] },
    { id: 'e-3', type: 'line', start: [x1, y1], end: [x0, y1] },
    { id: 'e-4', type: 'line', start: [x0, y1], end: [x0, y0] },
  ]
}

const RECTANGLE_CONSTRAINTS: readonly JsonObject[] = [
  { id: 'c-1', type: 'coincident', points: ['e-1.end', 'e-2.start'] },
  { id: 'c-2', type: 'coincident', points: ['e-2.end', 'e-3.start'] },
  { id: 'c-3', type: 'coincident', points: ['e-3.end', 'e-4.start'] },
  { id: 'c-4', type: 'coincident', points: ['e-4.end', 'e-1.start'] },
  { id: 'c-5', type: 'horizontal', line: 'e-1' },
  { id: 'c-6', type: 'horizontal', line: 'e-3' },
  { id: 'c-7', type: 'vertical', line: 'e-2' },
  { id: 'c-8', type: 'vertical', line: 'e-4' },
]

/** `f-sk1` of rect-sketch.ptsm: a 40 by 20 rectangle with a corner on the origin. */
export const RECT_SKETCH_FEATURE: JsonObject = {
  id: 'f-sk1',
  type: 'sketch',
  plane: { kind: 'origin', plane: 'XY' },
  entities: rectangleEntities(0, 0, 40, 20),
  constraints: [
    ...RECTANGLE_CONSTRAINTS,
    { id: 'c-9', type: 'coincident', points: ['e-1.start', 'origin'] },
    { id: 'c-10', type: 'distance', line: 'e-1', expr: '40 mm' },
    { id: 'c-11', type: 'distance', line: 'e-4', expr: '20 mm' },
  ],
}

/** `f-sk1` of cube.ptsm: a 20 by 20 rectangle centered on the origin. */
export const CUBE_SKETCH_FEATURE: JsonObject = {
  id: 'f-sk1',
  type: 'sketch',
  plane: { kind: 'origin', plane: 'XY' },
  entities: rectangleEntities(-10, -10, 10, 10),
  constraints: [
    ...RECTANGLE_CONSTRAINTS,
    { id: 'c-9', type: 'symmetric', points: ['e-1.start', 'e-2.end'], line: 'y_axis' },
    { id: 'c-10', type: 'symmetric', points: ['e-1.start', 'e-3.start'], line: 'x_axis' },
    { id: 'c-11', type: 'distance', line: 'e-1', expr: '20 mm' },
    { id: 'c-12', type: 'distance', line: 'e-2', expr: '20 mm' },
  ],
}

export const CUBE_REGION: RegionInfo = {
  reference: { key: 'e-1,e-2,e-3,e-4', fp: 'c=0,0|a=400' },
  area: 400,
  centroid: [0, 0],
}

export const RECT_REGION: RegionInfo = {
  reference: { key: 'e-1,e-2,e-3,e-4', fp: 'c=20,10|a=800' },
  area: 800,
  centroid: [20, 10],
}

export const CUBE_EXTRUDE_FEATURE: JsonObject = {
  id: 'f-ext1',
  type: 'extrude',
  body: 'b-cube',
  profile: { sketch: 'f-sk1', regions: [CUBE_REGION.reference] },
  extent: { kind: 'distance', expr: '20 mm' },
  op: 'new',
}

/** References of the cube's four vertical edges and its top face. */
export const VERTICAL_EDGE_REFERENCES = [
  { q: 'edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)', fp: 'line|n=0,0,1|c=10,-10,10|l=20|adj=2,2' },
  { q: 'edge(f-ext1/lateral/e-2|f-ext1/lateral/e-3)', fp: 'line|n=0,0,1|c=10,10,10|l=20|adj=2,2' },
  { q: 'edge(f-ext1/lateral/e-3|f-ext1/lateral/e-4)', fp: 'line|n=0,0,1|c=-10,10,10|l=20|adj=2,2' },
  { q: 'edge(f-ext1/lateral/e-1|f-ext1/lateral/e-4)', fp: 'line|n=0,0,1|c=-10,-10,10|l=20|adj=2,2' },
] as const

export const TOP_FACE_REFERENCE = {
  q: 'f-ext1/end-cap',
  fp: 'plane|n=0,0,1|c=0,0,20|a=400|adj=4,4',
} as const

function document(
  features: readonly JsonObject[],
  bodies: Document['bodies'] = [],
  extras: Partial<Document> = {},
): Document {
  return {
    format: 'partsmith',
    format_version: 1,
    units: 'mm',
    parameters: [],
    bodies,
    features: features as Document['features'],
    ...extras,
  }
}

/** rect-sketch.ptsm. */
export const RECT_SKETCH_DOCUMENT: Document = document([RECT_SKETCH_FEATURE])

/** cube.ptsm. */
export const CUBE_DOCUMENT: Document = document(
  [CUBE_SKETCH_FEATURE, CUBE_EXTRUDE_FEATURE],
  [{ id: 'b-cube', name: 'Cube' }],
)

/** cube-features.ptsm: cube.ptsm plus a 2 mm fillet on the vertical edges and a 2 mm shell without the top. */
export const CUBE_FEATURES_DOCUMENT: Document = document(
  [
    CUBE_SKETCH_FEATURE,
    CUBE_EXTRUDE_FEATURE,
    {
      id: 'f-fil1',
      type: 'fillet',
      body: 'b-cube',
      edges: [...VERTICAL_EDGE_REFERENCES],
      radius: '2 mm',
    },
    { id: 'f-sh1', type: 'shell', body: 'b-cube', remove_faces: [TOP_FACE_REFERENCE], thickness: '2 mm' },
  ],
  [{ id: 'b-cube', name: 'Cube' }],
)

/** two-bodies.ptsm: cube.ptsm plus the body "Peg" and a cylinder. */
export const TWO_BODIES_DOCUMENT: Document = document(
  [
    CUBE_SKETCH_FEATURE,
    CUBE_EXTRUDE_FEATURE,
    {
      id: 'f-cyl1',
      type: 'cylinder',
      body: 'b-peg',
      radius: '4 mm',
      height: '20 mm',
      position: ['0', '0', '10'],
      op: 'new',
    },
  ],
  [
    { id: 'b-cube', name: 'Cube' },
    { id: 'b-peg', name: 'Peg' },
  ],
)

/** parametric.ptsm's parameter on cube.ptsm. */
export const PARAMETRIC_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  parameters: [{ id: 'p-size', name: 'size', expr: '20 mm' }],
  features: [
    CUBE_SKETCH_FEATURE,
    { ...CUBE_EXTRUDE_FEATURE, extent: { kind: 'distance', expr: 'size' } },
  ] as Document['features'],
}

/** A document whose only feature is an import (U-89). */
export const IMPORT_DOCUMENT: Document = document(
  [
    {
      id: 'f-imp1',
      type: 'import',
      body: 'b-imp',
      asset: '7c3e9a1f0b2d4c6e8a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f.step',
      op: 'new',
    },
  ],
  [{ id: 'b-imp', name: 'Bracket' }],
)

/** Three boxes on three bodies, for timeline and rollback tests. */
export const THREE_BOXES_DOCUMENT: Document = document(
  ['f-box1', 'f-box2', 'f-box3'].map((id, index) => ({
    id,
    type: 'box',
    body: `b-${String(index + 1)}`,
    size: ['10', '10', '10'],
    position: [String(20 * index), '0', '0'],
    op: 'new',
  })),
  [
    { id: 'b-1', name: 'Body 1' },
    { id: 'b-2', name: 'Body 2' },
    { id: 'b-3', name: 'Body 3' },
  ],
)

// Sketch solutions.

function lineEntity(id: string, start: readonly number[], end: readonly number[]) {
  return { id, type: 'line', start: [...start], end: [...end], constrained: 'fully_constrained' }
}

function rectangleSolution(
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  region: RegionInfo,
  constraintCount: number,
): SketchSolution {
  const corners = [
    [x0, y0],
    [x1, y0],
    [x1, y1],
    [x0, y1],
  ] as const
  const entities = corners.map((corner, index) =>
    lineEntity(`e-${String(index + 1)}`, corner, corners[(index + 1) % 4] ?? corner),
  )
  return {
    feature: 'f-sk1',
    status: 'fully_constrained',
    failed: false,
    dof: 0,
    redundant: [],
    conflicting: [],
    placement: XY_PLACEMENT,
    entities,
    points: entities.flatMap((entity) => [
      { point: `${entity.id}.start`, at: entity.start },
      { point: `${entity.id}.end`, at: entity.end },
    ]),
    constraints: Array.from({ length: constraintCount }, (_, index) => ({
      id: `c-${String(index + 1)}`,
      state: 'satisfied',
    })),
    regions: [region],
    profile_findings: [],
  } as SketchSolution
}

export const CUBE_SKETCH_SOLUTION: SketchSolution = rectangleSolution(-10, -10, 10, 10, CUBE_REGION, 12)
export const RECT_SKETCH_SOLUTION: SketchSolution = rectangleSolution(0, 0, 40, 20, RECT_REGION, 11)

// Element descriptors of the cube (list_elements order: faces sorted by name).

type FaceSpec = {
  readonly name: string
  readonly centroid: readonly [number, number, number]
  readonly normal: readonly [number, number, number]
  readonly xAxis: readonly [number, number, number]
}

const CUBE_FACE_SPECS: readonly FaceSpec[] = [
  { name: 'f-ext1/end-cap', centroid: [0, 0, 20], normal: [0, 0, 1], xAxis: [1, 0, 0] },
  { name: 'f-ext1/lateral/e-1', centroid: [0, -10, 10], normal: [0, -1, 0], xAxis: [1, 0, 0] },
  { name: 'f-ext1/lateral/e-2', centroid: [10, 0, 10], normal: [1, 0, 0], xAxis: [0, 1, 0] },
  { name: 'f-ext1/lateral/e-3', centroid: [0, 10, 10], normal: [0, 1, 0], xAxis: [1, 0, 0] },
  { name: 'f-ext1/lateral/e-4', centroid: [-10, 0, 10], normal: [-1, 0, 0], xAxis: [0, 1, 0] },
  { name: 'f-ext1/start-cap', centroid: [0, 0, 0], normal: [0, 0, -1], xAxis: [1, 0, 0] },
]

function fingerprint(spec: FaceSpec): string {
  return `plane|n=${spec.normal.join(',')}|c=${spec.centroid.join(',')}|a=400|adj=4,4`
}

export function planarFace(spec: FaceSpec, body = 'b-cube', feature = 'f-ext1'): ElementDescriptor {
  return {
    name: spec.name,
    kind: 'face',
    body,
    feature,
    reference: { q: spec.name, fp: fingerprint(spec) },
    surface: 'plane',
    centroid: [...spec.centroid],
    area: 400,
    normal: [...spec.normal],
    placement: { origin: [...spec.centroid], x_axis: [...spec.xAxis], normal: [...spec.normal] },
    adjacent: [],
  } as ElementDescriptor
}

/** The cube's six faces in list_elements order; display face id i is position i. */
export const CUBE_FACES: readonly ElementDescriptor[] = CUBE_FACE_SPECS.map((spec) => planarFace(spec))

/** The top face of the cube (display face id 0). */
export const TOP_FACE: ElementDescriptor = CUBE_FACES[0] as ElementDescriptor

/** A vertical straight edge of the cube. */
export const VERTICAL_EDGE: ElementDescriptor = {
  name: VERTICAL_EDGE_REFERENCES[0].q,
  kind: 'edge',
  body: 'b-cube',
  feature: 'f-ext1',
  reference: { ...VERTICAL_EDGE_REFERENCES[0] },
  curve: 'line',
  centroid: [10, -10, 10],
  length: 20,
  direction: [0, 0, 1],
  adjacent: [],
} as ElementDescriptor

/** A circular edge of the peg. */
export const CIRCULAR_EDGE: ElementDescriptor = {
  name: 'edge(f-cyl1/side|f-cyl1/top)',
  kind: 'edge',
  body: 'b-peg',
  feature: 'f-cyl1',
  reference: { q: 'edge(f-cyl1/side|f-cyl1/top)', fp: 'circle|n=0,0,1|c=0,0,30|l=25.132741|r=4' },
  curve: 'circle',
  centroid: [0, 0, 30],
  length: 25.132741,
  axis: { origin: [0, 0, 30], direction: [0, 0, 1] },
  radius: 4,
  adjacent: [],
} as ElementDescriptor

/** The peg's cylindrical side face. */
export const CYLINDRICAL_FACE: ElementDescriptor = {
  name: 'f-cyl1/side',
  kind: 'face',
  body: 'b-peg',
  feature: 'f-cyl1',
  reference: { q: 'f-cyl1/side', fp: 'cylinder|n=0,0,1|c=0,0,20|a=502.654825|r=4' },
  surface: 'cylinder',
  centroid: [0, 0, 20],
  area: 502.654825,
  axis: { origin: [0, 0, 0], direction: [0, 0, 1] },
  radius: 4,
  adjacent: [],
} as ElementDescriptor

/** A vertex of the cube. */
export const CORNER_VERTEX: ElementDescriptor = {
  name: 'vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-2)',
  kind: 'vertex',
  body: 'b-cube',
  feature: 'f-ext1',
  reference: {
    q: 'vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-2)',
    fp: 'vertex|c=10,-10,20|adj=3',
  },
  centroid: [10, -10, 20],
  adjacent: [],
} as ElementDescriptor

// Meshes.

/** Mesh buffers of an axis-aligned box whose display face ids follow the cube's list_elements order. */
export function boxMesh(min: readonly [number, number, number], max: readonly [number, number, number]): BodyMeshData {
  const [x0, y0, z0] = min
  const [x1, y1, z1] = max
  // Corners of each face counterclockwise seen from outside, in the order of CUBE_FACE_SPECS.
  const faces: readonly { normal: readonly number[]; corners: readonly (readonly number[])[] }[] = [
    { normal: [0, 0, 1], corners: [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]] },
    { normal: [0, -1, 0], corners: [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]] },
    { normal: [1, 0, 0], corners: [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]] },
    { normal: [0, 1, 0], corners: [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]] },
    { normal: [-1, 0, 0], corners: [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]] },
    { normal: [0, 0, -1], corners: [[x0, y1, z0], [x1, y1, z0], [x1, y0, z0], [x0, y0, z0]] },
  ]
  const positions: number[] = []
  const normals: number[] = []
  const indices: number[] = []
  const faceIds: number[] = []
  faces.forEach((face, faceIndex) => {
    const base = faceIndex * 4
    for (const corner of face.corners) {
      positions.push(...corner)
      normals.push(...face.normal)
      faceIds.push(faceIndex)
    }
    indices.push(base, base + 1, base + 2, base, base + 2, base + 3)
  })
  const corners: readonly (readonly [number, number, number])[] = [
    [x0, y0, z0],
    [x1, y0, z0],
    [x1, y1, z0],
    [x0, y1, z0],
    [x0, y0, z1],
    [x1, y0, z1],
    [x1, y1, z1],
    [x0, y1, z1],
  ]
  const edgePairs: readonly (readonly [number, number])[] = [
    [0, 1], [1, 2], [2, 3], [3, 0],
    [4, 5], [5, 6], [6, 7], [7, 4],
    [0, 4], [1, 5], [2, 6], [3, 7],
  ]
  const edgePositions = edgePairs.flatMap(([start, end]) => [
    ...(corners[start] ?? []),
    ...(corners[end] ?? []),
  ])
  return {
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    indices: new Uint32Array(indices),
    faceIds: new Uint32Array(faceIds),
    edgePositions: new Float32Array(edgePositions),
    edgeIds: new Uint32Array(edgePairs.map((_, index) => index)),
    vertexPositions: new Float32Array(corners.flat()),
    vertexIds: new Uint32Array(corners.map((_, index) => index)),
    faceCount: 6,
    edgeCount: 12,
    vertexCount: 8,
    boundingBox: { min: [x0, y0, z0], max: [x1, y1, z1] },
  }
}

export const CUBE_BOUNDS: BoundingBox = { min: [-10, -10, 0], max: [10, 10, 20] }
export const PEG_BOUNDS: BoundingBox = { min: [-4, -4, 10], max: [4, 4, 30] }

export function meshSummary(meshId: number, bounds: BoundingBox): MeshSummary {
  return { meshId, boundingBox: bounds, faceCount: 6, edgeCount: 12, vertexCount: 8 }
}

/** A `body_mesh` event of document 1. */
export function bodyMeshEvent(input: {
  readonly body: string
  readonly meshId: number
  readonly mesh: BodyMeshData | null
  readonly layer?: 'model' | 'preview'
  readonly revision?: number
}): CoreEvent {
  return {
    type: 'body_mesh',
    documentHandle: 1,
    revision: input.revision ?? 1,
    body: input.body,
    layer: input.layer ?? 'model',
    meshId: input.meshId,
    mesh: input.mesh,
  }
}

// Store states.

function okStatus(id: string): FeatureStatus {
  return { id, status: 'ok', messages: [], repairs: [] }
}

/** The store's document state for a recipe: every feature ok, no meshes unless given. */
export function documentState(
  recipe: Document,
  overrides: Partial<DocumentState> = {},
): DocumentState {
  return {
    documentHandle: 1,
    name: 'Untitled',
    file: null,
    revision: 1,
    recipe,
    dirty: false,
    localUnsaved: false,
    undoLabel: null,
    redoLabel: null,
    transactionOpen: false,
    focus: null,
    featureStatus: Object.fromEntries(recipe.features.map((feature) => [feature.id, okStatus(feature.id)])),
    parameterValues: {},
    diagnostics: [],
    sketches: {},
    datums: {},
    meshes: {},
    lastRebuildMilliseconds: null,
    ...overrides,
  }
}

/** cube.ptsm as the store holds it after opening: the cube mesh, the sketch solution, the sketch hidden. */
export function cubeDocumentState(overrides: Partial<DocumentState> = {}): DocumentState {
  return documentState(CUBE_DOCUMENT, {
    name: 'cube',
    sketches: { 'f-sk1': { solution: CUBE_SKETCH_SOLUTION, display: null } },
    meshes: { 'b-cube': meshSummary(1, CUBE_BOUNDS) },
    lastRebuildMilliseconds: 12,
    ...overrides,
  })
}

/** rect-sketch.ptsm as the store holds it after opening. */
export function rectSketchDocumentState(overrides: Partial<DocumentState> = {}): DocumentState {
  return documentState(RECT_SKETCH_DOCUMENT, {
    name: 'rect-sketch',
    sketches: { 'f-sk1': { solution: RECT_SKETCH_SOLUTION, display: null } },
    ...overrides,
  })
}

/** A ready app state around a document: the test catalog, the default printer, the light theme. */
export function readyState(
  document: DocumentState | null,
  overrides: Partial<AppState> = {},
): Partial<AppState> {
  return {
    core: { status: 'ready', engineVersion: '0.1.0', fatal: null, pendingMutations: 0 },
    catalog: TEST_CATALOG,
    printers: [GENERIC_FDM],
    document,
    settings: { ...DEFAULT_SETTINGS, theme: 'light' },
    ...overrides,
  }
}

// Selection items (ui.md 2.8 and the keys of 3.6).

export function bodyItem(id: string, name: string): SelectionItem {
  return {
    key: `body:${id}`,
    kind: 'body',
    label: name,
    body: id,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

export function featureItem(id: string, label: string): SelectionItem {
  return {
    key: `feature:${id}`,
    kind: 'feature',
    label,
    body: null,
    feature: id,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

export function datumItem(id: string, label: string): SelectionItem {
  return { ...featureItem(id, label), kind: 'datum_plane' }
}

export function parameterItem(id: string, name: string): SelectionItem {
  return {
    key: `parameter:${id}`,
    kind: 'parameter',
    label: name,
    body: null,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

/** A face, edge or vertex item from its descriptor. */
export function elementItem(element: ElementDescriptor, bodyName = 'Cube'): SelectionItem {
  const noun = element.kind === 'face' ? 'Face' : element.kind === 'edge' ? 'Edge' : 'Vertex'
  return {
    key: `element:${element.body}:${element.name}`,
    kind: element.kind,
    label: `${noun} of ${bodyName}`,
    body: element.body,
    feature: element.feature,
    element,
    sketchItem: null,
    origin: null,
  }
}

const ORIGIN_LABELS: Readonly<Record<OriginName, string>> = {
  XY: 'XY plane',
  XZ: 'XZ plane',
  YZ: 'YZ plane',
  X: 'X axis',
  Y: 'Y axis',
  Z: 'Z axis',
  O: 'Origin',
}

export function originItem(origin: OriginName): SelectionItem {
  const kind =
    origin === 'O'
      ? 'origin_point'
      : origin === 'XY' || origin === 'XZ' || origin === 'YZ'
        ? 'origin_plane'
        : 'origin_axis'
  return {
    key: `origin:${origin}`,
    kind,
    label: ORIGIN_LABELS[origin],
    body: null,
    feature: null,
    element: null,
    sketchItem: null,
    origin,
  }
}

export function regionItem(sketch: string, region: RegionInfo, label = 'Region 1'): SelectionItem {
  return {
    key: `region:${sketch}:${region.reference.key}`,
    kind: 'sketch_region',
    label,
    body: null,
    feature: sketch,
    element: null,
    sketchItem: { kind: 'region', sketch, region },
    origin: null,
  }
}

export function sketchLineItem(sketch: string, line: string): SelectionItem {
  return {
    key: `sketch:${sketch}:${line}`,
    kind: 'sketch_curve',
    label: `Line ${line}`,
    body: null,
    feature: sketch,
    element: null,
    sketchItem: { kind: 'entity', sketch, entity: line, entityType: 'line' },
    origin: null,
  }
}

export function sketchPointItem(sketch: string, point: string): SelectionItem {
  return {
    key: `point:${sketch}:${point}`,
    kind: 'sketch_point',
    label: `Point ${point}`,
    body: null,
    feature: sketch,
    element: null,
    sketchItem: { kind: 'point', sketch, point },
    origin: null,
  }
}
