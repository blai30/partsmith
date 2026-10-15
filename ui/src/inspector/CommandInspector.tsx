import { clsx } from 'clsx/lite'
import { ArrowUpDown, ChevronDown, ChevronRight } from 'lucide-react'
import type { ReactNode } from 'react'
import {
  cancelCommand,
  commandStartParams,
  confirmCommand,
  reverseAndUse,
  setActiveField,
  setCommandField,
} from '../app/actions/command'
import { updateSettings } from '../app/actions/settings'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { formatShortcut } from '../commands/shortcuts'
import { Button } from '../components/Button'
import { IconButton } from '../components/IconButton'
import { Tooltip } from '../components/Tooltip'
import { isPlainNumber } from '../expression/step'
import type { Diagnostic } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { SeverityIcon } from '../print/SeverityIcon'
import type { ActiveCommand, CatalogFeature } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { ChoiceField } from './fields/ChoiceField'
import { ExpressionField } from './fields/ExpressionField'
import { fieldInputId } from './fields/field-id'
import { SelectionField } from './fields/SelectionField'
import { NumberField, SwitchField, TextValueField, UnsupportedField } from './fields/ValueFields'
import { buildInspectorModel, fieldsOf, siblingsOf, type InspectorField } from './model'
import { messagesByField } from './status'

// The inspector of a feature command (ui.md 3.7): the status banner, the essential fields, the
// rest behind "More options", and Cancel and OK.

/** An inline toggle at the end of the row of the field it attaches to, such as Reverse (2.12, 3.7). */
function ToggleField({ field }: { readonly field: InspectorField }) {
  const app = useApp()
  return (
    <div data-testid="inspector-field" data-path={field.path} className="flex shrink-0">
      <Tooltip label={field.help ?? field.label}>
        <IconButton
          id={fieldInputId(field.path)}
          label={field.label}
          icon={ArrowUpDown}
          size="field"
          pressed={field.value === true}
          onClick={() => {
            setCommandField(app, field.path, field.value !== true)
          }}
        />
      </Tooltip>
    </div>
  )
}

function FieldControl({
  field,
  params,
  statusMessage,
  trailing,
}: {
  readonly field: InspectorField
  readonly params: Readonly<Record<string, unknown>>
  readonly statusMessage: string | null
  readonly trailing: ReactNode
}) {
  const control = field.control
  switch (control.type) {
    case 'expression':
      return (
        <ExpressionField
          field={field}
          unit={control.unit}
          statusMessage={statusMessage}
          trailing={trailing}
        />
      )
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
        <ChoiceField
          field={field}
          options={control.options}
          presentation={control.presentation}
          trailing={trailing}
        />
      )
    case 'switch':
      return <SwitchField field={field} />
    case 'toggle':
      return <ToggleField field={field} />
    case 'text':
      return <TextValueField field={field} statusMessage={statusMessage} />
    case 'number':
      return <NumberField field={field} integer={control.integer} statusMessage={statusMessage} />
    case 'unsupported':
      return <UnsupportedField field={field} />
  }
}

/** A fix action of a status message (4.2), and the field whose input gets focus after it (3.7). */
type Fix = { readonly label: string; readonly focusPath: string; readonly run: () => void }

/**
 * The Reverse fixes of a feature with an inline Reverse (4.2): a negative plain number out of range
 * in the field Reverse sits on is used positive with Reverse toggled; a cut or intersect that does
 * nothing, and a through-all extent that misses, offer Reverse.
 */
function reverseFixes(
  app: App,
  message: Diagnostic,
  field: InspectorField | null,
  toggle: InspectorField | null,
  params: Readonly<Record<string, unknown>>,
): readonly Fix[] {
  if (toggle === null) {
    return []
  }
  const reverse: Fix = {
    label: 'Reverse',
    // Focus on the Reverse button itself would make the next Enter toggle it back (3.5).
    focusPath: toggle.attachedTo ?? toggle.path,
    run: () => {
      setCommandField(app, toggle.path, toggle.value !== true)
    },
  }
  const text = typeof field?.value === 'string' ? field.value.trim() : ''
  if (
    message.code === 'out_of_range' &&
    field !== null &&
    field.path === toggle.attachedTo &&
    isPlainNumber(text) &&
    text.startsWith('-')
  ) {
    const positive = text.slice(1)
    return [
      {
        label: `Reverse and use ${positive}`,
        focusPath: field.path,
        run: () => {
          reverseAndUse(app, field.path, positive)
        },
      },
    ]
  }
  const operation = params['op']
  if (
    (message.code === 'no_effect' && (operation === 'cut' || operation === 'intersect')) ||
    message.code === 'through_all_misses'
  ) {
    return [reverse]
  }
  return []
}

/** The status banner: errors in an alert, else warnings, each with its fixes (3.7, 4.2). */
function StatusBanner({
  messages,
  fieldOf,
  toggle,
  params,
}: {
  readonly messages: readonly Diagnostic[]
  readonly fieldOf: (message: Diagnostic) => InspectorField | null
  /** The inline Reverse toggle, or null. */
  readonly toggle: InspectorField | null
  readonly params: Readonly<Record<string, unknown>>
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
        'flex flex-col gap-2 rounded-lg p-3 text-sm',
        error ? COLOR_CLASSES.errorSubtle.background : COLOR_CLASSES.warningSubtle.background,
      )}
    >
      {shown.map((message, index) => {
        const field = fieldOf(message)
        const suggestion = message.suggestion ?? null
        const fixes: readonly Fix[] = [
          ...(field === null || suggestion === null
            ? []
            : [
                {
                  label: `Use ${suggestion}`,
                  focusPath: field.path,
                  run: () => {
                    setCommandField(app, field.path, suggestion)
                  },
                },
              ]),
          ...reverseFixes(app, message, field, toggle, params),
        ]
        return (
          // Messages carry no id; their order is the core's.
          // oxlint-disable-next-line react/no-array-index-key
          <div key={index} className="flex items-start gap-2">
            <SeverityIcon severity={error ? 'error' : 'warning'} className="mt-0.5" />
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
              {fixes.map((fix) => (
                <Button
                  key={fix.label}
                  className="self-start"
                  onClick={() => {
                    fix.run()
                    // The banner goes when the fix clears its message, so focus stays in the
                    // inspector, where Enter confirms (3.5, 3.7).
                    document.getElementById(fieldInputId(fix.focusPath))?.focus()
                  }}
                >
                  {fix.label}
                </Button>
              ))}
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
  const startParams = useAppState(commandStartParams)
  const model = buildInspectorModel({
    feature,
    params: command.params,
    errors: command.fieldErrors,
    ...(startParams === null ? {} : { startParams }),
  })
  const fields = fieldsOf(model)
  const toggles = fields.filter((field) => field.control.type === 'toggle')
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
  // A More field the core reports an error on opens More too, like a field error (2.12, 3.7).
  const moreNeedsAttention = model.more.some(
    (field) => byField.get(field.path)?.severity === 'error',
  )
  const moreOpen = expanded || model.forceMore || moreNeedsAttention
  const complete = model.complete
  const statusMessageOf = (field: InspectorField): string | null =>
    byField.get(field.path)?.message ?? null

  const renderField = (field: InspectorField) =>
    // An inline toggle renders in the row of the field it attaches to.
    field.control.type === 'toggle' && field.attachedTo !== null ? null : (
      <div
        key={field.path}
        data-testid="inspector-field"
        data-path={field.path}
        className="flex min-w-0 flex-col"
      >
        <FieldControl
          field={field}
          params={command.params}
          statusMessage={statusMessageOf(field)}
          trailing={toggles
            .filter((toggle) => toggle.attachedTo === field.path)
            .map((toggle) => (
              <ToggleField key={toggle.path} field={toggle} />
            ))}
        />
      </div>
    )
  const moreLabel =
    model.changedMore.length === 0
      ? 'More options'
      : `More options · ${model.changedMore.join(', ')}`

  return (
    <>
      <StatusBanner
        messages={messages}
        fieldOf={fieldOf}
        toggle={toggles[0] ?? null}
        params={command.params}
      />
      <div className="flex flex-col gap-3">{model.essential.map(renderField)}</div>
      {model.more.length === 0 ? null : (
        <div className="flex flex-col gap-3">
          {/* The tooltip lists what More holds, so nothing is hidden out of reach (3.7). */}
          <Tooltip label={model.more.map((field) => field.label).join(', ')}>
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
                'inline-flex h-8 max-w-full min-w-0 items-center gap-1 self-start rounded-md px-1 text-left text-sm font-medium',
                COLOR_CLASSES.textMuted.foreground,
                COLOR_CLASSES.hover.background,
                COLOR_CLASSES.focusRing.outline,
              )}
            >
              {moreOpen ? (
                <ChevronDown aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
              ) : (
                <ChevronRight aria-hidden="true" strokeWidth={1.75} className="size-4 shrink-0" />
              )}
              <span className="truncate">{moreOpen ? 'Fewer options' : moreLabel}</span>
            </button>
          </Tooltip>
          {moreOpen ? model.more.map(renderField) : null}
        </div>
      )}
      <div className={clsx('flex justify-end gap-2 border-t pt-4', COLOR_CLASSES.divider.border)}>
        <Tooltip label="Cancel" shortcut={formatShortcut('Escape', app.platform)}>
          <Button
            onClick={() => {
              cancelCommand(app).catch(reportFailure('Canceling the command'))
            }}
          >
            Cancel
          </Button>
        </Tooltip>
        {/* One tooltip whose text follows completeness, so OK keeps its identity and focus. */}
        <Tooltip
          label={complete ? 'OK' : MESSAGES.fill_fields()}
          shortcut={complete ? formatShortcut('Enter', app.platform) : null}
        >
          <Button
            variant="primary"
            data-testid="inspector-ok"
            aria-disabled={complete ? undefined : true}
            onClick={() => {
              if (!complete) {
                return
              }
              confirmCommand(app).catch(reportFailure('Confirming the command'))
            }}
          >
            OK
          </Button>
        </Tooltip>
      </div>
    </>
  )
}
