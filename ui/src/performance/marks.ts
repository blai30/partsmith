// User Timing entries the performance tests read (ui.md 2.18). They are cheap and stay in production.

export const TIMING_NAMES = {
  usable: 'ps:usable',
  frame: 'ps:frame',
  pick: 'ps:pick',
  sketchDrag: 'ps:sketch-drag',
  preview: 'ps:preview',
  rebuild: 'ps:rebuild',
  open: 'ps:open',
  palette: 'ps:palette',
  dropCheck: 'ps:drop-check',
  applyEvents: 'ps:apply-events',
  expression: 'ps:expression',
} as const

export type TimingName = (typeof TIMING_NAMES)[keyof typeof TIMING_NAMES]

/** The one timing recorded as a mark; every other timing is a measure. */
export type MarkName = typeof TIMING_NAMES.usable
export type MeasureName = Exclude<TimingName, MarkName>

/** The most entries one measure name keeps. */
export const MAXIMUM_MEASURES = 500

/** Entries kept when a name reaches the maximum, so trimming happens once per hundred measures. */
const KEPT_AFTER_TRIM = MAXIMUM_MEASURES - 100

/** The User Timing part of `Performance` these functions use. */
export type TimingTarget = Pick<
  Performance,
  'now' | 'mark' | 'measure' | 'getEntriesByName' | 'clearMeasures'
>

/** Marks a moment, such as `ps:usable` when the first frame with the startup document renders. */
export function markTiming(name: MarkName, timeline: TimingTarget = performance): void {
  timeline.mark(name)
}

/**
 * Records a measure from `start` to `end` (both `performance.now()` times; `end` defaults to now).
 * A name keeps at most the latest 500 entries: when it has 500, `clearMeasures` drops them and
 * the latest 400 are recorded again.
 */
export function measureTiming(
  name: MeasureName,
  start: number,
  end?: number,
  timeline: TimingTarget = performance,
): void {
  const finish = end ?? timeline.now()
  const entries = timeline.getEntriesByName(name, 'measure')
  if (entries.length >= MAXIMUM_MEASURES) {
    timeline.clearMeasures(name)
    for (const entry of entries.slice(-KEPT_AFTER_TRIM)) {
      timeline.measure(name, { start: entry.startTime, duration: entry.duration })
    }
  }
  timeline.measure(name, { start, end: finish })
}
