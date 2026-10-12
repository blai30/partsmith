import type { Document } from '../generated/protocol'
import type { Catalog } from '../state/store'

// Default display labels (features.md 3.12, model.md).

/** features.md 3.12: the record's name, else "<manifest label> <n>" where n counts records of that type up to and including it. */
export function featureLabel(document: Document, featureId: string, catalog: Catalog | null): string {
  const features = document.features ?? []
  const index = features.findIndex((feature) => feature.id === featureId)
  const feature = features[index]
  if (feature === undefined) {
    return featureId
  }
  const name = feature.name ?? null
  if (name !== null && name !== '') {
    return name
  }
  const count = features
    .slice(0, index + 1)
    .filter((candidate) => candidate.type === feature.type).length
  const base = catalog?.byType[feature.type]?.manifest.label ?? feature.type
  return `${base} ${String(count)}`
}

/** "Body n" with the smallest n ≥ 1 not used by a body name. */
export function nextBodyName(document: Document): string {
  const names = new Set((document.bodies ?? []).map((body) => body.name))
  let number = 1
  while (names.has(`Body ${String(number)}`)) {
    number += 1
  }
  return `Body ${String(number)}`
}
