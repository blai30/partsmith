import { describe, expect, it } from 'vitest'
import { DEFAULT_CAMERA, standardViewRotation, wholeCanvas, type Camera } from './camera'
import { ORIGIN_PLANE_PLACEMENTS } from './planes'
import { sketchAxisSpans } from './sketch-drawables'

// Where the scene draws a sketch's axes (ui.md 3.16.2), which also places their labels.

const FRONT_CAMERA: Camera = {
  ...DEFAULT_CAMERA,
  rotation: standardViewRotation('front'),
  projection: 'orthographic',
  orthographicHeight: 100,
  target: [30, 0, 10],
}

describe('sketchAxisSpans', () => {
  it('reaches four visible heights each way in a view at most as wide as it is tall', () => {
    const view = wholeCanvas({ width: 400, height: 800 })
    expect(sketchAxisSpans(ORIGIN_PLANE_PLACEMENTS.XZ, FRONT_CAMERA, view).reach).toBe(400)
  })

  it('reaches further by the aspect in a wider view', () => {
    const view = wholeCanvas({ width: 1600, height: 800 })
    expect(sketchAxisSpans(ORIGIN_PLANE_PLACEMENTS.XZ, FRONT_CAMERA, view).reach).toBe(800)
  })

  it('reaches across the whole canvas around a smaller free area (2.11)', () => {
    const view = { width: 1600, height: 800, free: { x: 400, y: 0, width: 400, height: 400 } }
    expect(sketchAxisSpans(ORIGIN_PLANE_PLACEMENTS.XZ, FRONT_CAMERA, view).reach).toBe(1600)
  })

  it('centers on the camera target’s foot on the plane, rounded to an eighth of the reach', () => {
    const view = wholeCanvas({ width: 800, height: 800 })
    const spans = sketchAxisSpans(ORIGIN_PLANE_PLACEMENTS.XZ, FRONT_CAMERA, view)
    expect(spans.quantum).toBe(50)
    // The target lies 30 mm along the sketch's x axis (world X) and 10 mm along its y axis (world Z).
    expect(spans.center).toEqual([50, 0])
  })
})
