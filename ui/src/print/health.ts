import type { Finding, Location, Severity } from '../generated/protocol'
import { counted } from '../messages'
import { memoized, type HealthBadge } from '../state/selectors'
import type { AppState, FindingKey } from '../state/store'
import type { Vector3 } from '../viewport/math'
import { findingAt, isCheckHidden, PRINT_CHECKS } from './checks'
import { suggestionText } from './suggestion'

// The rows of the print check panel and the findings they point at (ui.md 3.17).

/** One finding of the print check panel. */
export type HealthRow = {
  readonly key: FindingKey
  readonly severity: Severity
  /** The finding's message, verbatim. */
  readonly message: string
  /** The words of the finding's suggestion (`suggestionText`), or null without any. */
  readonly suggestion: string | null
  readonly bodyName: string
}

const SEVERITY_ORDER: Readonly<Record<Severity, number>> = { error: 0, warning: 1, info: 2 }

/** Words for the severity icon, which screen readers read in its place. */
export const SEVERITY_NAMES: Readonly<Record<Severity, string>> = {
  error: 'Error',
  warning: 'Warning',
  info: 'Info',
}

const CHECK_ORDER: ReadonlyMap<string, number> = new Map(
  PRINT_CHECKS.map(({ check }, index) => [check, index]),
)

function checkOrder(check: string): number {
  return CHECK_ORDER.get(check) ?? PRINT_CHECKS.length
}

/**
 * The listed findings of the shown checks, errors first, then warnings, then info; within a
 * severity by the check order of the Checks popover, then in report order. Omitted findings are
 * only counted (by the badge), never listed.
 */
export const healthRows: (state: AppState) => readonly HealthRow[] = memoized(
  (state) =>
    [state.print.report, state.settings.hiddenChecks, state.document?.recipe.bodies] as const,
  (report, hiddenChecks, bodies): readonly HealthRow[] => {
    if (report === null) {
      return []
    }
    const names = new Map((bodies ?? []).map((body) => [body.id, body.name]))
    const rows = report.bodies.flatMap((body) =>
      body.report.checks
        .filter((check) => !isCheckHidden(hiddenChecks, check.check))
        .flatMap((check) =>
          check.findings.map((finding, index) => {
            const suggestion = finding.suggestion ?? null
            return {
              key: { body: body.body, check: check.check, index },
              severity: finding.severity,
              message: finding.message,
              suggestion: suggestion === null ? null : suggestionText(suggestion),
              bodyName: names.get(body.body) ?? body.body,
            }
          }),
        ),
    )
    // Array sort is stable, so rows that tie keep their report order.
    return rows.toSorted(
      (first, second) =>
        SEVERITY_ORDER[first.severity] - SEVERITY_ORDER[second.severity] ||
        checkOrder(first.key.check) - checkOrder(second.key.check),
    )
  },
)

/** The center of a finding's location: its point, or the center of its box. */
export function locationCenter(location: Location): Vector3 {
  if (location.kind === 'point') {
    return location.at
  }
  return [
    (location.min[0] + location.max[0]) / 2,
    (location.min[1] + location.max[1]) / 2,
    (location.min[2] + location.max[2]) / 2,
  ]
}

/**
 * The point a finding is shown at (3.17): a drain hole suggestion's point, else the center of its
 * location.
 */
export function findingPoint(finding: Finding): Vector3 {
  return finding.suggestion?.kind === 'drain_hole'
    ? finding.suggestion.at
    : locationCenter(finding.location)
}

/** Where the finding marker is drawn, in model coordinates: the focused finding's point, or null. */
export const focusedFindingPoint: (state: AppState) => Vector3 | null = memoized(
  (state) => [state.print.report, state.print.focusedFinding] as const,
  (report, focused): Vector3 | null => {
    const finding = focused === null ? undefined : findingAt(report, focused)
    return finding === undefined ? null : findingPoint(finding)
  },
)

/** Whether the document has a body the analysis checks: a visible part body with model geometry (engine.md 3.12). */
export const hasPrintableBody: (state: AppState) => boolean = memoized(
  (state) => [state.document?.recipe.bodies, state.document?.meshes] as const,
  (bodies, meshes): boolean =>
    meshes !== undefined &&
    (bodies ?? []).some(
      (body) =>
        body.visible !== false && (body.role ?? 'part') === 'part' && meshes[body.id] !== undefined,
    ),
)

/** Why the print check panel has no report to list: the first analysis is running, or none has run. */
export type MissingReport = 'checking' | 'unchecked'

/** Why there is no report to list, or null when there is one (a stale report is still listed). */
export function missingReport(state: AppState): MissingReport | null {
  if (state.print.report !== null) {
    return null
  }
  return state.print.running ? 'checking' : 'unchecked'
}

/**
 * The badge's visible text before the out-of-date suffix (3.17): "Check print", "Checking", "Ready
 * to print", "1 warning" or "{n} warnings", "1 error" or "{n} errors".
 */
export function healthBadgeText(badge: HealthBadge): string {
  switch (badge.state) {
    case 'none':
      return 'Check print'
    case 'running':
      return 'Checking'
    case 'ok':
      return 'Ready to print'
    case 'warning':
      return counted(badge.warnings, 'warning')
    case 'error':
      return counted(badge.errors, 'error')
  }
}

/** Whether the badge is shown out of date: a stale report while no analysis runs (3.17). */
export function healthBadgeOutdated(badge: HealthBadge): boolean {
  return badge.stale && badge.state !== 'running'
}

/** A text about the badge's report with " (out of date)" appended while the badge is out of date. */
export function withOutdatedSuffix(badge: HealthBadge, text: string): string {
  return healthBadgeOutdated(badge) ? `${text} (out of date)` : text
}

/**
 * The badge's accessible name, which starts with "Print check: " and its visible text (3.17,
 * WCAG 2.5.3): errors add ", {w} warning(s)" when there are warnings too, and an out-of-date badge
 * ends with ", out of date".
 */
export function healthBadgeName(badge: HealthBadge): string {
  const warnings =
    badge.state === 'error' && badge.warnings > 0 ? `, ${counted(badge.warnings, 'warning')}` : ''
  const outdated = healthBadgeOutdated(badge) ? ', out of date' : ''
  return `Print check: ${healthBadgeText(badge)}${warnings}${outdated}`
}
