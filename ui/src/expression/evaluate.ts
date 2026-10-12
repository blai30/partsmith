import type { Dimension, FieldUnit, Units } from '../generated/protocol'
import { formatNumber, formatQuantity, MILLIMETERS_PER_UNIT } from './format'

// What a field's value will be (ui.md 2.13), mirroring features.md 3.4: an evaluate() result
// converted for the field's unit, or the reason it does not fit.

export type FieldValue =
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly message: string }

/** The largest count a field takes (u32). */
const MAXIMUM_COUNT = 4_294_967_295
/** How close to a whole number a count may be. */
const WHOLE_TOLERANCE = 1e-9

const EXPECTED: Readonly<Record<FieldUnit, { readonly name: string; readonly example: string }>> = {
  length: { name: 'a length', example: 'a value such as 10 mm' },
  angle: { name: 'an angle', example: 'a value such as 45 deg' },
  scalar: { name: 'a plain number', example: 'a plain number such as 2' },
  count: { name: 'a whole number', example: 'a whole number such as 4' },
}

function isDimension(dimension: Dimension, length: number, angle: number): boolean {
  return dimension.length === length && dimension.angle === angle
}

function exponentPart(symbol: string, exponent: number): string | null {
  if (exponent === 0) {
    return null
  }
  return exponent === 1 ? symbol : `${symbol}^${String(exponent)}`
}

/** A dimension's units in internal units, such as `mm^3` or `mm rad`; empty for a plain number. */
export function dimensionUnits(dimension: Dimension): string {
  return [exponentPart('mm', dimension.length), exponentPart('rad', dimension.angle)]
    .filter((part) => part !== null)
    .join(' ')
}

/** What a value of this dimension is, for messages: `a length`, `an area`, `a mm^3`. */
export function dimensionName(dimension: Dimension): string {
  if (isDimension(dimension, 1, 0)) {
    return 'a length'
  }
  if (isDimension(dimension, 0, 1)) {
    return 'an angle'
  }
  if (isDimension(dimension, 0, 0)) {
    return 'a plain number'
  }
  if (isDimension(dimension, 2, 0)) {
    return 'an area'
  }
  return `a ${dimensionUnits(dimension)}`
}

/** A value of any dimension as the Parameters panel shows it: lengths and angles as fields do, others in internal units. */
export function formatEvaluated(value: number, dimension: Dimension, units: Units): string {
  if (isDimension(dimension, 1, 0)) {
    return formatQuantity(value, 'length', units)
  }
  if (isDimension(dimension, 0, 1)) {
    return formatQuantity(value, 'angle', units)
  }
  const number = formatNumber(value, 'scalar', units)
  return isDimension(dimension, 0, 0) ? number : `${number} ${dimensionUnits(dimension)}`
}

function wrongDimension(
  unit: FieldUnit,
  dimension: Dimension,
  label: string,
  source: string,
): FieldValue {
  const expected = EXPECTED[unit]
  return {
    ok: false,
    message: `${label} must be ${expected.name}, but '${source}' is ${dimensionName(dimension)}. Use ${expected.example}.`,
  }
}

function countValue(value: number, label: string, units: Units): FieldValue {
  const text = formatNumber(value, 'scalar', units)
  const nearest = Math.round(value)
  const whole = Math.abs(value - nearest) <= WHOLE_TOLERANCE
  if (value < 0 && !(whole && nearest === 0)) {
    return { ok: false, message: `${label} must be at least 0, but it is ${text}. Use 0 or more.` }
  }
  if (!whole) {
    return {
      ok: false,
      message: `${label} must be a whole number, but it is ${text}. Use ${String(Math.floor(value))} or ${String(Math.ceil(value))}.`,
    }
  }
  if (nearest > MAXIMUM_COUNT) {
    return {
      ok: false,
      message: `${label} must be at most ${String(MAXIMUM_COUNT)}, but it is ${text}. Use ${String(MAXIMUM_COUNT)} or less.`,
    }
  }
  // Adding 0 turns -0 into 0.
  return { ok: true, value: nearest + 0 }
}

/** Converts an evaluate() result for a field of the given unit, or explains why it does not fit. */
export function convertForField(
  result: { value: number; dimension: Dimension },
  unit: FieldUnit,
  units: Units,
  label: string,
  source: string,
): FieldValue {
  const { value, dimension } = result
  const plain = isDimension(dimension, 0, 0)
  switch (unit) {
    case 'length':
      if (isDimension(dimension, 1, 0)) {
        return { ok: true, value }
      }
      // A plain number means the document unit.
      return plain
        ? { ok: true, value: value * MILLIMETERS_PER_UNIT[units] }
        : wrongDimension(unit, dimension, label, source)
    case 'angle':
      if (isDimension(dimension, 0, 1)) {
        return { ok: true, value }
      }
      // A plain number means degrees.
      return plain
        ? { ok: true, value: (value * Math.PI) / 180 }
        : wrongDimension(unit, dimension, label, source)
    case 'scalar':
      return plain ? { ok: true, value } : wrongDimension(unit, dimension, label, source)
    case 'count':
      return plain
        ? countValue(value, label, units)
        : wrongDimension(unit, dimension, label, source)
  }
}
