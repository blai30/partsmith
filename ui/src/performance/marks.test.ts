import { describe, expect, it } from 'vitest'
import {
  MAXIMUM_MEASURES,
  markTiming,
  measureTiming,
  TIMING_NAMES,
  type TimingTarget,
} from './marks'

type Entry = {
  readonly name: string
  readonly entryType: string
  readonly startTime: number
  readonly duration: number
}

// A User Timing timeline that keeps entries in memory, with a clock the test sets.
function fakeTimeline(now = 0): TimingTarget & { readonly entries: Entry[]; time: number } {
  const entries: Entry[] = []
  const timeline = {
    entries,
    time: now,
    now: () => timeline.time,
    mark: (name: string) => {
      const entry = { name, entryType: 'mark', startTime: timeline.time, duration: 0 }
      entries.push(entry)
      return entry as unknown as PerformanceMark
    },
    measure: (name: string, options?: string | PerformanceMeasureOptions) => {
      const range = typeof options === 'object' ? options : {}
      const start = Number(range.start ?? 0)
      const duration = range.duration ?? Number(range.end ?? timeline.time) - start
      const entry = { name, entryType: 'measure', startTime: start, duration }
      entries.push(entry)
      return entry as unknown as PerformanceMeasure
    },
    getEntriesByName: (name: string, type?: string) =>
      entries.filter(
        (entry) => entry.name === name && (type === undefined || entry.entryType === type),
      ) as unknown as PerformanceEntryList,
    clearMeasures: (name?: string) => {
      const kept = entries.filter(
        (entry) => entry.entryType !== 'measure' || (name !== undefined && entry.name !== name),
      )
      entries.splice(0, entries.length, ...kept)
    },
  }
  return timeline
}

function measures(timeline: { readonly entries: readonly Entry[] }, name: string): Entry[] {
  return timeline.entries.filter((entry) => entry.entryType === 'measure' && entry.name === name)
}

describe('TIMING_NAMES', () => {
  it('names every timing of ui.md 2.18', () => {
    expect(Object.values(TIMING_NAMES).toSorted()).toEqual(
      [
        'ps:frame',
        'ps:open',
        'ps:palette',
        'ps:pick',
        'ps:preview',
        'ps:rebuild',
        'ps:sketch-drag',
        'ps:usable',
      ].toSorted(),
    )
  })
})

describe('markTiming', () => {
  it('records a mark at the current time', () => {
    const timeline = fakeTimeline(1234)
    markTiming(TIMING_NAMES.usable, timeline)
    expect(timeline.entries).toEqual([
      { name: 'ps:usable', entryType: 'mark', startTime: 1234, duration: 0 },
    ])
  })
})

describe('measureTiming', () => {
  it('records a measure from start to end', () => {
    const timeline = fakeTimeline()
    measureTiming(TIMING_NAMES.pick, 100, 112.5, timeline)
    expect(measures(timeline, 'ps:pick')).toEqual([
      { name: 'ps:pick', entryType: 'measure', startTime: 100, duration: 12.5 },
    ])
  })

  it('ends at the current time when no end is given', () => {
    const timeline = fakeTimeline(250)
    measureTiming(TIMING_NAMES.rebuild, 200, undefined, timeline)
    expect(measures(timeline, 'ps:rebuild')[0]?.duration).toBe(50)
  })

  it('keeps at most the last 500 entries of a name, dropping the oldest', () => {
    const timeline = fakeTimeline()
    for (let i = 0; i < MAXIMUM_MEASURES + 1; i++) {
      measureTiming(TIMING_NAMES.frame, i, i + 1, timeline)
    }
    const frames = measures(timeline, 'ps:frame')
    expect(frames.length).toBeLessThanOrEqual(MAXIMUM_MEASURES)
    expect(frames.at(-1)?.startTime).toBe(MAXIMUM_MEASURES)
    expect(frames.some((entry) => entry.startTime === 0)).toBe(false)
  })

  it('never holds more than 500 entries of a name over many measures', () => {
    const timeline = fakeTimeline()
    for (let i = 0; i < 3 * MAXIMUM_MEASURES; i++) {
      measureTiming(TIMING_NAMES.frame, i, i + 1, timeline)
      expect(measures(timeline, 'ps:frame').length).toBeLessThanOrEqual(MAXIMUM_MEASURES)
    }
    const frames = measures(timeline, 'ps:frame')
    // The kept entries are the latest ones, in order.
    expect(frames.map((entry) => entry.startTime)).toEqual(
      Array.from(
        { length: frames.length },
        (_, index) => 3 * MAXIMUM_MEASURES - frames.length + index,
      ),
    )
  })

  it('leaves the entries of other names alone when it trims one name', () => {
    const timeline = fakeTimeline()
    measureTiming(TIMING_NAMES.pick, 0, 1, timeline)
    for (let i = 0; i < MAXIMUM_MEASURES + 1; i++) {
      measureTiming(TIMING_NAMES.frame, i, i + 1, timeline)
    }
    expect(measures(timeline, 'ps:pick')).toHaveLength(1)
  })
})
