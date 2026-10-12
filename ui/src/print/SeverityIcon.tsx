import { clsx } from 'clsx/lite'
import { CircleX, Info, TriangleAlert } from 'lucide-react'
import type { Severity } from '../generated/protocol'
import { COLOR_CLASSES } from '../theme/roles'

/**
 * The icon of a finding's severity (ui.md 2.16.3, 3.17): `info` in `textMuted`, the `triangle-alert`
 * hazard sign of `warningMark`, `circle-x` in `errorMark`. Decorative: the text next to it names the severity.
 */
export function SeverityIcon({ severity }: { readonly severity: Severity }) {
  switch (severity) {
    case 'info':
      return (
        <Info
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
        />
      )
    case 'warning':
      return (
        <TriangleAlert
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx(
            'size-4 shrink-0',
            COLOR_CLASSES.warningMark.fill,
            COLOR_CLASSES.warningMark.foreground,
          )}
        />
      )
    case 'error':
      return (
        <CircleX
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4 shrink-0', COLOR_CLASSES.errorMark.foreground)}
        />
      )
  }
}
