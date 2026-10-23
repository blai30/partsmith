import type { FeatureRecord, SelectionKind, Use } from '../generated/protocol'
import { getParam } from '../inspector/model'
import { isRecord } from '../inspector/param-path'
import { fieldForPointer } from '../inspector/status'
import { MESSAGES } from '../messages'
import { selectFeatureLabel } from '../state/selectors'
import type { AppState } from '../state/store'
import { dropIndex } from './strip'

// What features use, from `list_uses` (ui.md 3.9, 3.16.10, engine.md 3.24): the timeline's drop
// warning, "Used by" in an item's menu and the used marks of sketch entities. `refreshUses` keeps
// `document.uses` at the document's revision.

/** A use a drop would break: `user` would come before `feature`, whose `what` it uses. */
export type DropWarning = {
  readonly user: string
  readonly feature: string
  /** The used thing with its article, such as "an edge" (3.9). */
  readonly what: string
  readonly moved: string
}

/** What a use names, by the selection kind of the user's field it maps to (3.9). */
type UsedThing =
  | 'edge'
  | 'face'
  | 'plane'
  | 'axis'
  | 'region'
  | 'point'
  | 'sketch'
  | 'body'
  | 'shape'

const WITH_ARTICLE: Readonly<Record<UsedThing, string>> = {
  edge: 'an edge',
  face: 'a face',
  plane: 'a plane',
  axis: 'an axis',
  region: 'a region',
  point: 'a point',
  sketch: 'a sketch',
  body: 'a body',
  shape: 'a shape',
}

/** The thing a selection kind names, for the kinds whose value does not decide it. */
const THING_OF_KIND: Readonly<Record<Exclude<SelectionKind, 'plane' | 'axis'>, UsedThing>> = {
  edge: 'edge',
  face: 'face',
  sketch_region: 'region',
  sketch_point: 'point',
  sketch: 'sketch',
  body: 'body',
}

const positionsByFeatures = new WeakMap<readonly FeatureRecord[], ReadonlyMap<string, number>>()

/** The timeline index of each feature of the open document, computed once per recipe. */
function timelinePositions(state: AppState): ReadonlyMap<string, number> {
  const features = state.document?.recipe.features ?? []
  let positions = positionsByFeatures.get(features)
  if (positions === undefined) {
    positions = new Map(features.map((feature, index) => [feature.id, index]))
    positionsByFeatures.set(features, positions)
  }
  return positions
}

/** The latest `list_uses` result of the open document, whatever revision it describes. */
function knownUses(state: AppState): readonly Use[] {
  return state.document?.uses?.uses ?? []
}

/** Whether the uses of the open document are known at its current revision (3.9). */
export function usesAreCurrent(state: AppState): boolean {
  const document = state.document
  return document !== null && document.uses !== null && document.uses.revision === document.revision
}

/** The thing `use` names, from the selection kind of the user's field its `field` maps to (3.7). */
function usedThing(state: AppState, use: Use): UsedThing {
  const record = state.document?.recipe.features?.find((feature) => feature.id === use.user)
  if (record === undefined) {
    return 'shape'
  }
  const hints = state.catalog?.byType[record.type]?.manifest.fields ?? []
  const path = fieldForPointer(
    hints.map((hint) => hint.path),
    use.field,
  )
  const kind = hints.find((hint) => hint.path === path)?.selection ?? null
  if (path === null || kind === null) {
    return 'shape'
  }
  if (kind !== 'plane' && kind !== 'axis') {
    return THING_OF_KIND[kind]
  }
  // A plane or axis field names what its value holds: a face, an edge, or a plane or axis.
  const value = getParam(record, path)
  const valueKind = isRecord(value) ? value.kind : undefined
  if (valueKind === 'face' || (kind === 'axis' && valueKind === 'edge')) {
    return valueKind
  }
  return kind
}

/**
 * The timeline index of the feature at index `position` once the moved feature goes from index
 * `from` to index `to` (3.9: every feature between the two shifts by one toward where the moved
 * one was).
 */
function positionAfterMove(position: number, from: number, to: number): number {
  if (position === from) {
    return to
  }
  if (from < to && position > from && position <= to) {
    return position - 1
  }
  if (to < from && position >= to && position < from) {
    return position + 1
  }
  return position
}

/**
 * The first use, in `list_uses` order, that holds before a drop of `moved` at gap `gap` (the used
 * feature comes before its user) and not after it; null when none or when the uses are not current.
 */
export function dropWarning(state: AppState, moved: string, gap: number): DropWarning | null {
  if (!usesAreCurrent(state)) {
    return null
  }
  const positions = timelinePositions(state)
  const from = positions.get(moved)
  if (from === undefined) {
    return null
  }
  const to = dropIndex(from, gap)
  if (to === from) {
    return null
  }
  for (const use of knownUses(state)) {
    if (use.user !== moved && use.feature !== moved) {
      // The drop keeps the order of every other pair.
      continue
    }
    const user = positions.get(use.user)
    const feature = positions.get(use.feature)
    if (user === undefined || feature === undefined || feature >= user) {
      continue
    }
    if (positionAfterMove(feature, from, to) > positionAfterMove(user, from, to)) {
      return {
        user: use.user,
        feature: use.feature,
        what: WITH_ARTICLE[usedThing(state, use)],
        moved,
      }
    }
  }
  return null
}

/** `ids` without repeats and without ids that are not in the document, in timeline order. */
function inTimelineOrder(state: AppState, ids: Iterable<string>): readonly string[] {
  const positions = timelinePositions(state)
  return [...new Set(ids)]
    .filter((id) => positions.has(id))
    .toSorted((first, second) => (positions.get(first) ?? 0) - (positions.get(second) ?? 0))
}

/** The features in the document, other than `feature`, that have a use of it, in timeline order. */
export function usersOf(state: AppState, feature: string): readonly string[] {
  return inTimelineOrder(
    state,
    knownUses(state)
      .filter((use) => use.feature === feature && use.user !== feature)
      .map((use) => use.user),
  )
}

/** The `used_by` text that names `users` by their labels (3.9, 3.16.10). */
export function usedByText(state: AppState, users: readonly string[]): string {
  return MESSAGES.used_by(users.map((user) => selectFeatureLabel(state, user)))
}

/** The features that link `sketch`: those with a use of it without an entity. */
function sketchLinks(state: AppState, sketch: string): ReadonlySet<string> {
  return new Set(
    knownUses(state)
      .filter((use) => use.feature === sketch && (use.entity ?? null) === null)
      .map((use) => use.user),
  )
}

/** The uses of the entities of `sketch`: those with an entity, through a feature that links it. */
function entityUses(
  state: AppState,
  sketch: string,
): readonly (Use & { readonly entity: string })[] {
  const links = sketchLinks(state, sketch)
  return knownUses(state).filter(
    (use): use is Use & { readonly entity: string } =>
      (use.entity ?? null) !== null && links.has(use.feature),
  )
}

/**
 * For the sketch `sketch`: each entity that a use names (`entity` set, with a `feature` that has a
 * use of `sketch` without an entity, a feature link), mapped to its users in timeline order.
 */
export function usedEntities(
  state: AppState,
  sketch: string,
): Readonly<Record<string, readonly string[]>> {
  const users = new Map<string, string[]>()
  for (const use of entityUses(state, sketch)) {
    users.set(use.entity, [...(users.get(use.entity) ?? []), use.user])
  }
  const used: Record<string, readonly string[]> = {}
  for (const [entity, entityUsers] of users) {
    const ordered = inTimelineOrder(state, entityUsers)
    if (ordered.length > 0) {
      used[entity] = ordered
    }
  }
  return used
}

/** The entities of `sketch` that show a used mark: those `usedEntities` names (3.16.10). */
export function usedMarkEntities(state: AppState, sketch: string): readonly string[] {
  return Object.keys(usedEntities(state, sketch))
}

/** What a later feature uses of an entity of a sketch: its first user and the noun of that use. */
export type EntityUse = {
  readonly user: string
  /** The used thing without its article, such as "edge" (3.16.10). */
  readonly noun: string
}

/**
 * The first user, in timeline order, of the entity `entity` of `sketch`, and the noun of its first
 * use of that entity in `list_uses` order (3.16.10); null when nothing uses the entity.
 */
export function entityUse(state: AppState, sketch: string, entity: string): EntityUse | null {
  const uses = entityUses(state, sketch).filter((use) => use.entity === entity)
  const [user] = inTimelineOrder(
    state,
    uses.map((use) => use.user),
  )
  const use = uses.find((candidate) => candidate.user === user)
  return user === undefined || use === undefined ? null : { user, noun: usedThing(state, use) }
}
