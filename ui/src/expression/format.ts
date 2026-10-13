import type { BoundingBox, FieldUnit, Units } from '../generated/protocol'
import type { Vector3 } from '../viewport/math'

/** Millimeters in one document unit (model.md 2.4). */
export const MILLIMETERS_PER_UNIT: Readonly<Record<Units, number>> = {
  mm: 1,
  cm: 10,
  m: 1000,
  in: 25.4,
}

/** The document units, in menu order. */
export const DOCUMENT_UNITS: readonly Units[] = ['mm', 'cm', 'm', 'in']

/** Decimals a length keeps in each document unit. */
const LENGTH_DECIMALS: Readonly<Record<Units, number>> = { mm: 3, cm: 4, m: 6, in: 4 }

const ANGLE_DECIMALS = 2
const SCALAR_SIGNIFICANT_DIGITS = 6

/** `value × 10^exponent`, computed on the decimal digits so no binary rounding error creeps in. */
function shifted(value: number, exponent: number): number {
  const [mantissa = '0', power = '0'] = String(value).split('e')
  return Number(`${mantissa}e${String(Number(power) + exponent)}`)
}

/**
 * Rounds half away from zero to a number of decimals, on the value's shortest decimal digits, so
 * 1.005 rounds to 1.01 as it reads. The result has at most that many decimals.
 */
export function roundToDecimals(value: number, decimals: number): number {
  if (!Number.isFinite(value)) {
    return value
  }
  const magnitude = shifted(Math.round(shifted(Math.abs(value), decimals)), -decimals)
  return value < 0 ? -magnitude : magnitude
}

/** The shortest decimal text of a number, with `-0` written `0` and no digit grouping. */
export function numberText(value: number): string {
  return Object.is(value, -0) ? '0' : String(value)
}

/** A value in the field's display unit, without the unit: lengths in the document unit, angles in degrees. */
export function formatNumber(value: number, unit: FieldUnit, units: Units): string {
  switch (unit) {
    case 'length':
      return numberText(
        roundToDecimals(value / MILLIMETERS_PER_UNIT[units], LENGTH_DECIMALS[units]),
      )
    case 'angle':
      return numberText(roundToDecimals((value * 180) / Math.PI, ANGLE_DECIMALS))
    case 'scalar':
      return numberText(Number(value.toPrecision(SCALAR_SIGNIFICANT_DIGITS)))
    case 'count':
      return numberText(Math.round(value))
  }
}

/** A value with its unit: `12.5 mm`, `45°`; scalars and counts have no suffix. */
export function formatQuantity(value: number, unit: FieldUnit, units: Units): string {
  const text = formatNumber(value, unit, units)
  switch (unit) {
    case 'length':
      return `${text} ${units}`
    case 'angle':
      return `${text}°`
    case 'scalar':
    case 'count':
      return text
  }
}

/** The extents of a box in the document unit: `40 × 20 × 10 mm`. */
export function formatSize(bounds: BoundingBox, units: Units): string {
  const [minimumX = 0, minimumY = 0, minimumZ = 0] = bounds.min
  const [maximumX = 0, maximumY = 0, maximumZ = 0] = bounds.max
  const extents = [maximumX - minimumX, maximumY - minimumY, maximumZ - minimumZ].map((extent) =>
    formatNumber(extent, 'length', units),
  )
  return `${extents.join(' × ')} ${units}`
}

/** A point in the document unit: `10, 0, 10 mm`. */
export function formatPoint(point: Vector3, units: Units): string {
  const coordinates = point.map((coordinate) => formatNumber(coordinate, 'length', units))
  return `${coordinates.join(', ')} ${units}`
}
