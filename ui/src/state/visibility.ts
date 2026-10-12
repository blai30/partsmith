import type { Document } from '../generated/protocol'

// Session-only visibility of sketches, datum planes and origin items (ui.md 2.8 ViewState, 3.10).

const ORIGIN_KEYS = ['XY', 'XZ', 'YZ', 'X', 'Y', 'Z', 'O'].map((name) => `origin:${name}`)

/** Whether some feature of the recipe refers to the sketch (a `sketch` key anywhere in its params). */
function referencesSketch(value: unknown, sketchId: string): boolean {
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

/**
 * Session visibility for a newly opened recipe, or for a document whose recipe has not arrived yet
 * (null): sketches that a feature uses and origin items start hidden (3.10).
 */
export function initialSessionVisibility(recipe: Document | null): Readonly<Record<string, boolean>> {
  const features = recipe?.features ?? []
  const visibility: Record<string, boolean> = {}
  for (const feature of features) {
    if (feature.type === 'sketch') {
      visibility[`feature:${feature.id}`] = !features.some(
        (other) => other.id !== feature.id && referencesSketch(other, feature.id),
      )
    } else if (feature.type === 'datum_plane') {
      visibility[`feature:${feature.id}`] = true
    }
  }
  for (const key of ORIGIN_KEYS) {
    visibility[key] = false
  }
  return visibility
}
