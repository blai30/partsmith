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
  /**
   * The fill of the pressed state: `selection` for tools and modes, `neutral` (the `hover` role's
   * `active` fill) for a toggle of what the window shows, such as the top bar's Toggle browser
   * and Dark theme, which is neither a tool nor a selection (3.23).
   */
  readonly pressedFill?: 'selection' | 'neutral'
  /** `toolbar` is size-9 with a size-5 icon; `field` is size-8 with a size-4 icon, beside a field's input; `compact` is size-6 with a size-4 icon, for rows and fields. */
  readonly size?: 'toolbar' | 'field' | 'compact'
  /** The focus ring: `inset` inside the button, for islands under 2.5rem tall whose edge would cut an outer ring (3.23). */
  readonly focusRing?: 'outline' | 'inset'
}

/** An icon-only button named by its label; a toggle when `pressed` is given. */
export function IconButton({
  label,
  icon: Icon,
  pressed,
  pressedFill = 'selection',
  size = 'toolbar',
  focusRing = 'outline',
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
        'inline-flex shrink-0 items-center justify-center rounded-md transition-colors duration-150 motion-reduce:transition-none',
        size === 'toolbar' && 'size-9',
        size === 'field' && 'size-8',
        size === 'compact' && 'size-6',
        COLOR_CLASSES.focusRing[focusRing],
        // A resting outline color equal to the ring's, so transition-colors does not fade the
        // ring in from the text color; a toggle rests with its transparent state outline instead,
        // which forced colors draw while it is pressed (3.23).
        pressed === undefined ? COLOR_CLASSES.accent.outline : COLOR_CLASSES.forcedEdge.outline,
        selected && 'outline -outline-offset-1',
        selected && pressedFill === 'selection' && COLOR_CLASSES.selection.background,
        selected && pressedFill === 'neutral' && COLOR_CLASSES.hover.active,
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
