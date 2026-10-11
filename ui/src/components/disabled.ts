import type { ComponentProps } from 'react'

/** Whether a control is shown as unavailable: `disabled`, or `aria-disabled` (which keeps it focusable). */
export function isInactive(
  disabled: boolean | undefined,
  ariaDisabled: ComponentProps<'button'>['aria-disabled'],
): boolean {
  return disabled === true || ariaDisabled === true || ariaDisabled === 'true'
}
