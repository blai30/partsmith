// Test support for the sketch module: sketch records, solutions and selection items built from
// sketch.md's JSON, and a fake core, so tests read like the spec's scenarios.

import type { CoreApi, SolvedSketch } from '../core/api'
import type {
  Document,
  SketchConstraint,
  SketchEntity,
  SketchParams,
  SketchSolution,
} from '../generated/protocol'
import type { DocumentState, SelectionItem, SketchSession } from '../state/store'
import { pointOnCircle, type Vector2 } from './geometry'
import type { SolvedEntity } from './operands'
import type { IdSource } from './recipes'

export type FeatureRecord = NonNullable<Document['features']>[number]
export type EntityState = SolvedEntity['constrained']
export type EntityType = 'point' | 'line' | 'circle' | 'arc'

/** The sketch every fixture edits. */
export const SKETCH_ID = 'f-sk1'
/** The document handle of every fixture document. */
export const DOCUMENT_HANDLE = 1

export const XY_PLANE: SketchParams['plane'] = { kind: 'origin', plane: 'XY' }
export const XY_PLACEMENT: SketchSolution['placement'] = {
  origin: [0, 0, 0],
  x_axis: [1, 0, 0],
  normal: [0, 0, 1],
}

/** An `IdSource` that numbers ids per prefix: e-1, e-2, ... and c-1, c-2, ... */
export function sequentialIds(): IdSource {
  const counts = { e: 0, c: 0 }
  return {
    next(prefix) {
      counts[prefix] += 1
      return `${prefix}-${String(counts[prefix])}`
    },
  }
}

export function pointEntity(id: string, position: Vector2, construction = false): SketchEntity {
  const entity: SketchEntity = { id, type: 'point', position: [position[0], position[1]] }
  return construction ? { ...entity, construction: true } : entity
}

export function lineEntity(
  id: string,
  start: Vector2,
  end: Vector2,
  construction = false,
): SketchEntity {
  const entity: SketchEntity = {
    id,
    type: 'line',
    start: [start[0], start[1]],
    end: [end[0], end[1]],
  }
  return construction ? { ...entity, construction: true } : entity
}

export function circleEntity(id: string, center: Vector2, radius: number): SketchEntity {
  return { id, type: 'circle', center: [center[0], center[1]], radius }
}

/** An arc running counterclockwise from `startAngle` to `endAngle` (radians from +u). */
export function arcEntity(
  id: string,
  center: Vector2,
  radius: number,
  startAngle: number,
  endAngle: number,
): SketchEntity {
  return {
    id,
    type: 'arc',
    center: [center[0], center[1]],
    radius,
    start_angle: startAngle,
    end_angle: endAngle,
  }
}

/** The addressable points of an entity in role order (protocol.md `SketchSolution.points`). */
export function solvedPoints(entity: SketchEntity): SketchSolution['points'] {
  switch (entity.type) {
    case 'point':
      return [{ point: entity.id, at: entity.position }]
    case 'line':
      return [
        { point: `${entity.id}.start`, at: entity.start },
        { point: `${entity.id}.end`, at: entity.end },
      ]
    case 'circle':
      return [{ point: `${entity.id}.center`, at: entity.center }]
    case 'arc': {
      const at = (angle: number): [number, number] => {
        const [x, y] = pointOnCircle(entity.center, entity.radius, angle)
        return [x, y]
      }
      return [
        { point: `${entity.id}.start`, at: at(entity.start_angle) },
        { point: `${entity.id}.end`, at: at(entity.end_angle) },
        { point: `${entity.id}.center`, at: entity.center },
      ]
    }
  }
}

/** An entity as the solver reports it, with its constraint state. */
export function solvedAs(
  entity: SketchEntity,
  constrained: EntityState = 'under_constrained',
): SolvedEntity {
  return { ...entity, constrained }
}

/** A solution of `entities` as solved, every entity under-constrained unless `fields` says otherwise. */
export function solutionFor(
  entities: readonly SketchEntity[],
  fields: Partial<SketchSolution> = {},
): SketchSolution {
  return {
    feature: SKETCH_ID,
    status: 'under_constrained',
    failed: false,
    dof: 0,
    redundant: [],
    conflicting: [],
    placement: XY_PLACEMENT,
    entities: entities.map((entity) => solvedAs(entity)),
    points: entities.flatMap(solvedPoints),
    constraints: [],
    regions: [],
    profile_findings: [],
    messages: [],
    display: null,
    ...fields,
  }
}

export function solvedSketch(solution: SketchSolution): SolvedSketch {
  return { solution, display: null }
}

/** The `sketch` feature record `f-sk1`, on the XY plane unless `plane` says otherwise. */
export function sketchRecord(
  options: {
    readonly entities?: SketchEntity[]
    readonly constraints?: SketchConstraint[]
    readonly plane?: SketchParams['plane']
  } = {},
): FeatureRecord {
  return {
    id: SKETCH_ID,
    type: 'sketch',
    plane: options.plane ?? XY_PLANE,
    entities: options.entities ?? [],
    constraints: options.constraints ?? [],
  }
}

export function recipeWith(features: FeatureRecord[]): Document {
  return {
    format: 'partsmith',
    format_version: 1,
    units: 'mm',
    parameters: [],
    bodies: [],
    features,
  }
}

export function documentState(options: {
  readonly features: FeatureRecord[]
  readonly sketches?: Readonly<Record<string, SolvedSketch>>
  readonly revision?: number
  readonly undoLabel?: string | null
}): DocumentState {
  return {
    documentHandle: DOCUMENT_HANDLE,
    name: 'Sketch test',
    file: null,
    revision: options.revision ?? 1,
    recipe: recipeWith(options.features),
    dirty: false,
    localUnsaved: false,
    recoveryId: 'session-1',
    undoLabel: options.undoLabel ?? null,
    redoLabel: null,
    transactionOpen: false,
    focus: null,
    featureStatus: {},
    parameterValues: {},
    diagnostics: [],
    sketches: options.sketches ?? {},
    datums: {},
    meshes: {},
    lastRebuildMilliseconds: null,
    ghosts: {},
  }
}

/** A sketch session editing `f-sk1` with no tool active. */
export function sketchSession(fields: Partial<SketchSession> = {}): SketchSession {
  return {
    sketchId: SKETCH_ID,
    reattaching: null,
    tool: null,
    construction: false,
    showConstraints: true,
    selection: [],
    dragging: false,
    cameraBefore: null,
    ...fields,
  }
}

/** Constraints without their ids, for comparing constraints whose ids are generated. */
export function withoutIds(constraints: readonly SketchConstraint[] | undefined): unknown[] {
  return (constraints ?? []).map(({ id: _id, ...rest }) => rest)
}

const ENTITY_LABELS: Readonly<Record<EntityType, string>> = {
  point: 'Point',
  line: 'Line',
  circle: 'Circle',
  arc: 'Arc',
}

/** A selected sketch curve (or point entity picked as an entity) of `f-sk1` (ui.md 3.6 keys). */
export function curveItem(entity: string, entityType: EntityType): SelectionItem {
  return {
    key: `sketch:${SKETCH_ID}:${entity}`,
    kind: 'sketch_curve',
    label: `${ENTITY_LABELS[entityType]} ${entity}`,
    body: null,
    feature: SKETCH_ID,
    element: null,
    sketchItem: { kind: 'entity', sketch: SKETCH_ID, entity, entityType },
    origin: null,
  }
}

/** A selected sketch point of `f-sk1`, addressed by its `PointRef`. */
export function pointItem(point: string): SelectionItem {
  return {
    key: `point:${SKETCH_ID}:${point}`,
    kind: 'sketch_point',
    label: `Point ${point}`,
    body: null,
    feature: SKETCH_ID,
    element: null,
    sketchItem: { kind: 'point', sketch: SKETCH_ID, point },
    origin: null,
  }
}

function unexpected(name: string): () => Promise<never> {
  return () => Promise.reject(new Error(`The sketch tests do not expect the core call ${name}.`))
}

/** A `CoreApi` that answers only `methods`; every other call rejects. */
export function createFakeCore(methods: Partial<CoreApi>): CoreApi {
  return {
    engineInfo: unexpected('engineInfo'),
    featureCatalog: unexpected('featureCatalog'),
    listPrinterProfiles: unexpected('listPrinterProfiles'),
    newDocument: unexpected('newDocument'),
    openDocument: unexpected('openDocument'),
    saveDocument: unexpected('saveDocument'),
    markSaved: unexpected('markSaved'),
    closeDocument: unexpected('closeDocument'),
    applyEdits: unexpected('applyEdits'),
    applyFix: unexpected('applyFix'),
    renameParameter: unexpected('renameParameter'),
    beginTransaction: unexpected('beginTransaction'),
    setFocus: unexpected('setFocus'),
    commitTransaction: unexpected('commitTransaction'),
    cancelTransaction: unexpected('cancelTransaction'),
    undo: unexpected('undo'),
    redo: unexpected('redo'),
    solveSketch: unexpected('solveSketch'),
    dragSketch: unexpected('dragSketch'),
    endSketchDrag: unexpected('endSketchDrag'),
    evaluate: unexpected('evaluate'),
    listElements: unexpected('listElements'),
    pickReference: unexpected('pickReference'),
    measure: unexpected('measure'),
    resolveReferences: unexpected('resolveReferences'),
    analyzePrint: unexpected('analyzePrint'),
    exportBodies: unexpected('exportBodies'),
    ...methods,
  }
}
