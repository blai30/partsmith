import { CoreError } from '../../core/client'
import { generateId, usedIds } from '../../core/ids'
import { createLatestOnly } from '../../core/queue'
import { featureLabel } from '../../core/labels'
import { CoreFatalError } from '../../core/transport'
import type {
  DocHandle,
  Edit,
  FeatureRecord,
  FeatureStatus,
  ProfileFinding,
  SketchConstraint,
  SketchEntity,
  SketchParams,
  SketchPlane,
  SketchSolution,
} from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { measureTiming, TIMING_NAMES } from '../../performance/marks'
import type { SolvedSketch } from '../../core/api'
import {
  CONSTRAINT_TITLES,
  constraintTitle,
  constraintsFor,
  joinTitles,
  pickVerdict,
  referencedEntities,
  type ConstraintType,
} from '../../sketch/constraints'
import { pointSelectionItem } from '../../sketch/hit'
import { failedSolveMessage, findingPoints, outlineProblems } from '../../sketch/issues'
import { worldBounds } from '../../sketch/placement'
import type { Vector2 } from '../../sketch/geometry'
import type { IdSource } from '../../sketch/recipes'
import type { SketchChange, SketchCommitResult, SketchToolId } from '../../sketch/session'
import { constraintTool } from '../../sketch/tools'
import type { AppStore, SelectionItem, SketchSession } from '../../state/store'
import { cameraAfterSketch, type Camera, type Vector3 } from '../../viewport/camera'
import { addScaled, cross } from '../../viewport/math'
import type { App } from '../createApp'
import { cancelCommand } from './command'
import { coreRedo, coreUndo } from './document'
import { commitEdits, documentOf, reportNewFailures, showCoreError } from './edits'
import { dismiss, notify, showPickHint } from './notifications'
import { select } from './selection'

// Sketch mode (ui.md 3.16): entering, leaving and navigating it (3.16.1, 3.16.11, 3.20), and the
// changes its tools commit: geometry, constraints, dimensions, construction and deletion.

/** One step of the sketch view history (3.20): a camera to go back to, or a committed sketch edit. */
type HistoryEntry =
  | { readonly kind: 'view'; readonly camera: Camera }
  | { readonly kind: 'document' }

/** What sketch mode keeps for its session beside the store's `SketchSession` (3.16.11, 3.20). */
type SketchModeRecord = {
  readonly undo: HistoryEntry[]
  readonly redo: HistoryEntry[]
  /** Feature statuses when the session started; failures since then are reported on finishing (4.2). */
  readonly statusesBefore: Readonly<Record<string, FeatureStatus>>
}

const sessionRecords = new WeakMap<AppStore, SketchModeRecord>()

/** A fresh session on a sketch, or plane picking for a new one (no camera to go back to yet). */
function sketchSession(sketchId: string | null, cameraBefore: Camera | null): SketchSession {
  return {
    sketchId,
    tool: null,
    construction: false,
    showConstraints: true,
    selection: [],
    dragging: false,
    cameraBefore,
  }
}

function editedSketch(app: App): string | null {
  const state = app.store.getState()
  return state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null
}

/**
 * Records the camera from before a view change in sketch mode (an orbit or pan gesture, a wheel
 * burst, a view cube click, fit, home or look at), so Mod+Z can go back to it.
 */
export function recordSketchView(app: App, before: Camera): void {
  const history = sessionRecords.get(app.store)
  if (history === undefined || editedSketch(app) === null) {
    return
  }
  history.undo.push({ kind: 'view', camera: before })
  history.redo.length = 0
}

/** Records a committed sketch edit, which Mod+Z in sketch mode undoes in the core. */
export function recordSketchEdit(app: App): void {
  const history = sessionRecords.get(app.store)
  if (history === undefined || editedSketch(app) === null) {
    return
  }
  history.undo.push({ kind: 'document' })
  history.redo.length = 0
}

/** Discards the sketch view history and the session's starting statuses, as leaving sketch mode does. */
export function leaveSketchHistory(app: App): void {
  sessionRecords.delete(app.store)
}

/** Steps back through the sketch view history; false when there is no step to take here. */
export async function undoInSketch(app: App): Promise<boolean> {
  return stepSketchHistory(app, 'undo')
}

/** Steps forward through the sketch view history; false when there is no step to take here. */
export async function redoInSketch(app: App): Promise<boolean> {
  return stepSketchHistory(app, 'redo')
}

async function stepSketchHistory(app: App, direction: 'undo' | 'redo'): Promise<boolean> {
  const history = sessionRecords.get(app.store)
  if (history === undefined || editedSketch(app) === null) {
    return false
  }
  const [from, to] =
    direction === 'undo' ? [history.undo, history.redo] : [history.redo, history.undo]
  const entry = from.pop()
  if (entry === undefined) {
    return false
  }
  if (entry.kind === 'document') {
    await (direction === 'undo' ? coreUndo(app) : coreRedo(app))
    to.push(entry)
    return true
  }
  const viewport = app.viewport()
  if (viewport !== null) {
    to.push({ kind: 'view', camera: viewport.getCamera() })
    viewport.setCamera(entry.camera, { animate: true, record: false })
  }
  return true
}

/** The sketch.md plane of a plane-like selection item, or null for anything else. */
export function sketchPlaneOf(item: SelectionItem): SketchPlane | null {
  if (
    item.kind === 'origin_plane' &&
    (item.origin === 'XY' || item.origin === 'XZ' || item.origin === 'YZ')
  ) {
    return { kind: 'origin', plane: item.origin }
  }
  if (item.kind === 'datum_plane' && item.feature !== null) {
    return { kind: 'datum', feature: item.feature }
  }
  const element = item.element
  if (
    item.kind === 'face' &&
    element?.surface === 'plane' &&
    element.placement !== undefined &&
    element.placement !== null
  ) {
    return {
      kind: 'face',
      body: element.body,
      face: element.reference,
      fallback: element.placement,
    }
  }
  return null
}

/** Sketch mode for a new sketch: on the one selected plane, else plane picking (3.16.1). */
export async function startNewSketch(app: App): Promise<void> {
  if (app.store.getState().command !== null) {
    await cancelCommand(app)
  }
  if (editedSketch(app) !== null) {
    await finishSketch(app)
  }
  const { selection } = app.store.getState()
  const [only] = selection
  if (selection.length === 1 && only !== undefined && sketchPlaneOf(only) !== null) {
    await createSketchOn(app, only)
    return
  }
  leaveSketchHistory(app)
  app.store.setState({ context: 'sketch', sketch: sketchSession(null, null) })
}

/** Adds an empty sketch on a plane-like item in one edit labeled "New sketch", then edits it. */
export async function createSketchOn(app: App, plane: SelectionItem): Promise<void> {
  const document = documentOf(app)
  const sketchPlane = sketchPlaneOf(plane)
  if (document === null) {
    return
  }
  if (sketchPlane === null) {
    // While picking the plane of a new sketch, the hint shows in the sketch hint (3.16.1).
    showPickHint(app, MESSAGES.pick_plane(), null)
    return
  }
  const features = document.recipe.features ?? []
  const taken = usedIds(document.recipe)
  const sketchId = generateId('f', (id) => taken.has(id))
  // A new feature goes in front of the rollback marker, so it is active (3.7).
  const rollback = document.recipe.rollback ?? null
  const markerIndex =
    rollback === null ? -1 : features.findIndex((feature) => feature.id === rollback)
  const index = markerIndex === -1 ? features.length : markerIndex
  const added = await commitEdits(app, 'New sketch', [
    {
      type: 'add_feature',
      index,
      feature: { id: sketchId, type: 'sketch', plane: sketchPlane, entities: [], constraints: [] },
    },
  ])
  if (added) {
    await enterSketch(app, sketchId)
  }
}

/** The 2D extent of a solution's entities in sketch coordinates, or null without entities. */
function sketchExtent(
  solution: SketchSolution,
): { min: [number, number]; max: [number, number] } | null {
  const xs: number[] = []
  const ys: number[] = []
  function include(x: number, y: number): void {
    xs.push(x)
    ys.push(y)
  }
  for (const entity of solution.entities) {
    switch (entity.type) {
      case 'point':
        include(...entity.position)
        break
      case 'line':
        include(...entity.start)
        include(...entity.end)
        break
      case 'circle':
      case 'arc': {
        const [x, y] = entity.center
        include(x - entity.radius, y - entity.radius)
        include(x + entity.radius, y + entity.radius)
      }
    }
  }
  if (xs.length === 0) {
    return null
  }
  return { min: [Math.min(...xs), Math.min(...ys)], max: [Math.max(...xs), Math.max(...ys)] }
}

/** Looks at the sketch plane from its front and frames the sketch's entities (3.16.1). */
function lookAtSketch(app: App, solution: SketchSolution): void {
  const viewport = app.viewport()
  if (viewport === null) {
    return
  }
  const { origin, x_axis: xAxis, normal } = solution.placement
  const yAxis = cross(normal, xAxis)
  viewport.lookAlong(normal, yAxis, origin)
  const extent = sketchExtent(solution)
  if (extent === null) {
    return
  }
  const corners: Vector3[] = [
    [extent.min[0], extent.min[1]],
    [extent.max[0], extent.min[1]],
    [extent.min[0], extent.max[1]],
    [extent.max[0], extent.max[1]],
  ].map(([x, y]) => addScaled(addScaled(origin, xAxis, x ?? 0), yAxis, y ?? 0))
  const along = (axis: 0 | 1 | 2, pick: (...values: number[]) => number): number =>
    pick(...corners.map((corner) => corner[axis]))
  const min: [number, number, number] = [along(0, Math.min), along(1, Math.min), along(2, Math.min)]
  const max: [number, number, number] = [along(0, Math.max), along(1, Math.max), along(2, Math.max)]
  viewport.zoomToBounds({ min, max }, 0.1)
}

/** Edits a sketch: Sketch context, the sketch shown, and the camera on its plane (3.16.1). */
export async function enterSketch(app: App, sketchId: string): Promise<void> {
  if (app.store.getState().command !== null) {
    await cancelCommand(app)
  }
  const before = app.viewport()?.getCamera() ?? null
  // A session that moves on to another sketch keeps its starting statuses until it finishes, and
  // the camera from before it started, which looked at the model rather than at a sketch.
  const statusesBefore =
    sessionRecords.get(app.store)?.statusesBefore ?? documentOf(app)?.featureStatus ?? {}
  const cameraBefore =
    editedSketch(app) === null ? before : (app.store.getState().sketch?.cameraBefore ?? before)
  app.store.setState((state) => ({
    context: 'sketch',
    sketch: sketchSession(sketchId, cameraBefore),
    view: {
      ...state.view,
      sessionVisible: { ...state.view.sessionVisible, [`feature:${sketchId}`]: true },
    },
  }))
  const document = documentOf(app)
  if (document === null) {
    return
  }
  let solution = document.sketches[sketchId]?.solution ?? null
  if (solution === null) {
    try {
      const solved = await app.core.solveSketch(document.documentHandle, sketchId)
      if (solved !== null) {
        solution = solved.solution
        showSketchSolution(app, sketchId, solved)
      }
    } catch (error) {
      // A refused solve leaves nothing to look at; the session still starts.
      showCoreError(app, error)
    }
  }
  if (solution !== null) {
    lookAtSketch(app, solution)
  }
  // Set after the camera moved onto the sketch, so the history starts with the camera from before.
  sessionRecords.set(app.store, {
    undo: before === null ? [] : [{ kind: 'view', camera: before }],
    redo: [],
    statusesBefore,
  })
}

export type FinishSketchOptions = {
  /**
   * Selects what the finished sketch makes (3.16.11), as Finish sketch and the Model radio do. A
   * feature command preselects by its own rule (3.3), and the Print radio keeps the selection.
   */
  readonly selectResult?: boolean
}

/** The selection item of a sketch's region, labeled by its position in the solution. */
function regionSelectionItem(
  sketchId: string,
  solution: SketchSolution,
  index: number,
): SelectionItem | null {
  const region = solution.regions[index]
  return region === undefined
    ? null
    : {
        key: `region:${sketchId}:${region.reference.key}`,
        kind: 'sketch_region',
        label: `Region ${String(index + 1)}`,
        body: null,
        feature: sketchId,
        element: null,
        sketchItem: { kind: 'region', sketch: sketchId, region },
        origin: null,
      }
}

/**
 * What finishing a sketch selects (3.16.11): its region when it has exactly one; without regions,
 * its non-construction point entities; otherwise, when it has entities, the sketch feature.
 */
function finishedSketchSelection(
  app: App,
  sketchId: string,
  solution: SketchSolution,
): SelectionItem[] {
  if (solution.regions.length === 1) {
    const region = regionSelectionItem(sketchId, solution, 0)
    return region === null ? [] : [region]
  }
  if (solution.regions.length === 0) {
    const points = solution.entities.filter(
      (entity) => entity.type === 'point' && entity.construction !== true,
    )
    if (points.length > 0) {
      return points.map((point) => pointSelectionItem(sketchId, point.id))
    }
  }
  const recipe = documentOf(app)?.recipe
  if (solution.entities.length === 0 || recipe === undefined) {
    return []
  }
  return [
    {
      key: `feature:${sketchId}`,
      kind: 'feature',
      label: featureLabel(recipe, sketchId, app.store.getState().catalog),
      body: null,
      feature: sketchId,
      element: null,
      sketchItem: null,
      origin: null,
    },
  ]
}

/**
 * Turns a camera that still looks straight at the sketch plane back to the view from before the
 * sketch, or to iso, so the next feature's preview is seen in depth (3.16.11).
 */
function leaveSketchPlane(app: App, cameraBefore: Camera | null, normal: Vector3): void {
  const viewport = app.viewport()
  const next = viewport === null ? null : cameraAfterSketch(viewport.getCamera(), cameraBefore, normal)
  if (viewport !== null && next !== null) {
    viewport.setCamera(next, { animate: true })
  }
}

/**
 * Ends sketch mode (3.16.11): Model context, the sketch kept visible for the session, and a camera
 * that still looks straight at the sketch turned so the model is seen in depth. Features the
 * session's edits made fail are reported now rather than after each edit, and an outline with
 * problems is named in a warning that leads back to the sketch.
 */
export async function finishSketch(app: App, options: FinishSketchOptions = {}): Promise<void> {
  const state = app.store.getState()
  const sketchId = state.sketch?.sketchId ?? null
  const cameraBefore = state.sketch?.cameraBefore ?? null
  const statusesBefore = sessionRecords.get(app.store)?.statusesBefore ?? null
  leaveSketchHistory(app)
  app.store.setState((current) => ({
    context: 'model',
    sketch: null,
    view:
      sketchId === null
        ? current.view
        : {
            ...current.view,
            sessionVisible: { ...current.view.sessionVisible, [`feature:${sketchId}`]: true },
          },
  }))
  const document = documentOf(app)
  if (sketchId === null || document === null) {
    return
  }
  if (statusesBefore !== null) {
    reportNewFailures(app, statusesBefore)
  }
  const solution = document.sketches[sketchId]?.solution ?? null
  if (solution !== null) {
    leaveSketchPlane(app, cameraBefore, solution.placement.normal)
  }
  if (options.selectResult === true && solution !== null) {
    select(app, finishedSketchSelection(app, sketchId, solution), 'replace')
  }
  const outline = solution === null ? null : outlineProblems(solution)
  if (outline !== null) {
    // A warning: the sketch stays valid and its other shapes still extrude (3.16.2).
    notify(app, {
      severity: 'warning',
      code: 'profile_problems',
      message: MESSAGES.profile_problems(
        featureLabel(document.recipe, sketchId, state.catalog),
        outline,
      ),
      actions: [
        {
          label: 'Back to sketch',
          run: () => {
            void enterSketch(app, sketchId)
          },
        },
      ],
    })
  }
}

/** The session of the sketch being edited, or null outside sketch mode and while picking a plane. */
function editingSession(app: App): (SketchSession & { readonly sketchId: string }) | null {
  const state = app.store.getState()
  const session = state.sketch
  if (state.context !== 'sketch' || session === null || session.sketchId === null) {
    return null
  }
  return { ...session, sketchId: session.sketchId }
}

function updateSession(app: App, change: Partial<SketchSession>): void {
  app.store.setState((state) =>
    state.sketch === null || state.sketch.sketchId === null
      ? {}
      : { sketch: { ...state.sketch, ...change } },
  )
}

/**
 * Activates a sketch tool, or none, cancelling the current operation. A constraint tool whose
 * operands are already selected applies its constraint at once instead of waiting for picks
 * (3.16.6).
 */
export function setSketchTool(app: App, tool: SketchToolId | null): void {
  const session = editingSession(app)
  if (session === null) {
    return
  }
  const type = constraintTool(tool)
  const solution = solutionOf(app, session.sketchId)
  if (type !== null && solution !== null && session.selection.length > 0) {
    const constraints = constraintsFor(type, session.selection, solution, createSketchIds(app))
    if (constraints.length > 0) {
      updateSession(app, { tool: null })
      void applySketchConstraints(app, type, constraints)
      return
    }
    // Pick mode starts from the selection when more picks can complete it, else from nothing.
    const verdict = pickVerdict(type, session.selection, solution)
    updateSession(app, { tool, selection: verdict === 'partial' ? session.selection : [] })
    return
  }
  updateSession(app, { tool })
}

/** Replaces or toggles the in-sketch selection (3.16.10). */
export function selectInSketch(
  app: App,
  items: readonly SelectionItem[],
  mode: 'replace' | 'toggle',
): void {
  const session = editingSession(app)
  if (session === null) {
    return
  }
  if (mode === 'replace') {
    updateSession(app, { selection: [...items] })
    return
  }
  // Keys keep insertion order, so toggled-on items follow the ones already selected.
  const selection = new Map(session.selection.map((item) => [item.key, item]))
  for (const item of items) {
    if (!selection.delete(item.key)) {
      selection.set(item.key, item)
    }
  }
  updateSession(app, { selection: [...selection.values()] })
}

/** Marks whether a point drag is in progress, which the session shows as `dragging`. */
export function setSketchDragging(app: App, dragging: boolean): void {
  updateSession(app, { dragging })
}

// Ids and params.

/** Ids handed out per store and not yet in the document, so two pending changes never share one. */
const issuedIds = new WeakMap<AppStore, Set<string>>()

/**
 * Fresh entity and constraint ids (model.md 3.2): free in the whole document and never handed out
 * before in this session.
 */
export function createSketchIds(app: App): IdSource {
  const recipe = documentOf(app)?.recipe
  const taken = recipe === undefined ? new Set<string>() : usedIds(recipe)
  let issued = issuedIds.get(app.store)
  if (issued === undefined) {
    issued = new Set()
    issuedIds.set(app.store, issued)
  }
  const handedOut = issued
  return {
    next(prefix) {
      const id = generateId(prefix, (candidate) => taken.has(candidate) || handedOut.has(candidate))
      handedOut.add(id)
      return id
    },
  }
}

/** The sketch record with this id in the recipe, or null. */
function sketchRecord(app: App, sketchId: string): FeatureRecord | null {
  const record = documentOf(app)?.recipe.features?.find((feature) => feature.id === sketchId)
  return record?.type === 'sketch' ? record : null
}

/** A sketch record's params, read from the core's recipe (sketch.md 2.2). */
export function sketchParamsOf(record: FeatureRecord): SketchParams | null {
  const plane: unknown = record['plane']
  const entities: unknown = record['entities'] ?? []
  const constraints: unknown = record['constraints'] ?? []
  if (
    typeof plane !== 'object' ||
    plane === null ||
    !Array.isArray(entities) ||
    !Array.isArray(constraints)
  ) {
    return null
  }
  // The core validated the record against sketch.md before sending it.
  return {
    plane: plane as SketchPlane,
    entities: entities as SketchEntity[],
    constraints: constraints as SketchConstraint[],
  }
}

/** The current solution of a sketch, or null before it is solved. */
export function solutionOf(app: App, sketchId: string): SketchSolution | null {
  return documentOf(app)?.sketches[sketchId]?.solution ?? null
}

/** Shows a solution for a sketch, such as a drag frame or the stored geometry again. */
export function showSketchSolution(app: App, sketchId: string, solved: SolvedSketch): void {
  app.store.setState((state) =>
    state.document === null
      ? {}
      : {
          document: {
            ...state.document,
            sketches: { ...state.document.sketches, [sketchId]: solved },
          },
        },
  )
}

/** The candidate params: the stored ones with the change applied (3.16.3 step 1). */
export function applySketchChange(params: SketchParams, change: SketchChange): SketchParams {
  const removedEntities = new Set(change.removeEntities ?? [])
  const removedConstraints = new Set(change.removeConstraints ?? [])
  const toggled = new Set(change.toggleConstruction ?? [])
  const entities = (params.entities ?? [])
    .filter((entity) => !removedEntities.has(entity.id))
    .map((entity) => (toggled.has(entity.id) ? toggledConstruction(entity) : entity))
  const replacement = change.replaceConstraint
  const constraints = (params.constraints ?? [])
    .filter(
      (constraint) =>
        !removedConstraints.has(constraint.id) &&
        !referencedEntities(constraint).some((entity) => removedEntities.has(entity)),
    )
    .map((constraint) =>
      replacement !== undefined && constraint.id === replacement.id ? replacement : constraint,
    )
  return {
    plane: params.plane,
    entities: [...entities, ...(change.add?.entities ?? [])],
    constraints: [...constraints, ...(change.add?.constraints ?? [])],
  }
}

/** The entity with its construction flag flipped; sketch.md omits the flag when false. */
function toggledConstruction(entity: SketchEntity): SketchEntity {
  if (entity.construction === true) {
    const { construction: _construction, ...rest } = entity
    return rest
  }
  return { ...entity, construction: true }
}

/** The solved geometry of each candidate entity, without the solver's `constrained` state. */
function solvedEntities(
  candidate: readonly SketchEntity[],
  solution: SketchSolution,
): SketchEntity[] {
  const solved = new Map(solution.entities.map((entity) => [entity.id, entity]))
  return candidate.map((entity) => {
    const match = solved.get(entity.id)
    if (match === undefined) {
      return entity
    }
    const { constrained: _constrained, ...geometry } = match
    return geometry
  })
}

// Commits.

/** How many times a probe superseded by another solve of the same sketch is sent again. */
const SUPERSEDED_RETRIES = 3

/** The core's refusal of a probe or commit as a failed result; anything else is a bug. */
function failure(error: unknown): SketchCommitResult {
  if (error instanceof CoreError || error instanceof CoreFatalError) {
    return { outcome: 'failed', message: error.message }
  }
  throw error
}

/**
 * Commits one sketch change as one undo entry (3.16.3): the candidate is probed with the solver
 * first; a requested constraint that is redundant or conflicting refuses the change, inferred ones
 * are dropped silently, and a failed solve or a refused probe applies nothing. The committed
 * entities carry the solved geometry, and a sketch on a face records the solved placement as its
 * fallback. Reading the params, probing and applying are one task of the mutation queue, so no
 * drag end, undo or other commit lands between them.
 */
export function commitSketchChange(
  app: App,
  label: string,
  change: SketchChange,
): Promise<SketchCommitResult> {
  return app.mutations.run(() => commitNow(app, label, change))
}

async function commitNow(
  app: App,
  label: string,
  change: SketchChange,
): Promise<SketchCommitResult> {
  const document = documentOf(app)
  const sketchId = app.store.getState().sketch?.sketchId ?? null
  const record = sketchId === null ? null : sketchRecord(app, sketchId)
  const params = record === null ? null : sketchParamsOf(record)
  if (document === null || sketchId === null || record === null || params === null) {
    return { outcome: 'failed', message: 'There is no sketch being edited.' }
  }
  return probe(app, label, {
    documentHandle: document.documentHandle,
    sketchId,
    record,
    candidate: applySketchChange(params, change),
    requested: change.requested,
    inferred: change.inferred,
    retries: 0,
  })
}

/** One solver probe of a commit and what it still has to check. */
type Probe = {
  readonly documentHandle: DocHandle
  readonly sketchId: string
  readonly record: FeatureRecord
  readonly candidate: SketchParams
  readonly requested: readonly string[]
  /** Inferred constraints still in the candidate. */
  readonly inferred: readonly string[]
  /** Probes sent again after another solve of the sketch superseded them. */
  readonly retries: number
}

/** Probes the candidate (3.16.3 step 2) and applies it, or says why not; each retry is one more probe. */
async function probe(app: App, label: string, current: Probe): Promise<SketchCommitResult> {
  let solved: SolvedSketch | null
  try {
    solved = await app.core.solveSketch(current.documentHandle, current.sketchId, current.candidate)
  } catch (error) {
    return failure(error)
  }
  if (solved === null) {
    return current.retries < SUPERSEDED_RETRIES
      ? probe(app, label, { ...current, retries: current.retries + 1 })
      : { outcome: 'failed', message: MESSAGES.core_not_running() }
  }
  const solution = solved.solution
  const refused =
    current.requested.find((id) => solution.conflicting.includes(id)) ??
    current.requested.find((id) => solution.redundant.includes(id))
  if (refused !== undefined) {
    const conflicting = solution.conflicting.includes(refused)
    return {
      outcome: 'over_constrained',
      constraint: refused,
      conflicting: conflicting ? solution.conflicting.filter((id) => id !== refused) : [],
      redundant: !conflicting,
    }
  }
  const dropped = new Set(
    current.inferred.filter(
      (id) => solution.conflicting.includes(id) || solution.redundant.includes(id),
    ),
  )
  if (dropped.size > 0) {
    // Inferred constraints that over-constrain the sketch are dropped and the rest solved again.
    return probe(app, label, {
      ...current,
      candidate: {
        ...current.candidate,
        constraints: (current.candidate.constraints ?? []).filter(
          (constraint) => !dropped.has(constraint.id),
        ),
      },
      inferred: current.inferred.filter((id) => !dropped.has(id)),
    })
  }
  if (solution.failed) {
    return { outcome: 'failed', message: failedSolveMessage(solution) }
  }
  return applyCandidate(app, label, current.record, current.candidate, solution)
}

/** Applies a probed candidate; it runs inside the commit's task of the mutation queue. */
async function applyCandidate(
  app: App,
  label: string,
  record: FeatureRecord,
  candidate: SketchParams,
  solution: SketchSolution,
): Promise<SketchCommitResult> {
  const document = documentOf(app)
  if (document === null) {
    return { outcome: 'failed', message: 'There is no document open.' }
  }
  const plane: SketchPlane =
    candidate.plane.kind === 'face'
      ? { ...candidate.plane, fallback: solution.placement }
      : candidate.plane
  const edit: Edit = {
    type: 'update_feature',
    feature: {
      ...record,
      plane,
      entities: solvedEntities(candidate.entities ?? [], solution),
      constraints: [...(candidate.constraints ?? [])],
    },
  }
  let revision: number
  try {
    revision = await app.core.applyEdits(document.documentHandle, label, [edit])
  } catch (error) {
    return failure(error)
  }
  // No failure notification here: features this edit made fail are reported on finishing (U-110).
  recordSketchEdit(app)
  return { outcome: 'committed', revision }
}

/** The ids a selection names as entities to delete: curves and point entities. */
function selectedEntities(items: readonly SelectionItem[]): string[] {
  return items.flatMap((item) => {
    const reference = item.sketchItem
    if (reference?.kind === 'entity') {
      return [reference.entity]
    }
    // A point entity is selected by its point reference, which is its bare id.
    if (
      reference?.kind === 'point' &&
      !reference.point.includes('.') &&
      reference.point !== 'origin'
    ) {
      return [reference.point]
    }
    return []
  })
}

/**
 * Deletes the selected entities and constraints, plus every constraint that references a deleted
 * entity or one of its points, in one commit labeled "Delete" (3.16.10).
 */
export async function deleteSketchItems(app: App, items: readonly SelectionItem[]): Promise<void> {
  const entities = selectedEntities(items)
  const constraints = items.flatMap((item) =>
    item.sketchItem?.kind === 'constraint' ? [item.sketchItem.constraint] : [],
  )
  if (entities.length === 0 && constraints.length === 0) {
    return
  }
  const result = await commitSketchChange(app, 'Delete', {
    removeEntities: entities,
    removeConstraints: constraints,
    requested: [],
    inferred: [],
  })
  if (result.outcome === 'committed') {
    const removed = new Set(items.map((item) => item.key))
    const session = editingSession(app)
    if (session !== null) {
      updateSession(app, {
        selection: session.selection.filter((item) => !removed.has(item.key)),
      })
    }
    return
  }
  showSketchRefusal(app, result)
}

/** Deletes constraints by id in one commit labeled "Delete", as the sketch problems list offers. */
export async function deleteSketchConstraints(
  app: App,
  constraints: readonly string[],
): Promise<void> {
  if (constraints.length === 0) {
    return
  }
  const result = await commitSketchChange(app, 'Delete', {
    removeConstraints: constraints,
    requested: [],
    inferred: [],
  })
  showSketchRefusal(app, result)
}

/** Shows why a change was not applied: the solver's or core's message as an error. */
function showSketchRefusal(app: App, result: SketchCommitResult): void {
  if (result.outcome === 'failed') {
    notify(app, { severity: 'error', message: result.message, actions: [] })
  }
}

/** The constraints of the edited sketch by id, for naming them in messages. */
function storedConstraints(app: App, sketchId: string): ReadonlyMap<string, SketchConstraint> {
  const record = sketchRecord(app, sketchId)
  const params = record === null ? null : sketchParamsOf(record)
  return new Map((params?.constraints ?? []).map((constraint) => [constraint.id, constraint]))
}

/**
 * Stored constraints named for a message by their titles (dimensions with their value, "Distance
 * 40"), joined with ", " and " and " (3.16.6).
 */
function conflictList(app: App, sketchId: string | null, ids: readonly string[]): string {
  const stored =
    sketchId === null ? new Map<string, SketchConstraint>() : storedConstraints(app, sketchId)
  const solution = sketchId === null ? null : solutionOf(app, sketchId)
  const units = documentOf(app)?.recipe.units ?? 'mm'
  return joinTitles(
    ids.map((id) => {
      const constraint = stored.get(id)
      return constraint === undefined ? id : constraintTitle(constraint, solution, units)
    }),
  )
}

/**
 * Applies new constraints of one type (3.16.6): committed under the constraint's title, with the
 * over-constrained warning and its "Replace them" action, or the redundancy note, when the solver
 * refuses them. Resolves whether they were added.
 */
export async function applySketchConstraints(
  app: App,
  type: ConstraintType,
  constraints: readonly SketchConstraint[],
): Promise<boolean> {
  const sketchId = editingSession(app)?.sketchId ?? null
  if (sketchId === null || constraints.length === 0) {
    return false
  }
  const title = CONSTRAINT_TITLES[type]
  const requested = constraints.map((constraint) => constraint.id)
  const change: SketchChange = { add: { entities: [], constraints }, requested, inferred: [] }
  const result = await commitSketchChange(app, title, change)
  switch (result.outcome) {
    case 'committed':
      updateSession(app, { selection: [] })
      return true
    case 'failed':
      showSketchRefusal(app, result)
      return false
    case 'over_constrained':
      if (result.redundant) {
        notify(app, {
          severity: 'info',
          message: MESSAGES.redundant_constraint(title),
          actions: [],
          code: 'redundant_constraint',
        })
        return false
      }
      offerReplacement(app, sketchId, title, change, result.conflicting)
      return false
  }
}

/** The over-constrained warning with "Replace them", which swaps the conflicting constraints for the new ones. */
function offerReplacement(
  app: App,
  sketchId: string,
  title: string,
  change: SketchChange,
  conflicting: readonly string[],
): void {
  let id = ''
  id = notify(app, {
    severity: 'warning',
    message: MESSAGES.over_constrained(title, conflictList(app, sketchId, conflicting)),
    code: 'over_constrained',
    actions: [
      {
        label: 'Replace them',
        run: () => {
          dismiss(app, id)
          void replaceConstraints(app, title, change, conflicting)
        },
      },
      {
        label: 'Cancel',
        run: () => {
          dismiss(app, id)
        },
      },
    ],
  })
}

async function replaceConstraints(
  app: App,
  title: string,
  change: SketchChange,
  conflicting: readonly string[],
): Promise<void> {
  const result = await commitSketchChange(app, title, {
    ...change,
    removeConstraints: [...(change.removeConstraints ?? []), ...conflicting],
  })
  if (result.outcome === 'committed') {
    updateSession(app, { selection: [] })
    return
  }
  if (result.outcome === 'over_constrained') {
    const sketchId = editingSession(app)?.sketchId ?? null
    notify(app, {
      severity: 'warning',
      message: MESSAGES.over_constrained(title, conflictList(app, sketchId, result.conflicting)),
      actions: [],
      code: 'over_constrained',
    })
    return
  }
  showSketchRefusal(app, result)
}

/** Share of the sketch's size that pads a framed profile issue, and the least padding in mm. */
const FINDING_PADDING_SHARE = 0.15
const MINIMUM_FINDING_PADDING = 2

/** Frames a profile issue of the edited sketch in the view (3.16.8 "Show"). */
export function showSketchFinding(app: App, finding: ProfileFinding): void {
  const sketchId = editingSession(app)?.sketchId ?? null
  const solution = sketchId === null ? null : solutionOf(app, sketchId)
  const viewport = app.viewport()
  if (solution === null || viewport === null) {
    return
  }
  const extent = sketchExtent(solution)
  const size =
    extent === null ? 0 : Math.hypot(extent.max[0] - extent.min[0], extent.max[1] - extent.min[1])
  const padding = Math.max(MINIMUM_FINDING_PADDING, FINDING_PADDING_SHARE * size)
  const bounds = worldBounds(solution.placement, findingPoints(finding), padding)
  if (bounds !== null) {
    viewport.zoomToBounds(bounds, FINDING_MARGIN)
  }
}

/** Margin of a framed profile issue, as for a framed sketch. */
const FINDING_MARGIN = 0.1

// Dragging points (3.16.9).

/** A point drag: moves solved through the core one at a time, then one commit or a cancel. */
export type SketchDrag = {
  /** Asks the core to solve with the point pinned at `to`; `startedAt` is the pointer move's time. */
  move(to: Vector2, startedAt: number): void
  /** Ends the drag, committing the last solved frame as one undo entry when a move was sent. */
  finish(): Promise<void>
  /** Ends the drag without committing and shows the stored geometry again. */
  cancel(): Promise<void>
}

type DragRequest = { readonly to: Vector2; readonly startedAt: number }

/**
 * Starts dragging a point of the edited sketch. Each move goes through a LatestOnly outside the
 * mutation queue, since drag frames change nothing in the document; each solution is shown at
 * once. The end goes through the mutation queue.
 */
export function startSketchDrag(app: App, sketchId: string, point: string): SketchDrag | null {
  const document = documentOf(app)
  if (document === null) {
    return null
  }
  const { documentHandle } = document
  const before = document.sketches[sketchId] ?? null
  let sent = false
  // Closing: finish or cancel was called. Ended: solved frames are no longer shown.
  let closing = false
  let ended = false
  const frames = createLatestOnly<DragRequest, SolvedSketch | null>(
    (request) =>
      app.core.dragSketch(documentHandle, sketchId, point, [request.to[0], request.to[1]]),
    (solved, request) => {
      if (solved === null || ended) {
        return
      }
      showSketchSolution(app, sketchId, solved)
      // The viewport draws in the next animation frame, which runs before this one.
      requestAnimationFrame(() => {
        measureTiming(TIMING_NAMES.sketchDrag, request.startedAt)
      })
    },
    (error) => {
      showCoreError(app, error)
    },
  )
  setSketchDragging(app, true)

  /**
   * Ends the drag in the core as one task of the mutation queue, queued when the drag closes: a
   * commit or undo asked for right after the release runs after it and sees the dragged geometry.
   * A committing end first waits for the last drag frame.
   */
  async function end(commit: boolean): Promise<void> {
    try {
      await app.mutations.run(async () => {
        if (commit) {
          await frames.flush()
        }
        ended = true
        await app.core.endSketchDrag(documentHandle, sketchId, commit)
      })
      if (commit) {
        recordSketchEdit(app)
      }
    } catch (error) {
      if (!(error instanceof CoreError && error.code === 'no_drag')) {
        showCoreError(app, error)
      }
      if (before !== null) {
        showSketchSolution(app, sketchId, before)
      }
    }
  }

  return {
    move(to, startedAt) {
      if (closing) {
        return
      }
      sent = true
      frames.submit({ to, startedAt })
    },
    async finish() {
      if (closing) {
        return
      }
      closing = true
      if (sent) {
        await end(true)
      }
      ended = true
      setSketchDragging(app, false)
    },
    async cancel() {
      if (closing) {
        return
      }
      closing = true
      ended = true
      frames.cancel()
      if (before !== null) {
        showSketchSolution(app, sketchId, before)
      }
      if (sent) {
        await end(false)
      }
      setSketchDragging(app, false)
    },
  }
}

// Dimensions (3.16.7).

/** Adds a dimension the dimension tool made, labeled "Dimension"; a driving one is probed first. */
export function addSketchDimension(
  app: App,
  dimension: SketchConstraint,
): Promise<SketchCommitResult> {
  const driving = !('driving' in dimension) || dimension.driving !== false
  return commitSketchChange(app, 'Dimension', {
    add: { entities: [], constraints: [dimension] },
    requested: driving ? [dimension.id] : [],
    inferred: [],
  })
}

/** Replaces a stored dimension with a new expression or driving flag, labeled "Change dimension". */
export function changeSketchDimension(
  app: App,
  dimension: SketchConstraint,
): Promise<SketchCommitResult> {
  const driving = !('driving' in dimension) || dimension.driving !== false
  return commitSketchChange(app, 'Change dimension', {
    replaceConstraint: dimension,
    requested: driving ? [dimension.id] : [],
    inferred: [],
  })
}

/** Commits a drawing tool's shape; a refusal is shown, and resolves whether it was added. */
export async function addSketchShape(
  app: App,
  label: string,
  change: SketchChange,
): Promise<boolean> {
  const result = await commitSketchChange(app, label, change)
  switch (result.outcome) {
    case 'committed':
      return true
    case 'failed':
      showSketchRefusal(app, result)
      return false
    case 'over_constrained': {
      const sketchId = editingSession(app)?.sketchId ?? null
      const added = change.add?.constraints.find(
        (constraint) => constraint.id === result.constraint,
      )
      const units = documentOf(app)?.recipe.units ?? 'mm'
      const solution = sketchId === null ? null : solutionOf(app, sketchId)
      const title = added === undefined ? label : constraintTitle(added, solution, units)
      notify(app, {
        severity: result.redundant ? 'info' : 'warning',
        code: result.redundant ? 'redundant_constraint' : 'over_constrained',
        message: result.redundant
          ? MESSAGES.redundant_constraint(title)
          : MESSAGES.over_constrained(title, conflictList(app, sketchId, result.conflicting)),
        actions: [],
      })
      return false
    }
  }
}
