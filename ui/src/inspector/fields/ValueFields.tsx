import { clsx } from 'clsx/lite'
import { useState } from 'react'
import { setCommandField, setFieldStatus } from '../../app/actions/command'
import { useApp } from '../../app/AppProvider'
import { Switch } from '../../components/Switch'
import { TextField } from '../../components/TextField'
import { MESSAGES } from '../../messages'
import { COLOR_CLASSES, FONT_CLASSES } from '../../theme/roles'
import type { InspectorField } from '../model'
import { fieldInputId } from './field-id'

// The plain inspector controls (3.7): switches, text, numbers, and values the inspector cannot edit.

export function SwitchField({ field }: { readonly field: InspectorField }) {
  const app = useApp()
  return (
    <Switch
      label={field.label}
      help={field.help}
      checked={field.value === true}
      onChange={(checked) => {
        setCommandField(app, field.path, checked)
      }}
    />
  )
}

export function TextValueField({
  field,
  statusMessage,
}: {
  readonly field: InspectorField
  readonly statusMessage: string | null
}) {
  const app = useApp()
  return (
    <TextField
      id={fieldInputId(field.path)}
      label={field.label}
      help={field.help}
      value={typeof field.value === 'string' ? field.value : ''}
      error={statusMessage}
      onChange={(next) => {
        setCommandField(app, field.path, next === '' && !field.required ? undefined : next)
      }}
    />
  )
}

/** A number param typed as text; text that is not a number shows "{label} must be a number." */
export function NumberField({
  field,
  integer,
  statusMessage,
}: {
  readonly field: InspectorField
  readonly integer: boolean
  readonly statusMessage: string | null
}) {
  const app = useApp()
  const stored = typeof field.value === 'number' ? String(field.value) : ''
  const [text, setText] = useState(stored)
  const [error, setError] = useState<string | null>(null)
  // The params change from elsewhere (a suggestion, undo of a field): the text follows.
  const [previous, setPrevious] = useState(stored)
  if (stored !== previous) {
    setPrevious(stored)
    if (Number(text) !== field.value) {
      setText(stored)
      setError(null)
    }
  }
  return (
    <TextField
      id={fieldInputId(field.path)}
      label={field.label}
      help={field.help}
      value={text}
      inputMode={integer ? 'numeric' : 'decimal'}
      error={error ?? statusMessage}
      onChange={(next) => {
        setText(next)
        const trimmed = next.trim()
        const number = Number(trimmed)
        const valid =
          trimmed !== '' && Number.isFinite(number) && (!integer || Number.isInteger(number))
        if (trimmed === '' && !field.required) {
          setError(null)
          setFieldStatus(app, field.path, { error: null, pending: false })
          setCommandField(app, field.path, undefined)
          return
        }
        const message = valid ? null : MESSAGES.field_not_number(field.label)
        setError(message)
        setFieldStatus(app, field.path, { error: message, pending: false })
        if (valid) {
          setCommandField(app, field.path, number)
        }
      }}
    />
  )
}

/** A value the inspector has no control for: read-only compact JSON (2.12). */
export function UnsupportedField({ field }: { readonly field: InspectorField }) {
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <span className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
        {field.label}
      </span>
      <code
        className={clsx(
          'block overflow-x-auto rounded-sm border px-2 py-1 text-xs whitespace-pre',
          FONT_CLASSES.values,
          COLOR_CLASSES.field.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.text.foreground,
        )}
      >
        {JSON.stringify(field.value ?? null)}
      </code>
      <p className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
        Edit this value in the .ptsm file.
      </p>
    </div>
  )
}
