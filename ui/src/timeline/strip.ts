import type { FeatureRecord, FeatureStatus } from '../generated/protocol'

// The timeline strip's arithmetic (ui.md 3.9): the rollback marker's position and wording, item
// status wording, and the gaps a drag drops into.

/** The rollback marker position p: features 1 to p are active (3.9). */
export function markerPosition(records: readonly FeatureRecord[], rollback: string | null): number {
  const index = rollback === null ? -1 : records.findIndex((record) => record.id === rollback)
  return index === -1 ? records.length : index
}

/** The status the item shows: the latest recomputed one, or pending before it (3.9). */
export function itemStatus(status: FeatureStatus | undefined): FeatureStatus['status'] | 'pending' {
  return status?.status ?? 'pending'
}

/** The hidden description of an item's status, or null for ok and pending. */
export function statusText(status: FeatureStatus | undefined, suppressed: boolean): string | null {
  const first = status?.messages[0]?.message
  switch (status?.status) {
    case 'error':
      return first === undefined ? 'Error' : `Error: ${first}`
    case 'warning':
      return first === undefined ? 'Warning' : `Warning: ${first}`
    case 'suppressed':
      return 'Suppressed'
    case 'rolled_back':
      return 'Rolled back'
    default:
      return suppressed ? 'Suppressed' : null
  }
}

/** The x positions (client CSS pixels) of the gaps 0 to N around the strip's items. */
export function gapPositions(strip: HTMLElement): readonly number[] {
  const rectangles = [...strip.querySelectorAll<HTMLElement>('[data-testid="timeline-item"]')].map(
    (item) => item.getBoundingClientRect(),
  )
  const [first] = rectangles
  const last = rectangles.at(-1)
  if (first === undefined || last === undefined) {
    return [strip.getBoundingClientRect().left]
  }
  const second = rectangles[1]
  const spacing = second === undefined ? 0 : second.left - first.right
  const gaps = [first.left - spacing / 2]
  for (let i = 1; i < rectangles.length; i++) {
    const before = rectangles[i - 1]
    const after = rectangles[i]
    if (before !== undefined && after !== undefined) {
      gaps.push((before.right + after.left) / 2)
    }
  }
  gaps.push(last.right + spacing / 2)
  return gaps
}

/** The gap nearest to a client x position. */
export function nearestGap(gaps: readonly number[], x: number): number {
  let best = 0
  gaps.forEach((gap, index) => {
    if (Math.abs(gap - x) < Math.abs((gaps[best] ?? gap) - x)) {
      best = index
    }
  })
  return best
}

/** The marker's spoken position: "At the start", "After {label}" or "At the end" (3.9). */
export function markerText(
  records: readonly FeatureRecord[],
  position: number,
  labelOf: (id: string) => string,
): string {
  const record = records[position - 1]
  if (position >= records.length) {
    return 'At the end'
  }
  return record === undefined ? 'At the start' : `After ${labelOf(record.id)}`
}
