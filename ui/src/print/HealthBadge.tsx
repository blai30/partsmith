import { clsx } from 'clsx/lite'
import { CircleCheck, CircleDashed, LoaderCircle } from 'lucide-react'
import type { HealthBadge as HealthBadgeState } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import { healthBadgeName, healthBadgeOutdated, healthBadgeText, withOutdatedSuffix } from './health'
import { SeverityIcon } from './SeverityIcon'

export type HealthBadgeProps = {
  /** The badge as `selectHealthBadge` derives it. */
  readonly badge: HealthBadgeState
  /** Runs the analysis when the report is stale or missing and opens the print check panel. */
  readonly onClick: () => void
}

/** The icon of a badge state (3.17), also shown by the export popover's print check line (3.18). */
export function HealthBadgeIcon({ state }: { readonly state: HealthBadgeState['state'] }) {
  switch (state) {
    case 'none':
      return (
        <CircleDashed
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
        />
      )
    case 'running':
      return (
        <LoaderCircle
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx(
            'size-4 shrink-0 animate-spin motion-reduce:animate-none',
            COLOR_CLASSES.textMuted.foreground,
          )}
        />
      )
    case 'ok':
      return (
        <CircleCheck
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4 shrink-0', COLOR_CLASSES.successMark.foreground)}
        />
      )
    case 'warning':
    case 'error':
      return <SeverityIcon severity={state} />
  }
}

/**
 * The print check badge of the status bar (`health-badge`, ui.md 3.17): an icon and a short text
 * for the last report. A report of an older revision keeps its state, drawn at half opacity and
 * marked out of date.
 */
export function HealthBadge({ badge, onClick }: HealthBadgeProps) {
  // A running analysis checks the current revision, so the badge is not out of date while it
  // runs: not drawn, named or marked so.
  const outdated = healthBadgeOutdated(badge)
  return (
    <button
      type="button"
      data-testid="health-badge"
      data-state={badge.state}
      data-stale={outdated ? 'true' : 'false'}
      aria-label={healthBadgeName(badge)}
      onClick={onClick}
      className={clsx(
        'inline-flex h-6 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs whitespace-nowrap tabular-nums transition-colors duration-150 motion-reduce:transition-none',
        // An out-of-date report recedes: muted text, which keeps 4.5:1, and a dimmed icon (3.17).
        outdated ? COLOR_CLASSES.textMuted.foreground : COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.hover.background,
        COLOR_CLASSES.focusRing.inset,
        // A resting outline color equal to the ring's, so transition-colors does not fade the
        // ring in from the text color.
        COLOR_CLASSES.accent.outline,
      )}
    >
      <span className={clsx('inline-flex', outdated && 'opacity-50')}>
        <HealthBadgeIcon state={badge.state} />
      </span>
      {withOutdatedSuffix(badge, healthBadgeText(badge))}
    </button>
  )
}
