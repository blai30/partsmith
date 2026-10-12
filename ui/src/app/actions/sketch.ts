import { generateId, usedIds } from '../../core/ids'
import { featureLabel } from '../../core/labels'
import type { SketchPlane, SketchSolution } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import type { SketchToolId } from '../../sketch/session'
import type { AppStore, SelectionItem, SketchSession } from '../../state/store'
import type { Camera, Vector3 } from '../../viewport/camera'
import { addScaled, cross } from '../../viewport/math'
import type { App } from '../createApp'
import { cancelCommand } from './command'
import { coreRedo, coreUndo } from './document'
import { commitEdits, documentOf } from './edits'
import { notify } from './notifications'

// Entering, leaving and navigating sketch mode (ui.md 3.16.1, 3.16.11, 3.20). Drawing, constraints
// and their commits belong to the sketch controller.

/** One step of the sketch view history (3.20): a camera to go back to, or a committed sketch edit. */
type HistoryEntry =
  | { readonly kind: 'view'; readonly camera: Camera }
  | { readonly kind: 'document' }

type SketchHistory = { readonly undo: HistoryEntry[]; readonly redo: HistoryEntry[] }

const histories = new WeakMap<AppStore, SketchHistory>()

/** A fresh session on a sketch, or plane picking for a new one. */
function sketchSession(sketchId: string | null): SketchSession {
  return {
    sketchId,
    tool: null,
    construction: false,
    showConstraints: true,
    selection: [],
    dragging: false,
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
  const history = histories.get(app.store)
  if (history === undefined || editedSketch(app) === null) {
    return
  }
  history.undo.push({ kind: 'view', camera: before })
  history.redo.length = 0
}

/** Records a committed sketch edit, which Mod+Z in sketch mode undoes in the core. */
export function recordSketchEdit(app: App): void {
  const history = histories.get(app.store)
  if (history === undefined || editedSketch(app) === null) {
    return
  }
  history.undo.push({ kind: 'document' })
  history.redo.length = 0
}

/** Discards the sketch view history, as leaving sketch mode does. */
export function leaveSketchHistory(app: App): void {
  histories.delete(app.store)
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
  const history = histories.get(app.store)
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
  app.store.setState({ context: 'sketch', sketch: sketchSession(null) })
}

/** Adds an empty sketch on a plane-like item in one edit labeled "New sketch", then edits it. */
export async function createSketchOn(app: App, plane: SelectionItem): Promise<void> {
  const document = documentOf(app)
  const sketchPlane = sketchPlaneOf(plane)
  if (document === null) {
    return
  }
  if (sketchPlane === null) {
    notify(app, { severity: 'info', message: MESSAGES.pick_plane(), actions: [] })
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
  const min: Vector3 = [along(0, Math.min), along(1, Math.min), along(2, Math.min)]
  const max: Vector3 = [along(0, Math.max), along(1, Math.max), along(2, Math.max)]
  viewport.zoomToBounds({ min, max }, 0.1)
}

/** Edits a sketch: Sketch context, the sketch shown, and the camera on its plane (3.16.1). */
export async function enterSketch(app: App, sketchId: string): Promise<void> {
  if (app.store.getState().command !== null) {
    await cancelCommand(app)
  }
  const before = app.viewport()?.getCamera() ?? null
  app.store.setState((state) => ({
    context: 'sketch',
    sketch: sketchSession(sketchId),
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
    const solved = await app.core.solveSketch(document.documentHandle, sketchId)
    if (solved !== null) {
      solution = solved.solution
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
  }
  if (solution !== null) {
    lookAtSketch(app, solution)
  }
  histories.set(app.store, {
    undo: before === null ? [] : [{ kind: 'view', camera: before }],
    redo: [],
  })
}

/** Ends sketch mode: Model context, the sketch kept visible for the session, the camera kept (3.16.11). */
export async function finishSketch(app: App): Promise<void> {
  const state = app.store.getState()
  const sketchId = state.sketch?.sketchId ?? null
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
  const problems = document.sketches[sketchId]?.solution.profile_findings.length ?? 0
  if (problems > 0) {
    notify(app, {
      severity: 'info',
      message: MESSAGES.profile_problems(
        featureLabel(document.recipe, sketchId, state.catalog),
        problems,
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

/** Activates a sketch tool, or none. */
export function setSketchTool(app: App, tool: SketchToolId | null): void {
  app.store.setState((state) =>
    state.sketch === null || state.sketch.sketchId === null
      ? {}
      : { sketch: { ...state.sketch, tool } },
  )
}
