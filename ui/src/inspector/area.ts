import { formatNumber, MILLIMETERS_PER_UNIT } from '../expression/format'
import type { Units } from '../generated/protocol'

/** An area in mm² written in the document unit squared: `400 mm²`, `4 cm²` (3.8). */
export function formatArea(area: number, units: Units): string {
  // formatNumber divides a length by the unit once; dividing first makes it the unit squared.
  return `${formatNumber(area / MILLIMETERS_PER_UNIT[units], 'length', units)} ${units}²`
}
