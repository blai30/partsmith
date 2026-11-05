import { comesAfter } from '../../core/recipe'
import type { DocHandle, Placement, Reference, SketchPlane } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectFeatureLabel } from '../../state/selectors'
import type { ActiveCommand, AppState, SelectionItem } from '../../state/store'
import type { App } from '../createApp'
import { sameValue } from './command-values'
import { showCoreError } from './edits'
import { showPickHint } from './notifications'
import { planeTarget, sketchPlaneOf, unbuiltState } from './sketch-plane'

// A datum plane's new base (ui.md 3.7, engine.md 3.22): while a command edits a datum plane, each
// pick of its Base plane field asks the core before it lands, and a pick the core gives no plane
// for is refused with a hint under the field. A pick of the edited datum itself is refused without
// asking. The Restore of such an edit asks for its saved base first. A candidate of the base's
// repair that the core leaves without an action says why in its tooltip.

/** The path of a datum plane's Base plane field. */
export const BASE_FIELD = 'base'

/** The pointer of the face of a datum plane's base, where the repair of its base is. */
const BASE_FACE_POINTER = '/base/face'

/**
 * The fallback of a face base in the plane the core is asked with when no pick gives one (a
 * Restore). The core does not read a face's fallback for a datum plane (engine.md 3.22).
 */
const UNREAD_FALLBACK: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }

/** Why a new base is refused: the hint under the Base plane field, or the core's error (4.2). */
export type BaseRefusal = { readonly hint: string } | { readonly error: unknown }

/** Whether picks into `field` ask the core first: the Base plane field of an edited datum plane. */
export function asksCoreForBase(command: ActiveCommand, field: string): boolean {
  return command.editing && command.featureType === 'datum_plane' && field === BASE_FIELD
}

/**
 * Checks a pick into the Base plane field of the edited datum plane `datum` (3.7). A pick of the
 * datum itself is refused at once with `datum_base_self`. Any other pick asks `attachPlane` with
 * the plane a new sketch on it would get (3.16.1). Resolves with null when the pick lands.
 */
export async function pickedBaseRefusal(
  app: App,
  documentHandle: DocHandle,
  datum: string,
  item: SelectionItem,
): Promise<BaseRefusal | null> {
  if (item.kind === 'datum_plane' && item.feature === datum) {
    return { hint: MESSAGES.datum_base_self(selectFeatureLabel(app.store.getState(), datum)) }
  }
  const plane = sketchPlaneOf(item)
  if (plane === null) {
    return { hint: MESSAGES.pick_plane() }
  }
  return askedBase(app, documentHandle, datum, plane, item.feature)
}

/**
 * Checks the base the Restore of an edit of the datum plane `datum` reopens with (3.7): a saved
 * base that differs from the record's asks `attachPlane`. Resolves with null when the edit
 * reopens with the saved base.
 */
export async function restoredBaseRefusal(
  app: App,
  documentHandle: DocHandle,
  datum: string,
  recordBase: unknown,
  savedBase: unknown,
): Promise<BaseRefusal | null> {
  const plane = sameValue(savedBase, recordBase) ? null : planeOfBase(savedBase)
  return plane === null ? null : askedBase(app, documentHandle, datum, plane, null)
}

/**
 * The tooltip of a candidate the core leaves without an action in the repair at `repairPath` of
 * the edited datum plane's base (3.7): `datum_base_moved`, with the candidate's `description` as
 * the plane. Null for any other repair.
 */
export function unofferedBaseHint(
  state: AppState,
  repairPath: string,
  description: string,
): string | null {
  const command = state.command
  return command === null ||
    !asksCoreForBase(command, BASE_FIELD) ||
    repairPath !== BASE_FACE_POINTER
    ? null
    : MESSAGES.datum_base_moved(selectFeatureLabel(state, command.featureId), description)
}

/** Shows why a base was refused: its hint under the Base plane field, or the core's message. */
export function showBaseRefusal(app: App, refusal: BaseRefusal): void {
  if ('hint' in refusal) {
    showPickHint(app, refusal.hint, BASE_FIELD)
  } else {
    showCoreError(app, refusal.error)
  }
}

/**
 * Asks the core whether the datum plane `datum` takes `plane` as its new base. `maker` is the
 * feature that made a picked face, which the hint names.
 */
async function askedBase(
  app: App,
  documentHandle: DocHandle,
  datum: string,
  plane: SketchPlane,
  maker: string | null,
): Promise<BaseRefusal | null> {
  let stored: SketchPlane | null
  try {
    stored = await app.core.attachPlane(documentHandle, datum, plane)
  } catch (error) {
    return { error }
  }
  return stored === null ? { hint: refusalHint(app.store.getState(), datum, plane, maker) } : null
}

/**
 * The hint of a base the core refused (4.3), the first that applies: `attach_after` for a datum
 * plane after the edited one, `attach_datum_unbuilt` for one that is suppressed or has an error,
 * else `datum_base_moved`.
 */
function refusalHint(
  state: AppState,
  datum: string,
  plane: SketchPlane,
  maker: string | null,
): string {
  const label = selectFeatureLabel(state, datum)
  if (plane.kind === 'datum') {
    const picked = selectFeatureLabel(state, plane.feature)
    const recipe = state.document?.recipe
    if (recipe !== undefined && comesAfter(recipe, plane.feature, datum)) {
      return MESSAGES.attach_after(label, picked)
    }
    const unbuilt = unbuiltState(state, plane.feature)
    if (unbuilt !== null) {
      return MESSAGES.attach_datum_unbuilt(label, picked, unbuilt)
    }
  }
  return MESSAGES.datum_base_moved(label, planeTarget(state, plane, maker))
}

/**
 * The plane the core is asked with for a stored base: the base itself, and for a face the base
 * with a fallback the core does not read. Null for a value that is no base.
 */
function planeOfBase(base: unknown): SketchPlane | null {
  if (typeof base !== 'object' || base === null) {
    return null
  }
  const { kind, plane, feature, body, face } = base as Readonly<Record<string, unknown>>
  if (kind === 'origin' && (plane === 'XY' || plane === 'XZ' || plane === 'YZ')) {
    return { kind, plane }
  }
  if (kind === 'datum' && typeof feature === 'string') {
    return { kind, feature }
  }
  if (kind === 'face' && typeof body === 'string' && typeof face === 'object' && face !== null) {
    return { kind, body, face: face as Reference, fallback: UNREAD_FALLBACK }
  }
  return null
}
