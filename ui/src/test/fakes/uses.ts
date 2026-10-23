import type { Document, FeatureRecord, Reference, Use } from '../../generated/protocol'
import type { AppState } from '../../state/store'
import { stateWithDocument } from './state'

// What features use, for the drop warning, "Used by" and the used marks of sketch entities (ui.md
// 3.9, 3.16.10, U-135 to U-137, U-148): small timelines of one body `b-1` and the `list_uses`
// answers the engine gives for them (engine.md 3.24).

/** An edge reference of `b-1` by its name. */
export function edgeReference(name: string): Reference {
  return { q: name, fp: 'line|n=1,0,0|c=0,0,10|l=20|adj=2,2' }
}

/** A face reference of `b-1` by its name. */
export function faceReference(name: string): Reference {
  return { q: name, fp: 'plane|n=0,0,1|c=0,0,10|a=400|adj=4,4' }
}

/** A sketch on the XY plane with these entities (lines from `lines`, then points from `points`). */
export function xySketchRecord(
  id: string,
  lines: readonly {
    readonly id: string
    readonly from: [number, number]
    readonly to: [number, number]
  }[],
  points: readonly { readonly id: string; readonly at: [number, number] }[] = [],
): FeatureRecord {
  return {
    id,
    type: 'sketch',
    plane: { kind: 'origin', plane: 'XY' },
    entities: [
      ...lines.map((line) => ({ id: line.id, type: 'line', start: line.from, end: line.to })),
      ...points.map((point) => ({ id: point.id, type: 'point', position: point.at })),
    ],
    constraints: [],
  }
}

/** An extrude of the regions of `sketch` into `b-1`, 10 mm, joined unless `op` says otherwise. */
export function extrudeRecord(
  id: string,
  sketch: string,
  op: 'new' | 'join' = 'join',
): FeatureRecord {
  return {
    id,
    type: 'extrude',
    body: 'b-1',
    profile: { sketch, regions: [] },
    extent: { kind: 'distance', expr: '10 mm' },
    op,
  }
}

/** A 1 mm fillet of these edges of `b-1`, named `name` when given. */
export function filletRecord(
  id: string,
  edges: readonly Reference[],
  name: string | null = null,
): FeatureRecord {
  return {
    id,
    type: 'fillet',
    body: 'b-1',
    edges,
    radius: '1 mm',
    ...(name === null ? {} : { name }),
  }
}

/** A 1 mm chamfer of these edges of `b-1`. */
export function chamferRecord(id: string, edges: readonly Reference[]): FeatureRecord {
  return {
    id,
    type: 'chamfer',
    body: 'b-1',
    edges,
    distance: { kind: 'equal', expr: '1 mm' },
  }
}

/** A datum plane on a face of `b-1`, at no offset. */
export function datumOnFace(id: string, face: Reference): FeatureRecord {
  return { id, type: 'datum_plane', base: { kind: 'face', body: 'b-1', face }, offset: '0 mm' }
}

/** A document of body `b-1` "Body 1" with these features, at format version 2. */
export function usesDocument(features: readonly FeatureRecord[]): Document {
  return {
    format: 'partsmith',
    format_version: 2,
    units: 'mm',
    parameters: [],
    bodies: [{ id: 'b-1', name: 'Body 1' }],
    features: [...features],
  }
}

/** The store state of `document` at revision 5, with `uses` known at `usesRevision`. */
export function stateWithUses(
  document: Document,
  uses: readonly Use[],
  usesRevision = 5,
): AppState {
  return stateWithDocument(document, {}, { revision: 5, uses: { revision: usesRevision, uses } })
}

// U-135: 'Fillet 1' rounds an edge that 'Extrude 2' made.

/** The top edge of 'Extrude 2' that 'Fillet 1' rounds. */
const EXTRUDE_2_EDGE = edgeReference('edge(f-ext2/end-cap|f-ext2/lateral/e-1)')

/** 'Extrude 1', 'Extrude 2' and 'Fillet 1' (U-135). */
export const DROP_DOCUMENT = usesDocument([
  extrudeRecord('f-ext1', 'f-sk1', 'new'),
  extrudeRecord('f-ext2', 'f-sk2'),
  filletRecord('f-fil1', [EXTRUDE_2_EDGE]),
])

/** `list_uses` of `DROP_DOCUMENT`, as U-135 states it. */
export const DROP_USES: readonly Use[] = [{ user: 'f-fil1', field: '/edges/0', feature: 'f-ext2' }]

// U-136: 'Extrude 1' is used by 'Fillet 1' and 'Datum plane 1'.

/** The top of 'Extrude 1'. */
const EXTRUDE_1_TOP = faceReference('f-ext1/end-cap')

/** 'Sketch 1', 'Extrude 1', 'Fillet 1' on two of its edges and 'Datum plane 1' on its top (U-136). */
export const USED_BY_DOCUMENT = usesDocument([
  xySketchRecord('f-sk1', [
    { id: 'e-1', from: [0, 0], to: [80, 0] },
    { id: 'e-2', from: [80, 0], to: [80, 50] },
    { id: 'e-3', from: [80, 50], to: [0, 50] },
    { id: 'e-4', from: [0, 50], to: [0, 0] },
  ]),
  extrudeRecord('f-ext1', 'f-sk1', 'new'),
  filletRecord('f-fil1', [
    edgeReference('edge(f-ext1/end-cap|f-ext1/lateral/e-1)'),
    edgeReference('edge(f-ext1/end-cap|f-ext1/lateral/e-3)'),
  ]),
  datumOnFace('f-dp1', EXTRUDE_1_TOP),
])

/** `list_uses` of `USED_BY_DOCUMENT`, sorted as the engine sorts it (engine.md 3.24). */
export const USED_BY_USES: readonly Use[] = [
  { user: 'f-ext1', field: '/profile/sketch', feature: 'f-sk1' },
  { user: 'f-fil1', field: '/edges/0', feature: 'f-ext1', entity: 'e-1' },
  { user: 'f-fil1', field: '/edges/1', feature: 'f-ext1', entity: 'e-3' },
  { user: 'f-dp1', field: '/base/face', feature: 'f-ext1' },
]

// U-137: in 'Sketch 1', the back line e-3 made the edge 'Fillet 1' rounds, and the point e-p1 the
// edge 'Chamfer 1' bevels.

/** 'Sketch 1' (a rectangle e-1 to e-4 and a point e-p1), 'Extrude 1', 'Fillet 1' and 'Chamfer 1' (U-137). */
export const USED_SKETCH_DOCUMENT = usesDocument([
  xySketchRecord(
    'f-sk1',
    [
      { id: 'e-1', from: [0, 0], to: [80, 0] },
      { id: 'e-2', from: [80, 0], to: [80, 50] },
      { id: 'e-3', from: [80, 50], to: [0, 50] },
      { id: 'e-4', from: [0, 50], to: [0, 0] },
    ],
    [{ id: 'e-p1', at: [40, 25] }],
  ),
  extrudeRecord('f-ext1', 'f-sk1', 'new'),
  filletRecord('f-fil1', [edgeReference('edge(f-ext1/end-cap|f-ext1/lateral/e-3)')]),
  chamferRecord('f-cha1', [edgeReference('edge(f-ext1/lateral/e-p1)')]),
])

/** The two uses U-137 states: 'Extrude 1' links 'Sketch 1', and 'Fillet 1' rounds the edge of e-3. */
export const USED_SKETCH_USES: readonly Use[] = [
  { user: 'f-ext1', field: '/profile/sketch', feature: 'f-sk1' },
  { user: 'f-fil1', field: '/edges/0', feature: 'f-ext1', entity: 'e-3' },
]

/** `USED_SKETCH_USES` and 'Chamfer 1', which bevels an edge made from the point e-p1. */
export const USED_POINT_USES: readonly Use[] = [
  ...USED_SKETCH_USES,
  { user: 'f-cha1', field: '/edges/0', feature: 'f-ext1', entity: 'e-p1' },
]
