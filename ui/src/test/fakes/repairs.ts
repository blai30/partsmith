import type { SketchDisplayData } from '../../core/events'
import type {
  Cause,
  Diagnostic,
  DocHandle,
  Document,
  ElementDescriptor,
  ElementKind,
  FeatureStatus,
  FixAction,
  FixRef,
  Reference,
  RegionInfo,
  RepairCandidate,
  SketchSolution,
} from '../../generated/protocol'
import type { ElementRepair, RegionRepair } from '../../repair/repair'
import type { DocumentState } from '../../state/store'
import { CUBE_REGION, CUBE_SKETCH_SOLUTION, type FeatureRecord } from './documents'
import { meshSummary } from './state'

// Repair data for the repair mode, fix and loss tests (ui.md 6.4, U-127 on): corpus R16 after its
// edit, whose fillet lost its edge to a rabbet cut, as `R16-STATUS` states it, and builders of
// element repairs, messages and actions for other cases.

/** The body of the R16 document. */
export const R16_BODY = 'b-1'

/** The model-layer mesh id of `b-1` in the store states the repair tests start from. */
export const R16_MESH_ID = 3

/** The front top edge of 'Box 1' as 'Fillet 1' stores it. */
export const R16_STORED_EDGE: Reference = {
  q: 'edge(f-box1/front|f-box1/top)',
  fp: 'line|n=1,0,0|c=0,-10,20|l=20|adj=2,2',
}

/** C1's reference: the front edge of the cut made by 'Extrude 1'. */
export const R16_FRONT_CUT_EDGE: Reference = {
  q: 'edge(f-box1/front|f-ext2/lateral/e-1)',
  fp: 'line|n=1,0,0|c=0,-10,15|l=20|adj=2,2',
}

/** C2's reference: the top edge of the cut made by 'Extrude 1'. */
export const R16_TOP_CUT_EDGE: Reference = {
  q: 'edge(f-box1/top|f-ext2/lateral/e-2)',
  fp: 'line|n=1,0,0|c=0,-5,20|l=20|adj=2,2',
}

const BOX: FeatureRecord = {
  id: 'f-box1',
  type: 'box',
  body: R16_BODY,
  size: ['20 mm', '20 mm', '20 mm'],
  op: 'new',
}

const RABBET_SKETCH: FeatureRecord = {
  id: 'f-sk2',
  type: 'sketch',
  plane: { kind: 'origin', plane: 'YZ' },
  entities: [
    { id: 'e-1', type: 'line', start: [-11, 15], end: [-5, 15] },
    { id: 'e-2', type: 'line', start: [-5, 15], end: [-5, 21] },
    { id: 'e-3', type: 'line', start: [-5, 21], end: [-11, 21] },
    { id: 'e-4', type: 'line', start: [-11, 21], end: [-11, 15] },
  ],
  constraints: [],
}

const RABBET_CUT: FeatureRecord = {
  id: 'f-ext2',
  type: 'extrude',
  body: R16_BODY,
  profile: {
    sketch: 'f-sk2',
    regions: [{ key: 'e-1,e-2,e-3,e-4', fp: 'c=-8,18|a=36', outline: 'e-1,e-2,e-3,e-4' }],
  },
  extent: { kind: 'symmetric', expr: '30 mm' },
  op: 'cut',
}

/** 'Fillet 1' with the front top edge it lost. */
export const R16_FILLET: FeatureRecord = {
  id: 'f-fil1',
  type: 'fillet',
  body: R16_BODY,
  edges: [R16_STORED_EDGE],
  radius: '2 mm',
}

/** Corpus R16 after its edit: 'Box 1', the rabbet ('Sketch 1', 'Extrude 1') and 'Fillet 1'. */
export const R16_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 2,
  units: 'mm',
  parameters: [],
  bodies: [{ id: R16_BODY, name: 'Body 1' }],
  features: [BOX, RABBET_SKETCH, RABBET_CUT, R16_FILLET],
}

/** The revision `R16-STATUS` was offered at. */
export const R16_REVISION = 7

/** A FixRef of the R16 message, `references_not_found` at `/edges`. */
export function r16Fix(index: number): FixRef {
  return {
    feature: 'f-fil1',
    code: 'references_not_found',
    field: '/edges',
    cause: false,
    index,
    revision: R16_REVISION,
  }
}

/** A0: uses C1. */
export const R16_USE_FRONT: FixAction = {
  label: "Use the front edge of the cut made by 'Extrude 1'",
  run: { kind: 'apply', fix: r16Fix(0) },
}

/** A1: uses C2. */
export const R16_USE_TOP: FixAction = {
  label: "Use the top edge of the cut made by 'Extrude 1'",
  run: { kind: 'apply', fix: r16Fix(1) },
}

/** A2: picks an edge again. */
export const R16_PICK_AGAIN: FixAction = {
  label: 'Pick an edge again',
  run: { kind: 'pick', feature: 'f-fil1', path: '/edges/0' },
}

/** The one message of `R16-STATUS`. */
export const R16_MESSAGE: Diagnostic = {
  severity: 'error',
  code: 'references_not_found',
  message:
    "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.",
  field: '/edges',
  actions: [R16_USE_FRONT, R16_USE_TOP, R16_PICK_AGAIN],
  detail: 'edges[0] q=edge(f-box1/front|f-box1/top)',
}

/** C1 of `R16-STATUS`: edge 3 of `b-1`'s model-layer mesh. */
export const R16_CANDIDATE_FRONT: RepairCandidate = {
  reference: R16_FRONT_CUT_EDGE,
  description: "the front edge of the cut made by 'Extrude 1'",
  relation: "on the front side of 'Box 1', 5 mm from where it was",
  meaning: true,
  distance: 5,
  score: 0.913978,
  element: { body: R16_BODY, mesh_id: R16_MESH_ID, kind: 'edge', index: 3 },
}

/** C2 of `R16-STATUS`: edge 4 of `b-1`'s model-layer mesh. */
export const R16_CANDIDATE_TOP: RepairCandidate = {
  reference: R16_TOP_CUT_EDGE,
  description: "the top edge of the cut made by 'Extrude 1'",
  relation: "on the top of 'Box 1', 5 mm from where it was",
  meaning: true,
  distance: 5,
  score: 0.884184,
  element: { body: R16_BODY, mesh_id: R16_MESH_ID, kind: 'edge', index: 4 },
}

/** The `f32` points of the lost edge's ghost, buffer 0 of the frame: (-10, -10, 20) to (10, -10, 20). */
export const R16_GHOST_POINTS: readonly number[] = [-10, -10, 20, 10, -10, 20]

/** The element repair of `R16-STATUS`, with its ghost in buffer 0. */
export const R16_REPAIR: ElementRepair = {
  kind: 'element',
  path: '/edges/0',
  body: R16_BODY,
  reference: R16_STORED_EDGE,
  resolution: 'missing',
  description: "the front top edge of 'Box 1'",
  centroid: [0, -10, 20],
  ghost: { positions: 0, kind: 'lines', frame: 'world' },
  candidates: [R16_CANDIDATE_FRONT, R16_CANDIDATE_TOP],
}

/** `R16-STATUS` (ui.md 6.4): 'Fillet 1' in error with one message and one element repair. */
export const R16_STATUS: FeatureStatus = {
  id: 'f-fil1',
  status: 'error',
  messages: [R16_MESSAGE],
  repairs: [R16_REPAIR],
}

/** An edge descriptor of `b-1` with its name, reference and centroid. */
function edgeDescriptor(
  reference: Reference,
  centroid: [number, number, number],
): ElementDescriptor {
  return {
    name: reference.q,
    kind: 'edge',
    body: R16_BODY,
    feature: 'f-box1',
    reference,
    curve: 'line',
    centroid,
    length: 20,
    direction: [1, 0, 0],
    adjacent: [],
  }
}

/** The edges of `b-1` after the cut in `list_elements` order: display ids 3 and 4 are C1 and C2. */
export const R16_EDGES: readonly ElementDescriptor[] = [
  edgeDescriptor(
    { q: 'edge(f-box1/back|f-box1/top)', fp: 'line|n=1,0,0|c=0,10,20|l=20|adj=2,2' },
    [0, 10, 20],
  ),
  edgeDescriptor(
    { q: 'edge(f-box1/left|f-box1/top)', fp: 'line|n=0,1,0|c=-10,2.5,20|l=15|adj=2,2' },
    [-10, 2.5, 20],
  ),
  edgeDescriptor(
    { q: 'edge(f-box1/right|f-box1/top)', fp: 'line|n=0,1,0|c=10,2.5,20|l=15|adj=2,2' },
    [10, 2.5, 20],
  ),
  edgeDescriptor(R16_FRONT_CUT_EDGE, [0, -10, 15]),
  edgeDescriptor(R16_TOP_CUT_EDGE, [0, -5, 20]),
]

/** What the store holds besides the statuses in the R16 states the repair tests start from: `b-1`'s model mesh and the ghost buffer. */
export const R16_STORE_CHANGES: Partial<DocumentState> = {
  meshes: { 'b-1': meshSummary(R16_MESH_ID, { min: [-10, -10, 0], max: [10, 10, 20] }) },
  ghosts: { 0: Float32Array.from(R16_GHOST_POINTS) },
}

/** A `listElements` that answers with R16's edges, and with no faces or vertices. */
export function r16ListElements(
  _documentHandle: DocHandle,
  _body: string,
  kind?: ElementKind,
): Promise<readonly ElementDescriptor[]> {
  return Promise.resolve(kind === 'edge' ? R16_EDGES : [])
}

// A lost edge of cube-features.ptsm after a line of 'Sketch 1' was deleted (U-132).

/** The lost edge of 'Fillet 1' in cube-features.ptsm. */
export const BEVEL_EDGE = "the top edge of 'Extrude 1' along a line of 'Sketch 1'"

/** The cause line of a loss after the line from (80, 40) to (70, 50) of 'Sketch 1' was deleted. */
export const LINE_DELETED: Cause = {
  text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.",
  actions: [
    { label: 'Undo', run: { kind: 'undo' } },
    { label: 'Show the line', run: { kind: 'show', feature: 'f-sk1', entity: 'e-5' } },
  ],
}

/** An element repair of `f-fil1` at `/edges/0` lost in `b-cube`, with no ghost and no candidates unless given. */
export function lostEdgeRepair(overrides: Partial<ElementRepair> = {}): ElementRepair {
  return {
    kind: 'element',
    path: '/edges/0',
    body: 'b-cube',
    reference: { q: 'edge(top,front)', fp: 'fp-edge' },
    resolution: 'missing',
    description: "the front top edge of 'Extrude 1'",
    centroid: [0, 0, 20],
    candidates: [],
    ...overrides,
  }
}

/** A lost region of 'Sketch 1' around (40, 25) at `path`, with no candidates. */
export function regionRepair(path: string, description: string): RegionRepair {
  return {
    kind: 'region',
    path,
    sketch: 'f-sk1',
    region: { key: 'e-1,e-2,e-3,e-4', fp: 'c=40,25|a=4000' },
    resolution: 'missing',
    description,
    candidates: [],
  }
}

// A lost profile region of cube.ptsm: 'Sketch 1' split by a line `e-5` at x = 0 into a left and a
// right region, so 'Extrude 1' lost its region and is offered the right one, then both.

/** The left half of the split cube sketch, around (-5, 0). */
export const SPLIT_LEFT: RegionInfo = {
  reference: { key: 'e-1,e-5,e-3,e-4', fp: 'c=-5,0|a=200' },
  area: 200,
  centroid: [-5, 0],
}

/** The right half of the split cube sketch, around (5, 0). */
export const SPLIT_RIGHT: RegionInfo = {
  reference: { key: 'e-1,e-2,e-3,e-5', fp: 'c=5,0|a=200' },
  area: 200,
  centroid: [5, 0],
}

/** The cube's sketch with its two halves as regions. */
export const SPLIT_SOLUTION: SketchSolution = {
  ...CUBE_SKETCH_SOLUTION,
  regions: [SPLIT_LEFT, SPLIT_RIGHT],
}

/** One triangle per region of `SPLIT-SOLUTION`. */
export const SPLIT_DISPLAY: SketchDisplayData = {
  regionPositions: new Float32Array([-10, -10, 0, -10, 0, 10, 0, -10, 10, -10, 10, 10]),
  regionIndices: new Uint32Array([0, 1, 2, 3, 4, 5]),
  regionIds: new Uint32Array([0, 0, 0, 1, 1, 1]),
}

/** The region repair of 'Extrude 1': candidate 1 is the right half, candidate 2 both halves. */
export const SPLIT_REPAIR: RegionRepair = {
  kind: 'region',
  path: '/profile/regions/0',
  sketch: 'f-sk1',
  region: CUBE_REGION.reference,
  resolution: 'missing',
  description: "the region of 'Sketch 1' around (0, 0)",
  candidates: [
    { regions: [SPLIT_RIGHT.reference], description: "the region of 'Sketch 1' around (5, 0)" },
    {
      regions: [SPLIT_LEFT.reference, SPLIT_RIGHT.reference],
      description: "the regions of 'Sketch 1' around (-5, 0) and (5, 0)",
    },
  ],
}

/** An `apply` action of the profile message that stores `description`, at `index`. */
function useRegions(description: string, index: number): FixAction {
  return {
    label: `Use ${description}`,
    run: {
      kind: 'apply',
      fix: {
        feature: 'f-ext1',
        code: 'profile_not_found',
        field: '/profile',
        cause: false,
        index,
        revision: 4,
      },
    },
  }
}

/** 'Extrude 1' in error with its lost region and the two candidates' actions. */
export const SPLIT_STATUS: FeatureStatus = {
  id: 'f-ext1',
  status: 'error',
  messages: [
    {
      severity: 'error',
      code: 'profile_not_found',
      message:
        "The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again.",
      field: '/profile',
      actions: SPLIT_REPAIR.candidates.map((candidate, index) =>
        useRegions(candidate.description, index),
      ),
    },
  ],
  repairs: [SPLIT_REPAIR],
}
