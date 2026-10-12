import { describe, expect, it } from 'vitest'
import { circleEntity, lineEntity, pointEntity, SKETCH_ID, solutionFor } from './fixtures.testing'
import {
  constraintSelectionItem,
  hitSelectionItem,
  hitTest,
  originReference,
  type HitOptions,
} from './hit'

// What the pointer is over in sketch mode (3.6): points first, then curves.

const LINE = lineEntity('e-1', [0, 0], [10, 0])
const RING = circleEntity('e-2', [20, 0], 5)
const DOT = pointEntity('e-3', [5, 5])
const SOLUTION = solutionFor([LINE, RING, DOT])
// 0.1 mm per pixel: points reach 0.95 mm, curves 0.75 mm.
const OPTIONS: HitOptions = { millimetersPerPixel: 0.1, uiScale: 1, includeOrigin: true }

describe('hitTest', () => {
  it('prefers a point over the curve it lies on', () => {
    expect(hitTest(SOLUTION, [10.5, 0.2], OPTIONS)).toEqual({
      kind: 'point',
      point: 'e-1.end',
      at: [10, 0],
    })
  })

  it('hits a curve within its reach and nothing beyond', () => {
    expect(hitTest(SOLUTION, [5, 0.7], OPTIONS)).toMatchObject({
      kind: 'curve',
      entity: { id: 'e-1' },
    })
    expect(hitTest(SOLUTION, [5, 2], OPTIONS)).toBeNull()
  })

  it('gives a sketch curve tied with an axis the win, despite rounding', () => {
    expect(hitTest(SOLUTION, [25, 0], OPTIONS)).toMatchObject({
      kind: 'curve',
      entity: { id: 'e-2' },
    })
  })

  it('hits the origin and the axes only when asked to', () => {
    expect(hitTest(solutionFor([]), [0.3, 0.3], OPTIONS)).toEqual({
      kind: 'origin',
      part: 'point',
    })
    expect(hitTest(solutionFor([]), [30, 0.5], OPTIONS)).toEqual({
      kind: 'origin',
      part: 'x_axis',
    })
    expect(hitTest(solutionFor([]), [0.3, 0.3], { ...OPTIONS, includeOrigin: false })).toBeNull()
  })

  it('scales the reach with the UI scale', () => {
    expect(hitTest(SOLUTION, [5, 1.2], { ...OPTIONS, uiScale: 2 })).toMatchObject({
      kind: 'curve',
    })
  })
})

describe('selection items of hits', () => {
  it('keys points, point entities, curves and origin parts as the scene does', () => {
    expect(
      hitSelectionItem(SKETCH_ID, { kind: 'point', point: 'e-1.end', at: [10, 0] }, SOLUTION),
    ).toMatchObject({ key: `point:${SKETCH_ID}:e-1.end`, kind: 'sketch_point' })
    expect(
      hitSelectionItem(SKETCH_ID, { kind: 'point', point: 'e-3', at: [5, 5] }, SOLUTION),
    ).toMatchObject({
      key: `point:${SKETCH_ID}:e-3`,
      sketchItem: { kind: 'entity', entity: 'e-3', entityType: 'point' },
    })
    expect(hitSelectionItem(SKETCH_ID, { kind: 'origin', part: 'y_axis' }, SOLUTION)).toMatchObject(
      { key: `sketch:${SKETCH_ID}:y_axis`, kind: 'sketch_curve', label: 'Y axis' },
    )
    expect(constraintSelectionItem(SKETCH_ID, 'c-1', 'Horizontal')).toMatchObject({
      key: `constraint:${SKETCH_ID}:c-1`,
      kind: 'sketch_constraint',
      label: 'Horizontal',
    })
  })

  it('names the reserved references of the origin parts', () => {
    expect(originReference('point')).toBe('origin')
    expect(originReference('x_axis')).toBe('x_axis')
    expect(originReference('y_axis')).toBe('y_axis')
  })
})
