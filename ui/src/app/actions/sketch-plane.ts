import { generateId, usedIds } from '../../core/ids'
import { originPlaneName } from '../../core/labels'
import { comesAfter, markerPosition, sketchRecord } from '../../core/recipe'
import type { SketchPlane } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectFeatureLabel, selectReattachingSketch } from '../../state/selectors'
import type { AppState, AppStore, DocumentState, SelectionItem } from '../../state/store'
import type { App } from '../createApp'
import { commitEdits, documentOf, runChange } from './edits'
import { showPickHint } from './notifications'
import { openSketch } from './sketch'
import { startInTurn } from './starts'

// The plane a sketch goes on (ui.md 3.16.1): the sketch.md plane of a picked plane-like item, a
// new sketch on it, or the new plane of a re-attached sketch.

/**
 * The sketch.md plane of a plane-like selection item, or null for anything else (3.16.1). A flat
 * face gives its full reference and, as the fallback, its descriptor's `fallback`: the placement in
 * body coordinates (`fallback_frame: body`), so the sketch keeps its place on the face when a Move
 * body or a copy moves the face later (features/sketch.md section 2).
 */
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
  if (item.kind !== 'face' || element?.surface !== 'plane') {
    return null
  }
  const fallback = element.fallback
  if (fallback === undefined || fallback === null) {
    return null
  }
  return {
    kind: 'face',
    body: element.body,
    face: element.reference,
    fallback,
    fallback_frame: 'body',
  }
}

/** Whether a plane is being picked for a new sketch or a re-attached one (3.16.1). */
export function pickingPlane(state: AppState): boolean {
  return state.context === 'sketch' && state.sketch !== null && state.sketch.sketchId === null
}

/**
 * The plane or face picked while a plane is picked (3.16.1): a new sketch on it, or, while a
 * sketch is re-attached, that sketch's new plane. It takes its turn among the starts when it is
 * made (3.7 step 1), so `picked` is the item or, for a click on the model, the core's answer to its
 * pick still on its way; a pick whose turn comes once plane picking has closed does nothing.
 */
export function pickPlane(
  app: App,
  picked: SelectionItem | Promise<SelectionItem | null>,
): Promise<void> {
  return startInTurn(app, async () => {
    const item = await picked
    const state = app.store.getState()
    if (item === null || !pickingPlane(state)) {
      return
    }
    const reattaching = selectReattachingSketch(state)
    await (reattaching === null ? createSketchOn(app, item) : attachSketch(app, reattaching, item))
  })
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
 * Attaches the sketch to a plane-like item (3.16.1). The entities keep their sketch coordinates,
 * so the drawing lands on the new plane as it lay on the old one (`attachSketchOn`).
 */
async function attachSketch(app: App, sketchId: string, item: SelectionItem): Promise<void> {
  await placeSketch(app, sketchPlaneOf(item), (sketchPlane) =>
    attachSketchOn(app, sketchId, item, sketchPlane),
  )
}

/**
 * The hint that refuses a picked plane-like item before the core is asked (3.16.1), read from
 * `state` when the pick's turn in the mutation queue comes: `attach_after` for what a feature after
 * the sketch made, since the sketch is built before it, `attach_datum_unbuilt` for a datum plane
 * that is suppressed or has an error, since its motion is unknown; null for any other item.
 */
function attachRefusal(state: AppState, sketchId: string, item: SelectionItem): string | null {
  const label = selectFeatureLabel(state, sketchId)
  const maker = makerAfter(state, sketchId, item)
  if (maker !== null) {
    return MESSAGES.attach_after(label, selectFeatureLabel(state, maker))
  }
  if (item.kind !== 'datum_plane' || item.feature === null) {
    return null
  }
  const unbuilt = unbuiltState(state, item.feature)
  return unbuilt === null
    ? null
    : MESSAGES.attach_datum_unbuilt(label, selectFeatureLabel(state, item.feature), unbuilt)
}

/** Why a feature does not build: suppressed, or an error; null when it builds. */
function unbuiltState(state: AppState, feature: string): 'suppressed' | 'error' | null {
  const document = state.document
  const record = document?.recipe.features?.find((candidate) => candidate.id === feature)
  const status = document?.featureStatus[feature]?.status
  if (record?.suppressed === true || status === 'suppressed') {
    return 'suppressed'
  }
  return status === 'error' ? 'error' : null
}

/**
 * How `attach_moved` names a picked plane (4.3): the origin plane, the datum plane's label in
 * quotes, or a face by the feature that made it.
 */
function attachTarget(state: AppState, item: SelectionItem, plane: SketchPlane): string {
  switch (plane.kind) {
    case 'origin':
      return `the ${originPlaneName(plane.plane)}`
    case 'datum':
      return `'${selectFeatureLabel(state, plane.feature)}'`
    case 'face':
      return item.feature === null
        ? 'this face'
        : `this face of '${selectFeatureLabel(state, item.feature)}'`
  }
}

/**
 * Stores a sketch's new plane in one edit labeled "Attach '{label}'", then edits the sketch
 * (3.16.1). In one task of the mutation queue, with no change between them, it checks the picked
 * `item` against the document as it is then (`attachRefusal`), asks the core for the plane to
 * store for `plane` (`attachPlane`, engine.md section 3.22) and commits it. A sketch record that is
 * gone by then sends nothing. A refused item, or a plane the core gives none for (`attach_moved`),
 * shows its hint in the sketch hint and the pick stays open.
 */
async function attachSketchOn(
  app: App,
  sketchId: string,
  item: SelectionItem,
  plane: SketchPlane,
): Promise<void> {
  const { sent } = await runChange(app, async (document) => {
    const record = sketchRecord(document.recipe, sketchId)
    if (record === null) {
      return false
    }
    const state = app.store.getState()
    const refusal = attachRefusal(state, sketchId, item)
    if (refusal !== null) {
      showPickHint(app, refusal, null)
      return false
    }
    const stored = await app.core.attachPlane(document.documentHandle, sketchId, plane)
    const label = selectFeatureLabel(state, sketchId)
    if (stored === null) {
      showPickHint(app, MESSAGES.attach_moved(label, attachTarget(state, item, plane)), null)
      return false
    }
    await app.core.applyEdits(document.documentHandle, MESSAGES.attach_label(label), [
      { type: 'update_feature', feature: { ...record, plane: stored } },
    ])
    return true
  })
  if (sent) {
    await openSketch(app, sketchId)
  }
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
    await openSketch(app, sketchId, { added: true })
  }
}
