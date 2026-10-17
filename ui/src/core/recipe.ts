import type { Document, FeatureRecord } from '../generated/protocol'

// Reading a document recipe's timeline (ui.md 3.9).

/** The rollback marker position p: features 1 to p are active (3.9). */
export function markerPosition(records: readonly FeatureRecord[], rollback: string | null): number {
  const index = rollback === null ? -1 : records.findIndex((record) => record.id === rollback)
  return index === -1 ? records.length : index
}

/** The features before the rollback marker, in timeline order. */
export function activeFeatures(recipe: Document): readonly FeatureRecord[] {
  return (recipe.features ?? []).slice(
    0,
    markerPosition(recipe.features ?? [], recipe.rollback ?? null),
  )
}

/** The record of the sketch with this id, or null when the recipe has no sketch with it. */
export function sketchRecord(recipe: Document, sketchId: string): FeatureRecord | null {
  const record = recipe.features?.find((feature) => feature.id === sketchId)
  return record?.type === 'sketch' ? record : null
}

/** Whether feature `later` comes after feature `earlier` in the timeline; false when one is missing. */
export function comesAfter(recipe: Document, later: string, earlier: string): boolean {
  const features = recipe.features ?? []
  const position = (id: string): number => features.findIndex((feature) => feature.id === id)
  const earlierPosition = position(earlier)
  return earlierPosition !== -1 && position(later) > earlierPosition
}
