import { clsx } from 'clsx/lite'
import { ChevronDown, ChevronRight, CircleX, TriangleAlert } from 'lucide-react'
import {
  cancelCommand,
  confirmCommand,
  setActiveField,
  setCommandField,
} from '../app/actions/command'
import { updateSettings } from '../app/actions/settings'
import { useApp, useAppState } from '../app/AppProvider'
import { Button } from '../components/Button'
import { Tooltip } from '../components/Tooltip'
import type { Diagnostic } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { ActiveCommand, CatalogFeature } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { ChoiceField } from './fields/ChoiceField'
import { ExpressionField } from './fields/ExpressionField'
import { fieldInputId } from './fields/field-id'
import { SelectionField } from './fields/SelectionField'
import { NumberField, SwitchField, TextValueField, UnsupportedField } from './fields/ValueFields'
import { buildInspectorModel, getParam, type InspectorField } from './model'
import { messagesByField } from './status'

// The inspector of a feature command (ui.md 3.7): the status banner, the essential fields, the
// rest behind "More options", and Cancel and OK.

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** The params object that holds a field: the top level, or its parent object. */
function siblingsOf(
  params: Readonly<Record<string, unknown>>,
  path: string,
): Readonly<Record<string, unknown>> {
  const dot = path.lastIndexOf('.')
  if (dot === -1) {
    return params
  }
  const parent = getParam(params, path.slice(0, dot))
  return isRecord(parent) ? parent : {}
}

function FieldControl({
  field,
  params,
  statusMessage,
}: {
  readonly field: InspectorField
  readonly params: Readonly<Record<string, unknown>>
  readonly statusMessage: string | null
}) {
  const control = field.control
  switch (control.type) {
    case 'expression':
      return <ExpressionField field={field} unit={control.unit} statusMessage={statusMessage} />
    case 'selection':
      return (
        <SelectionField
          field={field}
          accepts={control.accepts}
          multiple={control.multiple}
          siblings={siblingsOf(params, field.path)}
          statusMessage={statusMessage}
        />
      )
    case 'choice':
      return (
        <ChoiceField field={field} options={control.options} presentation={control.presentation} />
      )
    case 'switch':
      return <SwitchField field={field} />
    case 'text':
      return <TextValueField field={field} statusMessage={statusMessage} />
    case 'number':
      return <NumberField field={field} integer={control.integer} statusMessage={statusMessage} />
    case 'unsupported':
      return <UnsupportedField field={field} />
  }
}

/** The status banner: errors in an alert, else warnings, each with its fixes (3.7, 4.2). */
function StatusBanner({
  messages,
  fieldOf,
}: {
  readonly messages: readonly Diagnostic[]
  readonly fieldOf: (message: Diagnostic) => InspectorField | null
}) {
  const app = useApp()
  const errors = messages.filter((message) => message.severity === 'error')
  const shown =
    errors.length > 0 ? errors : messages.filter((message) => message.severity === 'warning')
  if (shown.length === 0) {
    return null
  }
  const error = errors.length > 0
  const textClass = error ? COLOR_CLASSES.errorText.foreground : COLOR_CLASSES.text.foreground
  return (
    <div
      data-testid="inspector-error"
      role={error ? 'alert' : undefined}
      className={clsx(
        'flex flex-col gap-2 rounded-sm p-3 text-sm',
        error ? COLOR_CLASSES.errorSubtle.background : COLOR_CLASSES.warningSubtle.background,
      )}
    >
      {shown.map((message, index) => {
        const field = fieldOf(message)
        const suggestion = message.suggestion ?? null
        return (
          // Messages carry no id; their order is the core's.
          // oxlint-disable-next-line react/no-array-index-key
          <div key={index} className="flex items-start gap-2">
            {error ? (
              <CircleX
                aria-hidden="true"
                strokeWidth={1.75}
                className={clsx('mt-0.5 size-4 shrink-0', COLOR_CLASSES.errorMark.foreground)}
              />
            ) : (
              <TriangleAlert
                aria-hidden="true"
                strokeWidth={1.75}
                className={clsx(
                  'mt-0.5 size-4 shrink-0',
                  COLOR_CLASSES.warningMark.fill,
                  COLOR_CLASSES.warningMark.foreground,
                )}
              />
            )}
            <div className="flex min-w-0 flex-col gap-2">
              {field === null ? (
                <p className={textClass}>{message.message}</p>
              ) : (
                // Clicking a message about a field takes the user to that field (4.2).
                <button
                  type="button"
                  onClick={() => {
                    if (field.control.type === 'selection') {
                      setActiveField(app, field.path)
                    }
                    document.getElementById(fieldInputId(field.path))?.focus()
                  }}
                  className={clsx(
                    'rounded-xs text-left hover:underline',
                    textClass,
                    COLOR_CLASSES.focusRing.outline,
                  )}
                >
                  {message.message}
                </button>
              )}
              {field === null || suggestion === null ? null : (
                <Button
                  className="self-start"
                  onClick={() => {
                    setCommandField(app, field.path, suggestion)
                  }}
                >
                  Use {suggestion}
                </Button>
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}

export type CommandInspectorProps = {
  readonly command: ActiveCommand
  readonly feature: CatalogFeature
}

/** The command inspector's fields and footer; the panel around it is the Inspector's. */
export function CommandInspector({ command, feature }: CommandInspectorProps) {
  const app = useApp()
  const status = useAppState((state) => state.document?.featureStatus[command.featureId])
  const expanded = useAppState((state) => state.settings.moreExpanded[command.featureType] === true)
  const model = buildInspectorModel({
    feature,
    params: command.params,
    errors: command.fieldErrors,
  })
  const fields = [...model.essential, ...model.more]
  const messages = status?.messages ?? []
  const byField = messagesByField(
    fields.map((field) => field.path),
    messages,
  )
  const fieldOf = (message: Diagnostic): InspectorField | null => {
    for (const [path, mapped] of byField) {
      if (mapped === message) {
        return fields.find((field) => field.path === path) ?? null
      }
    }
    return null
  }
  const moreOpen = expanded || model.forceMore
  const complete = model.complete
  const statusMessageOf = (field: InspectorField): string | null =>
    byField.get(field.path)?.message ?? null

  const renderField = (field: InspectorField) => (
    <div
      key={field.path}
      data-testid="inspector-field"
      data-path={field.path}
      className="flex min-w-0 flex-col"
    >
      <FieldControl field={field} params={command.params} statusMessage={statusMessageOf(field)} />
    </div>
  )

  return (
    <>
      <StatusBanner messages={messages} fieldOf={fieldOf} />
      <div className="flex flex-col gap-3">{model.essential.map(renderField)}</div>
      {model.more.length === 0 ? null : (
        <div className="flex flex-col gap-3">
          <button
            type="button"
            data-testid="inspector-more"
            aria-expanded={moreOpen}
            onClick={() => {
              const settings = app.store.getState().settings
              updateSettings(app, {
                moreExpanded: { ...settings.moreExpanded, [command.featureType]: !moreOpen },
              })
            }}
            className={clsx(
              'inline-flex h-8 items-center gap-1 self-start rounded-sm px-1 text-sm font-medium',
              COLOR_CLASSES.textMuted.foreground,
              COLOR_CLASSES.hover.background,
              COLOR_CLASSES.focusRing.outline,
            )}
          >
            {moreOpen ? (
              <ChevronDown aria-hidden="true" strokeWidth={1.75} className="size-4" />
            ) : (
              <ChevronRight aria-hidden="true" strokeWidth={1.75} className="size-4" />
            )}
            {moreOpen ? 'Fewer options' : 'More options'}
          </button>
          {moreOpen ? model.more.map(renderField) : null}
        </div>
      )}
      <div className={clsx('flex justify-end gap-2 border-t pt-4', COLOR_CLASSES.border.border)}>
        <Button
          onClick={() => {
            cancelCommand(app).catch((error: unknown) => {
              console.error('Canceling the command failed.', error)
            })
          }}
        >
          Cancel
        </Button>
        {complete ? (
          <Button
            variant="primary"
            data-testid="inspector-ok"
            onClick={() => {
              confirmCommand(app).catch((error: unknown) => {
                console.error('Confirming the command failed.', error)
              })
            }}
          >
            OK
          </Button>
        ) : (
          <Tooltip label={MESSAGES.fill_fields()}>
            <Button variant="primary" data-testid="inspector-ok" aria-disabled="true">
              OK
            </Button>
          </Tooltip>
        )}
      </div>
    </>
  )
}
