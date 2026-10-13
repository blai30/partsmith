import type { Document, FeatureRecord } from '../generated/protocol'
import { selectOriginVisible } from './selectors'
import type { AppState } from './store'

// Session-only visibility of sketches, datum planes and origin items (ui.md 2.8 ViewState, 3.10).

/** Whether a value refers to the sketch through a `sketch` key anywhere inside it. */
export function referencesSketch(value: unknown, sketchId: string): boolean {
  if (Array.isArray(value)) {
    return value.some((item) => referencesSketch(item, sketchId))
  }
  if (typeof value !== 'object' || value === null) {
    return false
  }
  return Object.entries(value).some(
    ([key, item]) => (key === 'sketch' && item === sketchId) || referencesSketch(item, sketchId),
  )
}

/** Every sketch a value refers to through a `sketch` key (a ProfileRef, an AxisRef, a hole's sketch). */
export function referencedSketches(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.flatMap(referencedSketches)
  }
  if (typeof value !== 'object' || value === null) {
    return []
  }
  return Object.entries(value).flatMap(([key, entry]) =>
    key === 'sketch' && typeof entry === 'string' ? [entry] : referencedSketches(entry),
  )
}

/** Whether a sketch starts visible: no other feature uses it (3.10). */
export function sketchStartsVisible(features: readonly FeatureRecord[], sketchId: string): boolean {
  return !features.some((other) => other.id !== sketchId && referencesSketch(other, sketchId))
}

/**
 * Session visibility for a newly opened recipe, or for a document whose recipe has not arrived yet
 * (null): sketches that a feature uses start hidden (3.10). Origin items get no entry, so they
 * follow the document (`selectOriginVisible`) until their eye button or V writes one.
 */
export function initialSessionVisibility(
  recipe: Document | null,
): Readonly<Record<string, boolean>> {
  const features = recipe?.features ?? []
  const visibility: Record<string, boolean> = {}
  for (const feature of features) {
    if (feature.type === 'sketch') {
      visibility[`feature:${feature.id}`] = sketchStartsVisible(features, feature.id)
    } else if (feature.type === 'datum_plane') {
      visibility[`feature:${feature.id}`] = true
    }
  }
  return visibility
}

/**
 * Whether a sketch, datum plane or origin item is shown this session (3.10); without an entry, the
 * opening rule of 3.10 applies.
 */
export function isSessionVisible(state: AppState, key: string): boolean {
  const stored = state.view.sessionVisible[key]
  if (stored !== undefined) {
    return stored
  }
  if (key.startsWith('origin:')) {
    return selectOriginVisible(state, key)
  }
  const id = key.slice('feature:'.length)
  const features = state.document?.recipe.features ?? []
  const feature = features.find((candidate) => candidate.id === id)
  if (feature?.type !== 'sketch') {
    return true
  }
  return sketchStartsVisible(features, id)
}
