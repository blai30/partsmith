import { describe, expect, it } from 'vitest'
import type { BoundingBox, Plane, Suggestion } from '../generated/protocol'
import type { Vector3 } from '../viewport/math'
import { splitPlaneOutline, suggestionText } from './suggestion'

// The words of a print suggestion and the outlines of its split planes (ui.md 3.17, U-233 and
// U-234; print.md 3.12).

/** A split along these (origin, normal) planes, in order. */
function split(...planes: readonly (readonly [Vector3, Vector3])[]): Suggestion {
  return {
    kind: 'split',
    planes: planes.map(([origin, normal]) => ({ origin: [...origin], normal: [...normal] })),
  }
}

function drainHole(at: Vector3): Suggestion {
  return { kind: 'drain_hole', at: [...at] }
}

function plane(origin: Vector3, normal: Vector3): Plane {
  return { origin: [...origin], normal: [...normal] }
}

function box(min: Vector3, max: Vector3): BoundingBox {
  return { min: [...min], max: [...max] }
}

const SQRT_2 = Math.sqrt(2)
const SQRT_3 = Math.sqrt(3)

/** The shared cases of print.md 3.12 with their words; case 11 (no words) is tested on its own. */
const SHARED_CASES: readonly (readonly [string, Suggestion, string])[] = [
  [
    'case 1',
    split([
      [150, 50, 5],
      [1, 0, 0],
    ]),
    'Split at x = 150 mm',
  ],
  [
    'case 2',
    split(
      [
        [0, 0, 5],
        [1, 0, 0],
      ],
      [
        [0, 0, 5],
        [0, 1, 0],
      ],
    ),
    'Split at x = 0 mm and y = 0 mm',
  ],
  [
    'case 3',
    split(
      [
        [200, 0, 5],
        [1, 0, 0],
      ],
      [
        [400, 0, 5],
        [1, 0, 0],
      ],
      [
        [0, 150, 5],
        [0, 1, 0],
      ],
    ),
    'Split at x = 200 mm, x = 400 mm and y = 150 mm',
  ],
  [
    'case 4',
    split([
      [5, 5, 150],
      [0, 0, -1],
    ]),
    'Split at z = 150 mm',
  ],
  [
    'case 5',
    split([
      [3, 4, 12.5],
      [1e-10, 0, 1],
    ]),
    'Split at z = 12.5 mm',
  ],
  [
    'case 6',
    split([
      [0, 0, -0.0004],
      [0, 0, 1],
    ]),
    'Split at z = 0 mm',
  ],
  [
    'case 7',
    split([
      [150, 50, 5],
      [0.8660254037844387, -0.49999999999999994, 0],
    ]),
    'Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)',
  ],
  [
    'case 8',
    split([
      [10, 20, 30],
      [0.6, 0.8, 0],
    ]),
    'Split at the plane through (10, 20, 30 mm) with normal (0.6, 0.8, 0)',
  ],
  ['case 9', drainHole([5, 5.25, 0.0001]), 'Add a drain hole near (5, 5.25, 0 mm)'],
  ['case 10', drainHole([-8, 13, 2]), 'Add a drain hole near (-8, 13, 2 mm)'],
]

describe('U-233 suggestionText', () => {
  it.each(SHARED_CASES)(
    'U-233 words shared %s as print.md 3.12 does',
    (_name, suggestion, words) => {
      expect(suggestionText(suggestion)).toBe(words)
    },
  )

  it('U-233 shared case 11: a split without planes has no words', () => {
    expect(suggestionText({ kind: 'split', planes: [] })).toBeNull()
  })
})

describe('suggestionText beyond the shared cases', () => {
  it('joins four planes with commas and a final "and"', () => {
    expect(
      suggestionText(
        split(
          [
            [0, 0, 0],
            [1, 0, 0],
          ],
          [
            [1, 0, 0],
            [1, 0, 0],
          ],
          [
            [0, 2, 0],
            [0, 1, 0],
          ],
          [
            [0, 0, 3],
            [0, 0, 1],
          ],
        ),
      ),
    ).toBe('Split at x = 0 mm, x = 1 mm, y = 2 mm and z = 3 mm')
  })

  it('reads a plane by the coordinate of its own axis, whichever way its normal points', () => {
    expect(
      suggestionText(
        split([
          [7, -12.25, 1],
          [0, -1, 0],
        ]),
      ),
    ).toBe('Split at y = -12.25 mm')
  })

  it('reads a normal just inside the axis tolerance as the axis', () => {
    // The z component is 1 - 4.5e-10, at least 1 - 1e-9.
    const normal: Vector3 = [3e-5, 0, Math.sqrt(1 - 9e-10)]
    expect(suggestionText(split([[1, 2, 3], normal]))).toBe('Split at z = 3 mm')
  })

  it('reads a normal just outside the axis tolerance as a tilted plane', () => {
    // The z component is 1 - 1.25e-9, below 1 - 1e-9, though it rounds to 1.
    const normal: Vector3 = [5e-5, 0, Math.sqrt(1 - 2.5e-9)]
    expect(suggestionText(split([[1, 2, 3], normal]))).toBe(
      'Split at the plane through (1, 2, 3 mm) with normal (0, 0, 1)',
    )
  })

  it('writes minus zero as zero in a tilted plane', () => {
    expect(
      suggestionText(
        split([
          [-0.0001, 0, 4],
          [0.6, -0.8, -0],
        ]),
      ),
    ).toBe('Split at the plane through (0, 0, 4 mm) with normal (0.6, -0.8, 0)')
  })
})

/** Whether two points are within 1e-9 on every coordinate. */
function samePoint(first: Vector3, second: Vector3): boolean {
  return first.every((value, axis) => Math.abs(value - (second[axis] ?? Number.NaN)) <= 1e-9)
}

/**
 * The corners rounded to 1e-6 (so points within 1e-9 compare equal, with `-0` written `0`) and
 * sorted, to compare corners in any order; null stays null.
 */
function sortedCorners(corners: readonly Vector3[] | null): number[][] | null {
  if (corners === null) {
    return null
  }
  const rounded = corners.map((corner) => corner.map((value) => Math.round(value * 1e6) / 1e6 + 0))
  return rounded.toSorted(
    (first, second) =>
      (first[0] ?? 0) - (second[0] ?? 0) ||
      (first[1] ?? 0) - (second[1] ?? 0) ||
      (first[2] ?? 0) - (second[2] ?? 0),
  )
}

/** The index in `outline` of the corner at `point`. */
function indexOf(outline: readonly Vector3[], point: Vector3): number {
  return outline.findIndex((candidate) => samePoint(candidate, point))
}

/** Whether two points lie on one face of the box: they share a coordinate that is the box's minimum or maximum. */
function onOneFace(first: Vector3, second: Vector3, bounds: BoundingBox): boolean {
  return [0, 1, 2].some((axis) =>
    [bounds.min[axis], bounds.max[axis]].some(
      (side) =>
        side !== undefined &&
        Math.abs((first[axis] ?? Number.NaN) - side) <= 1e-9 &&
        Math.abs((second[axis] ?? Number.NaN) - side) <= 1e-9,
    ),
  )
}

describe('U-234 splitPlaneOutline', () => {
  const SLAB = box([0, 0, 0], [300, 100, 10])
  const CUBE = box([0, 0, 0], [10, 10, 10])

  it('U-234 cuts a box across its length in a rectangle whose neighbors share a face', () => {
    const outline = splitPlaneOutline(plane([150, 50, 5], [1, 0, 0]), SLAB)
    expect(sortedCorners(outline)).toEqual(
      sortedCorners([
        [150, 0, 0],
        [150, 100, 0],
        [150, 100, 10],
        [150, 0, 10],
      ]),
    )
    const corners = outline ?? []
    corners.forEach((corner, i) => {
      const next = corners[(i + 1) % corners.length] ?? corner
      expect(onOneFace(corner, next, SLAB)).toBe(true)
    })
  })

  it('U-234 cuts a cube through its center along a diagonal normal in a hexagon, in cyclic order', () => {
    const outline = splitPlaneOutline(plane([5, 5, 5], [1 / SQRT_3, 1 / SQRT_3, 1 / SQRT_3]), CUBE)
    const expected: Vector3[] = [
      [10, 5, 0],
      [5, 10, 0],
      [0, 10, 5],
      [0, 5, 10],
      [5, 0, 10],
      [10, 0, 5],
    ]
    expect(sortedCorners(outline)).toEqual(sortedCorners(expected))
    const corners = outline ?? []
    const start = indexOf(corners, expected[0] ?? [0, 0, 0])
    const step = samePoint(corners[(start + 1) % 6] ?? [0, 0, 0], expected[1] ?? [0, 0, 0]) ? 1 : 5
    expected.forEach((corner, i) => {
      expect(samePoint(corners[(start + step * i) % 6] ?? [0, 0, 0], corner)).toBe(true)
    })
  })

  it('U-234 gives the four corners of a face the plane lies in, each once', () => {
    expect(sortedCorners(splitPlaneOutline(plane([10, 5, 5], [1, 0, 0]), CUBE))).toEqual(
      sortedCorners([
        [10, 0, 0],
        [10, 10, 0],
        [10, 10, 10],
        [10, 0, 10],
      ]),
    )
  })

  it('U-234 is null for a plane that touches the box only along an edge', () => {
    expect(splitPlaneOutline(plane([10, 10, 5], [1 / SQRT_2, 1 / SQRT_2, 0]), CUBE)).toBeNull()
  })

  it('U-234 is null for a plane that misses the box', () => {
    expect(splitPlaneOutline(plane([20, 5, 5], [1, 0, 0]), CUBE)).toBeNull()
  })

  it('is null for a plane that touches the box only at a corner', () => {
    expect(
      splitPlaneOutline(plane([10, 10, 10], [1 / SQRT_3, 1 / SQRT_3, 1 / SQRT_3]), CUBE),
    ).toBeNull()
  })

  it('cuts a corner off the box in a triangle', () => {
    expect(
      sortedCorners(
        splitPlaneOutline(plane([10, 10, 9], [1 / SQRT_3, 1 / SQRT_3, 1 / SQRT_3]), CUBE),
      ),
    ).toEqual(
      sortedCorners([
        [10, 9, 10],
        [9, 10, 10],
        [10, 10, 9],
      ]),
    )
  })

  it('gives the same corners whichever way the normal points', () => {
    expect(sortedCorners(splitPlaneOutline(plane([150, 50, 5], [-1, 0, 0]), SLAB))).toEqual(
      sortedCorners([
        [150, 0, 0],
        [150, 100, 0],
        [150, 100, 10],
        [150, 0, 10],
      ]),
    )
  })
})
