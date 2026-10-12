import { describe, expect, it } from 'vitest'
import type { SelectionItem } from '../state/store'
import { pickVerdict, type ConstraintType } from './constraints'
import {
  circleEntity,
  curveItem,
  lineEntity,
  pointEntity,
  pointItem,
  solutionFor,
  SKETCH_ID,
} from './fixtures.testing'
import { originSelectionItem } from './hit'

// A constraint tool's pick list (3.16.6): complete sets apply, partial ones wait, others refuse.

const SOLUTION = solutionFor([
  lineEntity('e-1', [0, 0], [10, 0]),
  lineEntity('e-2', [0, 5], [10, 6]),
  circleEntity('e-3', [20, 0], 2),
  circleEntity('e-4', [30, 0], 3),
  pointEntity('e-5', [5, 5]),
])

const LINE = curveItem('e-1', 'line')
const OTHER_LINE = curveItem('e-2', 'line')
const CIRCLE = curveItem('e-3', 'circle')
const OTHER_CIRCLE = curveItem('e-4', 'circle')
const POINT = pointItem('e-1.start')
const OTHER_POINT = pointItem('e-2.end')

function verdict(type: ConstraintType, picks: SelectionItem[]) {
  return pickVerdict(type, picks, SOLUTION)
}

describe('pickVerdict', () => {
  it('completes horizontal with one line, waits after one point', () => {
    expect(verdict('horizontal', [LINE])).toBe('complete')
    expect(verdict('horizontal', [POINT])).toBe('partial')
    expect(verdict('horizontal', [POINT, OTHER_POINT])).toBe('complete')
    expect(verdict('horizontal', [CIRCLE])).toBe('refused')
  })

  it('completes coincident with two points or with a point and a curve', () => {
    expect(verdict('coincident', [POINT])).toBe('partial')
    expect(verdict('coincident', [CIRCLE])).toBe('partial')
    expect(verdict('coincident', [POINT, OTHER_POINT])).toBe('complete')
    expect(verdict('coincident', [POINT, CIRCLE])).toBe('complete')
    expect(verdict('coincident', [LINE, CIRCLE])).toBe('refused')
  })

  it('needs two lines for parallel', () => {
    expect(verdict('parallel', [LINE])).toBe('partial')
    expect(verdict('parallel', [LINE, OTHER_LINE])).toBe('complete')
    expect(verdict('parallel', [CIRCLE])).toBe('refused')
  })

  it('takes a line and a round curve, or two round curves, for tangent', () => {
    expect(verdict('tangent', [CIRCLE])).toBe('partial')
    expect(verdict('tangent', [CIRCLE, LINE])).toBe('complete')
    expect(verdict('tangent', [CIRCLE, OTHER_CIRCLE])).toBe('complete')
    expect(verdict('tangent', [LINE, OTHER_LINE])).toBe('refused')
  })

  it('fills symmetric with two points and a line in any order', () => {
    expect(verdict('symmetric', [LINE])).toBe('partial')
    expect(verdict('symmetric', [OTHER_POINT, LINE])).toBe('partial')
    expect(verdict('symmetric', [pointItem('e-5'), LINE, OTHER_POINT])).toBe('complete')
    expect(verdict('symmetric', [LINE, OTHER_LINE])).toBe('refused')
  })

  it('completes fix with any point or curve, but never the origin', () => {
    expect(verdict('fix', [CIRCLE])).toBe('complete')
    expect(verdict('fix', [originSelectionItem(SKETCH_ID, 'point')])).toBe('refused')
  })

  it('refuses a first pick no later pick can complete (axes and a point on its own line)', () => {
    const xAxis = originSelectionItem(SKETCH_ID, 'x_axis')
    expect(verdict('midpoint', [xAxis])).toBe('refused')
    expect(verdict('equal', [xAxis])).toBe('refused')
    expect(verdict('horizontal', [xAxis])).toBe('refused')
    expect(verdict('symmetric', [LINE, POINT])).toBe('refused')
    expect(verdict('symmetric', [POINT, LINE])).toBe('refused')
    expect(verdict('midpoint', [LINE, POINT])).toBe('refused')
    expect(verdict('coincident', [pointItem('e-1.start'), pointItem('e-1.end')])).toBe('refused')
  })

  it('keeps picks that later picks can complete, axes included where sketch.md takes them', () => {
    const xAxis = originSelectionItem(SKETCH_ID, 'x_axis')
    expect(verdict('parallel', [xAxis])).toBe('partial')
    expect(verdict('symmetric', [xAxis])).toBe('partial')
    expect(verdict('coincident', [xAxis])).toBe('partial')
    expect(verdict('midpoint', [POINT])).toBe('partial')
    expect(verdict('equal', [CIRCLE])).toBe('partial')
  })

  it('refuses the same item twice and an axis where sketch.md takes no axis', () => {
    expect(verdict('parallel', [LINE, LINE])).toBe('refused')
    expect(verdict('midpoint', [POINT, originSelectionItem(SKETCH_ID, 'x_axis')])).toBe('refused')
    expect(verdict('parallel', [originSelectionItem(SKETCH_ID, 'x_axis'), OTHER_LINE])).toBe(
      'complete',
    )
  })
})
