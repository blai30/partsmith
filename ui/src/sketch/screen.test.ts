import { describe, expect, it } from 'vitest'
import type { SketchConstraint } from '../generated/protocol'
import { circleEntity, lineEntity, pointEntity, solutionFor } from './fixtures.testing'
import type { Vector2 } from './geometry'
import {
  arrowheadPath,
  circlePath,
  drawInputsPlacement,
  entitiesInBox,
  glyphCenters,
  polylinePath,
  projectPolyline,
  screenBox,
  shapePoints,
  type Projector,
} from './screen'

/** 10 pixels per millimeter, +Y up, the origin at (100, 100). */
const PROJECT: Projector = (point) => [100 + 10 * point[0], 100 - 10 * point[1]]
/** PROJECT, with the points at x = 2 behind the eye. */
const BEHIND_AT_TWO: Projector = (point) => (point[0] === 2 ? null : PROJECT(point))

describe('paths', () => {
  it('projects a polyline and breaks it at points behind the eye', () => {
    expect(
      projectPolyline(
        [
          [0, 0],
          [1, 0],
          [2, 0],
          [3, 0],
          [4, 0],
        ],
        BEHIND_AT_TWO,
      ),
    ).toEqual([
      [
        [100, 100],
        [110, 100],
      ],
      [
        [130, 100],
        [140, 100],
      ],
    ])
  })

  it('writes polylines with two decimals and drops single points', () => {
    expect(
      polylinePath([
        [
          [1.234, 5],
          [7, 8.005],
        ],
        [[3, 3]],
      ]),
    ).toBe('M1.23 5 L7 8.01')
  })

  it('writes a circle as two arcs', () => {
    expect(circlePath([10, 20], 5)).toBe('M5 20 a5 5 0 1 0 10 0 a5 5 0 1 0 -10 0')
  })

  it('points an arrowhead from its tip back toward the other end', () => {
    expect(arrowheadPath([0, 0], [100, 0], 9)).toBe('M0 0 L9 3 L9 -3 Z')
    expect(arrowheadPath([0, 0], [0, 0], 8)).toBe('')
  })

  it('tessellates a rubber band circle within the tolerance and closes it', () => {
    const points = shapePoints({ kind: 'circle', center: [0, 0], radius: 10 }, 0.01)
    const first = points[0]
    const last = points.at(-1)
    expect(points.length).toBeGreaterThan(50)
    expect(first?.[0]).toBeCloseTo(last?.[0] ?? Number.NaN, 9)
    expect(first?.[1]).toBeCloseTo(last?.[1] ?? Number.NaN, 9)
  })
})

describe('glyphCenters (3.16.2)', () => {
  const FLAT = lineEntity('e-1', [0, 0], [10, 0])
  const UPRIGHT = lineEntity('e-2', [20, 0], [20, 10])
  const DOT = pointEntity('e-3', [-5, -5])

  function centers(
    constraints: SketchConstraint[],
    scale = 1,
  ): ReadonlyMap<string, Vector2 | null> {
    return glyphCenters(constraints, solutionFor([FLAT, UPRIGHT, DOT]), PROJECT, scale)
  }

  it('puts a line glyph 14 pixels above the line middle, times the UI scale', () => {
    expect(centers([{ id: 'c-1', type: 'horizontal', line: 'e-1' }]).get('c-1')).toEqual([150, 86])
    expect(centers([{ id: 'c-1', type: 'horizontal', line: 'e-1' }], 2).get('c-1')).toEqual([
      150, 72,
    ])
  })

  it('puts a vertical line glyph to its right', () => {
    expect(centers([{ id: 'c-1', type: 'vertical', line: 'e-2' }]).get('c-1')).toEqual([314, 50])
  })

  it('stacks further glyphs on the same anchor 20 pixels apart along the curve', () => {
    const placed = centers([
      { id: 'c-1', type: 'horizontal', line: 'e-1' },
      { id: 'c-2', type: 'equal', curves: ['e-1', 'e-2'] },
    ])
    expect(placed.get('c-1')).toEqual([150, 86])
    expect(placed.get('c-2')).toEqual([170, 86])
  })

  it('puts a point glyph up and to the right of the point', () => {
    const [x = 0, y = 0] = centers([{ id: 'c-1', type: 'fix', target: 'e-3' }]).get('c-1') ?? []
    expect(x).toBeCloseTo(50 + 14 / Math.SQRT2, 9)
    expect(y).toBeCloseTo(150 - 14 / Math.SQRT2, 9)
  })

  it('is null for a glyph whose anchor is behind the eye', () => {
    const placed = glyphCenters(
      [{ id: 'c-1', type: 'horizontal', line: 'e-1' }],
      solutionFor([FLAT]),
      () => null,
      1,
    )
    expect(placed.get('c-1')).toBeNull()
  })
})

describe('entitiesInBox (3.16.9)', () => {
  const INSIDE = lineEntity('e-1', [1, 1], [2, 2])
  const ACROSS = lineEntity('e-2', [-5, 1.5], [5, 1.5])
  const RING = circleEntity('e-3', [1.5, 1.5], 0.4)
  const OUTSIDE = pointEntity('e-4', [9, 9])
  const SOLUTION = solutionFor([INSIDE, ACROSS, RING, OUTSIDE])
  // Sketch (0.5, 0.5) to (3, 3).
  const BOX = screenBox([105, 95], [130, 70])

  it('takes entities entirely inside for a window selection', () => {
    expect(
      entitiesInBox(SOLUTION, PROJECT, BOX, 'window', 0.01).map((entity) => entity.id),
    ).toEqual(['e-1', 'e-3'])
  })

  it('takes entities with any part inside for a crossing selection', () => {
    expect(
      entitiesInBox(SOLUTION, PROJECT, BOX, 'crossing', 0.01).map((entity) => entity.id),
    ).toEqual(['e-1', 'e-2', 'e-3'])
  })

  it('normalizes the box whichever way it was dragged', () => {
    expect(screenBox([130, 70], [105, 95])).toEqual({ left: 105, top: 70, right: 130, bottom: 95 })
  })
})

describe('drawInputsPlacement (3.16.4)', () => {
  it('puts the draw inputs above and left of the midpoint when the pointer is below and right', () => {
    expect(drawInputsPlacement([150, 130], [200, 160], 12)).toEqual({
      at: [138, 118],
      left: true,
      above: true,
    })
  })

  it('puts them below and right of the midpoint when the pointer is above and left', () => {
    expect(drawInputsPlacement([150, 130], [100, 100], 12)).toEqual({
      at: [162, 142],
      left: false,
      above: false,
    })
  })

  it('puts them below and right without a pointer', () => {
    expect(drawInputsPlacement([150, 130], null, 12)).toEqual({
      at: [162, 142],
      left: false,
      above: false,
    })
  })
})
