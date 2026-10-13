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

/**
 * A synthetic toolbar: 900 px with every label; else a primary button with its separator, 140 px
 * labeled or 60 px as an icon, beside 5 movable icon buttons of 40 px with their gaps; More tools
 * is 40 px.
 */
const LABELED = 900
const MOVABLE = [40, 40, 40, 40, 40]
const MORE = 40

type Toolbar = { readonly primary: number; readonly none: number }

const WITH_PRIMARY: Toolbar = { primary: 140, none: 60 }

/** What a toolbar takes with a fit. */
function widthWith(toolbar: Toolbar, fit: ToolbarFit): number {
  if (fit.labels === 'all') {
    return LABELED
  }
  const kept = fit.kept ?? MOVABLE.length
  return toolbar[fit.labels] + kept * 40 + (fit.kept === null ? 0 : MORE)
}

function measured(available: number, needed: number, movable: readonly number[] | null = null) {
  return { available, needed, movable, more: MORE } satisfies FitMeasurement
}

/** Measures with each fit what the toolbar takes, until the fit stops changing. */
function settle(
  available: number,
  start: ToolbarFit = FULL_FIT,
  memory: FitMemory = EMPTY_MEMORY,
  toolbar: Toolbar = WITH_PRIMARY,
) {
  let fit = start
  let learned = memory
  for (let pass = 0; pass < 30; pass++) {
    const iconsShown = fit.labels !== 'all' && fit.kept === null
    const next = nextFit(
      fit,
      learned,
      measured(available, widthWith(toolbar, fit), iconsShown ? MOVABLE : null),
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
    // 140 + 5 × 40 = 340.
    expect(settle(700).fit).toEqual({ labels: 'primary', kept: null })
  })

  it('keeps the primary label and moves the last buttons into More tools when those do not fit', () => {
    // 140 + 3 × 40 + 40 = 300, although every icon would fit without the label (60 + 200 = 260).
    expect(settle(300).fit).toEqual({ labels: 'primary', kept: 3 })
  })

  it('keeps the primary label beside More tools with every movable button moved', () => {
    expect(settle(180).fit).toEqual({ labels: 'primary', kept: 0 })
  })

  it('drops the primary label only when it does not fit beside More tools', () => {
    // 60 + 1 × 40 + 40 = 140.
    expect(settle(170).fit).toEqual({ labels: 'none', kept: 1 })
  })

  it('keeps no movable button when only the fixed ones and More tools fit', () => {
    expect(settle(90).fit).toEqual({ labels: 'none', kept: 0 })
  })

  it('widens again once the toolbar has room', () => {
    const narrow = settle(170)
    expect(settle(1000, narrow.fit, narrow.memory).fit).toEqual(FULL_FIT)
    expect(settle(700, narrow.fit, narrow.memory).fit).toEqual({ labels: 'primary', kept: null })
    expect(settle(300, narrow.fit, narrow.memory).fit).toEqual({ labels: 'primary', kept: 3 })
    expect(settle(240, narrow.fit, narrow.memory).fit).toEqual({ labels: 'primary', kept: 1 })
  })

  it('moves buttons into More tools in a toolbar without primary buttons', () => {
    // Icons only either way: 2 × 40 + 40 = 120.
    expect(settle(120, FULL_FIT, EMPTY_MEMORY, { primary: 0, none: 0 }).fit).toEqual({
      labels: 'primary',
      kept: 2,
    })
  })

  it('a fit that measured too wide is not tried again at the same width', () => {
    const fit: ToolbarFit = { labels: 'primary', kept: 1 }
    const memory: FitMemory = {
      ...EMPTY_MEMORY,
      all: LABELED,
      primary: 140,
      movable: MOVABLE,
      tooWide: { fit: { labels: 'primary', kept: 2 }, available: 260 },
    }
    expect(nextFit(fit, memory, measured(260, 220)).fit).toBe(fit)
    expect(nextFit(fit, memory, measured(400, 220)).fit).toEqual({ labels: 'primary', kept: null })
  })

  it('a fit that measures wider than its widths promised narrows by at least one step', () => {
    const fit: ToolbarFit = { labels: 'primary', kept: 2 }
    const memory: FitMemory = { ...EMPTY_MEMORY, all: LABELED, primary: 140, movable: MOVABLE }
    const next = nextFit(fit, memory, measured(260, 270))
    expect(next.fit).toEqual({ labels: 'primary', kept: 1 })
    expect(next.memory.tooWide).toEqual({ fit, available: 260 })
  })
})
