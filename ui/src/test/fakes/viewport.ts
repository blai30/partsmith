import { expect, vi, type Mock } from 'vitest'
import type { BoundingBox } from '../../generated/protocol'
import { DEFAULT_CAMERA, wholeCanvas } from '../../viewport/camera'
import type { ViewportHandle } from '../../viewport/controller'

/** A `ViewportHandle` whose every method is a mock. */
export type FakeViewport = {
  readonly [Method in keyof ViewportHandle]: Mock<ViewportHandle[Method]>
}

/** A recording viewport at the default camera that projects nothing onto the screen: its canvas is not measured yet. */
export function createFakeViewport(): FakeViewport {
  return {
    getCamera: vi.fn<ViewportHandle['getCamera']>(() => DEFAULT_CAMERA),
    setCamera: vi.fn<ViewportHandle['setCamera']>(),
    fit: vi.fn<ViewportHandle['fit']>(),
    setStandardView: vi.fn<ViewportHandle['setStandardView']>(),
    toggleProjection: vi.fn<ViewportHandle['toggleProjection']>(),
    lookAlong: vi.fn<ViewportHandle['lookAlong']>(),
    zoomToBounds: vi.fn<ViewportHandle['zoomToBounds']>(),
    itemBounds: vi.fn<ViewportHandle['itemBounds']>(() => null),
    screenRectangle: vi.fn<ViewportHandle['screenRectangle']>(() => null),
    viewWindow: vi.fn<ViewportHandle['viewWindow']>(() => wholeCanvas({ width: 0, height: 0 })),
    requestRender: vi.fn<ViewportHandle['requestRender']>(),
  }
}

/** The box and margin of the viewport's last `zoomToBounds`; throws when it framed nothing. */
export function lastFramedBounds(viewport: FakeViewport): {
  readonly bounds: BoundingBox
  readonly margin: number
} {
  const [bounds, margin] = viewport.zoomToBounds.mock.calls.at(-1) ?? []
  if (bounds === undefined || margin === undefined) {
    throw new Error('The viewport framed nothing.')
  }
  return { bounds, margin }
}

/** Expects a box to hold every point on each axis. */
export function expectBoundsContain(
  bounds: BoundingBox,
  points: readonly (readonly [number, number, number])[],
): void {
  for (const point of points) {
    for (const axis of [0, 1, 2] as const) {
      expect(bounds.min[axis]).toBeLessThanOrEqual(point[axis])
      expect(bounds.max[axis]).toBeGreaterThanOrEqual(point[axis])
    }
  }
}
