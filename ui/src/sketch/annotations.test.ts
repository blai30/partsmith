import { describe, expect, it } from 'vitest'
import type { SketchConstraint } from '../generated/protocol'
import { dimensionGeometry, dimensionLabelText } from './annotations'
import { circleEntity, lineEntity, pointEntity, solutionFor } from './fixtures.testing'

// Dimension labels and geometry (ui.md 3.16.2).

type DimensionConstraint = Extract<SketchConstraint, { expr: string }>

const LENGTH: DimensionConstraint = { id: 'c-1', type: 'distance', line: 'e-1', expr: '40' }

describe('dimensionLabelText', () => {
  it('is the solved value without a unit', () => {
    expect(dimensionLabelText(LENGTH, 40, 'mm')).toBe('40')
    expect(dimensionLabelText(LENGTH, 25.4, 'in')).toBe('1')
  })

  it('prefixes ⌀ for a diameter and R for a radius', () => {
    expect(
      dimensionLabelText({ id: 'c-2', type: 'diameter', curve: 'e-2', expr: '10' }, 10, 'mm'),
    ).toBe('⌀10')
    expect(
      dimensionLabelText({ id: 'c-3', type: 'radius', curve: 'e-2', expr: '5' }, 5, 'mm'),
    ).toBe('R5')
  })

  it('suffixes ° for an angle', () => {
    const angle: DimensionConstraint = {
      id: 'c-4',
      type: 'angle',
      lines: ['x_axis', 'e-1'],
      expr: '30 deg',
    }
    expect(dimensionLabelText(angle, Math.PI / 6, 'mm')).toBe('30°')
  })

  it('prefixes fx for an expression that is not a plain number', () => {
    expect(dimensionLabelText({ ...LENGTH, expr: 'width / 2' }, 20, 'mm')).toBe('fx 20')
  })

  it('wraps a reference dimension in parentheses, after the other marks', () => {
    expect(dimensionLabelText({ ...LENGTH, driving: false }, 40, 'mm')).toBe('(40)')
    expect(dimensionLabelText({ ...LENGTH, expr: 'width', driving: false }, 40, 'mm')).toBe(
      '(fx 40)',
    )
  })

  it('shows the expression when the solution has no value', () => {
    expect(dimensionLabelText({ ...LENGTH, expr: '12.5' }, null, 'mm')).toBe('12.5')
  })
})

describe('dimensionGeometry', () => {
  it('draws extension lines out to the label and an arrowed line between their feet', () => {
    const solution = solutionFor([lineEntity('e-1', [0, 0], [20, 0])])
    expect(dimensionGeometry(LENGTH, solution, [10, 5])).toEqual({
      lines: [
        [
          [0, 0],
          [0, 5],
        ],
        [
          [20, 0],
          [20, 5],
        ],
        [
          [0, 5],
          [20, 5],
        ],
      ],
      arcs: [],
      arrows: [
        { tip: [0, 5], toward: [20, 5] },
        { tip: [20, 5], toward: [0, 5] },
      ],
    })
  })

  it('runs a horizontal distance along X whatever the slant of its points', () => {
    const solution = solutionFor([pointEntity('e-1', [0, 0]), pointEntity('e-2', [10, 5])])
    const horizontal: DimensionConstraint = {
      id: 'c-2',
      type: 'horizontal_distance',
      points: ['e-1', 'e-2'],
      expr: '10',
    }
    const geometry = dimensionGeometry(horizontal, solution, [5, 10])
    expect(geometry.lines.at(-1)).toEqual([
      [0, 10],
      [10, 10],
    ])
  })

  it('draws a radius from the center out to a label outside the circle', () => {
    const solution = solutionFor([circleEntity('e-1', [0, 0], 5)])
    const radius: DimensionConstraint = { id: 'c-3', type: 'radius', curve: 'e-1', expr: '5' }
    expect(dimensionGeometry(radius, solution, [10, 0])).toEqual({
      lines: [
        [
          [0, 0],
          [10, 0],
        ],
      ],
      arcs: [],
      arrows: [{ tip: [5, 0], toward: [0, 0] }],
    })
  })

  it('draws a diameter across the circle with arrows at both ends for a label inside', () => {
    const solution = solutionFor([circleEntity('e-1', [0, 0], 5)])
    const diameter: DimensionConstraint = { id: 'c-4', type: 'diameter', curve: 'e-1', expr: '10' }
    const geometry = dimensionGeometry(diameter, solution, [1, 0])
    const [
      [far, near] = [
        [0, 0],
        [0, 0],
      ],
    ] = geometry.lines
    expect(far[0]).toBeCloseTo(-5, 9)
    expect(far[1]).toBeCloseTo(0, 9)
    expect(near).toEqual([5, 0])
    expect(geometry.arrows).toHaveLength(2)
  })

  it('draws an angle arc through the label in the sector that holds it, with an arm along an axis', () => {
    const solution = solutionFor(
      [lineEntity('e-1', [0, 0], [Math.cos(Math.PI / 3), Math.sin(Math.PI / 3)])],
      {
        constraints: [{ id: 'c-5', state: 'satisfied', value: Math.PI / 3 }],
      },
    )
    const angle: DimensionConstraint = {
      id: 'c-5',
      type: 'angle',
      lines: ['x_axis', 'e-1'],
      expr: '60 deg',
    }
    const labelAt: [number, number] = [2 * Math.cos(Math.PI / 6), 2 * Math.sin(Math.PI / 6)]
    const geometry = dimensionGeometry(angle, solution, labelAt)
    expect(geometry.arcs).toHaveLength(1)
    const [arc] = geometry.arcs
    expect(arc?.center).toEqual([0, 0])
    expect(arc?.radius).toBeCloseTo(2, 9)
    expect(arc?.start).toBeCloseTo(0, 9)
    expect(arc?.end).toBeCloseTo(Math.PI / 3, 9)
    // The X axis has no ends, so its arm is drawn out to the arc.
    expect(geometry.lines).toHaveLength(1)
  })

  it('draws nothing for a dimension whose geometry is gone', () => {
    expect(dimensionGeometry(LENGTH, solutionFor([]), [0, 0])).toEqual({
      lines: [],
      arcs: [],
      arrows: [],
    })
  })
})
