import { clsx } from 'clsx/lite'
import { useId } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { describedBy } from './described-by'
import { HelpLabel } from './HelpLabel'

export type SwitchProps = {
  readonly label: string
  readonly checked: boolean
  /** Called with the state a click or Space asks for; the switch shows `checked` until it changes. */
  readonly onChange: (checked: boolean) => void
  /** Help text under the label, exposed as the switch's accessible description. */
  readonly description?: string
  /** Hover help on the label (3.7), also the switch's accessible description; null for none. */
  readonly help?: string | null
  readonly disabled?: boolean
  readonly className?: string
  /** The switch's id, for a control that moves focus to it; a generated one otherwise. */
  readonly id?: string
}

/** An on and off control (`role="switch"`) with its label on the left and the switch on the right. */
export function Switch({
  label,
  checked,
  onChange,
  description,
  help = null,
  disabled = false,
  className,
  id: givenId,
}: SwitchProps) {
  const generatedId = useId()
  const id = givenId ?? generatedId
  const labelId = `${id}-label`
  const descriptionId = `${id}-description`
  const helpId = `${id}-help`
  return (
    <div className={clsx('flex flex-col gap-1', className)}>
      <div className="flex min-h-8 items-center justify-between gap-3">
        <HelpLabel help={help} helpId={helpId}>
          <label
            id={labelId}
            htmlFor={id}
            className={clsx(
              'text-sm',
              disabled ? COLOR_CLASSES.textDisabled.foreground : COLOR_CLASSES.text.foreground,
              help !== null && 'cursor-help',
            )}
          >
            {label}
          </label>
        </HelpLabel>
        <button
          id={id}
          type="button"
          role="switch"
          aria-checked={checked}
          aria-labelledby={labelId}
          aria-describedby={describedBy(
            description === undefined ? null : descriptionId,
            help === null ? null : helpId,
          )}
          disabled={disabled}
          onClick={() => {
            onChange(!checked)
          }}
          className={clsx(
            'inline-flex h-6 w-10 shrink-0 items-center rounded-md border p-0.5 transition-colors duration-150 motion-reduce:transition-none',
            checked ? 'justify-end' : 'justify-start',
            COLOR_CLASSES.focusRing.outline,
            // A resting outline color equal to the ring's, so transition-colors does not fade the
            // ring in from the text color.
            COLOR_CLASSES.accent.outline,
            COLOR_CLASSES.borderControl.border,
            checked && !disabled ? COLOR_CLASSES.accent.background : COLOR_CLASSES.field.background,
            disabled && 'cursor-not-allowed',
          )}
        >
          <span
            aria-hidden="true"
            className={clsx(
              'size-4 rounded-sm bg-current',
              disabled && COLOR_CLASSES.textDisabled.foreground,
              !disabled && checked && COLOR_CLASSES.accent.foreground,
              !disabled && !checked && COLOR_CLASSES.textMuted.foreground,
            )}
          />
        </button>
      </div>
      {description === undefined ? null : (
        <p id={descriptionId} className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
          {description}
        </p>
      )}
    </div>
  )
}
