import { clsx } from 'clsx/lite'
import { useId, type ComponentProps } from 'react'
import { COLOR_CLASSES } from '../theme/roles'

export type TextFieldProps = Omit<
  ComponentProps<'input'>,
  'type' | 'value' | 'onChange' | 'className' | 'children'
> & {
  readonly label: string
  readonly value: string
  /** Called with the new text on every edit. */
  readonly onChange: (value: string) => void
  /** A message shown under the field; the field is then marked invalid and described by it. */
  readonly error?: string | null
  /** Keeps the label for assistive technology only, for fields whose row already says what they are. */
  readonly hideLabel?: boolean
  /** Layout classes for the field's wrapper, such as a width. */
  readonly className?: string
}

/** A labeled single-line text input with an optional error under it. */
export function TextField({
  label,
  value,
  onChange,
  error = null,
  hideLabel = false,
  className,
  id,
  disabled,
  ...rest
}: TextFieldProps) {
  const generatedId = useId()
  const inputId = id ?? generatedId
  const errorId = `${inputId}-error`
  const invalid = error !== null && error !== ''
  const describedBy = [rest['aria-describedby'], invalid ? errorId : undefined]
    .filter((part) => part !== undefined && part !== '')
    .join(' ')
  return (
    <div className={clsx('flex min-w-0 flex-col gap-1', className)}>
      <label
        htmlFor={inputId}
        className={clsx(
          hideLabel ? 'sr-only' : 'text-xs font-medium',
          !hideLabel && COLOR_CLASSES.textMuted.foreground,
        )}
      >
        {label}
      </label>
      <input
        {...rest}
        id={inputId}
        type="text"
        value={value}
        disabled={disabled}
        aria-invalid={invalid ? true : undefined}
        aria-describedby={describedBy === '' ? undefined : describedBy}
        onChange={(event) => {
          onChange(event.currentTarget.value)
        }}
        className={clsx(
          'h-8 w-full min-w-0 rounded-sm border px-2 text-sm',
          COLOR_CLASSES.field.background,
          COLOR_CLASSES.focusRing.outline,
          invalid ? COLOR_CLASSES.errorMark.border : COLOR_CLASSES.borderControl.border,
          disabled === true ? COLOR_CLASSES.textDisabled.foreground : COLOR_CLASSES.text.foreground,
          disabled === true && 'cursor-not-allowed',
        )}
      />
      <div aria-live="polite">
        {invalid ? (
          <p id={errorId} className={clsx('text-xs', COLOR_CLASSES.errorText.foreground)}>
            {error}
          </p>
        ) : null}
      </div>
    </div>
  )
}
