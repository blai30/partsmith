import { describe, expect, it } from 'vitest'
import { placeFloating, type PlacementOptions } from './placement'

const WINDOW = { width: 1000, height: 800 }
const SIZE = { width: 200, height: 100 }
const BELOW: PlacementOptions = { placement: 'below', gap: 4, margin: 8 }
const RIGHT: PlacementOptions = { placement: 'right', gap: 4, margin: 8 }

describe('placeFloating below', () => {
  it('puts the element under the anchor, aligned to its start', () => {
    const anchor = { x: 100, y: 100, width: 50, height: 20 }
    expect(placeFloating(anchor, SIZE, WINDOW, BELOW)).toEqual({ x: 100, y: 124 })
  })

  it('flips above the anchor when only that side has room', () => {
    const anchor = { x: 100, y: 700, width: 50, height: 20 }
    expect(placeFloating(anchor, SIZE, WINDOW, BELOW)).toEqual({ x: 100, y: 596 })
  })

  it('stays below when neither side has room but below has more, kept inside the window', () => {
    const anchor = { x: 100, y: 300, width: 50, height: 20 }
    const tall = { width: 200, height: 700 }
    expect(placeFloating(anchor, tall, WINDOW, BELOW)).toEqual({ x: 100, y: 92 })
  })

  it('moves left to keep the margin from the right edge', () => {
    const anchor = { x: 950, y: 100, width: 40, height: 20 }
    expect(placeFloating(anchor, SIZE, WINDOW, BELOW).x).toBe(792)
  })
})

describe('placeFloating right', () => {
  it('puts the element beside the anchor, aligned to its top', () => {
    const anchor = { x: 100, y: 100, width: 50, height: 20 }
    expect(placeFloating(anchor, SIZE, WINDOW, RIGHT)).toEqual({ x: 154, y: 100 })
  })

  it('flips to the left of the anchor when only that side has room', () => {
    const anchor = { x: 900, y: 100, width: 50, height: 20 }
    expect(placeFloating(anchor, SIZE, WINDOW, RIGHT)).toEqual({ x: 696, y: 100 })
  })

  it('moves up to keep the margin from the bottom edge', () => {
    const anchor = { x: 100, y: 750, width: 50, height: 20 }
    expect(placeFloating(anchor, SIZE, WINDOW, RIGHT).y).toBe(692)
  })
})

describe('placeFloating without an anchor', () => {
  it('centers the element in the window', () => {
    expect(placeFloating(null, SIZE, WINDOW, BELOW)).toEqual({ x: 400, y: 350 })
  })

  it('keeps an element larger than the window at the margin', () => {
    expect(placeFloating(null, { width: 2000, height: 900 }, WINDOW, BELOW)).toEqual({
      x: 8,
      y: 8,
    })
  })
})
