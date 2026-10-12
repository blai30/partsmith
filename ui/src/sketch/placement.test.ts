import { describe, expect, it } from 'vitest'
import type { Placement } from '../generated/protocol'
import { DEFAULT_CAMERA, standardViewRotation, type Camera } from '../viewport/camera'
import {
  millimetersPerPixelAt,
  pixelToSketch,
  sketchToPixel,
  sketchToWorld,
  worldBounds,
  worldToSketch,
} from './placement'

// Between sketch coordinates, model space and the canvas (3.16.3).

const FRONT_PLANE: Placement = { origin: [0, 0, 5], x_axis: [1, 0, 0], normal: [0, -1, 0] }
const SIZE = { width: 800, height: 400 }

function camera(view: 'top' | 'front' | 'right', projection: Camera['projection']): Camera {
  return {
    ...DEFAULT_CAMERA,
    target: [0, 0, 0],
    rotation: standardViewRotation(view),
    projection,
    orthographicHeight: 40,
  }
}

describe('sketch and model space', () => {
  it('maps sketch coordinates through the placement and back', () => {
    // On a plane facing -Y, +v is normal × x axis = +Z.
    expect(sketchToWorld(FRONT_PLANE, [2, 3])).toEqual([2, 0, 8])
    expect(worldToSketch(FRONT_PLANE, [2, 7, 8])).toEqual([2, 3])
  })

  it('boxes padded sketch points in model space', () => {
    expect(worldBounds(FRONT_PLANE, [[0, 0]], 1)).toEqual({ min: [-1, 0, 4], max: [1, 0, 6] })
    expect(worldBounds(FRONT_PLANE, [], 1)).toBeNull()
  })
})

describe('the canvas', () => {
  it('round-trips pixels through the sketch plane in both projections', () => {
    const xy: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
    for (const projection of ['orthographic', 'perspective'] as const) {
      const view = camera('top', projection)
      const point = pixelToSketch(view, SIZE, xy, [500, 120])
      expect(point).not.toBeNull()
      const pixel = sketchToPixel(view, SIZE, xy, point ?? [0, 0])
      expect(pixel?.[0]).toBeCloseTo(500, 6)
      expect(pixel?.[1]).toBeCloseTo(120, 6)
    }
  })

  it('maps the canvas center to the target and +X to the right in the top view', () => {
    const xy: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
    expect(pixelToSketch(camera('top', 'orthographic'), SIZE, xy, [400, 200])).toEqual([0, 0])
    expect(pixelToSketch(camera('top', 'orthographic'), SIZE, xy, [500, 100])).toEqual([10, 10])
  })

  it('ignores pointer input when the view ray is parallel to the plane', () => {
    const xy: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
    expect(pixelToSketch(camera('front', 'orthographic'), SIZE, xy, [400, 200])).toBeNull()
    expect(millimetersPerPixelAt(camera('front', 'orthographic'), SIZE, xy, [400, 200])).toBeNull()
  })

  it('measures millimeters per pixel on the plane', () => {
    const xy: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
    expect(millimetersPerPixelAt(camera('top', 'orthographic'), SIZE, xy, [10, 10])).toBeCloseTo(
      0.1,
      12,
    )
  })

  it('reaches nothing on an empty canvas', () => {
    const xy: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
    expect(
      pixelToSketch(camera('top', 'orthographic'), { width: 0, height: 0 }, xy, [0, 0]),
    ).toBeNull()
    expect(
      sketchToPixel(camera('top', 'orthographic'), { width: 0, height: 0 }, xy, [0, 0]),
    ).toBeNull()
  })
})
