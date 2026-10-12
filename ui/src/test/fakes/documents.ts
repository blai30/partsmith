import type {
  Document,
  ElementDescriptor,
  Placement,
  RegionInfo,
  SketchSolution,
  Units,
} from '../../generated/protocol'
import type { OpenedFile } from '../../files/file-service'

// Recipes of the e2e fixtures (ui.md 6.2) as the core sends them in document_changed, the sketch
// solutions the test core reports for their sketches, and element descriptors of the cube.

/** One timeline record of a document. */
export type FeatureRecord = NonNullable<Document['features']>[number]
type SolvedEntity = SketchSolution['entities'][number]
type Point2 = [number, number]

/** The parameters of a recipe; the protocol may omit the empty list. */
export function parametersOf(document: Document): NonNullable<Document['parameters']> {
  return document.parameters ?? []
}

/** The bodies of a recipe; the protocol may omit the empty list. */
export function bodiesOf(document: Document): NonNullable<Document['bodies']> {
  return document.bodies ?? []
}

/** The feature records of a recipe in timeline order; the protocol may omit the empty list. */
export function featuresOf(document: Document): NonNullable<Document['features']> {
  return document.features ?? []
}

/** The document with the fields of one feature record replaced. */
export function withFeatureChanges(
  document: Document,
  featureId: string,
  changes: Partial<FeatureRecord>,
): Document {
  const features = featuresOf(document).map((feature) =>
    feature.id === featureId ? Object.assign({}, feature, changes) : feature,
  )
  return { ...document, features }
}

/** An empty document, as `new_document` creates it. */
export function emptyDocument(units: Units = 'mm'): Document {
  return {
    format: 'partsmith',
    format_version: 1,
    units,
    parameters: [],
    bodies: [],
    features: [],
  }
}

/** The `.ptsm` text of a recipe as the core writes it (2-space indentation, trailing newline). */
export function documentText(document: Document): string {
  return `${JSON.stringify(document, null, 2)}\n`
}

/** A file as the file service returns it, holding the recipe's text. */
export function documentFile(
  name: string,
  document: Document,
  handle: FileSystemFileHandle | null = null,
): OpenedFile {
  // Copied into this realm's Uint8Array: under jsdom, TextEncoder returns Node's.
  return { name, bytes: Uint8Array.from(new TextEncoder().encode(documentText(document))), handle }
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

function xyPlacement(): Placement {
  return { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
}

function rectangleEntities(left: number, bottom: number, right: number, top: number) {
  return [
    { id: 'e-1', type: 'line', start: [left, bottom], end: [right, bottom] },
    { id: 'e-2', type: 'line', start: [right, bottom], end: [right, top] },
    { id: 'e-3', type: 'line', start: [right, top], end: [left, top] },
    { id: 'e-4', type: 'line', start: [left, top], end: [left, bottom] },
  ] as const
}

const RECTANGLE_TOPOLOGY = [
  { id: 'c-1', type: 'coincident', points: ['e-1.end', 'e-2.start'] },
  { id: 'c-2', type: 'coincident', points: ['e-2.end', 'e-3.start'] },
  { id: 'c-3', type: 'coincident', points: ['e-3.end', 'e-4.start'] },
  { id: 'c-4', type: 'coincident', points: ['e-4.end', 'e-1.start'] },
  { id: 'c-5', type: 'horizontal', line: 'e-1' },
  { id: 'c-6', type: 'horizontal', line: 'e-3' },
  { id: 'c-7', type: 'vertical', line: 'e-2' },
  { id: 'c-8', type: 'vertical', line: 'e-4' },
] as const

/** `f-sk1` of cube.ptsm: a fully constrained 20 by 20 rectangle centered on the origin of XY. */
export const CUBE_SKETCH: FeatureRecord = {
  id: 'f-sk1',
  type: 'sketch',
  plane: { kind: 'origin', plane: 'XY' },
  entities: rectangleEntities(-10, -10, 10, 10),
  constraints: [
    ...RECTANGLE_TOPOLOGY,
    { id: 'c-9', type: 'distance', line: 'e-1', expr: '20 mm' },
    { id: 'c-10', type: 'distance', line: 'e-4', expr: '20 mm' },
    { id: 'c-11', type: 'horizontal_distance', points: ['e-1.start', 'origin'], expr: '10 mm' },
    { id: 'c-12', type: 'vertical_distance', points: ['e-1.start', 'origin'], expr: '10 mm' },
  ],
}

/** `f-ext1` of cube.ptsm: the region of `f-sk1` extruded 20 mm into the new body `b-cube`. */
export const CUBE_EXTRUDE: FeatureRecord = {
  id: 'f-ext1',
  type: 'extrude',
  body: 'b-cube',
  profile: { sketch: 'f-sk1', regions: [CUBE_REGION.reference] },
  extent: { kind: 'distance', expr: '20 mm' },
  op: 'new',
}

/** cube.ptsm: body "Cube", timeline "Sketch 1", "Extrude 1". */
export const CUBE_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 1,
  units: 'mm',
  parameters: [],
  bodies: [{ id: 'b-cube', name: 'Cube' }],
  features: [CUBE_SKETCH, CUBE_EXTRUDE],
}

const VERTICAL_EDGES = [
  { q: 'edge(f-ext1/side-0|f-ext1/side-1)', fp: 'line|n=0,0,1|c=10,-10,10|l=20|adj=2,2' },
  { q: 'edge(f-ext1/side-1|f-ext1/side-2)', fp: 'line|n=0,0,1|c=10,10,10|l=20|adj=2,2' },
  { q: 'edge(f-ext1/side-2|f-ext1/side-3)', fp: 'line|n=0,0,1|c=-10,10,10|l=20|adj=2,2' },
  { q: 'edge(f-ext1/side-0|f-ext1/side-3)', fp: 'line|n=0,0,1|c=-10,-10,10|l=20|adj=2,2' },
] as const

const TOP_FACE_REFERENCE = {
  q: 'f-ext1/end-cap',
  fp: 'plane|n=0,0,1|c=0,0,20|a=400|adj=4,4',
} as const

/** cube-features.ptsm: cube.ptsm plus `f-fil1` (fillet 2 mm on the vertical edges) and `f-sh1` (shell 2 mm without the top face). */
export const CUBE_FEATURES_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  features: [
    CUBE_SKETCH,
    CUBE_EXTRUDE,
    { id: 'f-fil1', type: 'fillet', body: 'b-cube', edges: VERTICAL_EDGES, radius: '2 mm' },
    {
      id: 'f-sh1',
      type: 'shell',
      body: 'b-cube',
      thickness: '2 mm',
      remove_faces: [TOP_FACE_REFERENCE],
    },
  ],
}

/** two-bodies.ptsm: cube.ptsm plus body "Peg" built by `f-cyl1`. */
export const TWO_BODIES_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  bodies: [
    { id: 'b-cube', name: 'Cube' },
    { id: 'b-peg', name: 'Peg' },
  ],
  features: [
    CUBE_SKETCH,
    CUBE_EXTRUDE,
    {
      id: 'f-cyl1',
      type: 'cylinder',
      body: 'b-peg',
      radius: '4 mm',
      height: '20 mm',
      position: ['0', '0', '10 mm'],
      op: 'new',
    },
  ],
}

/** parametric.ptsm: cube.ptsm driven by the parameter `size` = 20 mm. */
export const PARAMETRIC_DOCUMENT: Document = {
  ...CUBE_DOCUMENT,
  parameters: [{ id: 'p-size', name: 'size', expr: '20 mm' }],
  features: [
    {
      ...CUBE_SKETCH,
      constraints: [
        ...RECTANGLE_TOPOLOGY,
        { id: 'c-9', type: 'distance', line: 'e-1', expr: 'size' },
        { id: 'c-10', type: 'distance', line: 'e-4', expr: 'size' },
      ],
    },
    { ...CUBE_EXTRUDE, extent: { kind: 'distance', expr: 'size' } },
  ],
}

/** rect-sketch.ptsm: `f-sk1` on XY, a 40 by 20 rectangle with a corner on the origin; no bodies. */
export const RECT_SKETCH_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 1,
  units: 'mm',
  parameters: [],
  bodies: [],
  features: [
    {
      id: 'f-sk1',
      type: 'sketch',
      plane: { kind: 'origin', plane: 'XY' },
      entities: rectangleEntities(0, 0, 40, 20),
      constraints: [
        ...RECTANGLE_TOPOLOGY,
        { id: 'c-9', type: 'coincident', points: ['e-1.start', 'origin'] },
        { id: 'c-10', type: 'distance', line: 'e-1', expr: '40 mm' },
        { id: 'c-11', type: 'distance', line: 'e-4', expr: '20 mm' },
      ],
    },
  ],
}

function satisfied(id: string): SketchSolution['constraints'][number] {
  return { id, state: 'satisfied' }
}

function solvedLine(id: string, start: Point2, end: Point2): SolvedEntity {
  return { id, type: 'line', start, end, constrained: 'fully_constrained' }
}

function rectangleSolution(
  left: number,
  bottom: number,
  right: number,
  top: number,
  region: RegionInfo,
  constraintIds: readonly string[],
): SketchSolution {
  return {
    feature: 'f-sk1',
    status: 'fully_constrained',
    failed: false,
    dof: 0,
    redundant: [],
    conflicting: [],
    placement: xyPlacement(),
    entities: [
      solvedLine('e-1', [left, bottom], [right, bottom]),
      solvedLine('e-2', [right, bottom], [right, top]),
      solvedLine('e-3', [right, top], [left, top]),
      solvedLine('e-4', [left, top], [left, bottom]),
    ],
    points: [
      { point: 'e-1.start', at: [left, bottom] },
      { point: 'e-1.end', at: [right, bottom] },
      { point: 'e-2.start', at: [right, bottom] },
      { point: 'e-2.end', at: [right, top] },
      { point: 'e-3.start', at: [right, top] },
      { point: 'e-3.end', at: [left, top] },
      { point: 'e-4.start', at: [left, top] },
      { point: 'e-4.end', at: [left, bottom] },
    ],
    constraints: constraintIds.map(satisfied),
    regions: [region],
    profile_findings: [],
  }
}

/** The solution of cube.ptsm's `f-sk1`: fully constrained, one region. */
export const CUBE_SKETCH_SOLUTION: SketchSolution = rectangleSolution(
  -10,
  -10,
  10,
  10,
  CUBE_REGION,
  ['c-1', 'c-2', 'c-3', 'c-4', 'c-5', 'c-6', 'c-7', 'c-8', 'c-9', 'c-10', 'c-11', 'c-12'],
)

/** The solution of rect-sketch.ptsm's `f-sk1`: fully constrained, one region of 40 by 20. */
export const RECT_SKETCH_SOLUTION: SketchSolution = rectangleSolution(0, 0, 40, 20, RECT_REGION, [
  'c-1',
  'c-2',
  'c-3',
  'c-4',
  'c-5',
  'c-6',
  'c-7',
  'c-8',
  'c-9',
  'c-10',
  'c-11',
])

/** The top face of the cube (cube.ptsm), as `pick_reference` describes it. */
export const CUBE_TOP_FACE: ElementDescriptor = {
  name: 'f-ext1/end-cap',
  kind: 'face',
  body: 'b-cube',
  feature: 'f-ext1',
  reference: TOP_FACE_REFERENCE,
  surface: 'plane',
  centroid: [0, 0, 20],
  area: 400,
  normal: [0, 0, 1],
  placement: { origin: [0, 0, 20], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  adjacent: [
    'edge(f-ext1/end-cap|f-ext1/side-0)',
    'edge(f-ext1/end-cap|f-ext1/side-1)',
    'edge(f-ext1/end-cap|f-ext1/side-2)',
    'edge(f-ext1/end-cap|f-ext1/side-3)',
  ],
}

/** The front face of the cube (cube.ptsm), facing -Y. */
export const CUBE_FRONT_FACE: ElementDescriptor = {
  name: 'f-ext1/side-0',
  kind: 'face',
  body: 'b-cube',
  feature: 'f-ext1',
  reference: { q: 'f-ext1/side-0', fp: 'plane|n=0,-1,0|c=0,-10,10|a=400|adj=4,4' },
  surface: 'plane',
  centroid: [0, -10, 10],
  area: 400,
  normal: [0, -1, 0],
  placement: { origin: [0, -10, 10], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  adjacent: [
    'edge(f-ext1/end-cap|f-ext1/side-0)',
    'edge(f-ext1/side-0|f-ext1/side-1)',
    'edge(f-ext1/side-0|f-ext1/side-3)',
    'edge(f-ext1/start-cap|f-ext1/side-0)',
  ],
}

/** The top front edge of the cube (cube.ptsm). */
export const CUBE_TOP_FRONT_EDGE: ElementDescriptor = {
  name: 'edge(f-ext1/end-cap|f-ext1/side-0)',
  kind: 'edge',
  body: 'b-cube',
  feature: 'f-ext1',
  reference: {
    q: 'edge(f-ext1/end-cap|f-ext1/side-0)',
    fp: 'line|n=1,0,0|c=0,-10,20|l=20|adj=2,2',
  },
  curve: 'line',
  centroid: [0, -10, 20],
  length: 20,
  direction: [1, 0, 0],
  adjacent: [
    'f-ext1/end-cap',
    'f-ext1/side-0',
    'vertex(f-ext1/end-cap|f-ext1/side-0|f-ext1/side-1)',
    'vertex(f-ext1/end-cap|f-ext1/side-0|f-ext1/side-3)',
  ],
}
