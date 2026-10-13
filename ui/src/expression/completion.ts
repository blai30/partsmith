import { fuzzyScore } from '../commands/fuzzy'
import { resolvedPrinter } from '../print/printer'
import type { AppState } from '../state/store'
import { formatParameterValue } from './evaluate'

// Parameter name completion in expression fields (ui.md 2.13): the names a field can use, the
// identifier the caret ends, and the names that match it.

/** A name an expression can use, and how the suggestion list shows it. */
export type CompletionName = { readonly name: string; readonly display: string }

/** The identifier the caret ends, from `start` to `end`, and the names that match it, best first. */
export type Completion = {
  readonly start: number
  readonly end: number
  readonly identifier: string
  readonly options: readonly CompletionName[]
}

/** The most suggestions shown at once. */
const MAXIMUM_OPTIONS = 8

/** Words an expression reserves (expressions.md): unit names, the constant and function names. */
const RESERVED: ReadonlySet<string> = new Set([
  'um',
  'mm',
  'cm',
  'm',
  'in',
  'ft',
  'deg',
  'rad',
  'pi',
  'abs',
  'sqrt',
  'sin',
  'cos',
  'tan',
  'asin',
  'acos',
  'atan',
  'atan2',
  'min',
  'max',
  'floor',
  'ceil',
  'round',
])

/** The printer names of print.md every profile has; FDM profiles add `printer.nozzle`. */
const PRINTER_NAMES: readonly string[] = [
  'printer.fit.press',
  'printer.fit.snug',
  'printer.fit.slip',
  'printer.fit.loose',
  'printer.layer_height',
]

/** The names an expression field offers: the document's parameters with their values, then the printer names. */
export function completionNames(state: AppState): readonly CompletionName[] {
  const document = state.document
  if (document === null) {
    return []
  }
  const units = document.recipe.units
  const parameters = (document.recipe.parameters ?? []).map((parameter) => {
    const shown = formatParameterValue(document.parameterValues[parameter.id], units)
    return {
      name: parameter.name,
      display: shown === null ? parameter.name : `${parameter.name} = ${shown}`,
    }
  })
  const printer = resolvedPrinter(state.printers, document.recipe.printer)
  const printerNames = [
    ...PRINTER_NAMES,
    ...(printer?.technology === 'fdm' ? ['printer.nozzle'] : []),
  ]
  return [...parameters, ...printerNames.map((name) => ({ name, display: name }))]
}

const NAME_CHARACTER = /[A-Za-z0-9_.]/

/** A parameter name: a letter or `_`, then letters, digits and `_`. */
export const PARAMETER_NAME = /^[A-Za-z_][A-Za-z0-9_]*$/

/**
 * The completion at the caret, or null when the list stays closed: the caret must end an
 * identifier (letters, digits, `_` and `.`, starting with a letter or `_`) that does not follow a
 * number, is not a unit word, `pi` or a function name, equals no name exactly, and matches some
 * name.
 */
export function completionAt(
  text: string,
  caret: number,
  names: readonly CompletionName[],
): Completion | null {
  if (caret < text.length && NAME_CHARACTER.test(text.charAt(caret))) {
    return null
  }
  let start = caret
  while (start > 0 && NAME_CHARACTER.test(text.charAt(start - 1))) {
    start -= 1
  }
  const identifier = text.slice(start, caret)
  if (!/^[A-Za-z_]/.test(identifier) || RESERVED.has(identifier)) {
    return null
  }
  // A word after a number is its unit (`10 mm`, `1 in`).
  if (/[0-9.]$/.test(text.slice(0, start).trimEnd())) {
    return null
  }
  if (names.some((candidate) => candidate.name === identifier)) {
    return null
  }
  const options = names
    .flatMap((candidate) => {
      const score = fuzzyScore(identifier, candidate.name)
      return score === null ? [] : [{ candidate, score }]
    })
    .toSorted(
      (first, second) =>
        second.score - first.score ||
        first.candidate.name.localeCompare(second.candidate.name, 'en'),
    )
    .slice(0, MAXIMUM_OPTIONS)
    .map((match) => match.candidate)
  return options.length === 0 ? null : { start, end: caret, identifier, options }
}

/** The text with the completion's identifier replaced by `name`, and the caret after it. */
export function insertCompletion(
  text: string,
  completion: Completion,
  name: string,
): { readonly text: string; readonly caret: number } {
  return {
    text: text.slice(0, completion.start) + name + text.slice(completion.end),
    caret: completion.start + name.length,
  }
}

/**
 * The name an invalid field could add as a parameter (3.7 "Add parameter {name}"): the offending
 * text when it is an identifier that names no parameter and does not start with `printer.`.
 */
export function missingParameterName(
  text: string,
  range: { readonly start: number; readonly end: number } | null,
  state: AppState,
): string | null {
  if (range === null) {
    return null
  }
  const name = text.slice(range.start, range.end)
  // An identifier has no dot, so `printer.*` names are never offered.
  if (!PARAMETER_NAME.test(name) || RESERVED.has(name)) {
    return null
  }
  const parameters = state.document?.recipe.parameters ?? []
  return parameters.some((parameter) => parameter.name === name) ? null : name
}
