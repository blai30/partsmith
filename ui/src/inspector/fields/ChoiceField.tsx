import { clsx } from 'clsx/lite'
import { setCommandField } from '../../app/actions/command'
import { useApp } from '../../app/AppProvider'
import { SegmentedControl } from '../../components/SegmentedControl'
import { COLOR_CLASSES } from '../../theme/roles'
import type { InspectorField } from '../model'
import { fieldInputId } from './field-id'

export type ChoiceFieldProps = {
  readonly field: InspectorField
  readonly options: readonly { readonly value: string; readonly label: string }[]
  readonly presentation: 'segmented' | 'select'
}

/** One of a few options: a segmented control up to 4 options, else a native select (3.7). */
export function ChoiceField({ field, options, presentation }: ChoiceFieldProps) {
  const app = useApp()
  const value = typeof field.value === 'string' ? field.value : (options[0]?.value ?? '')
  const id = fieldInputId(field.path)
  if (presentation === 'segmented') {
    return (
      <div className="flex min-w-0 flex-col gap-1">
        <span
          aria-hidden="true"
          className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}
        >
          {field.label}
        </span>
        <SegmentedControl
          label={field.label}
          options={options}
          value={value}
          onChange={(next) => {
            setCommandField(app, field.path, next)
          }}
          className="self-start"
        />
      </div>
    )
  }
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <label
        htmlFor={id}
        className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}
      >
        {field.label}
      </label>
      <select
        id={id}
        value={value}
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
    </div>
  )
}
