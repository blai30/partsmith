import { describe, expect, it } from 'vitest'
import type { FeatureRecord, FeatureStatus } from '../generated/protocol'
import { gapsBetween, itemStatus, markerText, nearestGap, statusText } from './strip'

function records(...ids: readonly string[]): FeatureRecord[] {
  return ids.map((id) => ({ id, type: 'box' }))
}

function status(state: FeatureStatus['status'], message?: string): FeatureStatus {
  return {
    id: 'f-1',
    status: state,
    messages: message === undefined ? [] : [{ severity: 'warning', code: 'no_effect', message }],
    repairs: [],
  }
}

function labels(id: string): string {
  return `Label of ${id}`
}

describe('markerText', () => {
  it('reads "At the start" before the first feature', () => {
    expect(markerText(records('f-1', 'f-2'), 0, labels)).toBe('At the start')
  })

  it('reads "After {label}" of the last active feature', () => {
    expect(markerText(records('f-1', 'f-2'), 1, labels)).toBe('After Label of f-1')
  })

  it('reads "At the end" after the last feature', () => {
    expect(markerText(records('f-1', 'f-2'), 2, labels)).toBe('At the end')
  })
})

describe('item status', () => {
  it('is pending before the first recomputed status', () => {
    expect(itemStatus(undefined)).toBe('pending')
    expect(itemStatus(status('warning'))).toBe('warning')
  })

  it('describes errors and warnings with their first message', () => {
    expect(statusText(status('error', 'It broke.'), false)).toBe('Error: It broke.')
    expect(statusText(status('warning', 'Careful.'), false)).toBe('Warning: Careful.')
  })

  it('describes suppressed and rolled-back features, and nothing for ok ones', () => {
    expect(statusText(status('suppressed'), true)).toBe('Suppressed')
    expect(statusText(status('rolled_back'), false)).toBe('Rolled back')
    expect(statusText(status('ok'), false)).toBeNull()
    expect(statusText(undefined, false)).toBeNull()
  })

  it('describes a suppressed feature before its status arrives', () => {
    expect(statusText(undefined, true)).toBe('Suppressed')
  })
})

describe('gaps', () => {
  it('lie halfway between neighbors and half a spacing outside the ends', () => {
    const items = [
      { left: 100, right: 140 },
      { left: 144, right: 184 },
      { left: 188, right: 228 },
    ]
    expect(gapsBetween(items, 0)).toEqual([98, 142, 186, 230])
  })

  it('hug a single item, and sit at the start without items', () => {
    expect(gapsBetween([{ left: 10, right: 50 }], 0)).toEqual([10, 50])
    expect(gapsBetween([], 24)).toEqual([24])
  })

  it('choose the gap nearest to the pointer, the earlier one on a tie', () => {
    const gaps = [98, 142, 186, 230]
    expect(nearestGap(gaps, 0)).toBe(0)
    expect(nearestGap(gaps, 150)).toBe(1)
    expect(nearestGap(gaps, 164)).toBe(1)
    expect(nearestGap(gaps, 400)).toBe(3)
  })
})
