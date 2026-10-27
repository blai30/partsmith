import type { CheckKind, Finding, PrintReport } from '../generated/protocol'
import type { FindingKey } from '../state/store'

// The printability checks as the UI presents them, and the findings of a report by check (ui.md
// 3.17, print.md 3.9).

export type PrintCheck = {
  readonly check: CheckKind
  readonly title: string
}

/**
 * Every check in display order: the order of the Checks popover's switches and the tie-break order
 * of the print check list. Mesh validity comes last because it cannot be switched off.
 */
export const PRINT_CHECKS: readonly PrintCheck[] = [
  { check: 'overhangs', title: 'Overhangs' },
  { check: 'bed_fit', title: 'Bed fit' },
  { check: 'first_layer_contact', title: 'First layer contact' },
  { check: 'thin_walls', title: 'Thin walls' },
  { check: 'small_features', title: 'Small features' },
  { check: 'islands', title: 'Islands' },
  { check: 'trapped_volumes', title: 'Trapped volumes' },
  { check: 'mesh_validity', title: 'Mesh validity' },
]

/** The check that always runs and is always shown: every export depends on a valid mesh. */
export const ALWAYS_SHOWN_CHECK: CheckKind = 'mesh_validity'

/**
 * Whether a check's findings are hidden by `settings.hiddenChecks`. Mesh validity is never hidden,
 * even when stored settings (an older build, a hand edit) list it.
 */
export function isCheckHidden(hiddenChecks: readonly string[], check: string): boolean {
  return check !== ALWAYS_SHOWN_CHECK && hiddenChecks.includes(check)
}

/** The finding a key names in a report, or undefined when the report has no such finding. */
export function findingAt(report: PrintReport | null, key: FindingKey): Finding | undefined {
  return report?.bodies
    .find((body) => body.body === key.body)
    ?.report.checks.find((check) => check.check === key.check)?.findings[key.index]
}
