import { clsx } from 'clsx/lite'
import { useId } from 'react'
import { COLOR_CLASSES } from '../theme/roles'

export type SwitchProps = {
  readonly label: string
  readonly checked: boolean
  /** Called with the state a click or Space asks for; the switch shows `checked` until it changes. */
  readonly onChange: (checked: boolean) => void
  /** Help text under the label, exposed as the switch's accessible description. */
  readonly description?: string
  readonly disabled?: boolean
  readonly className?: string
}

/** An on and off control (`role="switch"`) with its label on the left and the switch on the right. */
export function Switch({
  label,
  checked,
  onChange,
  description,
  disabled = false,
  className,
}: SwitchProps) {
  const id = useId()
  const labelId = `${id}-label`
  const descriptionId = `${id}-description`
  return (
    <div className={clsx('flex flex-col gap-1', className)}>
      <div className="flex min-h-8 items-center justify-between gap-3">
        <label
          id={labelId}
          htmlFor={id}
          className={clsx(
            'text-sm',
            disabled ? COLOR_CLASSES.textDisabled.foreground : COLOR_CLASSES.text.foreground,
          )}
        >
          {label}
        </label>
        <button
          id={id}
          type="button"
          role="switch"
          aria-checked={checked}
          aria-labelledby={labelId}
          aria-describedby={description === undefined ? undefined : descriptionId}
          disabled={disabled}
          onClick={() => {
            onChange(!checked)
          }}
          className={clsx(
            'inline-flex h-6 w-10 shrink-0 items-center rounded-sm border p-0.5 transition-colors duration-150 motion-reduce:transition-none',
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
              'size-4 rounded-xs bg-current',
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
