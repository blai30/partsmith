// How much of a context toolbar fits the space the top bar leaves it (ui.md 3.2): labels on every
// button; else labels on the primary buttons only; else the primary labels with the last movable
// buttons in the More tools menu; and only when a labeled primary button does not fit beside More
// tools, no labels, with as many movable buttons as then fit.

/** Which buttons show their label. */
type LabelLevel = 'all' | 'primary' | 'none'

export type ToolbarFit = {
  readonly labels: LabelLevel
  /** How many of the movable buttons stay in the toolbar, in order; null keeps them all. */
  readonly kept: number | null
}

/** Every button shown, with its label: where a toolbar starts, and starts again when its content changes. */
export const FULL_FIT: ToolbarFit = { labels: 'all', kept: null }

/** Widths in CSS pixels learned from the measurements of one toolbar content. */
export type FitMemory = {
  /** The width with every button labeled. */
  readonly all: number | null
  /** The width of everything but the movable buttons, with the primary buttons labeled. */
  readonly primary: number | null
  /** The width of everything but the movable buttons, with no labels. */
  readonly none: number | null
  /** Icon-only widths of the movable buttons, each with the gap after it, in order. */
  readonly movable: readonly number[] | null
  /** The last fit that measured too wide, and the width it had then. */
  readonly tooWide: { readonly fit: ToolbarFit; readonly available: number } | null
}

export const EMPTY_MEMORY: FitMemory = {
  all: null,
  primary: null,
  none: null,
  movable: null,
  tooWide: null,
}

/** One look at the toolbar as it is drawn now. */
export type FitMeasurement = {
  /** The toolbar's inner width. */
  readonly available: number
  /** The width its buttons take now. */
  readonly needed: number
  /** Icon-only widths of the movable buttons with their gaps, when every one is drawn icon only. */
  readonly movable: readonly number[] | null
  /** The width of the More tools button with its gap. */
  readonly more: number
}

/** The sum of the first `count` widths. */
function sumOf(widths: readonly number[], count: number): number {
  return widths.slice(0, count).reduce((sum, width) => sum + width, 0)
}

/**
 * Every fit from the widest to the narrowest: all labels; primary labels with every movable
 * button, then with one fewer down to none; no labels likewise.
 */
function ladder(count: number): readonly ToolbarFit[] {
  const fewer = (labels: LabelLevel): ToolbarFit[] =>
    Array.from({ length: count }, (_, index) => ({ labels, kept: count - 1 - index }))
  return [
    FULL_FIT,
    { labels: 'primary', kept: null },
    ...fewer('primary'),
    { labels: 'none', kept: null },
    ...fewer('none'),
  ]
}

function sameFit(first: ToolbarFit, second: ToolbarFit): boolean {
  return first.labels === second.labels && first.kept === second.kept
}

/** What a measurement of `fit` teaches: the widths it shows, apart from the movable buttons. */
function learn(fit: ToolbarFit, memory: FitMemory, measurement: FitMeasurement): FitMemory {
  const { needed, more } = measurement
  if (fit.labels === 'all') {
    return fit.kept === null ? { ...memory, all: needed } : memory
  }
  const movable =
    fit.kept === null && measurement.movable !== null ? measurement.movable : memory.movable
  if (movable === null) {
    return memory
  }
  const fixed =
    fit.kept === null
      ? needed - sumOf(movable, movable.length)
      : needed - sumOf(movable, fit.kept) - more
  return { ...memory, movable, [fit.labels]: fixed }
}

/** The width a fit takes by the learned widths, or null when they do not tell. */
function widthOf(fit: ToolbarFit, memory: FitMemory, more: number): number | null {
  if (fit.labels === 'all') {
    return fit.kept === null ? memory.all : null
  }
  const fixed = memory[fit.labels]
  if (fixed === null || memory.movable === null) {
    return null
  }
  return fit.kept === null
    ? fixed + sumOf(memory.movable, memory.movable.length)
    : fixed + sumOf(memory.movable, fit.kept) + more
}

/**
 * The next fit after a measurement, and what it taught. A toolbar that is too wide narrows to the
 * widest narrower fit its widths say fits, or to the next fit when they cannot tell, so it is
 * measured; one that fits widens to the widest fit its widths say fits. A fit that measured too
 * wide is not tried again until the toolbar is wider than it was then, so the fit settles. Each
 * step shows at most one frame, before the browser paints.
 */
export function nextFit(
  fit: ToolbarFit,
  memory: FitMemory,
  measurement: FitMeasurement,
): { readonly fit: ToolbarFit; readonly memory: FitMemory } {
  const { available, needed, more } = measurement
  const learned = learn(fit, memory, measurement)
  const steps = ladder(learned.movable?.length ?? 0)
  const current = steps.findIndex((step) => sameFit(step, fit))
  if (current === -1) {
    return { fit: FULL_FIT, memory: learned }
  }
  if (needed > available) {
    const remembered: FitMemory = { ...learned, tooWide: { fit, available } }
    for (const step of steps.slice(current + 1)) {
      const width = widthOf(step, remembered, more)
      if (width === null || width <= available) {
        return { fit: step, memory: remembered }
      }
    }
    const narrowest = steps.at(-1) ?? fit
    return { fit: sameFit(narrowest, fit) ? fit : narrowest, memory: remembered }
  }
  // Fits at or above one that measured too wide at this width or more stay untried.
  const tooWide = learned.tooWide
  const limit =
    tooWide !== null && available <= tooWide.available
      ? steps.findIndex((step) => sameFit(step, tooWide.fit)) + 1
      : 0
  const wider = steps
    .slice(limit, current)
    .find((step) => (widthOf(step, learned, more) ?? Number.POSITIVE_INFINITY) <= available)
  return { fit: wider ?? fit, memory: learned }
}
