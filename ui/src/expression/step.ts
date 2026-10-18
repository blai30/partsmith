import type { FieldUnit, Units } from '../generated/protocol'
import { numberText, roundToDecimals } from './format'

// An optional sign, digits with an optional fraction, then an optional unit word, with the spacing
// around each part captured so a step can keep it exactly as typed.
const PLAIN_NUMBER = /^(\s*)([+-]?(?:\d+(?:\.\d*)?|\.\d+))(\s*)(mm|cm|m|in|deg|rad)?(\s*)$/

/** Step of a length field per unit: the unit word typed, else the document unit. */
const LENGTH_STEPS: Readonly<Record<Units, number>> = { mm: 1, cm: 0.1, m: 0.01, in: 0.1 }

/** An angle steps by one degree, which is this many radians when the typed word is rad. */
const RADIANS_PER_DEGREE = Math.PI / 180

const RESULT_DECIMALS = 10

type PlainNumber = {
  readonly leading: string
  readonly value: number
  readonly spacing: string
  readonly word: string
  readonly trailing: string
}

function parsePlainNumber(text: string): PlainNumber | null {
  const match = PLAIN_NUMBER.exec(text)
  if (match === null) {
    return null
  }
  const [, leading = '', number = '', spacing = '', word = '', trailing = ''] = match
  // A space with no unit word after it belongs to the trailing whitespace.
  return word === ''
    ? { leading, value: Number(number), spacing: '', word, trailing: spacing + trailing }
    : { leading, value: Number(number), spacing, word, trailing }
}

function isLengthUnit(word: string): word is Units {
  return Object.hasOwn(LENGTH_STEPS, word)
}

function baseStep(unit: FieldUnit, word: string, units: Units): number {
  switch (unit) {
    case 'length':
      return LENGTH_STEPS[isLengthUnit(word) ? word : units]
    case 'angle':
      // A plain number in an angle field is in degrees, like a value typed with deg.
      return word === 'rad' ? RADIANS_PER_DEGREE : 1
    case 'scalar':
    case 'count':
      return 1
  }
}

function modifierFactor(modifier: 'none' | 'coarse' | 'fine', unit: FieldUnit): number {
  if (modifier === 'coarse') {
    return 10
  }
  // Count fields step by whole numbers, so they ignore fine.
  return modifier === 'fine' && unit !== 'count' ? 0.1 : 1
}

/** True for an optional sign, digits, an optional fraction, and an optional unit word (mm, cm, m, in, deg, rad). */
export function isPlainNumber(text: string): boolean {
  return parsePlainNumber(text) !== null
}

/** True for a plain number without a unit word, whose meaning the document units decide in a length field (model.md 3.12). */
export function isNumberWithoutUnit(text: string): boolean {
  return parsePlainNumber(text)?.word === ''
}

/** The text after an arrow-key step, or null when text is not a plain number. */
export function stepExpression(
  text: string,
  direction: 1 | -1,
  modifier: 'none' | 'coarse' | 'fine',
  unit: FieldUnit,
  units: Units,
): string | null {
  const parsed = parsePlainNumber(text)
  if (parsed === null) {
    return null
  }
  const step = baseStep(unit, parsed.word, units) * modifierFactor(modifier, unit)
  const stepped = roundToDecimals(parsed.value + direction * step, RESULT_DECIMALS)
  const value = unit === 'count' ? Math.max(0, stepped) : stepped
  return parsed.leading + numberText(value) + parsed.spacing + parsed.word + parsed.trailing
}

/** The plain number multiplied by `factor`, keeping its unit word and spacing as typed, or null when the text is not a plain number. */
export function scalePlainNumber(text: string, factor: number): string | null {
  const parsed = parsePlainNumber(text)
  if (parsed === null) {
    return null
  }
  const value = roundToDecimals(parsed.value * factor, RESULT_DECIMALS)
  return parsed.leading + numberText(value) + parsed.spacing + parsed.word + parsed.trailing
}
