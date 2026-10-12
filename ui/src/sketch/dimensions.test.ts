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
import { dimensionFor, dimensionTypeFor, measuredValue } from './dimensions'

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

  it('U-59 a point and a circle are unsupported with "Partsmith cannot dimension a point and a circle."', () => {
    const solution = solutionFor([pointEntity('e-1', [20, 0]), circleEntity('e-2', [0, 0], 5)])
    expect(
      dimensionTypeFor([pointItem('e-1'), curveItem('e-2', 'circle')], [10, 10], solution),
    ).toEqual({ unsupported: 'Partsmith cannot dimension a point and a circle.' })
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
