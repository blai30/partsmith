import type { ReactNode } from 'react'
import { noteFieldFocused, setCommandField, setFieldStatus } from '../../app/actions/command'
import { useApp } from '../../app/AppProvider'
import type { FieldUnit } from '../../generated/protocol'
import { ExpressionInput } from '../../expression/ExpressionInput'
import { confirmIfComplete } from '../../layout/ladders'
import type { InspectorField } from '../model'
import { fieldInputId } from './field-id'

export type ExpressionFieldProps = {
  readonly field: InspectorField
  readonly unit: FieldUnit
  /** A feature status message about this field, shown under it (3.7). */
  readonly statusMessage: string | null
  /** An inline toggle at the end of the row, such as Reverse (3.7). */
  readonly trailing?: ReactNode
}

/** An expression param: live validation, the value it will have, and arrow-key steps (3.7). */
export function ExpressionField({
  field,
  unit,
  statusMessage,
  trailing = null,
}: ExpressionFieldProps) {
  const app = useApp()
  const value = typeof field.value === 'string' ? field.value : ''
  return (
    <ExpressionInput
      id={fieldInputId(field.path)}
      label={field.label}
      value={value}
      unit={unit}
      optional={!field.required}
      error={statusMessage}
      help={field.help}
      offerParameter
      trailing={trailing}
      onFocus={() => {
        noteFieldFocused(app, field.path)
      }}
      onChange={(next, state) => {
        if (next !== value) {
          // An optional field left empty is removed from the params (2.12); the edits since the
          // field gained focus are one change of the command history (3.7).
          setCommandField(
            app,
            field.path,
            next.trim() === '' && !field.required ? undefined : next,
            { typing: true },
          )
        }
        setFieldStatus(app, field.path, {
          error: state.state === 'invalid' ? state.message : null,
          pending: state.state === 'pending',
        })
      }}
      // Enter confirms the command (3.5), also when pressed before the text was evaluated.
      onCommit={() => {
        confirmIfComplete(app)
      }}
    />
  )
}
