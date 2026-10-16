import { clsx } from 'clsx/lite'
import type { ComponentProps } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { isInactive } from './disabled'

export type ButtonProps = Omit<ComponentProps<'button'>, 'type'> & {
  /**
   * `primary` is the accent-filled primary action (OK, Finish sketch, Export); `banner` is a
   * default button on a status banner, whose fill the default tonal step nearly matches (3.23).
   */
  readonly variant?: 'default' | 'primary' | 'banner'
  /** `compact` is 1.5rem tall with extra-small text, for a row in a slim bar. */
  readonly size?: 'default' | 'compact'
}

/**
 * A text button that never submits a form: the accent-filled primary, or a tonal default that reads
 * as a button without an outline (3.23). Given `aria-pressed`, it is a toggle. `aria-disabled`
 * only changes how it looks, so a click can still explain why the action is unavailable.
 */
export function Button({
  variant = 'default',
  size = 'default',
  className,
  disabled,
  ...rest
}: ButtonProps) {
  const inactive = isInactive(disabled, rest['aria-disabled'])
  const primary = variant === 'primary' && !inactive
  const toggle = rest['aria-pressed'] !== undefined
  const pressed = toggle && rest['aria-pressed'] !== false && rest['aria-pressed'] !== 'false'
  const tonal = !primary && !pressed
  // On a status banner the default fills are the banner facets, which keep their step there.
  const fill = variant === 'banner' ? COLOR_CLASSES.tonal.banner : COLOR_CLASSES.tonal.background
  const hoverFill =
    variant === 'banner' ? COLOR_CLASSES.hover.banner : COLOR_CLASSES.hover.background
  return (
    <button
      {...rest}
      type="button"
      disabled={disabled}
      className={clsx(
        'inline-flex shrink-0 items-center justify-center gap-2 rounded-md border font-medium whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none',
        size === 'default' && 'h-8 px-3 text-sm',
        size === 'compact' && 'h-6 px-2 text-xs',
        COLOR_CLASSES.focusRing.outline,
        // A resting outline color equal to the ring's, so transition-colors does not fade the
        // ring in from the text color; a toggle rests with its transparent state outline instead.
        toggle ? COLOR_CLASSES.forcedEdge.outline : COLOR_CLASSES.accent.outline,
        pressed && 'outline -outline-offset-1',
        primary && COLOR_CLASSES.accent.border,
        primary && COLOR_CLASSES.accent.background,
        primary && COLOR_CLASSES.accent.foreground,
        primary && COLOR_CLASSES.accentHover.background,
        !primary && COLOR_CLASSES.forcedEdge.border,
        pressed && COLOR_CLASSES.selection.background,
        tonal && fill,
        tonal && !inactive && hoverFill,
        !primary && !inactive && COLOR_CLASSES.text.foreground,
        inactive && COLOR_CLASSES.textDisabled.foreground,
        inactive && 'cursor-not-allowed',
        className,
      )}
    />
  )
}
