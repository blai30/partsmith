import { clsx } from 'clsx/lite'
import { Plus, Trash2 } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import {
  addParameter,
  deleteParameter,
  renameParameter,
  updateParameter,
} from '../app/actions/parameters'
import { useApp, useAppState } from '../app/AppProvider'
import { reportFailure } from '../app/report'
import { Button } from '../components/Button'
import { IconButton } from '../components/IconButton'
import { TextField } from '../components/TextField'
import { PARAMETER_NAME } from '../expression/completion'
import { formatParameterValue } from '../expression/evaluate'
import { ExpressionInput, type ExpressionState } from '../expression/ExpressionInput'
import type { Parameter, ParameterValue, Units } from '../generated/protocol'
import { PanelHeader } from '../layout/PanelHeader'
import { useUiRequest } from '../layout/requests'
import { MESSAGES } from '../messages'
import { SeverityIcon } from '../print/SeverityIcon'
import { COLOR_CLASSES, FONT_CLASSES } from '../theme/roles'

// The Parameters panel (ui.md 3.11): one row per parameter with its name, expression, value and
// comment, each committed on Enter or blur and reverted with Esc, and a draft row to add one.

const MAXIMUM_NAME_LENGTH = 64

function validName(name: string): boolean {
  return PARAMETER_NAME.test(name) && name.length <= MAXIMUM_NAME_LENGTH
}

function expressionId(parameterId: string): string {
  return `parameter-expression-${parameterId}`
}

function ParameterValueText({
  value,
  units,
}: {
  readonly value: ParameterValue | undefined
  readonly units: Units
}) {
  if (value?.error !== undefined && value.error !== null) {
    return (
      <span
        data-testid="parameter-value"
        className={clsx('flex items-start gap-1 text-xs', COLOR_CLASSES.errorText.foreground)}
      >
        <SeverityIcon severity="error" />
        {value.error}
      </span>
    )
  }
  return (
    <span
      data-testid="parameter-value"
      className={clsx(
        'text-xs tabular-nums',
        FONT_CLASSES.values,
        COLOR_CLASSES.textMuted.foreground,
      )}
    >
      {formatParameterValue(value, units) ?? ''}
    </span>
  )
}

function ParameterRow({
  parameter,
  value,
  units,
}: {
  readonly parameter: Parameter
  readonly value: ParameterValue | undefined
  readonly units: Units
}) {
  const app = useApp()
  const [name, setName] = useState<string | null>(null)
  const [nameError, setNameError] = useState<string | null>(null)
  const [expression, setExpression] = useState<string | null>(null)
  const [comment, setComment] = useState<string | null>(null)
  const storedComment = parameter.comment ?? ''

  function commitName(): void {
    const next = (name ?? parameter.name).trim()
    if (next === parameter.name) {
      setName(null)
      setNameError(null)
      return
    }
    if (!validName(next)) {
      setNameError(MESSAGES.parameter_name())
      return
    }
    setNameError(null)
    renameParameter(app, parameter.id, next, { onRefused: setNameError })
      .then((renamed) => {
        if (renamed) {
          setName(null)
        }
      })
      .catch(reportFailure('A parameter change'))
  }

  function commitExpression(text: string, state: ExpressionState): void {
    if (state.state !== 'valid' || text === parameter.expr) {
      return
    }
    updateParameter(app, parameter.id, { expression: text })
      .then(() => {
        setExpression(null)
      })
      .catch(reportFailure('A parameter change'))
  }

  function commitComment(): void {
    const next = comment ?? storedComment
    if (next.trim() === storedComment) {
      setComment(null)
      return
    }
    updateParameter(app, parameter.id, { comment: next })
      .then(() => {
        setComment(null)
      })
      .catch(reportFailure('A parameter change'))
  }

  return (
    <li
      data-testid="parameter-row"
      data-id={parameter.id}
      className={clsx('flex flex-col gap-2 border-b py-3', COLOR_CLASSES.divider.border)}
    >
      <div className="flex items-start gap-2">
        <TextField
          label={`Name of ${parameter.name}`}
          hideLabel
          value={name ?? parameter.name}
          error={nameError}
          className="w-32"
          spellCheck={false}
          onChange={(next) => {
            setName(next)
            setNameError(null)
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              commitName()
            } else if (event.key === 'Escape') {
              event.preventDefault()
              setName(null)
              setNameError(null)
            }
          }}
          onBlur={() => {
            if (name !== null) {
              commitName()
            }
          }}
        />
        <div className="min-w-0 flex-1">
          <ExpressionInput
            id={expressionId(parameter.id)}
            label={`Expression of ${parameter.name}`}
            hideLabel
            value={expression ?? parameter.expr}
            unit="any"
            onChange={(next) => {
              if (next !== (expression ?? parameter.expr)) {
                setExpression(next)
              }
            }}
            onCommit={commitExpression}
            onBlur={(text, state) => {
              if (expression !== null) {
                commitExpression(text, state)
              }
            }}
            onCancel={() => {
              setExpression(null)
            }}
          />
        </div>
        <IconButton
          label={`Delete ${parameter.name}`}
          icon={Trash2}
          size="compact"
          className="mt-1"
          onClick={() => {
            deleteParameter(app, parameter.id).catch(reportFailure('A parameter change'))
          }}
        />
      </div>
      <ParameterValueText value={value} units={units} />
      <TextField
        label={`Comment of ${parameter.name}`}
        hideLabel
        placeholder="Comment"
        value={comment ?? storedComment}
        onChange={setComment}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault()
            commitComment()
          } else if (event.key === 'Escape') {
            event.preventDefault()
            setComment(null)
          }
        }}
        onBlur={() => {
          if (comment !== null) {
            commitComment()
          }
        }}
      />
    </li>
  )
}

/** A draft row's starting name and expression, when a field asked for the parameter (3.7). */
type Draft = {
  readonly name: string
  readonly expression: string
  /** The element id of the field that asked, which gets focus back when the draft ends. */
  readonly returnFocus: string | null
}

const EMPTY_DRAFT: Draft = { name: '', expression: '', returnFocus: null }

/**
 * The row of a parameter being added: name, then expression, then Enter adds it (3.11). A draft a
 * field asked for starts named, with its expression focused, and gives focus back when it ends.
 */
function DraftRow({ draft, onDone }: { readonly draft: Draft; readonly onDone: () => void }) {
  const app = useApp()
  const nameInput = useRef<HTMLInputElement>(null)
  const [name, setName] = useState(draft.name)
  const [nameError, setNameError] = useState<string | null>(null)
  const [expression, setExpression] = useState(draft.expression)
  const [expressionError, setExpressionError] = useState<string | null>(null)
  const id = useId()
  const draftExpressionId = `${id}-expression`

  useEffect(() => {
    if (draft.name === '') {
      nameInput.current?.focus()
    } else {
      document.getElementById(draftExpressionId)?.focus()
    }
  }, [draft.name, draftExpressionId])

  function done(): void {
    onDone()
    if (draft.returnFocus !== null) {
      document.getElementById(draft.returnFocus)?.focus()
    }
  }

  return (
    <li
      data-testid="parameter-draft"
      className={clsx('flex flex-col gap-2 border-b py-3', COLOR_CLASSES.divider.border)}
    >
      <div className="flex items-start gap-2">
        <TextField
          ref={nameInput}
          label="Name of the new parameter"
          hideLabel
          placeholder="Name"
          value={name}
          error={nameError}
          className="w-32"
          spellCheck={false}
          onChange={(next) => {
            setName(next)
            setNameError(null)
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              if (!validName(name.trim())) {
                setNameError(MESSAGES.parameter_name())
                return
              }
              document.getElementById(draftExpressionId)?.focus()
            } else if (event.key === 'Escape') {
              event.preventDefault()
              done()
            }
          }}
        />
        <div className="min-w-0 flex-1">
          <ExpressionInput
            id={draftExpressionId}
            label="Expression of the new parameter"
            hideLabel
            value={expression}
            unit="any"
            error={expressionError}
            onChange={(next) => {
              setExpression(next)
              setExpressionError(null)
            }}
            onCommit={(text, state) => {
              const trimmed = name.trim()
              if (state.state !== 'valid') {
                return
              }
              if (!validName(trimmed)) {
                setNameError(MESSAGES.parameter_name())
                return
              }
              addParameter(app, trimmed, text, { onRefused: setExpressionError })
                .then((added) => {
                  if (added) {
                    done()
                  }
                })
                .catch(reportFailure('A parameter change'))
            }}
            onCancel={done}
          />
        </div>
      </div>
    </li>
  )
}

export function ParametersPanel() {
  const app = useApp()
  const titleId = useId()
  const parameters = useAppState((state) => state.document?.recipe.parameters)
  const values = useAppState((state) => state.document?.parameterValues)
  const units = useAppState((state) => state.document?.recipe.units ?? 'mm')
  const commandTitle = useAppState((state) => state.command?.title ?? null)
  const [draft, setDraft] = useState<Draft | null>(null)

  // The palette's parameter items focus their row's expression (3.4).
  useUiRequest(app, 'parameter', (request) => {
    const input = document.getElementById(expressionId(request.id))
    if (input === null) {
      return false
    }
    input.focus()
    return true
  })
  // "Add parameter {name}" under a field opens a draft named after it (3.7).
  useUiRequest(app, 'parameter_draft', (request) => {
    setDraft({
      name: request.name,
      expression: request.expression,
      returnFocus: request.returnFocus,
    })
    return true
  })

  return (
    <section
      data-testid="parameters-panel"
      aria-labelledby={titleId}
      className="flex min-h-0 flex-1 flex-col"
    >
      <PanelHeader title="Parameters" titleId={titleId} />
      {commandTitle === null ? null : (
        <p
          data-testid="parameters-transaction"
          className={clsx('px-4 pb-2 text-xs', COLOR_CLASSES.textMuted.foreground)}
        >
          Changes are part of {commandTitle} until you press OK.
        </p>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        <ul className="flex flex-col">
          {(parameters ?? []).map((parameter) => (
            <ParameterRow
              key={parameter.id}
              parameter={parameter}
              value={values?.[parameter.id]}
              units={units}
            />
          ))}
          {draft === null ? null : (
            <DraftRow
              // A new request starts a fresh draft.
              key={`${draft.name}:${draft.returnFocus ?? ''}`}
              draft={draft}
              onDone={() => {
                setDraft(null)
              }}
            />
          )}
        </ul>
        {(parameters ?? []).length === 0 ? (
          <p
            data-testid="parameters-empty"
            className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}
          >
            Parameters are named values, such as wall = 2 mm. Type a parameter&apos;s name in any
            size or angle field to use it.
          </p>
        ) : null}
        <div className="pt-3">
          <Button
            onClick={() => {
              setDraft(EMPTY_DRAFT)
            }}
          >
            <Plus aria-hidden="true" strokeWidth={1.75} className="size-4" />
            Add parameter
          </Button>
        </div>
      </div>
    </section>
  )
}
