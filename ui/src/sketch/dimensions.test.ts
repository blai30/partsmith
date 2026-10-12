import { describe, expect, it } from 'vitest'
import {
  arcEntity,
  circleEntity,
  curveItem,
  lineEntity,
  pointEntity,
  pointItem,
  sequentialIds,
  solutionFor,
} from './fixtures.testing'
import { defaultLabelPosition, dimensionFor, dimensionTypeFor, measuredValue } from './dimensions'

const DIGITS = 9
const DEGREE = Math.PI / 180

// Two points that are not axis-aligned: x range [0, 10], y range [0, 5].
const SKEWED_POINTS = solutionFor([pointEntity('e-1', [0, 0]), pointEntity('e-2', [10, 5])])
const SKEWED_SELECTION = [pointItem('e-1'), pointItem('e-2')]

describe('dimensionTypeFor', () => {
  it('U-59 points (0, 0) and (10, 5) with the label at (5, 10) give horizontal_distance', () => {
    expect(dimensionTypeFor(SKEWED_SELECTION, [5, 10], SKEWED_POINTS)).toBe('horizontal_distance')
  })

  it('U-59 points (0, 0) and (10, 5) with the label at (15, 2) give vertical_distance', () => {
    expect(dimensionTypeFor(SKEWED_SELECTION, [15, 2], SKEWED_POINTS)).toBe('vertical_distance')
  })

  it('U-59 points (0, 0) and (10, 5) with the label at (12, 8) give distance', () => {
    expect(dimensionTypeFor(SKEWED_SELECTION, [12, 8], SKEWED_POINTS)).toBe('distance')
  })

  it('U-59 points (0, 0) and (10, 0) with the label at (5, 5) give distance', () => {
    const solution = solutionFor([pointEntity('e-1', [0, 0]), pointEntity('e-2', [10, 0])])
    expect(dimensionTypeFor([pointItem('e-1'), pointItem('e-2')], [5, 5], solution)).toBe(
      'distance',
    )
  })

  it('a line counts as its two ends (3.16.7)', () => {
    const solution = solutionFor([lineEntity('e-1', [0, 0], [10, 5])])
    const line = [curveItem('e-1', 'line')]
    expect(dimensionTypeFor(line, [5, 10], solution)).toBe('horizontal_distance')
    expect(dimensionTypeFor(line, [15, 2], solution)).toBe('vertical_distance')
    expect(dimensionTypeFor(line, [12, 8], solution)).toBe('distance')
  })

  it('a point and a line give distance (3.16.7)', () => {
    const solution = solutionFor([pointEntity('e-1', [5, 5]), lineEntity('e-2', [0, 0], [10, 0])])
    expect(dimensionTypeFor([pointItem('e-1'), curveItem('e-2', 'line')], [5, 2], solution)).toBe(
      'distance',
    )
  })

  it('U-59 a circle gives diameter', () => {
    const solution = solutionFor([circleEntity('e-1', [0, 0], 5)])
    expect(dimensionTypeFor([curveItem('e-1', 'circle')], [8, 8], solution)).toBe('diameter')
  })

  it('U-59 an arc gives radius', () => {
    const solution = solutionFor([arcEntity('e-1', [0, 0], 5, 0, Math.PI / 2)])
    expect(dimensionTypeFor([curveItem('e-1', 'arc')], [8, 8], solution)).toBe('radius')
  })

  it('U-59 lines at 90 degrees give angle', () => {
    const solution = solutionFor([
      lineEntity('e-1', [0, 0], [10, 0]),
      lineEntity('e-2', [0, 0], [0, 10]),
    ])
    expect(
      dimensionTypeFor([curveItem('e-1', 'line'), curveItem('e-2', 'line')], [3, 3], solution),
    ).toBe('angle')
  })

  it('U-59 parallel lines give distance', () => {
    const solution = solutionFor([
      lineEntity('e-1', [0, 0], [10, 0]),
      lineEntity('e-2', [0, 5], [10, 5]),
    ])
    expect(
      dimensionTypeFor([curveItem('e-1', 'line'), curveItem('e-2', 'line')], [5, 2.5], solution),
    ).toBe('distance')
  })

  it('U-59 a point and a circle are unsupported with the full cannot_dimension text', () => {
    const solution = solutionFor([pointEntity('e-1', [20, 0]), circleEntity('e-2', [0, 0], 5)])
    expect(
      dimensionTypeFor([pointItem('e-1'), curveItem('e-2', 'circle')], [10, 10], solution),
    ).toEqual({
      unsupported:
        'Partsmith cannot dimension a point and a circle. Pick two points, a point and a line, two lines, or one circle or arc.',
    })
  })
})

describe('measuredValue', () => {
  // A line along +X and a line at 60 degrees, both through the origin.
  const crossingLines = solutionFor([
    lineEntity('e-1', [-20, 0], [20, 0]),
    lineEntity(
      'e-2',
      [-10 * Math.cos(60 * DEGREE), -10 * Math.sin(60 * DEGREE)],
      [10 * Math.cos(60 * DEGREE), 10 * Math.sin(60 * DEGREE)],
    ),
  ])
  const bothLines = [curveItem('e-1', 'line'), curveItem('e-2', 'line')]

  it('U-60 the angle with the label at (cos 30°, sin 30°) is π/3', () => {
    const label = [Math.cos(30 * DEGREE), Math.sin(30 * DEGREE)] as const
    expect(measuredValue('angle', bothLines, label, crossingLines)).toBeCloseTo(Math.PI / 3, DIGITS)
  })

  it('U-60 the angle with the label at (-1, 0.5) is 2π/3', () => {
    expect(measuredValue('angle', bothLines, [-1, 0.5], crossingLines)).toBeCloseTo(
      (2 * Math.PI) / 3,
      DIGITS,
    )
  })

  it('the distance between two points is their separation', () => {
    expect(measuredValue('distance', SKEWED_SELECTION, [12, 8], SKEWED_POINTS)).toBeCloseTo(
      Math.hypot(10, 5),
      DIGITS,
    )
  })

  it('the diameter of a circle is twice its radius', () => {
    const solution = solutionFor([circleEntity('e-1', [0, 0], 5)])
    expect(measuredValue('diameter', [curveItem('e-1', 'circle')], [8, 8], solution)).toBeCloseTo(
      10,
      DIGITS,
    )
  })

  it('the radius of an arc is its radius', () => {
    const solution = solutionFor([arcEntity('e-1', [0, 0], 7, 0, Math.PI / 2)])
    expect(measuredValue('radius', [curveItem('e-1', 'arc')], [8, 8], solution)).toBeCloseTo(
      7,
      DIGITS,
    )
  })
})

describe('dimensionFor', () => {
  it('a driving length of a line is a distance on the line with the typed expression', () => {
    const constraint = dimensionFor(
      'distance',
      [curveItem('e-1', 'line')],
      '40 mm',
      true,
      [5, 5],
      sequentialIds(),
    )
    expect(constraint).toMatchObject({ id: 'c-1', type: 'distance', line: 'e-1', expr: '40 mm' })
    expect(constraint).not.toHaveProperty('driving', false)
  })

  it('a reference diameter of a circle has driving false', () => {
    const constraint = dimensionFor(
      'diameter',
      [curveItem('e-1', 'circle')],
      'hole_d',
      false,
      [8, 8],
      sequentialIds(),
    )
    expect(constraint).toMatchObject({
      id: 'c-1',
      type: 'diameter',
      curve: 'e-1',
      expr: 'hole_d',
      driving: false,
    })
  })
})

describe('dimensionFor operand order (3.16.7)', () => {
  it('orders the points of a horizontal distance so it reads positive', () => {
    const constraint = dimensionFor(
      'horizontal_distance',
      [pointItem('e-2'), pointItem('e-1')],
      '10',
      true,
      [5, 10],
      sequentialIds(),
      SKEWED_POINTS,
    )
    expect(constraint).toMatchObject({ type: 'horizontal_distance', points: ['e-1', 'e-2'] })
  })

  it('orders the ends of a line for a vertical distance so it reads positive', () => {
    const solution = solutionFor([lineEntity('e-1', [0, 5], [10, 0])])
    const constraint = dimensionFor(
      'vertical_distance',
      [curveItem('e-1', 'line')],
      '5',
      true,
      [15, 2],
      sequentialIds(),
      solution,
    )
    expect(constraint).toMatchObject({
      type: 'vertical_distance',
      points: ['e-1.end', 'e-1.start'],
    })
  })

  it('starts an angle at the line that bounds the labeled sector counterclockwise', () => {
    const solution = solutionFor([
      lineEntity('e-1', [0, 0], [1, 0]),
      lineEntity('e-2', [0, 0], [Math.cos(60 * DEGREE), Math.sin(60 * DEGREE)]),
    ])
    const lines = [curveItem('e-1', 'line'), curveItem('e-2', 'line')]
    const inside = dimensionFor(
      'angle',
      lines,
      '60 deg',
      true,
      [Math.cos(30 * DEGREE), Math.sin(30 * DEGREE)],
      sequentialIds(),
      solution,
    )
    expect(inside).toMatchObject({ type: 'angle', lines: ['e-1', 'e-2'] })
    const outside = dimensionFor(
      'angle',
      lines,
      '120 deg',
      true,
      [-1, 0.5],
      sequentialIds(),
      solution,
    )
    expect(outside).toMatchObject({ type: 'angle', lines: ['e-2', 'e-1'] })
  })

  it('keeps the selection order without a solution', () => {
    const constraint = dimensionFor(
      'horizontal_distance',
      [pointItem('e-2'), pointItem('e-1')],
      '10',
      true,
      [5, 10],
      sequentialIds(),
    )
    expect(constraint).toMatchObject({ points: ['e-2', 'e-1'] })
  })
})

describe('defaultLabelPosition', () => {
  const MILLIMETERS_PER_PIXEL = 0.1
  // Labels sit 28 CSS pixels from their geometry.
  const OFFSET = 28 * MILLIMETERS_PER_PIXEL

  it('puts a length beside its line, on the side away from the middle of the sketch', () => {
    const solution = solutionFor([lineEntity('e-1', [0, 0], [20, 0]), pointEntity('e-2', [10, 20])])
    const at = defaultLabelPosition(
      { id: 'c-1', type: 'distance', line: 'e-1', expr: '20' },
      solution,
      MILLIMETERS_PER_PIXEL,
    )
    expect(at[0]).toBeCloseTo(10, DIGITS)
    expect(at[1]).toBeCloseTo(-OFFSET, DIGITS)
  })

  it('puts a horizontal distance above or below its points, away from the middle', () => {
    const solution = solutionFor([
      pointEntity('e-1', [0, 10]),
      pointEntity('e-2', [10, 15]),
      pointEntity('e-3', [5, 0]),
    ])
    const at = defaultLabelPosition(
      { id: 'c-1', type: 'horizontal_distance', points: ['e-1', 'e-2'], expr: '10' },
      solution,
      MILLIMETERS_PER_PIXEL,
    )
    expect(at[0]).toBeCloseTo(5, DIGITS)
    expect(at[1]).toBeCloseTo(15 + OFFSET, DIGITS)
  })

  it('puts a diameter outside its circle at 45 degrees', () => {
    const solution = solutionFor([circleEntity('e-1', [0, 0], 5)])
    const at = defaultLabelPosition(
      { id: 'c-1', type: 'diameter', curve: 'e-1', expr: '10' },
      solution,
      MILLIMETERS_PER_PIXEL,
    )
    expect(at[0]).toBeCloseTo((5 + OFFSET) * Math.SQRT1_2, DIGITS)
    expect(at[1]).toBeCloseTo((5 + OFFSET) * Math.SQRT1_2, DIGITS)
  })
})
