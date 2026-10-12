import { describe, expect, it } from 'vitest'
import type { SketchConstraint, SketchSolution } from '../generated/protocol'
import { drawClick, drawFields, drawPreview, IDLE, type DrawInput, type DrawState } from './drawing'
import { lineEntity, sequentialIds, solutionFor } from './fixtures.testing'
import type { Vector2 } from './geometry'

// The drawing tools' clicks and rubber bands (3.16.4, 3.16.5) at 0.1 mm per pixel.

const EDGE = lineEntity('e-edge', [20, 5], [10, 5])
const SOLUTION: SketchSolution = solutionFor([EDGE])

function input(cursor: Vector2, fields: Partial<DrawInput> = {}): DrawInput {
  return {
    cursor,
    millimetersPerPixel: 0.1,
    uiScale: 1,
    suppressed: false,
    solution: SOLUTION,
    typed: {},
    ...fields,
  }
}

function withoutIds(constraints: readonly SketchConstraint[] | undefined): unknown[] {
  return (constraints ?? []).map(({ id: _id, ...rest }) => rest)
}

describe('rectangles', () => {
  it('a 2-point rectangle applies a snap at C1 to L1.start as an inferred coincident', () => {
    const ids = sequentialIds()
    const first = drawClick('rectangle_2point', IDLE, input([0.05, 0.05]), ids, false)
    expect(first.commit).toBeNull()
    const second = drawClick('rectangle_2point', first.state, input([40, 20]), ids, false)
    expect(second.state).toBe(IDLE)
    const change = second.commit?.change
    expect(second.commit?.label).toBe('Rectangle')
    expect(change?.add?.entities).toHaveLength(4)
    expect(withoutIds(change?.add?.constraints).at(-1)).toEqual({
      type: 'coincident',
      points: ['e-1.start', 'origin'],
    })
    expect(change?.inferred).toEqual(['c-9'])
    expect(change?.requested).toHaveLength(8)
  })

  it('a center rectangle applies a snap at P to its center point Q', () => {
    const ids = sequentialIds()
    const first = drawClick('rectangle_center', IDLE, input([0, 0.05]), ids, false)
    const second = drawClick('rectangle_center', first.state, input([20, 10]), ids, true)
    const added = second.commit?.change.add
    expect(added?.entities.every((entity) => entity.construction === true)).toBe(true)
    expect(withoutIds(added?.constraints).at(-1)).toEqual({
      type: 'coincident',
      points: ['e-6', 'origin'],
    })
  })

  it('ignores a rectangle under 2 pixels high', () => {
    const first = drawClick('rectangle_2point', IDLE, input([30, 30]), sequentialIds(), false)
    const second = drawClick(
      'rectangle_2point',
      first.state,
      input([40, 30.1]),
      sequentialIds(),
      false,
    )
    expect(second).toEqual({ state: first.state, commit: null })
  })

  it('typed width and height take their signs from the cursor quadrant', () => {
    const ids = sequentialIds()
    const first = drawClick('rectangle_2point', IDLE, input([30, 30]), ids, false)
    const typed = { Width: { text: '12', value: 12 }, Height: { text: '5', value: 5 } }
    const preview = drawPreview('rectangle_2point', first.state, input([25, 20], { typed }))
    expect(preview.measures).toEqual({ Width: 12, Height: 5 })
    expect(preview.shapes[0]).toEqual({ kind: 'line', start: [30, 30], end: [18, 30] })
    const second = drawClick(
      'rectangle_2point',
      first.state,
      input([25, 20], { typed }),
      ids,
      false,
    )
    expect(withoutIds(second.commit?.change.add?.constraints).slice(-2)).toEqual([
      { type: 'distance', line: 'e-1', expr: '12' },
      { type: 'distance', line: 'e-4', expr: '5' },
    ])
  })
})

describe('circle', () => {
  it('a point snap on the rim adds point_on_object for that point', () => {
    const ids = sequentialIds()
    const first = drawClick('circle', IDLE, input([10, 15]), ids, false)
    const second = drawClick('circle', first.state, input([10.03, 5.02]), ids, false)
    const added = second.commit?.change.add
    expect(added?.entities).toEqual([{ id: 'e-1', type: 'circle', center: [10, 15], radius: 10 }])
    expect(withoutIds(added?.constraints)).toEqual([
      { type: 'point_on_object', point: 'e-edge.end', curve: 'e-1' },
    ])
  })

  it('shows the Diameter draw input after the first click only', () => {
    const state: DrawState = { stage: 'second', first: { at: [0, 0], snap: null } }
    expect(drawFields('circle', IDLE)).toEqual([])
    expect(drawFields('circle', state)).toEqual([{ name: 'Diameter', unit: 'length' }])
  })
})

describe('3-point arc', () => {
  it('stores a clockwise pick counterclockwise, so the snap at A applies to the arc end', () => {
    const ids = sequentialIds()
    const first = drawClick('arc_3point', IDLE, input([10.02, 5]), ids, false)
    const second = drawClick('arc_3point', first.state, input([-10, 5]), ids, false)
    expect(drawFields('arc_3point', second.state)).toEqual([{ name: 'Radius', unit: 'length' }])
    const third = drawClick('arc_3point', second.state, input([0, -5]), ids, false)
    const [arc] = third.commit?.change.add?.entities ?? []
    expect(arc).toMatchObject({ type: 'arc', center: [0, 5], radius: 10 })
    expect(withoutIds(third.commit?.change.add?.constraints)).toEqual([
      { type: 'coincident', points: [`${arc?.id ?? ''}.end`, 'e-edge.end'] },
    ])
  })

  it('ignores the third click while the three points are in a line', () => {
    const ids = sequentialIds()
    const first = drawClick('arc_3point', IDLE, input([0, 10]), ids, false)
    const second = drawClick('arc_3point', first.state, input([20, 10]), ids, false)
    const third = drawClick('arc_3point', second.state, input([10, 10]), ids, false)
    expect(third).toEqual({ state: second.state, commit: null })
  })
})

describe('line', () => {
  const start: DrawState = {
    stage: 'line',
    start: { at: [0, 30], snap: null },
    previousEnd: null,
    chainStart: [0, 30],
  }

  it('a typed angle of 90 degrees makes the line vertical', () => {
    const typed = { Angle: { text: '90', value: Math.PI / 2 }, Length: { text: '8', value: 8 } }
    const step = drawClick('line', start, input([3, 40], { typed }), sequentialIds(), false)
    const [line] = step.commit?.change.add?.entities ?? []
    expect(line).toMatchObject({ start: [0, 30] })
    expect(withoutIds(step.commit?.change.add?.constraints)).toEqual([
      { type: 'distance', line: 'e-1', expr: '8' },
      { type: 'vertical', line: 'e-1' },
    ])
  })

  it('another typed angle is measured from the sketch X axis', () => {
    const typed = { Angle: { text: '30 deg', value: Math.PI / 6 } }
    const step = drawClick('line', start, input([10, 36]), sequentialIds(), false)
    expect(step.commit).not.toBeNull()
    const angled = drawClick('line', start, input([10, 36], { typed }), sequentialIds(), false)
    expect(withoutIds(angled.commit?.change.add?.constraints)).toEqual([
      { type: 'angle', lines: ['x_axis', 'e-1'], expr: '30 deg' },
    ])
  })

  it('a click on the chain start closes the chain', () => {
    const solution = solutionFor([lineEntity('e-7', [0, 30], [10, 30])])
    const chained: DrawState = {
      stage: 'line',
      start: { at: [10, 40], snap: null },
      previousEnd: 'e-8.end',
      chainStart: [0, 30],
    }
    const step = drawClick('line', chained, input([0.02, 30], { solution }), sequentialIds(), false)
    expect(step.state).toBe(IDLE)
    expect(withoutIds(step.commit?.change.add?.constraints)).toEqual([
      { type: 'coincident', points: ['e-8.end', 'e-1.start'] },
      { type: 'coincident', points: ['e-1.end', 'e-7.start'] },
    ])
  })

  it('suppression gives the raw cursor and no glyphs', () => {
    const preview = drawPreview('line', start, input([10.03, 0.02], { suppressed: true }))
    expect(preview.snap).toBeNull()
    expect(preview.glyphs).toEqual([])
    expect(preview.shapes).toEqual([{ kind: 'line', start: [0, 30], end: [10.03, 0.02] }])
  })
})

describe('point', () => {
  it('each click adds a point entity, coincident with a snapped point', () => {
    const step = drawClick('point', IDLE, input([20.03, 5]), sequentialIds(), false)
    expect(step.state).toBe(IDLE)
    expect(step.commit?.label).toBe('Point')
    expect(step.commit?.change).toEqual({
      add: {
        entities: [{ id: 'e-1', type: 'point', position: [20, 5] }],
        constraints: [{ id: 'c-1', type: 'coincident', points: ['e-1', 'e-edge.start'] }],
      },
      requested: [],
      inferred: ['c-1'],
    })
  })
})
