import type { Document } from '../generated/protocol'

// Short random ids (model.md 3.2, ADR 0008).

export type IdPrefix = 'p' | 'b' | 'f' | 'e' | 'c'

const MAXIMUM_ATTEMPTS = 64
const RANDOM_BYTES = 6

/** 48 random bits as an integer in [0, 2^48). */
function randomBits(): number {
  const bytes = crypto.getRandomValues(new Uint8Array(RANDOM_BYTES))
  return bytes.reduce((value, byte) => value * 256 + byte, 0)
}

/** model.md 3.2: prefix, '-', 12 lowercase hex digits from 48 random bits; retries while taken. */
export function generateId(
  prefix: IdPrefix,
  isTaken: (id: string) => boolean,
  random: () => number = randomBits,
): string {
  for (let attempt = 0; attempt < MAXIMUM_ATTEMPTS; attempt += 1) {
    const digits = random()
      .toString(16)
      .padStart(RANDOM_BYTES * 2, '0')
    const id = `${prefix}-${digits}`
    if (!isTaken(id)) {
      return id
    }
  }
  throw new Error(`Could not generate a free id after ${String(MAXIMUM_ATTEMPTS)} attempts.`)
}

function idsOf(records: unknown): string[] {
  if (!Array.isArray(records)) {
    return []
  }
  return records.flatMap((record: unknown) => {
    const id: unknown =
      typeof record === 'object' && record !== null ? Reflect.get(record, 'id') : undefined
    return typeof id === 'string' ? [id] : []
  })
}

/** Every id in use: parameter, body and feature ids plus sketch entity and constraint ids. */
export function usedIds(document: Document): Set<string> {
  const ids = new Set<string>([
    ...idsOf(document.parameters),
    ...idsOf(document.bodies),
    ...idsOf(document.features),
  ])
  for (const feature of document.features ?? []) {
    if (feature.type === 'sketch') {
      for (const id of [...idsOf(feature['entities']), ...idsOf(feature['constraints'])]) {
        ids.add(id)
      }
    }
  }
  return ids
}
