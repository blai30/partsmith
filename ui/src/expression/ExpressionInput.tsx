import { clsx } from 'clsx/lite'
import { useEffect, useRef, type KeyboardEvent } from 'react'
import { useAppState } from '../app/AppProvider'
import type { Dimension, FieldUnit, Units } from '../generated/protocol'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import { formatEvaluated } from './evaluate'
import { formatQuantity } from './format'
import { isPlainNumber, stepExpression } from './step'
import { useExpression } from './useExpression'

// A text field for an expression (ui.md 2.13, 3.7): live validation through the core, the value
// it will have, arrow-key steps for plain numbers, and Enter and Esc for inline editors.

export type ExpressionState =
  | { readonly state: 'empty' }
  | { readonly state: 'pending' }
  | { readonly state: 'valid'; readonly value: number; readonly dimension: Dimension }
  /** `range` is the offending part of the text in UTF-16 indices (from `utf16Range`), or null when the core gave no span. */
  | {
      readonly state: 'invalid'
      readonly message: string
      readonly range: { readonly start: number; readonly end: number } | null
    }

export type ExpressionInputProps = {
  readonly id: string
  readonly label: string
  readonly value: string
  /** A field unit, or 'any' for parameter expressions (every dimension accepted). */
  readonly unit: FieldUnit | 'any'
  readonly optional?: boolean
  readonly autoFocus?: boolean
  readonly selectOnFocus?: boolean
  /** Keeps the label for assistive technology only, for rows that already say what the field is. */
  readonly hideLabel?: boolean
  /** A message about the value from elsewhere (a feature status), shown while the text itself is not invalid. */
  readonly error?: string | null
  /** Called with the text on every edit, and again when the state of the text changes. */
  readonly onChange: (value: string, state: ExpressionState) => void
  readonly onCommit?: (value: string, state: ExpressionState) => void
  readonly onCancel?: () => void
  readonly onBlur?: (value: string, state: ExpressionState) => void
}

const EMPTY: ExpressionState = { state: 'empty' }
const PENDING: ExpressionState = { state: 'pending' }

/** A unit word at the end of a plain number. */
const UNIT_WORD = /(?:mm|cm|m|in|deg|rad)\s*$/

/** The unit an arrow step uses for parameter expressions: the typed word decides. */
function stepUnit(unit: FieldUnit | 'any', text: string): FieldUnit {
  if (unit !== 'any') {
    return unit
  }
  if (/(?:deg|rad)\s*$/.test(text)) {
    return 'angle'
  }
  return /(?:mm|cm|m|in)\s*$/.test(text) ? 'length' : 'scalar'
}

/** The symbol a plain number without a unit word means: the document unit or degrees. */
function unitSymbol(unit: FieldUnit | 'any', units: Units): string {
  switch (unit) {
    case 'length':
      return units
    case 'angle':
      return '°'
    default:
      return ''
  }
}

/** The text after the field: the implied unit, `= {value}`, or nothing while empty or pending. */
function resultText(text: string, state: ExpressionState, unit: FieldUnit | 'any', units: Units): string {
  if (state.state !== 'valid') {
    return ''
  }
  if (isPlainNumber(text) && !UNIT_WORD.test(text)) {
    return unitSymbol(unit, units)
  }
  const value =
    unit === 'any'
      ? formatEvaluated(state.value, state.dimension, units)
      : formatQuantity(state.value, unit, units)
  return `= ${value}`
}

function sameState(first: ExpressionState, second: ExpressionState): boolean {
  return JSON.stringify(first) === JSON.stringify(second)
}

export function ExpressionInput({
  id,
  label,
  value,
  unit,
  optional = false,
  autoFocus = false,
  selectOnFocus = false,
  hideLabel = false,
  error = null,
  onChange,
  onCommit,
  onCancel,
  onBlur,
}: ExpressionInputProps): React.JSX.Element {
  const input = useRef<HTMLInputElement>(null)
  const units = useAppState((state) => state.document?.recipe.units ?? 'mm')
  const evaluated = useExpression(value, unit, label)
  const state = optional && value.trim() === '' ? EMPTY : evaluated
  const errorId = `${id}-error`
  const invalid = state.state === 'invalid'
  const message = invalid ? state.message : (error ?? '')

  // The owner hears about a new state of the same text too (a result arriving, a revision change).
  const reported = useRef<{ readonly value: string; readonly state: ExpressionState } | null>(null)
  const latestOnChange = useRef(onChange)
  useEffect(() => {
    latestOnChange.current = onChange
  })
  useEffect(() => {
    const last = reported.current
    if (last !== null && last.value === value && sameState(last.state, state)) {
      return
    }
    reported.current = { value, state }
    // A first state worth knowing (a required field left empty) is reported too.
    if ((last !== null && last.value === value) || (last === null && state.state !== 'pending')) {
      latestOnChange.current(value, state)
    }
  }, [value, state])

  useEffect(() => {
    if (autoFocus) {
      input.current?.focus()
    }
  }, [autoFocus])

  function change(next: string): void {
    const nextState = optional && next.trim() === '' ? EMPTY : PENDING
    reported.current = { value: next, state: nextState }
    onChange(next, nextState)
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    switch (event.key) {
      case 'Enter': {
        if (state.state === 'invalid' && state.range !== null) {
          event.preventDefault()
          input.current?.setSelectionRange(state.range.start, state.range.end)
          return
        }
        if (onCommit !== undefined && (state.state === 'valid' || state.state === 'empty')) {
          event.preventDefault()
          onCommit(value, state)
        }
        return
      }
      case 'Escape':
        if (onCancel !== undefined) {
          event.preventDefault()
          onCancel()
        }
        return
      case 'ArrowUp':
      case 'ArrowDown': {
        const modifier = event.shiftKey ? 'coarse' : event.altKey ? 'fine' : 'none'
        const stepped = stepExpression(
          value,
          event.key === 'ArrowUp' ? 1 : -1,
          modifier,
          stepUnit(unit, value),
          units,
        )
        if (stepped !== null) {
          event.preventDefault()
          change(stepped)
        }
        return
      }
      default:
    }
  }

  return (
    <div className="flex min-w-0 flex-col gap-1">
      <label
        htmlFor={id}
        className={clsx(
          hideLabel ? 'sr-only' : 'text-xs font-medium',
          !hideLabel && COLOR_CLASSES.textMuted.foreground,
        )}
      >
        {label}
      </label>
      <div className="flex min-w-0 items-center gap-2">
        <input
          ref={input}
          id={id}
          type="text"
          value={value}
          spellCheck={false}
          autoComplete="off"
          aria-invalid={message !== '' ? true : undefined}
          aria-describedby={errorId}
          onChange={(event) => {
            change(event.currentTarget.value)
          }}
          onKeyDown={onKeyDown}
          onFocus={(event) => {
            if (selectOnFocus) {
              event.currentTarget.select()
            }
          }}
          onBlur={() => {
            onBlur?.(value, state)
          }}
          className={clsx(
            'h-8 w-full min-w-0 rounded-sm border px-2 text-sm',
            FONT_CLASSES.values,
            COLOR_CLASSES.field.background,
            COLOR_CLASSES.text.foreground,
            COLOR_CLASSES.focusRing.outline,
            message !== '' ? COLOR_CLASSES.errorMark.border : COLOR_CLASSES.borderControl.border,
          )}
        />
        <span
          data-testid="expression-result"
          className={clsx(
            'shrink-0 text-sm whitespace-nowrap',
            FONT_CLASSES.values,
            COLOR_CLASSES.textMuted.foreground,
          )}
        >
          {resultText(value, state, unit, units)}
        </span>
      </div>
      <p
        id={errorId}
        data-testid="field-error"
        aria-live="polite"
        className={clsx('text-xs', COLOR_CLASSES.errorText.foreground)}
      >
        {message}
      </p>
    </div>
  )
}
