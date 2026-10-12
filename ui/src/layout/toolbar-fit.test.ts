import { describe, expect, it } from 'vitest'
import {
  EMPTY_MEMORY,
  FULL_FIT,
  nextFit,
  type FitMeasurement,
  type FitMemory,
  type ToolbarFit,
} from './toolbar-fit'

// The toolbar width rules of ui.md 3.2, as a pure step from one measurement to the next fit.

/** A toolbar whose movable buttons are 40 px wide with their gaps, beside 100 px of fixed buttons. */
const MOVABLE = [40, 40, 40, 40, 40]
const ICONS = 100 + MOVABLE.length * 40
const MORE = 40

function measured(available: number, needed: number, movable: readonly number[] | null = null) {
  return { available, needed, movable, more: MORE } satisfies FitMeasurement
}

/** Measures with each fit what a toolbar of these widths takes, until the fit stops changing. */
function settle(available: number, start: ToolbarFit = FULL_FIT, memory: FitMemory = EMPTY_MEMORY) {
  let fit = start
  let learned = memory
  for (let pass = 0; pass < 20; pass++) {
    const kept = fit.kept ?? MOVABLE.length
    const width =
      fit.labels === 'all'
        ? 900
        : fit.labels === 'primary'
          ? 600
          : 100 + kept * 40 + (fit.kept === null ? 0 : MORE)
    const next = nextFit(
      fit,
      learned,
      measured(available, width, fit.labels === 'none' && fit.kept === null ? MOVABLE : null),
    )
    if (next.fit === fit) {
      return { fit, memory: next.memory }
    }
    fit = next.fit
    learned = next.memory
  }
  throw new Error('The toolbar never settled.')
}

describe('nextFit (3.2)', () => {
  it('keeps every label when the labeled toolbar fits', () => {
    expect(settle(1000).fit).toEqual(FULL_FIT)
  })

  it('keeps only the primary labels when the full labels do not fit but those do', () => {
    expect(settle(700).fit).toEqual({ labels: 'primary', kept: null })
  })

  it('shows icons only when the primary labels do not fit either', () => {
    expect(settle(300).fit).toEqual({ labels: 'none', kept: null })
  })

  it('moves the last movable buttons into More tools when even the icons do not fit', () => {
    // 100 fixed + 2 × 40 kept + 40 More = 220.
    expect(settle(230).fit).toEqual({ labels: 'none', kept: 2 })
  })

  it('keeps no movable button when only the fixed ones and More tools fit', () => {
    expect(settle(120).fit).toEqual({ labels: 'none', kept: 0 })
  })

  it('widens again step by step once the toolbar has room', () => {
    const narrow = settle(230)
    expect(settle(1000, narrow.fit, narrow.memory).fit).toEqual(FULL_FIT)
    expect(settle(700, narrow.fit, narrow.memory).fit).toEqual({ labels: 'primary', kept: null })
    expect(settle(300, narrow.fit, narrow.memory).fit).toEqual({ labels: 'none', kept: null })
    expect(settle(270, narrow.fit, narrow.memory).fit).toEqual({ labels: 'none', kept: 3 })
  })

  it('a count that measured too wide is not tried again at the same width', () => {
    const fit: ToolbarFit = { labels: 'none', kept: 3 }
    const memory: FitMemory = {
      ...EMPTY_MEMORY,
      none: ICONS,
      movable: MOVABLE,
      tooWide: { kept: 4, available: 280 },
    }
    expect(nextFit(fit, memory, measured(280, 260)).fit).toBe(fit)
    expect(nextFit(fit, memory, measured(400, 260)).fit).toEqual({ labels: 'none', kept: null })
  })
})
