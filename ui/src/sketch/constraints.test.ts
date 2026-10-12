import { describe, expect, it } from 'vitest'
import {
  circleEntity,
  curveItem,
  lineEntity,
  pointEntity,
  pointItem,
  sequentialIds,
  solutionFor,
} from './fixtures.testing'
import {
  applicableConstraints,
  constraintFor,
  constraintRequirement,
  constraintTitle,
  joinTitles,
  type ConstraintType,
} from './constraints'

// Two lines, two point entities and two circles, far enough apart that nothing coincides.
const SOLUTION = solutionFor([
  lineEntity('e-1', [0, 0], [40, 0]),
  lineEntity('e-2', [0, 10], [40, 30]),
  pointEntity('e-3', [10, 20]),
  pointEntity('e-4', [30, 20]),
  circleEntity('e-5', [60, 0], 5),
  circleEntity('e-6', [80, 0], 8),
])

const FIRST_LINE = curveItem('e-1', 'line')
const SECOND_LINE = curveItem('e-2', 'line')
const FIRST_POINT = pointItem('e-3')
const SECOND_POINT = pointItem('e-4')
const FIRST_CIRCLE = curveItem('e-5', 'circle')
const SECOND_CIRCLE = curveItem('e-6', 'circle')

describe('applicableConstraints', () => {
  it('U-57 two lines give Parallel, Perpendicular, Collinear, Equal, Fix', () => {
    expect(applicableConstraints([FIRST_LINE, SECOND_LINE], SOLUTION)).toEqual([
      'parallel',
      'perpendicular',
      'collinear',
      'equal',
      'fix',
    ])
  })

  it('U-57 one line gives Horizontal, Vertical, Fix', () => {
    expect(applicableConstraints([FIRST_LINE], SOLUTION)).toEqual(['horizontal', 'vertical', 'fix'])
  })

  it('U-57 two points give Coincident, Horizontal, Vertical, Fix', () => {
    expect(applicableConstraints([FIRST_POINT, SECOND_POINT], SOLUTION)).toEqual([
      'coincident',
      'horizontal',
      'vertical',
      'fix',
    ])
  })

  it('U-57 a point and a line give Point on object, Midpoint, Fix', () => {
    expect(applicableConstraints([FIRST_POINT, FIRST_LINE], SOLUTION)).toEqual([
      'point_on_object',
      'midpoint',
      'fix',
    ])
  })

  it('U-57 two points and a line give Symmetric, Fix', () => {
    expect(applicableConstraints([FIRST_POINT, SECOND_POINT, FIRST_LINE], SOLUTION)).toEqual([
      'symmetric',
      'fix',
    ])
  })

  it('U-57 a line and a circle give Tangent, Fix', () => {
    expect(applicableConstraints([FIRST_LINE, FIRST_CIRCLE], SOLUTION)).toEqual(['tangent', 'fix'])
  })

  it('U-57 two circles give Tangent, Concentric, Equal, Fix', () => {
    expect(applicableConstraints([FIRST_CIRCLE, SECOND_CIRCLE], SOLUTION)).toEqual([
      'tangent',
      'concentric',
      'equal',
      'fix',
    ])
  })

  it('the order of the selection does not change the result', () => {
    expect(applicableConstraints([FIRST_LINE, FIRST_POINT], SOLUTION)).toEqual([
      'point_on_object',
      'midpoint',
      'fix',
    ])
    expect(applicableConstraints([FIRST_LINE, SECOND_POINT, FIRST_POINT], SOLUTION)).toEqual([
      'symmetric',
      'fix',
    ])
  })

  it('an empty selection gives nothing', () => {
    expect(applicableConstraints([], SOLUTION)).toEqual([])
  })
})

describe('constraintFor', () => {
  it.each([
    ['line first', [FIRST_LINE, FIRST_POINT, SECOND_POINT]],
    ['line second', [FIRST_POINT, FIRST_LINE, SECOND_POINT]],
    ['line last', [FIRST_POINT, SECOND_POINT, FIRST_LINE]],
  ])(
    'U-58 symmetric uses the line as the axis whatever the selection order (%s)',
    (_order, selection) => {
      const constraint = constraintFor('symmetric', selection, SOLUTION, sequentialIds())
      expect(constraint).toMatchObject({ type: 'symmetric', line: 'e-1' })
      expect(constraint).toHaveProperty('points', expect.arrayContaining(['e-3', 'e-4']))
      expect(constraint).toHaveProperty('points.length', 2)
    },
  )

  it('U-58 coincident with a point and a line returns a point_on_object constraint', () => {
    const constraint = constraintFor(
      'coincident',
      [FIRST_POINT, FIRST_LINE],
      SOLUTION,
      sequentialIds(),
    )
    expect(constraint).toEqual({ id: 'c-1', type: 'point_on_object', point: 'e-3', curve: 'e-1' })
  })

  it('coincident with two points returns a coincident constraint on both', () => {
    const constraint = constraintFor(
      'coincident',
      [FIRST_POINT, SECOND_POINT],
      SOLUTION,
      sequentialIds(),
    )
    expect(constraint).toEqual({ id: 'c-1', type: 'coincident', points: ['e-3', 'e-4'] })
  })

  it('horizontal on one line returns the line form', () => {
    const constraint = constraintFor('horizontal', [FIRST_LINE], SOLUTION, sequentialIds())
    expect(constraint).toEqual({ id: 'c-1', type: 'horizontal', line: 'e-1' })
  })

  it('parallel on two lines names both lines', () => {
    const constraint = constraintFor(
      'parallel',
      [FIRST_LINE, SECOND_LINE],
      SOLUTION,
      sequentialIds(),
    )
    expect(constraint).toEqual({ id: 'c-1', type: 'parallel', lines: ['e-1', 'e-2'] })
  })

  it('returns null when the selection is not a valid operand set', () => {
    expect(constraintFor('parallel', [FIRST_LINE], SOLUTION, sequentialIds())).toBeNull()
    expect(
      constraintFor('concentric', [FIRST_LINE, FIRST_CIRCLE], SOLUTION, sequentialIds()),
    ).toBeNull()
  })
})

describe('constraintRequirement', () => {
  const REQUIREMENTS: readonly (readonly [ConstraintType, string])[] = [
    ['coincident', 'Select two points, or a point and a curve.'],
    ['point_on_object', 'Select a point and a curve.'],
    ['horizontal', 'Select a line or two points.'],
    ['vertical', 'Select a line or two points.'],
    ['parallel', 'Select two lines.'],
    ['perpendicular', 'Select two lines.'],
    ['tangent', 'Select a line and a circle or arc, or two circles or arcs.'],
    ['collinear', 'Select two lines.'],
    ['concentric', 'Select two circles or arcs.'],
    ['equal', 'Select two lines, or two circles or arcs.'],
    ['symmetric', 'Select two points and a line.'],
    ['midpoint', 'Select a point and a line.'],
    ['fix', 'Select points or curves.'],
  ]

  it.each(REQUIREMENTS)('the requirement text of %s is "%s" (3.16.6)', (type, text) => {
    expect(constraintRequirement(type)).toBe(text)
  })
})

describe('joinTitles (3.16.6)', () => {
  it('joins titles as a sentence list', () => {
    expect(joinTitles([])).toBe('')
    expect(joinTitles(['Horizontal'])).toBe('Horizontal')
    expect(joinTitles(['Horizontal', 'Distance 40'])).toBe('Horizontal and Distance 40')
    expect(joinTitles(['Horizontal', 'Equal', 'Distance 40'])).toBe(
      'Horizontal, Equal and Distance 40',
    )
  })
})

describe('constraintTitle (3.16.6)', () => {
  it('titles a dimension with its solved value as the label shows it', () => {
    const solution = solutionFor([], {
      constraints: [
        { id: 'c-1', state: 'satisfied', value: 40 },
        { id: 'c-2', state: 'satisfied', value: Math.PI / 6 },
      ],
    })
    expect(
      constraintTitle({ id: 'c-1', type: 'distance', line: 'e-1', expr: 'w' }, solution, 'mm'),
    ).toBe('Distance 40')
    expect(
      constraintTitle(
        { id: 'c-2', type: 'angle', lines: ['x_axis', 'e-1'], expr: '30 deg' },
        solution,
        'mm',
      ),
    ).toBe('Angle 30°')
    expect(
      constraintTitle({ id: 'c-3', type: 'parallel', lines: ['e-1', 'e-2'] }, null, 'mm'),
    ).toBe('Parallel')
  })
})
