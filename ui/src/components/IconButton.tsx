import { clsx } from 'clsx/lite'
import type { LucideIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { isInactive } from './disabled'

export type IconButtonProps = Omit<
  ComponentProps<'button'>,
  'type' | 'children' | 'aria-label' | 'aria-pressed'
> & {
  /** The accessible name; icon-only buttons show it in their tooltip. */
  readonly label: string
  readonly icon: LucideIcon
  /** For toggle buttons: the state, exposed as aria-pressed. Absent for plain buttons. */
  readonly pressed?: boolean
  /** `toolbar` is size-9 with a size-5 icon; `compact` is size-6 with a size-4 icon, for rows and fields. */
  readonly size?: 'toolbar' | 'compact'
}

/** An icon-only button named by its label; a toggle when `pressed` is given. */
export function IconButton({
  label,
  icon: Icon,
  pressed,
  size = 'toolbar',
  className,
  disabled,
  ...rest
}: IconButtonProps) {
  const inactive = isInactive(disabled, rest['aria-disabled'])
  const selected = pressed === true && !inactive
  return (
    <button
      {...rest}
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      className={clsx(
        'inline-flex shrink-0 items-center justify-center rounded-sm transition-colors duration-150 motion-reduce:transition-none',
        size === 'toolbar' ? 'size-9' : 'size-6',
        COLOR_CLASSES.focusRing.outline,
        // A resting outline color equal to the ring's, so transition-colors does not fade the
        // ring in from the text color.
        COLOR_CLASSES.accent.outline,
        selected && COLOR_CLASSES.selection.background,
        selected && COLOR_CLASSES.text.foreground,
        !selected && !inactive && COLOR_CLASSES.hover.background,
        !selected && !inactive && COLOR_CLASSES.textMuted.foreground,
        inactive && COLOR_CLASSES.textDisabled.foreground,
        inactive && 'cursor-not-allowed',
        className,
      )}
    >
      <Icon
        aria-hidden="true"
        strokeWidth={1.75}
        className={size === 'toolbar' ? 'size-5' : 'size-4'}
      />
    </button>
  )
}
