import { clsx } from 'clsx/lite'
import { CircleX, Info, TriangleAlert } from 'lucide-react'
import type { Severity } from '../generated/protocol'
import { COLOR_CLASSES } from '../theme/roles'

export type SeverityIconProps = {
  readonly severity: Severity
  /** The icon's accessible name; without it the icon is decorative and the text next to it names the severity. */
  readonly label?: string
  /** Classes added before the icon's own, such as a margin. */
  readonly className?: string
}

/**
 * The icon of a finding's severity (ui.md 2.16.3, 3.17): `info` in `textMuted`, the `triangle-alert`
 * hazard sign of `warningMark`, `circle-x` in `errorMark`.
 */
export function SeverityIcon({ severity, label, className }: SeverityIconProps) {
  const accessibility = {
    'aria-hidden': label === undefined ? ('true' as const) : undefined,
    'aria-label': label,
  }
  switch (severity) {
    case 'info':
      return (
        <Info
          {...accessibility}
          strokeWidth={1.75}
          className={clsx(className, 'size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
        />
      )
    case 'warning':
      return (
        <TriangleAlert
          {...accessibility}
          strokeWidth={1.75}
          className={clsx(
            className,
            'size-4 shrink-0',
            COLOR_CLASSES.warningMark.fill,
            COLOR_CLASSES.warningMark.foreground,
          )}
        />
      )
    case 'error':
      return (
        <CircleX
          {...accessibility}
          strokeWidth={1.75}
          className={clsx(className, 'size-4 shrink-0', COLOR_CLASSES.errorMark.foreground)}
        />
      )
  }
}
