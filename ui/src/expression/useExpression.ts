import { useEffect, useMemo, useState } from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import { CoreError } from '../core/client'
import { createLatestOnly } from '../core/queue'
import type { DocHandle, Dimension, FieldUnit, Units } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { convertForField } from './evaluate'
import type { ExpressionState } from './ExpressionInput'
import { utf16Range } from './span'

// Live validation of an expression through the core (ui.md 2.13): the core evaluates the text, the
// UI converts the result for the field, and results for an older text never show.

/** What an evaluation was asked for: the text and everything its conversion depends on. */
type Request = {
  readonly documentHandle: DocHandle
  readonly text: string
  readonly unit: FieldUnit | 'any'
  readonly units: Units
  readonly label: string
}

type Evaluated = { readonly request: Request; readonly state: ExpressionState }

const PENDING: ExpressionState = { state: 'pending' }

/** The dimension of each field unit's values. */
const FIELD_DIMENSIONS: Readonly<Record<FieldUnit, Dimension>> = {
  length: { length: 1, angle: 0 },
  angle: { length: 0, angle: 1 },
  scalar: { length: 0, angle: 0 },
  count: { length: 0, angle: 0 },
}

function sameRequest(first: Request, second: Request): boolean {
  return (
    first.documentHandle === second.documentHandle &&
    first.text === second.text &&
    first.unit === second.unit &&
    first.units === second.units &&
    first.label === second.label
  )
}

/** The field's state for the core's answer. */
function stateFor(
  request: Request,
  result: { value: number; dimension: Dimension },
): ExpressionState {
  if (request.unit === 'any') {
    return { state: 'valid', value: result.value, dimension: result.dimension }
  }
  const converted = convertForField(
    result,
    request.unit,
    request.units,
    request.label,
    request.text,
  )
  // The converted value has the field's dimension: a plain 2 in a length field is 2 document units.
  return converted.ok
    ? { state: 'valid', value: converted.value, dimension: FIELD_DIMENSIONS[request.unit] }
    : { state: 'invalid', message: converted.message, range: null }
}

/** The field's state for a refusal: the core's message, with the offending range when it gave one. */
function stateForError(request: Request, error: unknown): ExpressionState {
  if (error instanceof CoreError) {
    const range = error.span === null ? null : utf16Range(request.text, error.span)
    return { state: 'invalid', message: error.message, range }
  }
  return { state: 'invalid', message: MESSAGES.core_not_running(), range: null }
}

/**
 * Evaluates `text` through the core and converts it for `unit` (`any` keeps the returned value and
 * dimension). Empty text is invalid with "{label} needs a value."; a text waiting for its result
 * is pending. A change of the document revision evaluates again, keeping the last result shown
 * until the new one arrives.
 */
export function useExpression(
  text: string,
  unit: FieldUnit | 'any',
  label: string,
): ExpressionState {
  const app = useApp()
  const documentHandle = useAppState((state) => state.document?.documentHandle ?? null)
  const revision = useAppState((state) => state.document?.revision ?? null)
  const units = useAppState((state) => state.document?.recipe.units ?? 'mm')
  const [evaluated, setEvaluated] = useState<Evaluated | null>(null)
  const empty = text.trim() === ''

  const latest = useMemo(
    () =>
      createLatestOnly<Request, { value: number; dimension: Dimension }>(
        (request) => app.core.evaluate(request.documentHandle, request.text),
        (result, request) => {
          setEvaluated({ request, state: stateFor(request, result) })
        },
        (error, request) => {
          setEvaluated({ request, state: stateForError(request, error) })
        },
      ),
    [app],
  )

  useEffect(
    () => () => {
      latest.cancel()
    },
    [latest],
  )

  useEffect(() => {
    if (empty || documentHandle === null) {
      return
    }
    latest.submit({ documentHandle, text, unit, units, label })
    // The revision is a dependency so a document change evaluates again.
  }, [latest, empty, documentHandle, revision, text, unit, units, label])

  const emptyState = useMemo<ExpressionState>(
    () => ({ state: 'invalid', message: MESSAGES.field_required(label), range: null }),
    [label],
  )
  if (empty) {
    return emptyState
  }
  if (documentHandle === null) {
    return PENDING
  }
  const current: Request = { documentHandle, text, unit, units, label }
  return evaluated !== null && sameRequest(evaluated.request, current) ? evaluated.state : PENDING
}
