// How much of a context toolbar fits the space the top bar leaves it (ui.md 3.2): labels on every
// button, labels on the primary buttons only, or icons only; and when even the icons do not fit,
// how many of the buttons that may move stay, the rest going into the More tools menu.

/** Which buttons show their label. */
export type LabelLevel = 'all' | 'primary' | 'none'

export type ToolbarFit = {
  readonly labels: LabelLevel
  /** How many of the movable buttons stay in the toolbar, in order; null keeps them all. */
  readonly kept: number | null
}

/** Every button shown, with its label: where a toolbar starts, and starts again when its content changes. */
export const FULL_FIT: ToolbarFit = { labels: 'all', kept: null }

/** Content widths in CSS pixels, each recorded while its label level showed every button. */
export type FitMemory = {
  readonly all: number | null
  readonly primary: number | null
  readonly none: number | null
  /** Icon-only widths of the movable buttons, each with the gap after it, in order. */
  readonly movable: readonly number[] | null
  /** The last count of kept buttons that measured too wide, and the width it had then. */
  readonly tooWide: { readonly kept: number; readonly available: number } | null
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

const NARROWER: Readonly<Record<LabelLevel, LabelLevel | null>> = {
  all: 'primary',
  primary: 'none',
  none: null,
}

const WIDER: Readonly<Record<LabelLevel, LabelLevel | null>> = {
  all: null,
  primary: 'all',
  none: 'primary',
}

/** The most movable buttons that fit beside the others and the More tools button, or null without the widths to tell. */
function mostKept(memory: FitMemory, available: number, more: number): number | null {
  const { none: iconsWidth, movable } = memory
  if (iconsWidth === null || movable === null) {
    return null
  }
  let width = iconsWidth + more
  for (let kept = movable.length; kept > 0; kept--) {
    if (width <= available) {
      return kept
    }
    width -= movable[kept - 1] ?? 0
  }
  return 0
}

/**
 * The next fit after a measurement, and what it taught: a toolbar that is too wide narrows by one
 * step (labels, then primary labels, then buttons into More tools); one that fits widens again
 * once the width a wider step needed is available. Each step shows at most one frame, before the
 * browser paints.
 */
export function nextFit(
  fit: ToolbarFit,
  memory: FitMemory,
  measurement: FitMeasurement,
): { readonly fit: ToolbarFit; readonly memory: FitMemory } {
  const { available, needed } = measurement
  const learned: FitMemory =
    fit.kept === null
      ? {
          ...memory,
          [fit.labels]: needed,
          ...(fit.labels === 'none' ? { movable: measurement.movable } : {}),
        }
      : memory
  if (needed > available) {
    const narrower = NARROWER[fit.labels]
    if (narrower !== null) {
      return { fit: { labels: narrower, kept: null }, memory: learned }
    }
    const current = fit.kept ?? learned.movable?.length ?? 0
    const fitting = mostKept(learned, available, measurement.more)
    // Each pass moves at least one more button, so a toolbar wider than its widths promised
    // still settles, and that count is not tried again at this width.
    const kept = Math.max(0, Math.min(current - 1, fitting ?? current - 1))
    const remembered: FitMemory = { ...learned, tooWide: { kept: current, available } }
    return kept === fit.kept
      ? { fit, memory: remembered }
      : { fit: { labels: 'none', kept }, memory: remembered }
  }
  if (fit.kept !== null) {
    if (learned.none !== null && learned.none <= available) {
      return { fit: { labels: 'none', kept: null }, memory: learned }
    }
    const fitting = mostKept(learned, available, measurement.more)
    const tooWide = learned.tooWide
    const limit =
      tooWide !== null && available <= tooWide.available
        ? tooWide.kept - 1
        : Number.POSITIVE_INFINITY
    const kept = fitting === null ? null : Math.min(fitting, limit)
    return kept !== null && kept > fit.kept
      ? { fit: { labels: 'none', kept }, memory: learned }
      : { fit, memory: learned }
  }
  const wider = WIDER[fit.labels]
  const widerNeeds = wider === null ? null : learned[wider]
  return wider !== null && widerNeeds !== null && widerNeeds <= available
    ? { fit: { labels: wider, kept: null }, memory: learned }
    : { fit, memory: learned }
}
