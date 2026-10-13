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
