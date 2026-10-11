import { describe, expect, it } from 'vitest'
import {
  classifyView,
  DEFAULT_CAMERA,
  depthRange,
  easeOutCubic,
  eye,
  fitBounds,
  forward,
  interpolateCameras,
  normalizedPosition,
  pan,
  pointAtDepth,
  right,
  rotationLookingAlong,
  standardViewRotation,
  up,
  type Camera,
} from './camera'
import {
  angleBetween,
  cross,
  dot,
  multiplyQuaternions,
  quaternionFromAxisAngle,
  scale,
  type Vector3,
} from './math'

function closeTo(vector: readonly number[], digits = 9): unknown[] {
  return vector.map((component) => expect.closeTo(component, digits))
}

const front: Camera = {
  ...DEFAULT_CAMERA,
  rotation: standardViewRotation('front'),
  target: [10, 0, 5],
  distance: 100,
}
const frontOrthographic: Camera = { ...front, projection: 'orthographic', orthographicHeight: 40 }

describe('right', () => {
  it('completes a right-handed basis with up and the backward axis', () => {
    const camera = { ...DEFAULT_CAMERA }
    expect(cross(right(camera), up(camera))).toEqual(closeTo(scale(forward(camera), -1)))
    expect(right({ ...camera, rotation: standardViewRotation('front') })).toEqual(
      closeTo([1, 0, 0]),
    )
  })
})

describe('rotationLookingAlong', () => {
  it('throws for a zero direction', () => {
    expect(() => rotationLookingAlong([0, 0, 0], [0, 0, 1])).toThrow(RangeError)
  })

  it('uses the world axis least aligned with the direction when the hint is parallel to it', () => {
    const camera = { ...DEFAULT_CAMERA, rotation: rotationLookingAlong([0, 0, -1], [0, 0, 1]) }
    expect(forward(camera)).toEqual(closeTo([0, 0, -1]))
    expect(Math.abs(dot(up(camera), [0, 0, 1]))).toBeLessThan(1e-12)
    expect(Math.hypot(...up(camera))).toBeCloseTo(1, 12)
  })
})

describe('fitBounds', () => {
  it('rejects a margin outside 0 to below 0.5', () => {
    const bounds = { min: [0, 0, 0] as Vector3, max: [1, 1, 1] as Vector3 }
    expect(() => fitBounds(DEFAULT_CAMERA, bounds, 1, 0.5)).toThrow(RangeError)
    expect(() => fitBounds(DEFAULT_CAMERA, bounds, 1, -0.1)).toThrow(RangeError)
  })
})

describe('pan', () => {
  it('leaves the camera alone for a viewport without height', () => {
    expect(pan(front, 10, 10, 0)).toBe(front)
  })
})

describe('depthRange', () => {
  it('keeps far beyond near when the scene is behind a perspective eye', () => {
    const range = depthRange(front, [10, -500, 5], 1)
    expect(range.near).toBe(0.01)
    expect(range.far).toBeGreaterThan(range.near)
  })
})

describe('pointAtDepth and normalizedPosition', () => {
  it.each([
    ['perspective', front],
    ['orthographic', frontOrthographic],
  ] as const)('are inverse projections in %s', (_name, camera) => {
    const point = pointAtDepth(camera, [0.25, -0.5], 1.5, 80)
    const position = normalizedPosition(camera, point, 1.5)
    expect(position?.x).toBeCloseTo(0.25, 12)
    expect(position?.y).toBeCloseTo(-0.5, 12)
    expect(position?.depth).toBeCloseTo(80, 12)
  })

  it('put the target at the center at the camera distance', () => {
    expect(pointAtDepth(front, [0, 0], 2, front.distance)).toEqual(closeTo(front.target))
    expect(normalizedPosition(front, front.target, 2)).toEqual({
      x: expect.closeTo(0, 12),
      y: expect.closeTo(0, 12),
      depth: expect.closeTo(100, 12),
    })
  })

  it('reach the top edge of the view at half the visible height', () => {
    // Perspective at depth 100 with a 30 degree field of view: half height 100 tan 15°.
    const top = pointAtDepth(front, [0, 1], 1, 100)
    expect(top[2] - front.target[2]).toBeCloseTo(100 * Math.tan(Math.PI / 12), 12)
    const orthographicTop = pointAtDepth(frontOrthographic, [0, 1], 1, 30)
    expect(orthographicTop[2] - front.target[2]).toBeCloseTo(20, 12)
  })

  it('give no position for a point behind a perspective eye', () => {
    expect(normalizedPosition(front, [10, -200, 5], 1)).toBeNull()
    expect(normalizedPosition(front, eye(front), 1)).toBeNull()
  })

  it('give a position for a point behind an orthographic eye', () => {
    expect(normalizedPosition(frontOrthographic, [10, -200, 5], 1)?.depth).toBeCloseTo(-100, 12)
  })
})

describe('easeOutCubic', () => {
  it('runs from 0 to 1, fast at first', () => {
    expect(easeOutCubic(0)).toBe(0)
    expect(easeOutCubic(1)).toBe(1)
    expect(easeOutCubic(0.5)).toBe(0.875)
  })
})

describe('interpolateCameras', () => {
  const from: Camera = { ...front, distance: 100, orthographicHeight: 10 }
  const to: Camera = {
    ...DEFAULT_CAMERA,
    target: [0, 0, 0],
    distance: 400,
    orthographicHeight: 40,
    projection: 'orthographic',
  }

  it('starts at the first camera and ends exactly at the second', () => {
    const start = interpolateCameras(from, to, 0)
    expect(start.target).toEqual(closeTo(from.target))
    expect(start.distance).toBeCloseTo(100, 12)
    expect(classifyView(start)).toBe('front')
    expect(interpolateCameras(from, to, 1)).toBe(to)
  })

  it('changes lengths geometrically and the target linearly', () => {
    const middle = interpolateCameras(from, to, 0.5)
    expect(middle.distance).toBeCloseTo(200, 9)
    expect(middle.orthographicHeight).toBeCloseTo(20, 9)
    expect(middle.target).toEqual(closeTo([5, 0, 2.5]))
    expect(middle.projection).toBe('orthographic')
  })

  it('turns along the shorter arc', () => {
    const turned: Camera = {
      ...from,
      rotation: multiplyQuaternions(quaternionFromAxisAngle([0, 0, 1], 0.2), from.rotation),
    }
    const middle = interpolateCameras(from, turned, 0.5)
    expect(angleBetween(forward(middle), forward(from))).toBeCloseTo(0.1, 9)
  })
})
