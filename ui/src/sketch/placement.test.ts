import { describe, expect, it } from 'vitest'
import type { Placement } from '../generated/protocol'
import {
  DEFAULT_CAMERA,
  standardViewRotation,
  wholeCanvas,
  windowProjection,
  viewMatrix,
  type Camera,
  type ViewWindow,
} from '../viewport/camera'
import type { Vector2 } from './geometry'
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
const SIZE = wholeCanvas({ width: 800, height: 400 })

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
      pixelToSketch(
        camera('top', 'orthographic'),
        wholeCanvas({ width: 0, height: 0 }),
        xy,
        [0, 0],
      ),
    ).toBeNull()
    expect(
      sketchToPixel(
        camera('top', 'orthographic'),
        wholeCanvas({ width: 0, height: 0 }),
        xy,
        [0, 0],
      ),
    ).toBeNull()
  })
})

/** A column-major 4 by 4 matrix times a 4-vector. */
function apply(matrix: Float32Array, vector: readonly number[]): number[] {
  return [0, 1, 2, 3].map((row) =>
    [0, 1, 2, 3].reduce(
      (sum, column) => sum + (matrix[column * 4 + row] ?? 0) * (vector[column] ?? 0),
      0,
    ),
  )
}

/** The canvas pixel of a world point through the view and window projection matrices. */
function projected(view: ViewWindow, eyeCamera: Camera, point: readonly number[]): Vector2 {
  const viewed = apply(viewMatrix(eyeCamera), [...point, 1])
  const [x = 0, y = 0, , homogeneous = 1] = apply(
    windowProjection(eyeCamera, view, 1, 1000),
    viewed,
  )
  return [((x / homogeneous + 1) / 2) * view.width, ((1 - y / homogeneous) / 2) * view.height]
}

describe('the free area (U-176)', () => {
  const view: ViewWindow = {
    width: 1200,
    height: 800,
    free: { x: 280, y: 64, width: 640, height: 600 },
  }
  const xy: Placement = { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] }
  const iso: Camera = {
    ...DEFAULT_CAMERA,
    target: [0, 0, 0],
    distance: 200,
    rotation: standardViewRotation('iso'),
  }

  it.each(['perspective', 'orthographic'] as const)(
    'U-176 sketchToPixel and pixelToSketch follow windowProjection (%s)',
    (projection) => {
      const looking = { ...iso, projection }
      const pixel = sketchToPixel(looking, view, xy, [30, -20])
      const expected = projected(view, looking, [30, -20, 0])
      expect(pixel?.[0]).toBeCloseTo(expected[0], 3)
      expect(pixel?.[1]).toBeCloseTo(expected[1], 3)
      const back = pixelToSketch(looking, view, xy, pixel ?? [0, 0])
      expect(back?.[0]).toBeCloseTo(30, 6)
      expect(back?.[1]).toBeCloseTo(-20, 6)
    },
  )
})
