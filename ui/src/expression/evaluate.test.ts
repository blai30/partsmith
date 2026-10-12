import { describe, expect, it } from 'vitest'
import type { Dimension } from '../generated/protocol'
import { convertForField } from './evaluate'

const LENGTH: Dimension = { length: 1, angle: 0 }
const ANGLE: Dimension = { length: 0, angle: 1 }
const NONE: Dimension = { length: 0, angle: 0 }
const AREA: Dimension = { length: 2, angle: 0 }

describe('convertForField', () => {
  it('U-38 converts a plain 2 for a length in an inch document to 50.8 mm', () => {
    expect(convertForField({ value: 2, dimension: NONE }, 'length', 'in', 'Distance', '2')).toEqual(
      {
        ok: true,
        value: 50.8,
      },
    )
  })

  it('U-38 converts a plain 90 for an angle to π/2', () => {
    const result = convertForField({ value: 90, dimension: NONE }, 'angle', 'mm', 'Angle', '90')
    expect(result.ok).toBe(true)
    expect(result.ok && result.value).toBeCloseTo(Math.PI / 2, 12)
  })

  it('U-38 rounds 3.0000000001 for a count to 3', () => {
    expect(
      convertForField(
        { value: 3.0000000001, dimension: NONE },
        'count',
        'mm',
        'Count',
        '3.0000000001',
      ),
    ).toEqual({ ok: true, value: 3 })
  })

  it('U-38 explains that 2.5 is not a whole number', () => {
    expect(convertForField({ value: 2.5, dimension: NONE }, 'count', 'mm', 'Count', '2.5')).toEqual(
      {
        ok: false,
        message: 'Count must be a whole number, but it is 2.5. Use 2 or 3.',
      },
    )
  })

  it('U-38 explains that an angle does not fit a length', () => {
    expect(
      convertForField(
        { value: Math.PI / 18, dimension: ANGLE },
        'length',
        'mm',
        'Distance',
        '10 deg',
      ),
    ).toEqual({
      ok: false,
      message: "Distance must be a length, but '10 deg' is an angle. Use a value such as 10 mm.",
    })
  })

  it('keeps a length in millimeters whatever the document unit', () => {
    expect(
      convertForField({ value: 12.5, dimension: LENGTH }, 'length', 'in', 'Distance', '12.5 mm'),
    ).toEqual({
      ok: true,
      value: 12.5,
    })
  })

  it('reads a plain number for a length in the document unit', () => {
    expect(convertForField({ value: 3, dimension: NONE }, 'length', 'cm', 'Distance', '3')).toEqual(
      {
        ok: true,
        value: 30,
      },
    )
    expect(convertForField({ value: 3, dimension: NONE }, 'length', 'm', 'Distance', '3')).toEqual({
      ok: true,
      value: 3000,
    })
    expect(convertForField({ value: 3, dimension: NONE }, 'length', 'mm', 'Distance', '3')).toEqual(
      {
        ok: true,
        value: 3,
      },
    )
  })

  it('keeps an angle in radians', () => {
    expect(
      convertForField({ value: 0.5, dimension: ANGLE }, 'angle', 'mm', 'Angle', '0.5 rad'),
    ).toEqual({
      ok: true,
      value: 0.5,
    })
  })

  it('keeps a plain number for a scalar', () => {
    expect(
      convertForField({ value: 1.5, dimension: NONE }, 'scalar', 'mm', 'Factor', '1.5'),
    ).toEqual({
      ok: true,
      value: 1.5,
    })
  })

  it('explains that a length does not fit an angle', () => {
    expect(
      convertForField({ value: 5, dimension: LENGTH }, 'angle', 'mm', 'Angle', '5 mm'),
    ).toEqual({
      ok: false,
      message: "Angle must be an angle, but '5 mm' is a length. Use a value such as 45 deg.",
    })
  })

  it('explains that a length does not fit a scalar', () => {
    expect(
      convertForField({ value: 3, dimension: LENGTH }, 'scalar', 'mm', 'Factor', '3 mm'),
    ).toEqual({
      ok: false,
      message:
        "Factor must be a plain number, but '3 mm' is a length. Use a plain number such as 2.",
    })
  })

  it('explains that a length does not fit a count', () => {
    expect(
      convertForField({ value: 3, dimension: LENGTH }, 'count', 'mm', 'Count', '3 mm'),
    ).toEqual({
      ok: false,
      message:
        "Count must be a whole number, but '3 mm' is a length. Use a whole number such as 4.",
    })
  })

  it('reads a plain number for an angle in degrees, and refuses an angle for a count', () => {
    expect(convertForField({ value: 2, dimension: NONE }, 'angle', 'mm', 'Angle', '2')).toEqual({
      ok: true,
      value: (2 * Math.PI) / 180,
    })
    expect(
      convertForField({ value: 2, dimension: ANGLE }, 'count', 'mm', 'Count', '2 rad'),
    ).toEqual({
      ok: false,
      message:
        "Count must be a whole number, but '2 rad' is an angle. Use a whole number such as 4.",
    })
  })

  it('names an area as an area', () => {
    expect(
      convertForField({ value: 6, dimension: AREA }, 'length', 'mm', 'Distance', '2 mm * 3 mm'),
    ).toEqual({
      ok: false,
      message:
        "Distance must be a length, but '2 mm * 3 mm' is an area. Use a value such as 10 mm.",
    })
  })

  it('names other dimensions by their exponents, leaving out zeros and writing 1 without ^', () => {
    expect(
      convertForField(
        { value: 1, dimension: { length: 3, angle: 0 } },
        'length',
        'mm',
        'Distance',
        'v',
      ),
    ).toEqual({
      ok: false,
      message: "Distance must be a length, but 'v' is a mm^3. Use a value such as 10 mm.",
    })
    expect(
      convertForField(
        { value: 1, dimension: { length: 1, angle: 1 } },
        'angle',
        'mm',
        'Angle',
        'w',
      ),
    ).toEqual({
      ok: false,
      message: "Angle must be an angle, but 'w' is a mm rad. Use a value such as 45 deg.",
    })
    expect(
      convertForField(
        { value: 1, dimension: { length: 0, angle: 2 } },
        'scalar',
        'mm',
        'Factor',
        'x',
      ),
    ).toEqual({
      ok: false,
      message: "Factor must be a plain number, but 'x' is a rad^2. Use a plain number such as 2.",
    })
  })

  it('refuses a negative count', () => {
    expect(convertForField({ value: -1, dimension: NONE }, 'count', 'mm', 'Count', '-1')).toEqual({
      ok: false,
      message: 'Count must be at least 0, but it is -1. Use 0 or more.',
    })
  })

  it('accepts 0 as a count', () => {
    expect(convertForField({ value: 0, dimension: NONE }, 'count', 'mm', 'Count', '0')).toEqual({
      ok: true,
      value: 0,
    })
  })
})
