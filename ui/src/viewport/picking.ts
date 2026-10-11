/**
 * What a pick target or selection item is (ui.md 2.8). The store's selection items use this kind; it lives here so picking does not depend on the store.
 */
export type SelectionItemKind =
  | 'face'
  | 'edge'
  | 'vertex'
  | 'body'
  | 'feature'
  | 'parameter'
  | 'sketch_region'
  | 'sketch_curve'
  | 'sketch_point'
  | 'sketch_constraint'
  | 'origin_plane'
  | 'origin_axis'
  | 'origin_point'
  | 'datum_plane'

/** RGBA8 encoding of a 32-bit pick id: red = bits 0-7, green = 8-15, blue = 16-23, alpha = 24-31. 0 means nothing. */
export function encodePickId(id: number): readonly [number, number, number, number] {
  const unsigned = id >>> 0
  return [unsigned & 0xff, (unsigned >>> 8) & 0xff, (unsigned >>> 16) & 0xff, unsigned >>> 24]
}

export function decodePickId(red: number, green: number, blue: number, alpha: number): number {
  return (red | (green << 8) | (blue << 16) | (alpha << 24)) >>> 0
}

export type PickClass = 'point' | 'curve' | 'area'
export type PickCandidate = {
  readonly key: string
  readonly kind: SelectionItemKind
  readonly pickClass: PickClass
  readonly distance: number
}

const ORIGIN_KINDS: ReadonlySet<SelectionItemKind> = new Set([
  'origin_plane',
  'origin_axis',
  'origin_point',
])

// Whether `candidate` beats `best` within one pick class: the smaller distance, then anything over origin items. Exact ties keep the earlier candidate, so callers list candidates in ascending pick id order.
function beats(candidate: PickCandidate, best: PickCandidate): boolean {
  if (candidate.distance !== best.distance) {
    return candidate.distance < best.distance
  }
  return ORIGIN_KINDS.has(best.kind) && !ORIGIN_KINDS.has(candidate.kind)
}

/**
 * The candidate the pointer means (section 3.6), or null: accepted `point` candidates within `radius` CSS pixels win over `curve` candidates within the radius, which win over `area` candidates at distance 0. Within a class the smallest distance wins; at equal distance, active sketch items and model elements win over origin items, then the earlier candidate (the smaller pick id) wins.
 */
export function choosePick(
  candidates: readonly PickCandidate[],
  accepts: (kind: SelectionItemKind) => boolean,
  radius: number,
): PickCandidate | null {
  const best: Partial<Record<PickClass, PickCandidate>> = {}
  for (const candidate of candidates) {
    const inReach =
      candidate.pickClass === 'area' ? candidate.distance === 0 : candidate.distance <= radius
    if (!inReach || !accepts(candidate.kind)) {
      continue
    }
    const current = best[candidate.pickClass]
    if (current === undefined || beats(candidate, current)) {
      best[candidate.pickClass] = candidate
    }
  }
  return best.point ?? best.curve ?? best.area ?? null
}
