import { clsx } from 'clsx/lite'
import { CircleCheck, CircleDashed, LoaderCircle } from 'lucide-react'
import type { HealthBadge as HealthBadgeState } from '../state/selectors'
import { COLOR_CLASSES } from '../theme/roles'
import { SeverityIcon } from './SeverityIcon'

export type HealthBadgeProps = {
  /** The badge as `selectHealthBadge` derives it. */
  readonly badge: HealthBadgeState
  /** Runs the analysis when the report is stale or missing and opens the health panel. */
  readonly onClick: () => void
}

function plural(count: number, noun: string): string {
  return `${String(count)} ${noun}${count === 1 ? '' : 's'}`
}

/** The visible text and the accessible name of a badge state, before the stale suffixes (ui.md 3.17). */
function wording(badge: HealthBadgeState): { readonly text: string; readonly name: string } {
  switch (badge.state) {
    case 'none':
      return { text: 'Check print', name: 'not checked' }
    case 'running':
      return { text: 'Checking', name: 'checking' }
    case 'ok':
      return { text: 'Ready to print', name: 'no problems' }
    case 'warning': {
      const warnings = plural(badge.warnings, 'warning')
      return { text: warnings, name: warnings }
    }
    case 'error': {
      const errors = plural(badge.errors, 'error')
      return { text: errors, name: `${errors}, ${plural(badge.warnings, 'warning')}` }
    }
  }
}

function BadgeIcon({ state }: { readonly state: HealthBadgeState['state'] }) {
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
 * The print health badge of the status bar (`health-badge`, ui.md 3.17): an icon and a short text
 * for the last report. A report of an older revision keeps its state, drawn at half opacity and
 * marked out of date.
 */
export function HealthBadge({ badge, onClick }: HealthBadgeProps) {
  const { text, name } = wording(badge)
  // A running analysis replaces the state the old report gave, so it is not drawn as out of date.
  const outdated = badge.stale && badge.state !== 'running'
  return (
    <button
      type="button"
      data-testid="health-badge"
      data-state={badge.state}
      data-stale={badge.stale ? 'true' : 'false'}
      aria-label={`Print health: ${name}${outdated ? ', out of date' : ''}`}
      onClick={onClick}
      className={clsx(
        'inline-flex h-6 shrink-0 items-center gap-1.5 rounded-sm px-2 text-xs whitespace-nowrap tabular-nums transition-colors duration-150 motion-reduce:transition-none',
        COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.hover.background,
        COLOR_CLASSES.focusRing.outline,
        // A resting outline color equal to the ring's, so transition-colors does not fade the
        // ring in from the text color.
        COLOR_CLASSES.accent.outline,
        outdated && 'opacity-50',
      )}
    >
      <BadgeIcon state={badge.state} />
      {outdated ? `${text} (out of date)` : text}
    </button>
  )
}
