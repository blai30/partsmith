import { generateId, usedIds } from '../../core/ids'
import { comesAfter, markerPosition, sketchRecord } from '../../core/recipe'
import type { HintFrame, SketchPlane } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectFeatureLabel, selectReattachingSketch } from '../../state/selectors'
import type { AppState, AppStore, DocumentState, SelectionItem } from '../../state/store'
import type { App } from '../createApp'
import { commitBuiltEdits, commitEdits, documentOf } from './edits'
import { showPickHint } from './notifications'
import { openSketch, storedSketchParams } from './sketch'
import { startInTurn } from './starts'

// The plane a sketch goes on (ui.md 3.16.1): the sketch.md plane of a picked plane-like item, a
// new sketch on it, or the new plane of a re-attached sketch.

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
 * so the drawing lands on the new plane as it lay on the old one. A face is stored in the
 * coordinates that keep the sketch's motion where its stored plane tells it (`frameKeepingMotion`);
 * what a feature after the sketch made is refused with `attach_after`, since the sketch is built
 * before it.
 */
async function attachSketch(app: App, sketchId: string, item: SelectionItem): Promise<void> {
  const state = app.store.getState()
  const stored = storedSketchParams(state, sketchId)?.plane
  if (stored === undefined) {
    return
  }
  const pickedPlane = sketchPlaneOf(item, frameKeepingMotion(stored))
  const maker = pickedPlane === null ? null : makerAfter(state, sketchId, item)
  if (maker !== null) {
    const message = MESSAGES.attach_after(
      selectFeatureLabel(state, sketchId),
      selectFeatureLabel(state, maker),
    )
    showPickHint(app, message, null)
    return
  }
  await placeSketch(app, pickedPlane, (sketchPlane) => attachSketchOn(app, sketchId, sketchPlane))
}

/** Stores a sketch's new plane in one edit labeled "Attach '{label}'", then edits the sketch (3.16.1). */
async function attachSketchOn(app: App, sketchId: string, plane: SketchPlane): Promise<void> {
  const label = MESSAGES.attach_label(selectFeatureLabel(app.store.getState(), sketchId))
  const attached = await commitBuiltEdits(app, (document) => {
    const record = sketchRecord(document.recipe, sketchId)
    return record === null
      ? null
      : { label, edits: [{ type: 'update_feature', feature: { ...record, plane } }] }
  })
  if (attached) {
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
