import { describe, expect, it } from 'vitest'
import type { SelectionItem } from '../state/store'
import {
  DIMENSION_START,
  dimensionClick,
  dimensionPreview,
  pickedItems,
  type DimensionPicking,
} from './dimension-tool'
import {
  arcEntity,
  circleEntity,
  curveItem,
  lineEntity,
  pointItem,
  solutionFor,
  SKETCH_ID,
} from './fixtures.testing'
import { originSelectionItem } from './hit'

// The dimension tool's clicks (3.16.7).

const BASE = lineEntity('e-1', [0, 0], [20, 0])
const SIDE = lineEntity('e-2', [0, 0], [0, 10])
const RING = circleEntity('e-3', [40, 0], 5)
const BEND = arcEntity('e-4', [40, 40], 5, 0, Math.PI / 2)
const SOLUTION = solutionFor([BASE, SIDE, RING, BEND])

const LINE = curveItem('e-1', 'line')
const OTHER_LINE = curveItem('e-2', 'line')
const CIRCLE = curveItem('e-3', 'circle')
const ARC = curveItem('e-4', 'arc')
const END = pointItem('e-1.end')

function click(
  picking: DimensionPicking,
  item: SelectionItem | null,
  at: [number, number] = [5, -5],
) {
  return dimensionClick(picking, item, at, SOLUTION)
}

describe('dimensionClick', () => {
  it('waits on empty space before the first pick', () => {
    expect(click(DIMENSION_START, null)).toEqual({
      picking: DIMENSION_START,
      draft: null,
      refused: null,
    })
  })

  it('places a circle or arc label next', () => {
    expect(click(DIMENSION_START, CIRCLE).picking).toEqual({ stage: 'place', picks: [CIRCLE] })
    expect(click(DIMENSION_START, ARC).picking).toEqual({ stage: 'place', picks: [ARC] })
  })

  it('after a line, a click on empty space opens its length dimension there', () => {
    const afterLine = click(DIMENSION_START, LINE).picking
    expect(afterLine).toEqual({ stage: 'line', first: LINE })
    expect(click(afterLine, null, [10, -4])).toEqual({
      picking: DIMENSION_START,
      draft: { picks: [LINE], labelAt: [10, -4], type: 'distance' },
      refused: null,
    })
  })

  it('after a line, a second line is picked and the label placed next', () => {
    const afterLine = click(DIMENSION_START, LINE).picking
    const placing = click(afterLine, OTHER_LINE).picking
    expect(placing).toEqual({ stage: 'place', picks: [LINE, OTHER_LINE] })
    expect(click(placing, null, [5, 5]).draft).toEqual({
      picks: [LINE, OTHER_LINE],
      labelAt: [5, 5],
      type: 'angle',
    })
  })

  it('after a point, the next pick must be a point or a line', () => {
    const afterPoint = click(DIMENSION_START, END).picking
    expect(afterPoint).toEqual({ stage: 'point', first: END })
    expect(click(afterPoint, null).picking).toBe(afterPoint)
    const refused = click(afterPoint, CIRCLE)
    expect(refused.picking).toBe(afterPoint)
    expect(refused.draft).toBeNull()
    expect(refused.refused).toMatch(/^Partsmith cannot dimension a point and a circle\./)
    expect(click(afterPoint, OTHER_LINE).picking).toEqual({
      stage: 'place',
      picks: [END, OTHER_LINE],
    })
  })

  it('an axis needs a second operand like a point', () => {
    const axis = originSelectionItem(SKETCH_ID, 'x_axis')
    expect(click(DIMENSION_START, axis).picking).toEqual({ stage: 'point', first: axis })
  })

  it('ignores a second click on the first item', () => {
    const afterLine = click(DIMENSION_START, LINE).picking
    expect(click(afterLine, LINE).picking).toBe(afterLine)
  })

  it('lists the picked items', () => {
    expect(pickedItems(DIMENSION_START)).toEqual([])
    expect(pickedItems({ stage: 'line', first: LINE })).toEqual([LINE])
    expect(pickedItems({ stage: 'place', picks: [LINE, END] })).toEqual([LINE, END])
  })
})

describe('dimensionPreview', () => {
  it('measures a circle as a diameter while placing', () => {
    const preview = dimensionPreview({ stage: 'place', picks: [CIRCLE] }, [50, 5], SOLUTION)
    expect(preview?.value).toBe(10)
    expect(preview?.constraint).toMatchObject({ type: 'diameter', curve: 'e-3', expr: '0' })
  })

  it("previews a line's length after the line is picked", () => {
    const preview = dimensionPreview({ stage: 'line', first: LINE }, [10, -4], SOLUTION)
    expect(preview?.value).toBe(20)
  })

  it('is null before anything is picked and after a lone point', () => {
    expect(dimensionPreview(DIMENSION_START, [0, 0], SOLUTION)).toBeNull()
    expect(dimensionPreview({ stage: 'point', first: END }, [0, 0], SOLUTION)).toBeNull()
  })
})
