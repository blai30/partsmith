import type { Finding, Location, PrintReport, Severity } from '../generated/protocol'
import type { AppState, FindingKey } from '../state/store'
import type { Vector3 } from '../viewport/math'
import { isCheckHidden, PRINT_CHECKS } from './checks'
import { memoized } from './memoize'

// The rows of the print check panel and the findings they point at (ui.md 3.17).

/** One finding of the print check panel. */
export type HealthRow = {
  readonly key: FindingKey
  readonly severity: Severity
  /** The finding's message, verbatim. */
  readonly message: string
  readonly bodyName: string
}

const SEVERITY_ORDER: Readonly<Record<Severity, number>> = { error: 0, warning: 1, info: 2 }

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
          check.findings.map((finding, index) => ({
            key: { body: body.body, check: check.check, index },
            severity: finding.severity,
            message: finding.message,
            bodyName: names.get(body.body) ?? body.body,
          })),
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

/** The finding a key names in a report, or undefined when the report has no such finding. */
export function findingAt(report: PrintReport | null, key: FindingKey): Finding | undefined {
  return report?.bodies
    .find((body) => body.body === key.body)
    ?.report.checks.find((check) => check.check === key.check)?.findings[key.index]
}

/** The point a finding is shown at: its point, or the center of its box. */
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

/** Where the finding marker is drawn, in model coordinates: the focused finding's center, or null. */
export const focusedFindingPoint: (state: AppState) => Vector3 | null = memoized(
  (state) => [state.print.report, state.print.focusedFinding] as const,
  (report, focused): Vector3 | null => {
    if (focused === null) {
      return null
    }
    const finding = findingAt(report, focused)
    return finding === undefined ? null : locationCenter(finding.location)
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
