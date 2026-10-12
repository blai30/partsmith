import { clsx } from 'clsx/lite'
import { CircleCheck, LoaderCircle, RefreshCw, X } from 'lucide-react'
import { Button } from '../components/Button'
import { IconButton } from '../components/IconButton'
import type { Severity } from '../generated/protocol'
import type { ReactNode } from 'react'
import type { FindingKey } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import type { HealthRow, MissingReport } from './health'
import { SeverityIcon } from './SeverityIcon'

export type HealthListProps = {
  /** The name of the profile the findings were checked against. */
  readonly profileName: string
  /** The findings to list, in display order (`healthRows`). */
  readonly rows: readonly HealthRow[]
  /** Whether the document has a body the analysis checks; without one the panel asks for a body. */
  readonly hasGeometry: boolean
  /** Why there is no report to list yet (`missingReport`); absent or null when there is one. */
  readonly missingReport?: MissingReport | null
  readonly onCheckAgain: () => void
  /** Frames a finding in the viewport and marks it (`focusFinding`). */
  readonly onFocusFinding: (finding: FindingKey) => void
  /** The finding the viewport marks, highlighted in the list. */
  readonly focusedFinding?: FindingKey | null
  /** Closes the panel; without it the panel has no close button. */
  readonly onClose?: () => void
  /** The id of the panel's heading, so the region around the panel can be labeled by it. */
  readonly headingId?: string
}

/** Words for the severity icon, which screen readers read in its place. */
const SEVERITY_NAMES: Readonly<Record<Severity, string>> = {
  error: 'Error',
  warning: 'Warning',
  info: 'Info',
}

function sameFinding(first: FindingKey, second: FindingKey | null | undefined): boolean {
  return (
    second !== null &&
    second !== undefined &&
    first.body === second.body &&
    first.check === second.check &&
    first.index === second.index
  )
}

/** What the panel says instead of listing findings, or null when it lists them. */
function noticeFor(
  hasGeometry: boolean,
  missingReport: MissingReport | null,
  rowCount: number,
  profileName: string,
): ReactNode {
  if (!hasGeometry) {
    return (
      <p className={clsx('p-4 text-sm', COLOR_CLASSES.textMuted.foreground)}>
        Add a body to check it for printing.
      </p>
    )
  }
  if (missingReport === 'checking') {
    return (
      <p className={clsx('flex items-start gap-2 p-4 text-sm', COLOR_CLASSES.textMuted.foreground)}>
        <LoaderCircle
          aria-hidden="true"
          strokeWidth={1.75}
          className="mt-0.5 size-4 shrink-0 animate-spin motion-reduce:animate-none"
        />
        <span>Checking the model for {profileName}.</span>
      </p>
    )
  }
  if (missingReport === 'unchecked') {
    return (
      <p className={clsx('p-4 text-sm', COLOR_CLASSES.textMuted.foreground)}>Not checked yet.</p>
    )
  }
  if (rowCount === 0) {
    return (
      <p className="flex items-start gap-2 p-4 text-sm">
        <CircleCheck
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('mt-0.5 size-4 shrink-0', COLOR_CLASSES.successMark.foreground)}
        />
        <span>No print issues found with {profileName}.</span>
      </p>
    )
  }
  return null
}

/**
 * The print check panel (`health-list`, ui.md 3.17): "Print check: {profile name}", "Check
 * again", and one row per finding. Clicking a row, or Enter on it, frames the finding in the
 * viewport.
 */
export function HealthList({
  profileName,
  rows,
  hasGeometry,
  missingReport = null,
  onCheckAgain,
  onFocusFinding,
  focusedFinding = null,
  onClose,
  headingId,
}: HealthListProps) {
  const notice = noticeFor(hasGeometry, missingReport, rows.length, profileName)
  return (
    <div data-testid="health-list" className="flex h-full min-h-0 flex-col">
      <div className={clsx('flex flex-col gap-3 border-b p-4', COLOR_CLASSES.border.border)}>
        <div className="flex min-h-6 items-start justify-between gap-2">
          <h2 id={headingId} className="min-w-0 text-base font-semibold break-words">
            Print check: {profileName}
          </h2>
          {onClose === undefined ? null : (
            <IconButton size="compact" label="Close print check" icon={X} onClick={onClose} />
          )}
        </div>
        <div className="flex">
          <Button onClick={onCheckAgain}>
            <RefreshCw aria-hidden="true" strokeWidth={1.75} className="size-4" />
            Check again
          </Button>
        </div>
      </div>
      {notice ?? (
        <ul
          aria-label="Findings"
          className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2"
        >
          {rows.map((row) => {
            const focused = sameFinding(row.key, focusedFinding)
            return (
              <li key={`${row.key.body}:${row.key.check}:${String(row.key.index)}`}>
                <button
                  type="button"
                  data-testid="health-item"
                  data-check={row.key.check}
                  data-severity={row.severity}
                  aria-current={focused ? 'true' : undefined}
                  aria-label={`${SEVERITY_NAMES[row.severity]}: ${row.message} ${row.bodyName}`}
                  onClick={() => {
                    onFocusFinding(row.key)
                  }}
                  className={clsx(
                    'flex w-full items-start gap-2 rounded-sm px-2 py-2 text-left text-sm transition-colors duration-150 motion-reduce:transition-none',
                    COLOR_CLASSES.text.foreground,
                    COLOR_CLASSES.focusRing.outline,
                    // A resting outline color equal to the ring's, so transition-colors does not
                    // fade the ring in from the text color.
                    COLOR_CLASSES.accent.outline,
                    focused ? COLOR_CLASSES.selection.background : COLOR_CLASSES.hover.background,
                  )}
                >
                  <span className="flex h-5 shrink-0 items-center">
                    <SeverityIcon severity={row.severity} />
                  </span>
                  <span className="flex min-w-0 flex-col gap-0.5">
                    <span>{row.message}</span>
                    <span className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
                      {row.bodyName}
                    </span>
                  </span>
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
