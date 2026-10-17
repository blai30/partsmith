import { generateId, usedIds } from '../../core/ids'
import { comesAfter, markerPosition, sketchRecord } from '../../core/recipe'
import type {
  FeatureRecord,
  FeatureStatus,
  HintFrame,
  ProfileFinding,
  SketchConstraint,
  SketchEntity,
  SketchParams,
  SketchPlane,
  SketchSolution,
} from '../../generated/protocol'
import { featureSelectionItem } from '../../inspector/selection-values'
import { MESSAGES } from '../../messages'
import type { SolvedSketch } from '../../core/api'
import {
  CONSTRAINT_TITLES,
  constraintReferences,
  constraintTitle,
  constraintsFor,
  joinTitles,
  pickVerdict,
  type ConstraintType,
} from '../../sketch/constraints'
import { drawnPointItems, regionSelectionItem } from '../../sketch/hit'
import { findingPoints, outlineProblems } from '../../sketch/issues'
import { worldBounds } from '../../sketch/placement'
import { isReservedReference } from '../../sketch/refs'
import type { IdSource } from '../../sketch/recipes'
import type { SketchChange, SketchCommitResult, SketchToolId } from '../../sketch/session'
import { constraintTool } from '../../sketch/tools'
import {
  editingSession,
  selectFeatureLabel,
  selectReattachedSketch,
  selectSketchFaceLost,
} from '../../state/selectors'
import type {
  AppState,
  AppStore,
  DocumentState,
  Notification,
  SelectionItem,
  SketchSession,
} from '../../state/store'
import { cameraAfterSketch, type Camera, type Vector3 } from '../../viewport/camera'
import { addScaled, cross } from '../../viewport/math'
import type { App } from '../createApp'
import { cancelCommandImplicitly } from './command'
import { coreRedo, coreUndo } from './document'
import { commitBuiltEdits, commitEdits, documentOf, showCoreError } from './edits'
import { reportChange } from './loss-notifications'
import { dismiss, notify, showPickHint } from './notifications'
import { select, toggledByKey } from './selection'
import { commitSketchChange } from './sketch-commit'

export { commitSketchChange }

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
  /** Feature statuses when the session started; losses and failures since then are reported on finishing (4.2). */
  readonly statusesBefore: Readonly<Record<string, FeatureStatus>>
}

const sessionRecords = new WeakMap<AppStore, SketchModeRecord>()

/** The `fitBounds` margin of a framed sketch or profile issue: content fills 80 percent of the view. */
const SKETCH_FRAME_MARGIN = 0.1

/** A fresh session on a sketch, or plane picking for a new one (no camera to go back to yet). */
function sketchSession(sketchId: string | null, cameraBefore: Camera | null): SketchSession {
  return {
    sketchId,
    reattaching: null,
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

/**
 * Records a committed sketch edit, which Mod+Z in sketch mode undoes in the core. With `sketchId`,
 * only while that sketch is the one being edited.
 */
export function recordSketchEdit(app: App, sketchId: string | null = null): void {
  const history = sessionRecords.get(app.store)
  const edited = editedSketch(app)
  if (history === undefined || edited === null || (sketchId !== null && sketchId !== edited)) {
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

/**
 * The sketch.md plane of a plane-like selection item, or null for anything else (3.16.1). A flat
 * face gives its full reference and, as the fallback, its descriptor's `fallback`: the placement in
 * body coordinates (`fallback_frame: body`), so the sketch keeps its place on the face when a Move
 * body or a copy moves the face later (features/sketch.md section 2); with `frame` `world`, its
 * descriptor's `placement` in world coordinates, without `fallback_frame`.
 */
export function sketchPlaneOf(item: SelectionItem, frame: HintFrame = 'body'): SketchPlane | null {
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
  if (item.kind !== 'face' || element?.surface !== 'plane') {
    return null
  }
  const fallback = frame === 'body' ? element.fallback : element.placement
  if (fallback === undefined || fallback === null) {
    return null
  }
  const face = { kind: 'face' as const, body: element.body, face: element.reference, fallback }
  return frame === 'body' ? { ...face, fallback_frame: 'body' } : face
}

/**
 * The coordinates a re-attached sketch stores a face's placement in (3.16.1): world coordinates
 * when its stored plane gives it no motion (an origin plane, or a face whose fallback is in world
 * coordinates), which keeps it without one, so the faces built from it and what later features
 * store on them stay in place (engine.md section 3.22); else body coordinates, as a new sketch
 * stores them (engine.md open issue 16).
 */
function frameKeepingMotion(stored: SketchPlane): HintFrame {
  const withoutMotion =
    stored.kind === 'origin' || (stored.kind === 'face' && stored.fallback_frame !== 'body')
  return withoutMotion ? 'world' : 'body'
}

/** Ends a command or the edit of a sketch before plane picking starts (3.16.1). */
async function leaveForPlanePicking(app: App): Promise<void> {
  if (app.store.getState().command !== null) {
    await cancelCommandImplicitly(app)
  }
  if (editedSketch(app) !== null) {
    await finishSketch(app)
  }
}

/** Sketch mode for a new sketch: on the one selected plane, else plane picking (3.16.1). */
export async function startNewSketch(app: App): Promise<void> {
  await leaveForPlanePicking(app)
  const { selection } = app.store.getState()
  const [only] = selection
  if (selection.length === 1 && only !== undefined && sketchPlaneOf(only) !== null) {
    await createSketchOn(app, only)
    return
  }
  openPlanePicking(app, null)
}

/** Plane picking (3.16.1) for a new sketch, or for the sketch `reattaching` is the id of. */
function openPlanePicking(app: App, reattaching: string | null): void {
  leaveSketchHistory(app)
  app.store.setState({ context: 'sketch', sketch: { ...sketchSession(null, null), reattaching } })
}

/**
 * Picks a new plane or face for a sketch whose face is lost (3.16.1, re-attaching): the plane
 * picking of a new sketch for that sketch, while repair mode shows its repair (3.7).
 */
export async function reattachSketch(app: App, sketchId: string): Promise<void> {
  if (sketchRecordOf(app.store.getState(), sketchId) === null) {
    return
  }
  await leaveForPlanePicking(app)
  openPlanePicking(app, sketchId)
}

/**
 * Ends re-attaching once a fix attached the sketch (3.16.1): with its face no longer lost, the
 * sketch is edited, as after a pick.
 */
export async function enterAttachedSketch(app: App): Promise<void> {
  const state = app.store.getState()
  const sketchId = selectReattachedSketch(state)
  if (sketchId !== null && !selectSketchFaceLost(state, sketchId)) {
    await enterSketch(app, sketchId)
  }
}

/**
 * The plane or face picked while a plane is picked (3.16.1): a new sketch on it, or, while a
 * sketch is re-attached, that sketch's new plane.
 */
export async function pickPlane(app: App, item: SelectionItem): Promise<void> {
  const reattaching = selectReattachedSketch(app.store.getState())
  await (reattaching === null ? createSketchOn(app, item) : attachSketch(app, reattaching, item))
}

/** Stores whose sketch is being added or attached, so a second plane pick in the meantime does nothing. */
const placingSketch = new WeakSet<AppStore>()

/**
 * Runs `place` with the sketch plane of a picked item, once at a time per store; an item that is
 * not plane-like (`sketchPlane` null) shows `pick_plane` in the sketch hint instead (3.16.1).
 */
async function placeSketch(
  app: App,
  sketchPlane: SketchPlane | null,
  place: (sketchPlane: SketchPlane, document: DocumentState) => Promise<void>,
): Promise<void> {
  const document = documentOf(app)
  if (document === null || placingSketch.has(app.store)) {
    return
  }
  if (sketchPlane === null) {
    showPickHint(app, MESSAGES.pick_plane(), null)
    return
  }
  placingSketch.add(app.store)
  try {
    await place(sketchPlane, document)
  } finally {
    placingSketch.delete(app.store)
  }
}

/**
 * Adds an empty sketch on a plane-like item in one edit labeled "New sketch", then edits it. A
 * double-click on a plane, or on Sketch with a plane selected, adds one sketch.
 */
export async function createSketchOn(app: App, plane: SelectionItem): Promise<void> {
  await placeSketch(app, sketchPlaneOf(plane), (sketchPlane, document) =>
    addSketchOn(app, document, sketchPlane),
  )
}

/**
 * Stores a plane-like item as a sketch's plane in one edit labeled "Attach '{label}'", then edits
 * the sketch (3.16.1). The entities keep their sketch coordinates, so the drawing lands on the new
 * plane as it lay on the old one. A face is stored in the coordinates that keep the sketch's motion
 * where its stored plane tells it (`frameKeepingMotion`); what a feature after the sketch made is
 * refused with `attach_after`, since the sketch is built before it.
 */
async function attachSketch(app: App, sketchId: string, item: SelectionItem): Promise<void> {
  const state = app.store.getState()
  const stored = storedSketchParams(state, sketchId)?.plane
  if (stored === undefined) {
    return
  }
  const sketchPlane = sketchPlaneOf(item, frameKeepingMotion(stored))
  const maker = sketchPlane === null ? null : makerAfter(state, sketchId, item)
  if (maker !== null) {
    const message = MESSAGES.attach_after(
      selectFeatureLabel(state, sketchId),
      selectFeatureLabel(state, maker),
    )
    showPickHint(app, message, null)
    return
  }
  await placeSketch(app, sketchPlane, async (plane) => {
    const label = MESSAGES.attach_label(selectFeatureLabel(app.store.getState(), sketchId))
    const attached = await commitBuiltEdits(app, (document) => {
      const record = sketchRecord(document.recipe, sketchId)
      return record === null
        ? null
        : { label, edits: [{ type: 'update_feature', feature: { ...record, plane } }] }
    })
    if (attached) {
      await enterSketch(app, sketchId)
    }
  })
}

/** The feature that made a picked item when it comes after the sketch in the timeline, else null. */
function makerAfter(state: AppState, sketchId: string, item: SelectionItem): string | null {
  const recipe = state.document?.recipe
  const maker = item.feature
  return recipe !== undefined && maker !== null && comesAfter(recipe, maker, sketchId)
    ? maker
    : null
}

async function addSketchOn(
  app: App,
  document: DocumentState,
  sketchPlane: SketchPlane,
): Promise<void> {
  const taken = usedIds(document.recipe)
  const sketchId = generateId('f', (id) => taken.has(id))
  // A new feature goes in front of the rollback marker, so it is active (3.7).
  const index = markerPosition(document.recipe.features ?? [], document.recipe.rollback ?? null)
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

/**
 * The 2D extent of a solution's entities in sketch coordinates, with the sketch origin when
 * `withOrigin`, or null without entities.
 */
function sketchExtent(
  solution: SketchSolution,
  withOrigin = false,
): { min: [number, number]; max: [number, number] } | null {
  const xs: number[] = []
  const ys: number[] = []
  function include(x: number, y: number): void {
    xs.push(x)
    ys.push(y)
  }
  if (withOrigin && solution.entities.length > 0) {
    include(0, 0)
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

/** Whether a sketch's constraints refer to its origin or an axis (sketch.md's reserved names). */
function refersToOrigin(constraints: readonly SketchConstraint[]): boolean {
  return constraints.some((constraint) =>
    constraintReferences(constraint).some((reference) => isReservedReference(reference)),
  )
}

/**
 * Looks at the sketch plane from its front and frames the sketch's entities, with its origin when
 * a constraint refers to the origin or an axis, so that axis and the dimension to it show (3.16.1).
 */
function lookAtSketch(app: App, sketchId: string, solution: SketchSolution): void {
  const viewport = app.viewport()
  if (viewport === null) {
    return
  }
  const { origin, x_axis: xAxis, normal } = solution.placement
  const yAxis = cross(normal, xAxis)
  viewport.lookAlong(normal, yAxis, origin)
  const constraints = storedSketchParams(app.store.getState(), sketchId)?.constraints ?? []
  const extent = sketchExtent(solution, refersToOrigin(constraints))
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
  viewport.zoomToBounds({ min, max }, SKETCH_FRAME_MARGIN)
}

/**
 * Resolves once every change queued so far, such as a sketch commit still on its way, has landed;
 * null when nothing is queued, so an idle queue adds no wait.
 */
function queuedChangesLanded(app: App): Promise<void> | null {
  return app.mutations.pending > 0 ? app.mutations.run(() => Promise.resolve()) : null
}

/** Edits a sketch: Sketch context, the sketch shown, and the camera on its plane (3.16.1). */
export async function enterSketch(app: App, sketchId: string): Promise<void> {
  if (app.store.getState().command !== null) {
    await cancelCommandImplicitly(app)
  }
  const landing = editedSketch(app) === null ? null : queuedChangesLanded(app)
  if (landing !== null) {
    // Moving on to another sketch: the commits of this one land first.
    await landing
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
    // The plane or face the sketch started from has done its job; a feature command run from the
    // sketch then preselects what the sketch made (3.3).
    selection: [],
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
    lookAtSketch(app, sketchId, solution)
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
    const [region] = solution.regions
    return region === undefined ? [] : [regionSelectionItem(sketchId, region, 'Region 1')]
  }
  if (solution.regions.length === 0) {
    const points = drawnPointItems(sketchId, solution)
    if (points.length > 0) {
      return points
    }
  }
  if (solution.entities.length === 0 || documentOf(app) === null) {
    return []
  }
  return [featureSelectionItem(sketchId, selectFeatureLabel(app.store.getState(), sketchId))]
}

/**
 * Turns a camera that still looks straight at the sketch plane back to the view from before the
 * sketch, or to iso, so the next feature's preview is seen in depth (3.16.11).
 */
function leaveSketchPlane(app: App, cameraBefore: Camera | null, normal: Vector3): void {
  const viewport = app.viewport()
  const next =
    viewport === null ? null : cameraAfterSketch(viewport.getCamera(), cameraBefore, normal)
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
  const landing = queuedChangesLanded(app)
  if (landing !== null) {
    // Commits still on their way land in the sketch first, so what finishing selects and reports
    // comes from the sketch as the user drew it.
    await landing
  }
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
    reportChange(app, statusesBefore)
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
        selectFeatureLabel(app.store.getState(), sketchId),
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
  const session = editingSession(app.store.getState())
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
  const session = editingSession(app.store.getState())
  if (session === null) {
    return
  }
  if (mode === 'replace') {
    updateSession(app, { selection: [...items] })
    return
  }
  updateSession(app, { selection: toggledByKey(session.selection, items) })
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
export function sketchRecordOf(state: AppState, sketchId: string | null): FeatureRecord | null {
  const recipe = state.document?.recipe
  return sketchId === null || recipe === undefined ? null : sketchRecord(recipe, sketchId)
}

/** The params of the sketch with this id as the recipe stores them, or null. */
export function storedSketchParams(state: AppState, sketchId: string | null): SketchParams | null {
  const record = sketchRecordOf(state, sketchId)
  return record === null ? null : sketchParamsOf(record)
}

/** The body whose face the sketch is drawn on, or null for a sketch on a plane. */
export function sketchFaceBody(state: AppState, sketchId: string | null): string | null {
  const plane = storedSketchParams(state, sketchId)?.plane
  return plane?.kind === 'face' ? plane.body : null
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

// Deleting and constraining (3.16.6, 3.16.10).

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
    const session = editingSession(app.store.getState())
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
  const params = storedSketchParams(app.store.getState(), sketchId)
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
  const sketchId = editingSession(app.store.getState())?.sketchId ?? null
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
        showOverConstrained(app, sketchId, title, result)
      } else {
        offerReplacement(app, sketchId, title, change, result)
      }
      return false
  }
}

type OverConstrained = Extract<SketchCommitResult, { outcome: 'over_constrained' }>

/**
 * The refusal of a change that over-constrains the sketch: `redundant_constraint` as info when the
 * new constraint changes nothing, else `over_constrained` as a warning naming the conflicting
 * constraints (3.16.6). Returns the notification's id.
 */
function showOverConstrained(
  app: App,
  sketchId: string | null,
  title: string,
  result: OverConstrained,
  actions: Notification['actions'] = [],
): string {
  return notify(app, {
    severity: result.redundant ? 'info' : 'warning',
    code: result.redundant ? 'redundant_constraint' : 'over_constrained',
    message: result.redundant
      ? MESSAGES.redundant_constraint(title)
      : MESSAGES.over_constrained(title, conflictList(app, sketchId, result.conflicting)),
    actions,
  })
}

/** The over-constrained warning with "Replace them", which swaps the conflicting constraints for the new ones. */
function offerReplacement(
  app: App,
  sketchId: string,
  title: string,
  change: SketchChange,
  result: OverConstrained,
): void {
  let id = ''
  id = showOverConstrained(app, sketchId, title, result, [
    {
      label: 'Replace them',
      run: () => {
        dismiss(app, id)
        void replaceConstraints(app, title, change, result.conflicting)
      },
    },
    {
      label: 'Cancel',
      run: () => {
        dismiss(app, id)
      },
    },
  ])
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
    const sketchId = editingSession(app.store.getState())?.sketchId ?? null
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
  const sketchId = editingSession(app.store.getState())?.sketchId ?? null
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
    viewport.zoomToBounds(bounds, SKETCH_FRAME_MARGIN)
  }
}

// Dimensions (3.16.7).

/** Whether a dimension drives the sketch; a reference dimension only reports (sketch.md). */
function isDriving(dimension: SketchConstraint): boolean {
  return !('driving' in dimension) || dimension.driving !== false
}

/** Adds a dimension the dimension tool made, labeled "Dimension"; a driving one is probed first. */
export function addSketchDimension(
  app: App,
  dimension: SketchConstraint,
): Promise<SketchCommitResult> {
  return commitSketchChange(app, 'Dimension', {
    add: { entities: [], constraints: [dimension] },
    requested: isDriving(dimension) ? [dimension.id] : [],
    inferred: [],
  })
}

/** Replaces a stored dimension with a new expression or driving flag, labeled "Change dimension". */
export function changeSketchDimension(
  app: App,
  dimension: SketchConstraint,
): Promise<SketchCommitResult> {
  return commitSketchChange(app, 'Change dimension', {
    replaceConstraint: dimension,
    requested: isDriving(dimension) ? [dimension.id] : [],
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
      const sketchId = editingSession(app.store.getState())?.sketchId ?? null
      const added = change.add?.constraints.find(
        (constraint) => constraint.id === result.constraint,
      )
      const units = documentOf(app)?.recipe.units ?? 'mm'
      const solution = sketchId === null ? null : solutionOf(app, sketchId)
      const title = added === undefined ? label : constraintTitle(added, solution, units)
      showOverConstrained(app, sketchId, title, result)
      return false
    }
  }
}
