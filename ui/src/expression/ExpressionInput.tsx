import { clsx } from 'clsx/lite'
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import { openSidePanel } from '../app/actions/panels'
import { Button } from '../components/Button'
import { describedBy } from '../components/described-by'
import { HelpLabel } from '../components/HelpLabel'
import type { Dimension, FieldUnit, Units } from '../generated/protocol'
import { requestUi } from '../layout/requests'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'
import {
  completionAt,
  completionNames,
  insertCompletion,
  missingParameterName,
  type Completion,
} from './completion'
import { formatEvaluated } from './evaluate'
import { formatQuantity } from './format'
import { isPlainNumber, stepExpression } from './step'
import { useExpression } from './useExpression'

// A text field for an expression (ui.md 2.13, 3.7): live validation through the core, the value
// it will have, arrow-key steps for plain numbers, parameter name completion, and Enter and Esc
// for inline editors.

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
  /** A `data-testid` for the input itself, such as `entity-size` (3.8). */
  readonly testId?: string
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
  /** The field's hover help (3.7): a tooltip on the label and a description of the input, or null. */
  readonly help?: string | null
  /** Offers "Add parameter {name}" for a name the text uses that no parameter has (command and selection inspectors, 3.7). */
  readonly offerParameter?: boolean
  /** Parameter name completion (2.13); false keeps a plain text input, as the sketch dimension editor is. */
  readonly completion?: boolean
  /** Controls at the end of the input's row, such as an inline Reverse toggle (3.7). */
  readonly trailing?: ReactNode
  /** Called with the text on every edit, and again when the state of the text changes. */
  readonly onChange: (value: string, state: ExpressionState) => void
  readonly onCommit?: (value: string, state: ExpressionState) => void
  readonly onCancel?: () => void
  readonly onFocus?: () => void
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
function resultText(
  text: string,
  state: ExpressionState,
  unit: FieldUnit | 'any',
  units: Units,
): string {
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

/** The completion list's state: what it offers and the option the arrow keys chose, if any. */
type Suggestions = { readonly completion: Completion; readonly active: number | null }

export function ExpressionInput({
  id,
  testId,
  label,
  value,
  unit,
  optional = false,
  autoFocus = false,
  selectOnFocus = false,
  hideLabel = false,
  error = null,
  help = null,
  offerParameter = false,
  completion = true,
  trailing = null,
  onChange,
  onCommit,
  onCancel,
  onFocus,
  onBlur,
}: ExpressionInputProps): React.JSX.Element {
  const app = useApp()
  const input = useRef<HTMLInputElement>(null)
  const listId = useId()
  const units = useAppState((state) => state.document?.recipe.units ?? 'mm')
  const evaluated = useExpression(value, unit, label)
  const state = optional && value.trim() === '' ? EMPTY : evaluated
  const errorId = `${id}-error`
  const helpId = `${id}-help`
  const invalid = state.state === 'invalid'
  const message = invalid ? state.message : (error ?? '')
  const [suggestions, setSuggestions] = useState<Suggestions | null>(null)
  // The caret to restore after an inserted name re-renders the text.
  const caretAfterRender = useRef<number | null>(null)
  const missingName = useAppState((current) =>
    offerParameter && state.state === 'invalid'
      ? missingParameterName(value, state.range, current)
      : null,
  )

  // The owner hears about every new state of the text: a typed text is reported as pending when it
  // changes, a text set from outside (an undo, a fix) when it renders, and every later result. A
  // render that still shows an older typed text is not a change.
  const reported = useRef<{ readonly value: string; readonly state: ExpressionState } | null>(null)
  const renderedValue = useRef(value)
  const latestOnChange = useRef(onChange)
  useEffect(() => {
    latestOnChange.current = onChange
  })
  useEffect(() => {
    const last = reported.current
    const fromOutside = value !== renderedValue.current && value !== last?.value
    renderedValue.current = value
    if (last !== null && last.value === value && sameState(last.state, state)) {
      return
    }
    if (last !== null && last.value !== value && !fromOutside) {
      return
    }
    reported.current = { value, state }
    // A first state worth knowing (a required field left empty) is reported too.
    if (last !== null || state.state !== 'pending') {
      latestOnChange.current(value, state)
    }
  }, [value, state])

  // The last value the text evaluated to, the starting expression of a parameter made from it.
  const lastValid = useRef<number | null>(null)
  if (state.state === 'valid') {
    lastValid.current = state.value
  }

  // Enter pressed while the text is evaluated commits it once its result arrives.
  const commitWhenSettled = useRef<string | null>(null)
  const latestOnCommit = useRef(onCommit)
  useEffect(() => {
    latestOnCommit.current = onCommit
  })
  useEffect(() => {
    if (commitWhenSettled.current !== value || state.state === 'pending') {
      return
    }
    commitWhenSettled.current = null
    settleEnter(value, state)
  })

  useEffect(() => {
    if (autoFocus) {
      input.current?.focus()
    }
  }, [autoFocus])

  useLayoutEffect(() => {
    const caret = caretAfterRender.current
    if (caret !== null && input.current !== null) {
      caretAfterRender.current = null
      input.current.setSelectionRange(caret, caret)
    }
  })

  /** Opens, updates or closes the list for the text and the caret. */
  function suggest(text: string, caret: number | null): void {
    const found =
      caret === null || !completion
        ? null
        : completionAt(text, caret, completionNames(app.store.getState()))
    setSuggestions(found === null ? null : { completion: found, active: null })
  }

  function change(next: string, caret: number | null = null): void {
    commitWhenSettled.current = null
    const nextState = optional && next.trim() === '' ? EMPTY : PENDING
    reported.current = { value: next, state: nextState }
    suggest(next, caret)
    onChange(next, nextState)
  }

  /** Puts a name in place of the identifier at the caret; the list closes and focus stays. */
  function insert(name: string): void {
    if (suggestions === null) {
      return
    }
    const inserted = insertCompletion(value, suggestions.completion, name)
    caretAfterRender.current = inserted.caret
    change(inserted.text)
    setSuggestions(null)
  }

  /** What Enter does for a settled state: selects the offending range or commits. Returns whether it acted. */
  function settleEnter(text: string, settled: ExpressionState): boolean {
    if (settled.state === 'invalid' && settled.range !== null) {
      input.current?.setSelectionRange(settled.range.start, settled.range.end)
      return true
    }
    const commit = latestOnCommit.current
    if (commit !== undefined && (settled.state === 'valid' || settled.state === 'empty')) {
      commit(text, settled)
      return true
    }
    return false
  }

  /** Keys of the open suggestion list; returns whether the list took the key. */
  function onListKey(event: KeyboardEvent<HTMLInputElement>, open: Suggestions): boolean {
    const count = open.completion.options.length
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowUp': {
        const step = event.key === 'ArrowDown' ? 1 : -1
        const from = open.active ?? (step === 1 ? -1 : count)
        setSuggestions({ ...open, active: (from + step + count) % count })
        return true
      }
      case 'Tab': {
        const chosen = open.completion.options[open.active ?? 0]
        if (chosen !== undefined) {
          insert(chosen.name)
        }
        return true
      }
      case 'Enter': {
        // Without a choice made with the arrows, Enter acts as if the list were closed.
        const chosen = open.active === null ? undefined : open.completion.options[open.active]
        if (chosen === undefined) {
          setSuggestions(null)
          return false
        }
        insert(chosen.name)
        return true
      }
      case 'Escape':
        setSuggestions(null)
        return true
      default:
        return false
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (suggestions !== null && onListKey(event, suggestions)) {
      event.preventDefault()
      return
    }
    switch (event.key) {
      case 'Enter': {
        if (event.repeat) {
          // A held key commits once, so it never confirms a command twice (3.5).
          event.preventDefault()
          return
        }
        if (state.state === 'pending' && onCommit !== undefined) {
          event.preventDefault()
          commitWhenSettled.current = value
          return
        }
        if (settleEnter(value, state)) {
          event.preventDefault()
        }
        return
      }
      case 'Escape':
        commitWhenSettled.current = null
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

  function addParameter(name: string): void {
    const expression =
      lastValid.current === null || unit === 'any'
        ? ''
        : formatQuantity(lastValid.current, unit, units)
    openSidePanel(app, 'parameters')
    requestUi(app, { kind: 'parameter_draft', name, expression, returnFocus: id })
  }

  const open = suggestions !== null
  const optionId = (index: number) => `${listId}-${String(index)}`
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <HelpLabel help={help} helpId={helpId}>
        <label
          htmlFor={id}
          className={clsx(
            hideLabel ? 'sr-only' : 'self-start text-xs font-medium',
            !hideLabel && COLOR_CLASSES.textMuted.foreground,
            help !== null && 'cursor-help',
          )}
        >
          {label}
        </label>
      </HelpLabel>
      <div className="flex min-w-0 items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <input
            ref={input}
            id={id}
            data-testid={testId}
            type="text"
            role={completion ? 'combobox' : undefined}
            value={value}
            spellCheck={false}
            autoComplete="off"
            aria-autocomplete={completion ? 'list' : undefined}
            aria-expanded={completion ? open : undefined}
            aria-controls={completion ? listId : undefined}
            aria-activedescendant={
              suggestions?.active === null || suggestions === null
                ? undefined
                : optionId(suggestions.active)
            }
            aria-invalid={message !== '' ? true : undefined}
            aria-describedby={describedBy(errorId, help === null ? null : helpId)}
            onChange={(event) => {
              change(event.currentTarget.value, event.currentTarget.selectionStart)
            }}
            onKeyDown={onKeyDown}
            onSelect={(event) => {
              const caret = event.currentTarget.selectionStart
              const end = event.currentTarget.selectionEnd
              // A caret moved off the identifier closes the list; a selection range never opens it.
              if (open && (caret !== end || caret !== suggestions.completion.end)) {
                suggest(value, caret === end ? caret : null)
              }
            }}
            onFocus={(event) => {
              onFocus?.()
              if (selectOnFocus) {
                event.currentTarget.select()
              }
            }}
            onBlur={() => {
              setSuggestions(null)
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
          {completion ? (
            <ul
              id={listId}
              // The combobox pattern needs a listbox of rich options, which a native datalist cannot hold.
              // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role
              role="listbox"
              aria-label={`Names for ${label}`}
              data-testid={open ? 'expression-suggestions' : undefined}
              hidden={!open}
              className={clsx(
                'absolute top-full left-0 z-30 mt-1 flex w-full min-w-48 flex-col rounded-sm border py-1 shadow-lg',
                COLOR_CLASSES.surfaceRaised.background,
                COLOR_CLASSES.border.border,
              )}
            >
              {suggestions?.completion.options.map((option, index) => (
                // Options take no focus or keys: the input keeps both and points at the active option.
                // oxlint-disable-next-line jsx-a11y/click-events-have-key-events
                <li
                  key={option.name}
                  id={optionId(index)}
                  // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role, jsx-a11y/no-noninteractive-element-to-interactive-role
                  role="option"
                  aria-selected={suggestions.active === index}
                  onMouseDown={(event) => {
                    // The input keeps focus while an option is clicked.
                    event.preventDefault()
                  }}
                  onClick={() => {
                    insert(option.name)
                  }}
                  className={clsx(
                    'cursor-pointer truncate px-2 py-1 text-sm',
                    FONT_CLASSES.values,
                    COLOR_CLASSES.text.foreground,
                    suggestions.active === index
                      ? COLOR_CLASSES.hover.active
                      : COLOR_CLASSES.hover.background,
                  )}
                >
                  {option.display}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
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
        {trailing}
      </div>
      <p
        id={errorId}
        data-testid="field-error"
        aria-live="polite"
        className={clsx('text-xs', COLOR_CLASSES.errorText.foreground)}
      >
        {message}
      </p>
      {missingName === null ? null : (
        <Button
          className="self-start"
          onClick={() => {
            addParameter(missingName)
          }}
        >
          Add parameter {missingName}
        </Button>
      )}
    </div>
  )
}
