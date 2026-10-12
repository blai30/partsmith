import { clsx } from 'clsx/lite'
import type { ReactNode } from 'react'
import { setCommandField } from '../../app/actions/command'
import { useApp } from '../../app/AppProvider'
import { HelpLabel } from '../../components/HelpLabel'
import { SegmentedControl } from '../../components/SegmentedControl'
import { COLOR_CLASSES } from '../../theme/roles'
import type { InspectorField } from '../model'
import { fieldInputId } from './field-id'

export type ChoiceFieldProps = {
  readonly field: InspectorField
  readonly options: readonly { readonly value: string; readonly label: string }[]
  readonly presentation: 'segmented' | 'select'
  /** An inline toggle at the end of the row, such as Reverse (3.7). */
  readonly trailing?: ReactNode
}

/** One of a few options: a segmented control up to 4 options, else a native select (3.7). */
export function ChoiceField({ field, options, presentation, trailing = null }: ChoiceFieldProps) {
  const app = useApp()
  const value = typeof field.value === 'string' ? field.value : (options[0]?.value ?? '')
  const id = fieldInputId(field.path)
  const helpId = `${id}-help`
  const help = field.help
  if (presentation === 'segmented') {
    return (
      <div className="flex min-w-0 flex-col gap-1">
        <HelpLabel help={help} helpId={helpId}>
          <span
            aria-hidden="true"
            className={clsx(
              'self-start text-xs font-medium',
              COLOR_CLASSES.textMuted.foreground,
              help !== null && 'cursor-help',
            )}
          >
            {field.label}
          </span>
        </HelpLabel>
        <div className="flex min-w-0 items-center justify-between gap-2">
          <SegmentedControl
            label={field.label}
            options={options}
            value={value}
            describedBy={help === null ? undefined : helpId}
            onChange={(next) => {
              setCommandField(app, field.path, next)
            }}
            className="self-start"
          />
          {trailing}
        </div>
      </div>
    )
  }
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <HelpLabel help={help} helpId={helpId}>
        <label
          htmlFor={id}
          className={clsx(
            'self-start text-xs font-medium',
            COLOR_CLASSES.textMuted.foreground,
            help !== null && 'cursor-help',
          )}
        >
          {field.label}
        </label>
      </HelpLabel>
      <div className="flex min-w-0 items-center gap-2">
        <select
          id={id}
          value={value}
          aria-describedby={help === null ? undefined : helpId}
          onChange={(event) => {
            setCommandField(app, field.path, event.currentTarget.value)
          }}
          className={clsx(
            'h-8 w-full rounded-sm border px-2 text-sm',
            COLOR_CLASSES.field.background,
            COLOR_CLASSES.text.foreground,
            COLOR_CLASSES.borderControl.border,
            COLOR_CLASSES.focusRing.outline,
          )}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        {trailing}
      </div>
    </div>
  )
}
