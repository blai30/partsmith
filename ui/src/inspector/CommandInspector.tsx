import { clsx } from 'clsx/lite'
import { ArrowUpDown, ChevronDown, ChevronRight } from 'lucide-react'
import { useRef, type ReactNode } from 'react'
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
import { reportFailure } from '../app/report'
import { formatShortcut } from '../commands/shortcuts'
import { Button } from '../components/Button'
import { IconButton } from '../components/IconButton'
import { useOverflowEdges } from '../components/overflow-edges'
import { Tooltip } from '../components/Tooltip'
import { isPlainNumber } from '../expression/step'
import type { Diagnostic, FixAction, Severity } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { SeverityIcon } from '../print/SeverityIcon'
import { MessageActions } from '../repair/MessageActions'
import { selectRepairMode, type RepairView } from '../repair/repair'
import { RepairCandidates } from '../repair/RepairCandidates'
import { differingFields } from '../app/actions/command-values'
import { selectDocumentUnits } from '../state/selectors'
import type { ActiveCommand, CatalogFeature } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { ChoiceField } from './fields/ChoiceField'
import { ExpressionField } from './fields/ExpressionField'
import { fieldInputId, focusField } from './fields/field-id'
import { SelectionField } from './fields/SelectionField'
import { NumberField, SwitchField, TextValueField, UnsupportedField } from './fields/ValueFields'
import { buildInspectorModel, fieldsOf, siblingsOf, type InspectorField } from './model'
import { fieldLimit, messagesByField, pointsInto } from './status'

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
  limit,
  trailing,
}: {
  readonly field: InspectorField
  readonly params: Readonly<Record<string, unknown>>
  readonly statusMessage: string | null
  /** The largest value that works, under an expression field (3.7), or null. */
  readonly limit: string | null
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
          limit={limit}
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

/**
 * The text of the UI's own fix of a feature with an inline Reverse (4.2), which is exact: a negative
 * plain number out of range in the field Reverse is attached to is used positive with Reverse
 * toggled, "Reverse and use {text}". Null when the message does not call for it; every other fix is
 * the core's.
 */
function reversedText(
  message: Diagnostic,
  field: InspectorField | null,
  toggle: InspectorField | null,
): string | null {
  const text = typeof field?.value === 'string' ? field.value.trim() : ''
  const applies =
    toggle !== null &&
    field !== null &&
    message.code === 'out_of_range' &&
    field.path === toggle.attachedTo &&
    isPlainNumber(text) &&
    text.startsWith('-')
  return applies ? text.slice(1) : null
}

/** The banner's look per severity (3.7): errors on `errorSubtle`, warnings on `warningSubtle`, notices on `surface`. */
const BANNER_BACKGROUNDS: Readonly<Record<Severity, string>> = {
  error: COLOR_CLASSES.errorSubtle.background,
  warning: COLOR_CLASSES.warningSubtle.background,
  info: COLOR_CLASSES.surface.background,
}

const BANNER_TEXT: Readonly<Record<Severity, string>> = {
  error: COLOR_CLASSES.errorText.foreground,
  warning: COLOR_CLASSES.text.foreground,
  info: COLOR_CLASSES.text.foreground,
}

/**
 * The message of the shown ones that carries the current repair: the one with a candidate's
 * action, else the one about its field; none when the banner does not show it, such as a warning
 * under an error, so its candidates are never listed under another problem (3.7).
 */
function repairMessageOf(
  shown: readonly Diagnostic[],
  view: RepairView,
  candidateActions: ReadonlySet<FixAction>,
): Diagnostic | undefined {
  const path = view.repair.path
  return (
    shown.find((message) =>
      (message.actions ?? []).some((action) => candidateActions.has(action)),
    ) ??
    shown.find((message) => {
      const field = message.field ?? null
      return field !== null && pointsInto(path, field)
    })
  )
}

/**
 * The status banner (3.7): errors in an alert, else warnings, else info notices, each followed by
 * its actions, its cause line with the cause's actions and Copy details; in repair mode the current
 * repair's candidates replace the actions that store them.
 */
function StatusBanner({
  featureId,
  messages,
  fieldOf,
  fields,
  toggle,
  params,
}: {
  readonly featureId: string
  readonly messages: readonly Diagnostic[]
  readonly fieldOf: (message: Diagnostic) => InspectorField | null
  /** The command's fields in display order, for where focus goes after an action. */
  readonly fields: readonly InspectorField[]
  /** The inline Reverse toggle, or null. */
  readonly toggle: InspectorField | null
  readonly params: Readonly<Record<string, unknown>>
}) {
  const app = useApp()
  const mode = useAppState(selectRepairMode)
  const view = mode?.feature === featureId ? mode.view : null
  const bySeverity = (severity: Severity) =>
    messages.filter((message) => message.severity === severity)
  const errors = bySeverity('error')
  const warnings = bySeverity('warning')
  const shown = errors.length > 0 ? errors : warnings.length > 0 ? warnings : bySeverity('info')
  const [first] = shown
  if (first === undefined) {
    return null
  }
  const severity = first.severity
  const candidateActions: ReadonlySet<FixAction> = new Set(
    (view?.candidates ?? []).flatMap((candidate) =>
      candidate.action === null ? [] : [candidate.action],
    ),
  )
  const repairMessage = view === null ? undefined : repairMessageOf(shown, view, candidateActions)

  /**
   * After an action, focus goes to the first field, in display order, whose value it changed (for
   * a toggle, the field it is attached to, so the next Enter confirms instead of toggling it back),
   * else to the field the message is about (3.7).
   */
  function focusAfter(message: Diagnostic): () => void {
    return () => {
      const after = app.store.getState().command?.params ?? params
      const [changed] = differingFields(fields, params, after)
      const path =
        changed === undefined
          ? (fieldOf(message)?.path ?? null)
          : changed.control.type === 'toggle'
            ? (changed.attachedTo ?? changed.path)
            : changed.path
      if (path !== null) {
        focusField(path)
      }
    }
  }

  return (
    <div
      data-testid="inspector-error"
      data-severity={severity}
      role={severity === 'error' ? 'alert' : undefined}
      className={clsx('flex flex-col gap-3 rounded-lg p-3 text-sm', BANNER_BACKGROUNDS[severity])}
    >
      {shown.map((message, index) => {
        const field = fieldOf(message)
        const positive = reversedText(message, field, toggle)
        const textClass = BANNER_TEXT[severity]
        return (
          // Messages carry no id; their order is the core's.
          // oxlint-disable-next-line react/no-array-index-key
          <div key={index} className="flex items-start gap-2">
            <SeverityIcon severity={severity} className="mt-0.5" />
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
                    focusField(field.path)
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
              {view !== null && message === repairMessage ? (
                <RepairCandidates candidates={view.candidates} onRan={focusAfter(message)} />
              ) : null}
              <MessageActions
                message={message}
                place="banner"
                omitted={candidateActions}
                own={
                  field === null || positive === null ? null : (
                    <Button
                      variant="banner"
                      onClick={() => {
                        reverseAndUse(app, field.path, positive)
                        // Focus on the field, not on Reverse, so the next Enter confirms (3.5, 3.7).
                        focusField(field.path)
                      }}
                    >
                      {`Reverse and use ${positive}`}
                    </Button>
                  )
                }
                onRan={focusAfter(message)}
              />
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

/**
 * The command inspector's body of fields, which scrolls, and its footer, which stays in view under
 * it (3.7); the panel around them is the Inspector's.
 */
export function CommandInspector({ command, feature }: CommandInspectorProps) {
  const app = useApp()
  const body = useRef<HTMLDivElement>(null)
  useOverflowEdges(body)
  const status = useAppState((state) => state.document?.featureStatus[command.featureId])
  const units = useAppState(selectDocumentUnits)
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
  // An info notice is not shown as its field's error (3.7).
  const byField = messagesByField(
    fields.map((field) => field.path),
    messages.filter((message) => message.severity !== 'info'),
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
  const limitOf = (field: InspectorField): string | null =>
    fieldLimit(byField.get(field.path), field.value, units)

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
          limit={limitOf(field)}
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
      {/* The body's rules show while fields run under the header or the footer (3.2). */}
      <div
        ref={body}
        className={clsx(
          'flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto border-y px-4 pt-3 pb-4',
          COLOR_CLASSES.forcedEdge.border,
          COLOR_CLASSES.divider.overflowTop,
          COLOR_CLASSES.divider.overflowBottom,
        )}
      >
        <StatusBanner
          featureId={command.featureId}
          messages={messages}
          fieldOf={fieldOf}
          fields={fields}
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
      </div>
      <div className="flex shrink-0 justify-end gap-2 px-4 py-3">
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
