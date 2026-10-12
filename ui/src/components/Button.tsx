import { clsx } from 'clsx/lite'
import type { ComponentProps } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { isInactive } from './disabled'

export type ButtonProps = Omit<ComponentProps<'button'>, 'type'> & {
  /** `primary` is the accent-filled primary action (OK, Finish sketch, Export). */
  readonly variant?: 'default' | 'primary'
}

/**
 * A text button that never submits a form. `aria-disabled` only changes how it looks, so a click
 * can still explain why the action is unavailable.
 */
export function Button({ variant = 'default', className, disabled, ...rest }: ButtonProps) {
  const inactive = isInactive(disabled, rest['aria-disabled'])
  const primary = variant === 'primary' && !inactive
  return (
    <button
      {...rest}
      type="button"
      disabled={disabled}
      className={clsx(
        'inline-flex h-8 shrink-0 items-center justify-center gap-2 rounded-sm border px-3 text-sm font-medium whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.focusRing.outline,
        // A resting outline color equal to the ring's, so transition-colors does not fade the
        // ring in from the text color.
        COLOR_CLASSES.accent.outline,
        primary && COLOR_CLASSES.accent.border,
        primary && COLOR_CLASSES.accent.background,
        primary && COLOR_CLASSES.accent.foreground,
        primary && COLOR_CLASSES.accentHover.background,
        !primary && COLOR_CLASSES.border.border,
        !primary && !inactive && COLOR_CLASSES.text.foreground,
        !primary && !inactive && COLOR_CLASSES.hover.background,
        inactive && COLOR_CLASSES.textDisabled.foreground,
        inactive && 'cursor-not-allowed',
        className,
      )}
    />
  )
}
