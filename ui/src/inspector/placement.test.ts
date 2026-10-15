import { describe, expect, it } from 'vitest'
import { inspectorBounds, placeInspector, type Rectangle } from './placement'

const BOUNDS: Rectangle = { x: 0, y: 0, width: 1000, height: 600 }
const PANEL = { width: 320, height: 400 }

function anchor(x: number, y: number, width: number, height: number): Rectangle {
  return { x, y, width, height }
}

describe('placeInspector', () => {
  it('U-37 places the panel to the right of an anchor with room there', () => {
    expect(
      placeInspector({ anchor: anchor(100, 100, 50, 50), panel: PANEL, bounds: BOUNDS }),
    ).toEqual({
      x: 166,
      y: 100,
    })
  })

  it('U-37 places the panel to the left when the right side has no room', () => {
    expect(
      placeInspector({ anchor: anchor(700, 100, 200, 50), panel: PANEL, bounds: BOUNDS }),
    ).toEqual({
      x: 364,
      y: 100,
    })
  })

  it('U-37 falls back to the top-left corner when neither side has room', () => {
    expect(
      placeInspector({ anchor: anchor(100, 100, 800, 50), panel: PANEL, bounds: BOUNDS }),
    ).toEqual({
      x: 16,
      y: 16,
    })
  })

  it('U-37 clamps y so the panel stays a gap above the bottom', () => {
    expect(
      placeInspector({ anchor: anchor(100, 500, 50, 50), panel: PANEL, bounds: BOUNDS }).y,
    ).toBe(184)
  })

  it('U-37 places the panel at the top-left corner without an anchor', () => {
    expect(placeInspector({ anchor: null, panel: PANEL, bounds: BOUNDS })).toEqual({ x: 16, y: 16 })
  })

  it('clamps y so the panel stays a gap below the top', () => {
    expect(placeInspector({ anchor: anchor(100, 4, 50, 50), panel: PANEL, bounds: BOUNDS }).y).toBe(
      16,
    )
  })

  it('puts a panel taller than the bounds minus two gaps at the top', () => {
    const tall = { width: 320, height: 590 }
    expect(
      placeInspector({ anchor: anchor(100, 300, 50, 50), panel: tall, bounds: BOUNDS }),
    ).toEqual({
      x: 166,
      y: 16,
    })
  })

  it('measures from the bounds origin and honors a custom gap', () => {
    const bounds: Rectangle = { x: 200, y: 50, width: 1000, height: 600 }
    expect(placeInspector({ anchor: null, panel: PANEL, bounds, gap: 8 })).toEqual({
      x: 208,
      y: 58,
    })
    expect(
      placeInspector({ anchor: anchor(300, 150, 50, 50), panel: PANEL, bounds, gap: 8 }),
    ).toEqual({ x: 358, y: 150 })
  })
})

describe('inspectorBounds (U-187)', () => {
  const area: Rectangle = { x: 0, y: 0, width: 992, height: 548 }
  const controls: Rectangle = { x: 16, y: 16, width: 42, height: 150 }
  const slot: Rectangle = { x: 176, y: 16, width: 640, height: 80 }
  const panel = { width: 320, height: 300 }

  it('U-187 starts right of the viewport controls, so an inspector in the corner leaves them free', () => {
    const bounds = inspectorBounds({ area, controls, slot: null, panel })
    expect(bounds).toEqual({ x: 58, y: 0, width: 934, height: 548 })
    expect(placeInspector({ anchor: null, panel, bounds })).toEqual({ x: 74, y: 16 })
  })

  it('U-187 places an anchored inspector by placeInspector inside those bounds', () => {
    const bounds = inspectorBounds({ area, controls, slot: null, panel })
    const moved = anchor(600 - 280, 200 - 64, 40, 40)
    expect(placeInspector({ anchor: moved, panel, bounds })).toEqual({ x: 376, y: 136 })
  })

  it('U-187 starts under the top-center slot content where the panel fits under it', () => {
    const bounds = inspectorBounds({ area, controls, slot, panel })
    expect(placeInspector({ anchor: null, panel, bounds })).toEqual({ x: 74, y: 112 })
  })

  it('U-187 keeps the free area top for a panel too tall to fit under the slot content', () => {
    const tall = { width: 320, height: 450 }
    const bounds = inspectorBounds({ area, controls, slot, panel: tall })
    expect(placeInspector({ anchor: null, panel: tall, bounds })).toEqual({ x: 74, y: 16 })
  })

  it('U-187 keeps the free area left side where right of the controls is too narrow', () => {
    const narrow: Rectangle = { x: 0, y: 0, width: 384, height: 548 }
    const bounds = inspectorBounds({ area: narrow, controls, slot: null, panel })
    expect(bounds.x).toBe(0)
    expect(placeInspector({ anchor: null, panel, bounds })).toEqual({ x: 16, y: 16 })
  })
})
