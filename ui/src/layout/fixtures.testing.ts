// Test support for the ui-model slice, on top of the shared fakes in src/test/fakes: store states
// of the e2e fixture documents, the cube's element descriptors in list_elements order, mesh buffers
// and body_mesh events for the scene, and selection items with the keys of ui.md 3.6. Nothing here
// is part of the app.

import type { BodyMeshData, CoreEvent } from '../core/events'
import type {
  BoundingBox,
  Document,
  ElementDescriptor,
  FeatureStatus,
  RegionInfo,
} from '../generated/protocol'
import { DEFAULT_SETTINGS } from '../settings/settings'
import type {
  AppState,
  CatalogFeature,
  DocumentState,
  MeshSummary,
  OriginName,
  SelectionItem,
} from '../state/store'
import { TEST_CATALOG, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import {
  CUBE_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
  featuresOf,
  RECT_SKETCH_DOCUMENT,
  RECT_SKETCH_SOLUTION,
  type FeatureRecord,
} from '../test/fakes/documents'
import { TEST_PRINTERS } from '../test/fakes/printers'

/** Every type of the test catalog, sorted. */
export const CATALOG_TYPES: readonly string[] = TEST_CATALOG_ENTRIES.map(
  (entry) => entry.type,
).toSorted()

/** The catalog feature of one type of the test catalog. */
export function testFeature(type: string): CatalogFeature {
  const feature = TEST_CATALOG.byType[type]
  if (feature === undefined) {
    throw new Error(`The test catalog has no type ${type}.`)
  }
  return feature
}

/** A document whose only feature is an import (U-89). */
export const IMPORT_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 1,
  units: 'mm',
  parameters: [],
  bodies: [{ id: 'b-imp', name: 'Bracket' }],
  features: [
    {
      id: 'f-imp1',
      type: 'import',
      body: 'b-imp',
      asset: '7c3e9a1f0b2d4c6e8a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f.step',
      op: 'new',
    },
  ],
}

function boxRecord(index: number): FeatureRecord {
  return {
    id: `f-box${String(index + 1)}`,
    type: 'box',
    body: `b-${String(index + 1)}`,
    size: ['10', '10', '10'],
    position: [String(20 * index), '0', '0'],
    op: 'new',
  }
}

/** Three boxes on three bodies ("Box 1" to "Box 3"), for timeline, rollback and reorder tests. */
export const THREE_BOXES_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 1,
  units: 'mm',
  parameters: [],
  bodies: [
    { id: 'b-1', name: 'Body 1' },
    { id: 'b-2', name: 'Body 2' },
    { id: 'b-3', name: 'Body 3' },
  ],
  features: [boxRecord(0), boxRecord(1), boxRecord(2)],
}

/** A document with four boxes, for reordering by gap. */
export const FOUR_BOXES_DOCUMENT: Document = {
  ...THREE_BOXES_DOCUMENT,
  bodies: [
    { id: 'b-1', name: 'Body 1' },
    { id: 'b-2', name: 'Body 2' },
    { id: 'b-3', name: 'Body 3' },
    { id: 'b-4', name: 'Body 4' },
  ],
  features: [boxRecord(0), boxRecord(1), boxRecord(2), boxRecord(3)],
}

// Element descriptors of the cube, in list_elements order (faces sorted by name): display face id i
// is position i (protocol.md 3.4).

type FaceSpec = {
  readonly name: string
  readonly centroid: readonly [number, number, number]
  readonly normal: readonly [number, number, number]
  readonly xAxis: readonly [number, number, number]
}

function planarFace(spec: FaceSpec, feature = 'f-ext1'): ElementDescriptor {
  return {
    name: spec.name,
    kind: 'face',
    body: 'b-cube',
    feature,
    reference: {
      q: spec.name,
      fp: `plane|n=${spec.normal.join(',')}|c=${spec.centroid.join(',')}|a=400|adj=4,4`,
    },
    surface: 'plane',
    centroid: [...spec.centroid],
    area: 400,
    normal: [...spec.normal],
    placement: { origin: [...spec.centroid], x_axis: [...spec.xAxis], normal: [...spec.normal] },
    adjacent: [],
  }
}

const CUBE_RIGHT_FACE = planarFace({
  name: 'f-ext1/side-1',
  centroid: [10, 0, 10],
  normal: [1, 0, 0],
  xAxis: [0, 1, 0],
})
const CUBE_BACK_FACE = planarFace({
  name: 'f-ext1/side-2',
  centroid: [0, 10, 10],
  normal: [0, 1, 0],
  xAxis: [1, 0, 0],
})
const CUBE_LEFT_FACE = planarFace({
  name: 'f-ext1/side-3',
  centroid: [-10, 0, 10],
  normal: [-1, 0, 0],
  xAxis: [0, 1, 0],
})
const CUBE_BOTTOM_FACE = planarFace({
  name: 'f-ext1/start-cap',
  centroid: [0, 0, 0],
  normal: [0, 0, -1],
  xAxis: [1, 0, 0],
})

/** The cube's six faces: top, front (-Y), right (+X), back (+Y), left (-X), bottom. */
export const CUBE_FACES: readonly ElementDescriptor[] = [
  CUBE_TOP_FACE,
  CUBE_FRONT_FACE,
  CUBE_RIGHT_FACE,
  CUBE_BACK_FACE,
  CUBE_LEFT_FACE,
  CUBE_BOTTOM_FACE,
]

/** Cube faces where two (positions 1 and 4) come from `f-fil1`, so feature highlighting has faces to leave out. */
export const FILLETED_CUBE_FACES: readonly ElementDescriptor[] = [
  CUBE_TOP_FACE,
  { ...CUBE_FRONT_FACE, feature: 'f-fil1' },
  CUBE_RIGHT_FACE,
  CUBE_BACK_FACE,
  { ...CUBE_LEFT_FACE, feature: 'f-fil1' },
  CUBE_BOTTOM_FACE,
]

/** A circular edge of the peg (two-bodies.ptsm). */
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
}

/** The peg's cylindrical side face (two-bodies.ptsm). */
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
}

/** A corner of the cube. */
export const CORNER_VERTEX: ElementDescriptor = {
  name: 'vertex(f-ext1/end-cap|f-ext1/side-0|f-ext1/side-1)',
  kind: 'vertex',
  body: 'b-cube',
  feature: 'f-ext1',
  reference: {
    q: 'vertex(f-ext1/end-cap|f-ext1/side-0|f-ext1/side-1)',
    fp: 'vertex|c=10,-10,20|adj=3',
  },
  centroid: [10, -10, 20],
  adjacent: [],
}

// Meshes.

type Point = readonly [number, number, number]

/** Mesh buffers of an axis-aligned box; display face ids follow CUBE_FACES (top, front, right, back, left, bottom). */
export function boxMesh(min: Point, max: Point): BodyMeshData {
  const [x0, y0, z0] = min
  const [x1, y1, z1] = max
  // Corners of each face, counterclockwise seen from outside.
  const faces: readonly { readonly normal: Point; readonly corners: readonly Point[] }[] = [
    {
      normal: [0, 0, 1],
      corners: [
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
      ],
    },
    {
      normal: [0, -1, 0],
      corners: [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y0, z1],
        [x0, y0, z1],
      ],
    },
    {
      normal: [1, 0, 0],
      corners: [
        [x1, y0, z0],
        [x1, y1, z0],
        [x1, y1, z1],
        [x1, y0, z1],
      ],
    },
    {
      normal: [0, 1, 0],
      corners: [
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y1, z1],
        [x1, y1, z1],
      ],
    },
    {
      normal: [-1, 0, 0],
      corners: [
        [x0, y1, z0],
        [x0, y0, z0],
        [x0, y0, z1],
        [x0, y1, z1],
      ],
    },
    {
      normal: [0, 0, -1],
      corners: [
        [x0, y1, z0],
        [x1, y1, z0],
        [x1, y0, z0],
        [x0, y0, z0],
      ],
    },
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
  const corners: readonly Point[] = [
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
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
  ]
  return {
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    indices: new Uint32Array(indices),
    faceIds: new Uint32Array(faceIds),
    edgePositions: new Float32Array(
      edgePairs.flatMap(([start, end]) => [...(corners[start] ?? []), ...(corners[end] ?? [])]),
    ),
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

/** The store's document state for a recipe (document handle 1): every feature ok, no meshes unless given. */
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
    featureStatus: Object.fromEntries(
      featuresOf(recipe).map((feature) => [feature.id, okStatus(feature.id)]),
    ),
    parameterValues: {},
    diagnostics: [],
    sketches: {},
    datums: {},
    meshes: {},
    lastRebuildMilliseconds: null,
    ...overrides,
  }
}

/** cube.ptsm as the store holds it after opening: the cube's model-layer mesh and the sketch solution. */
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

/** A ready app state around a document: the test catalog and printers, the light theme, Fusion navigation. */
export function readyState(
  document: DocumentState | null,
  overrides: Partial<AppState> = {},
): Partial<AppState> {
  return {
    core: { status: 'ready', engineVersion: '0.1.0', fatal: null, pendingMutations: 0 },
    catalog: TEST_CATALOG,
    printers: TEST_PRINTERS,
    document,
    settings: { ...DEFAULT_SETTINGS, theme: 'light' },
    ...overrides,
  }
}

// Selection items with the keys of ui.md 3.6.

const NO_ITEM_DATA = {
  body: null,
  feature: null,
  element: null,
  sketchItem: null,
  origin: null,
} as const

export function bodyItem(id: string, name: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `body:${id}`, kind: 'body', label: name, body: id }
}

export function featureItem(id: string, label: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `feature:${id}`, kind: 'feature', label, feature: id }
}

export function datumItem(id: string, label: string): SelectionItem {
  return { ...featureItem(id, label), kind: 'datum_plane' }
}

export function parameterItem(id: string, name: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `parameter:${id}`, kind: 'parameter', label: name }
}

/** A face, edge or vertex item from its descriptor. */
export function elementItem(element: ElementDescriptor, bodyName = 'Cube'): SelectionItem {
  const noun = element.kind === 'face' ? 'Face' : element.kind === 'edge' ? 'Edge' : 'Vertex'
  return {
    ...NO_ITEM_DATA,
    key: `element:${element.body}:${element.name}`,
    kind: element.kind,
    label: `${noun} of ${bodyName}`,
    body: element.body,
    feature: element.feature,
    element,
  }
}

const ORIGIN_LABELS: Readonly<Record<OriginName, string>> = {
  XY: 'Top plane (XY)',
  XZ: 'Front plane (XZ)',
  YZ: 'Right plane (YZ)',
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
  return { ...NO_ITEM_DATA, key: `origin:${origin}`, kind, label: ORIGIN_LABELS[origin], origin }
}

export function regionItem(sketch: string, region: RegionInfo, label = 'Region 1'): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `region:${sketch}:${region.reference.key}`,
    kind: 'sketch_region',
    label,
    feature: sketch,
    sketchItem: { kind: 'region', sketch, region },
  }
}

export function sketchLineItem(sketch: string, line: string): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `sketch:${sketch}:${line}`,
    kind: 'sketch_curve',
    label: `Line ${line}`,
    feature: sketch,
    sketchItem: { kind: 'entity', sketch, entity: line, entityType: 'line' },
  }
}

export function sketchPointItem(sketch: string, point: string): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `point:${sketch}:${point}`,
    kind: 'sketch_point',
    label: `Point ${point}`,
    feature: sketch,
    sketchItem: { kind: 'point', sketch, point },
  }
}
