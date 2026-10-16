import type {
  BoundingBox,
  Cause,
  FixAction,
  FixRef,
  Ghost,
  MeshElement,
} from '../../generated/protocol'
import { featureSelectionItem } from '../../inspector/selection-values'
import { selectionFieldFor } from '../../inspector/status'
import { MESSAGES } from '../../messages'
import { ghostBuffer } from '../../repair/paint'
import { selectRegionCandidates, selectRepairMode, type RepairMode } from '../../repair/repair'
import { entitySelectionItem } from '../../sketch/hit'
import { selectFeatureLabel, type FixPlace } from '../../state/selectors'
import type { SelectionItem } from '../../state/store'
import { extendBounds, type Vector3 } from '../../viewport/math'
import { sketchBounds } from '../../viewport/sketch-drawables'
import type { App } from '../createApp'
import { applyFixInCommand, cancelCommandImplicitly, editFeature, setActiveField } from './command'
import { undo } from './document'
import { documentOf, refusedDuringCommand, runChange } from './edits'
import { listedElements } from './element-lists'
import { announce, notify } from './notifications'
import { elementSelectionItem } from './selection'
import { recordSketchEdit } from './sketch'

// The actions of messages and cause lines (ui.md 4.2: `runFixAction`), Copy details (4.1) and
// "Show what '{label}' lost" (3.7: `showLost`, which opens repair mode and frames what was lost).

/** The `fitBounds` margin of what a fix action or repair mode frames: content fills 80 percent of the view. */
const FRAME_MARGIN = 0.1

/** The share of the body's size on each axis that repair mode frames at least (3.7). */
const LEAST_REPAIR_SHARE = 1 / 3

/**
 * An `apply` action (4.2): offered by the command's own banner it joins the transaction (3.7);
 * offered anywhere else during a feature command it waits with `finish_command_first`; at rest it
 * is one change through `apply_fix`, which the core labels with the action's label, and in a
 * sketch session a step of the session's history, so the next Mod+Z undoes it (3.20, U-145).
 */
async function applyFix(app: App, fix: FixRef, place: FixPlace): Promise<void> {
  if (app.store.getState().command !== null) {
    if (place === 'banner') {
      await applyFixInCommand(app, fix)
    } else {
      refusedDuringCommand(app)
    }
    return
  }
  const { sent } = await runChange(app, async (document) => {
    await app.core.applyFix(document.documentHandle, fix)
    return true
  })
  if (sent) {
    recordSketchEdit(app)
  }
}

/**
 * A `pick` action (4.2): in the command editing its feature, the field its path maps to takes the
 * picks; otherwise the feature is edited with that field active (an implicit cancel of another
 * command first, 3.7).
 */
async function pickAgain(app: App, feature: string, path: string): Promise<void> {
  const state = app.store.getState()
  const command = state.command
  if (command?.featureId === feature) {
    const fields = state.catalog?.byType[command.featureType]?.manifest.fields ?? []
    const field = selectionFieldFor(fields, path)
    if (field !== null) {
      setActiveField(app, field.path)
    }
    return
  }
  await editFeature(app, feature, { field: path })
}

/**
 * The items a `show` frames (4.2): the entity a sketch still has, else what the feature made: a
 * sketch's entities, a datum plane, or the faces it created in the current meshes.
 */
async function shownItems(
  app: App,
  feature: string,
  entity: string | null,
): Promise<readonly SelectionItem[]> {
  const document = documentOf(app)
  const record = document?.recipe.features?.find((candidate) => candidate.id === feature)
  if (document === null || record === undefined) {
    return []
  }
  if (record.type === 'sketch') {
    const entities = document.sketches[feature]?.solution.entities ?? []
    const named = entities.filter((candidate) => candidate.id === entity)
    return (named.length > 0 ? named : entities).map((candidate) =>
      entitySelectionItem(feature, candidate),
    )
  }
  if (record.type === 'datum_plane') {
    return [
      featureSelectionItem(
        feature,
        selectFeatureLabel(app.store.getState(), feature),
        'datum_plane',
      ),
    ]
  }
  const lists = Object.entries(document.meshes).map(([body, mesh]) =>
    listedElements(app, body, 'face', mesh.meshId),
  )
  const faces = await Promise.all(lists.map((list) => list?.catch(() => []) ?? []))
  return faces
    .flat()
    .filter((face) => face.feature === feature)
    .map((face) => elementSelectionItem(app, face))
}

/** Frames selection items in the viewport; nothing without a viewport or anything drawn. */
function frameItems(app: App, items: readonly SelectionItem[]): void {
  const viewport = app.viewport()
  const bounds = items.length === 0 ? null : (viewport?.itemBounds(items) ?? null)
  if (viewport !== null && bounds !== null) {
    viewport.zoomToBounds(bounds, FRAME_MARGIN)
  }
}

/**
 * A `show` action (4.2): during a feature command, a show of the command's own feature frames its
 * preview and one of another feature first ends the command as an implicit cancel (3.7). Then the
 * feature is selected, or the entity it names while the sketch still has it, `AppState.shown`
 * keeps the cause's ghost, and what it made is framed.
 */
async function show(
  app: App,
  feature: string,
  entity: string | null,
  ghost: Ghost | null,
): Promise<void> {
  const command = app.store.getState().command
  if (command?.featureId === feature) {
    app.viewport()?.fit()
    return
  }
  if (command !== null) {
    await cancelCommandImplicitly(app)
  }
  const state = app.store.getState()
  const record = state.document?.recipe.features?.find((candidate) => candidate.id === feature)
  if (record === undefined) {
    return
  }
  const drawn = state.document?.sketches[feature]?.solution.entities.find(
    (candidate) => candidate.id === entity,
  )
  const item =
    drawn === undefined
      ? featureSelectionItem(
          feature,
          selectFeatureLabel(state, feature),
          record.type === 'datum_plane' ? 'datum_plane' : 'feature',
        )
      : entitySelectionItem(feature, drawn)
  // One change, so the selection change does not clear what is shown.
  app.store.setState({ selection: [item], shown: { feature, entity, ghost } })
  frameItems(app, await shownItems(app, feature, entity))
}

/**
 * Runs one action of a message or of its cause line (4.2), offered at `place`: `apply` through
 * `apply_fix`, `pick`, `show` and `undo` as host actions; `finish_transaction` is never offered,
 * since the UI refuses itself, during a command, the requests the core answers with it (3.7).
 * `cause` is the cause line the action belongs to, whose ghost a `show` draws.
 */
export async function runFixAction(
  app: App,
  action: FixAction,
  place: FixPlace,
  cause?: Cause,
): Promise<void> {
  const run = action.run
  switch (run.kind) {
    case 'apply':
      await applyFix(app, run.fix, place)
      return
    case 'pick':
      await pickAgain(app, run.feature, run.path)
      return
    case 'show':
      await show(app, run.feature, run.entity ?? null, cause?.ghost ?? null)
      return
    case 'undo':
      await undo(app)
      return
    case 'finish_transaction':
      return
  }
}

/**
 * Copies the code, the message, the cause and the detail of a message or core error for reports
 * (4.1), each on its own line, and announces it; a clipboard that refuses shows
 * `details_not_copied` as a warning.
 */
export async function copyDetails(
  app: App,
  message: {
    readonly code: string
    readonly message: string
    readonly cause?: Cause | null
    readonly detail?: string | null
  },
): Promise<void> {
  const lines = [message.code, message.message, message.cause?.text, message.detail].filter(
    (line): line is string => line !== null && line !== undefined,
  )
  try {
    await navigator.clipboard.writeText(lines.join('\n'))
  } catch (error) {
    console.info('The clipboard refused the details.', error)
    notify(app, {
      severity: 'warning',
      message: MESSAGES.details_not_copied(),
      actions: [],
      code: 'details_not_copied',
    })
    return
  }
  announce(app, MESSAGES.details_copied())
}

/**
 * The centroid of each element in its body's model-layer mesh (3.7): the `centroid` of the
 * `list_elements` descriptor at its display id; null for none, for an element of a mesh the body
 * no longer shows, or when the list cannot be had.
 */
export function elementCentroids(
  app: App,
  elements: readonly (MeshElement | null)[],
): Promise<readonly (Vector3 | null)[]> {
  return Promise.all(
    elements.map(async (element) => {
      if (element === null) {
        return null
      }
      const listed = listedElements(app, element.body, element.kind, element.mesh_id)
      const descriptors = await (listed ?? Promise.resolve([])).catch(() => [])
      return descriptors[element.index]?.centroid ?? null
    }),
  )
}

/** The points of a flat `f32` list of world points, such as a ghost's (2.7); none without one. */
function worldPoints(positions: Float32Array | undefined): Vector3[] {
  if (positions === undefined) {
    return []
  }
  const points: Vector3[] = []
  for (let i = 0; i + 3 <= positions.length; i += 3) {
    points.push([positions[i] ?? 0, positions[i + 1] ?? 0, positions[i + 2] ?? 0])
  }
  return points
}

/** A box grown about its center to at least `least` on each axis. */
function grownTo(bounds: BoundingBox, least: readonly number[]): BoundingBox {
  const axes = [0, 1, 2] as const
  const half = axes.map(
    (axis) => Math.max(bounds.max[axis] - bounds.min[axis], least[axis] ?? 0) / 2,
  )
  const center = axes.map((axis) => (bounds.min[axis] + bounds.max[axis]) / 2)
  const along = (axis: 0 | 1 | 2, sign: number) => (center[axis] ?? 0) + sign * (half[axis] ?? 0)
  return {
    min: [along(0, -1), along(1, -1), along(2, -1)],
    max: [along(0, 1), along(1, 1), along(2, 1)],
  }
}

/** The points repair mode frames, and the box of which it frames at least a third on each axis (3.7). */
type RepairFrame = {
  readonly points: readonly Vector3[]
  readonly around: { readonly min: readonly number[]; readonly max: readonly number[] } | null
}

/** The points that have a place, in order. */
function placed(points: readonly (Vector3 | null)[]): Vector3[] {
  return points.filter((point): point is Vector3 => point !== null)
}

/**
 * What repair mode frames (3.7): for an element repair the ghost's points, else the stored
 * centroid, with the candidates' markers, in the repair's body; for a region repair the
 * candidates' numbers in their sketch's drawing.
 */
async function repairFrame(app: App, mode: RepairMode): Promise<RepairFrame> {
  const { repair, candidates } = mode.view
  const document = documentOf(app)
  if (repair.kind === 'region') {
    const solution = document?.sketches[repair.sketch]?.solution
    return {
      points: placed(selectRegionCandidates(app.store.getState())?.anchors ?? []),
      around: solution === undefined ? null : sketchBounds(solution),
    }
  }
  const ghost = worldPoints(ghostBuffer(app.store.getState(), repair))
  const lost: Vector3[] =
    ghost.length > 0
      ? ghost
      : repair.centroid === null || repair.centroid === undefined
        ? []
        : [repair.centroid]
  const markers = await elementCentroids(
    app,
    candidates.map((candidate) => candidate.element),
  )
  return {
    points: [...lost, ...placed(markers)],
    around: document?.meshes[repair.body]?.boundingBox ?? null,
  }
}

/** Frames what repair mode shows, grown about its center to at least a third of what is around it on each axis (3.7). */
async function frameRepair(app: App, mode: RepairMode): Promise<void> {
  const { points, around } = await repairFrame(app, mode)
  const bounds = extendBounds(null, points)
  const viewport = app.viewport()
  if (bounds === null || viewport === null) {
    return
  }
  const least = [0, 1, 2].map((axis) =>
    around === null ? 0 : ((around.max[axis] ?? 0) - (around.min[axis] ?? 0)) * LEAST_REPAIR_SHARE,
  )
  viewport.zoomToBounds(
    grownTo({ min: [...bounds.min], max: [...bounds.max] }, least),
    FRAME_MARGIN,
  )
}

/**
 * "Show what '{label}' lost" (3.7): edits the feature, whose first repair's field takes the picks,
 * which shows repair mode, and frames what was lost with the candidates' markers.
 */
export async function showLost(app: App, featureId: string): Promise<void> {
  await editFeature(app, featureId)
  const state = app.store.getState()
  const mode = state.command?.featureId === featureId ? selectRepairMode(state) : null
  if (mode !== null) {
    await frameRepair(app, mode)
  }
}
