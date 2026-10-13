import type { ExportRequest } from '../app/actions/export'
import { featureLabel } from '../core/labels'
import { MESSAGES } from '../messages'
import { healthBadgeOutdated, healthBadgeText, withOutdatedSuffix } from '../print/health'
import { selectOrientationBody } from '../print/orientation'
import { selectActiveFeatureIds, selectHealthBadge, type HealthBadge } from '../state/selectors'
import type { AppState } from '../state/store'

// What an export is about to write (ui.md 3.18): up to three status lines above the export
// popover's Export button, so failures, the print check and the print orientation are clear while
// export stays one click with no confirmation.

/** One status line of the export popover, in display order. */
export type ExportStatusLine =
  /** Active features in error, which the export leaves out; offers "Show problems". */
  | { readonly kind: 'failed'; readonly text: string }
  /** The print check of 3MF and STL exports; offers "Check print" when `checkOffered`. */
  | {
      readonly kind: 'print_check'
      readonly text: string
      readonly badge: HealthBadge
      readonly checkOffered: boolean
    }
  /** The Lay on face orientation of 3MF and STL exports; offers "Print as modeled". */
  | { readonly kind: 'orientation'; readonly text: string }

/** Formats whose files go to a printer, so the print check and the orientation apply to them. */
const PRINT_FORMATS: ReadonlySet<ExportRequest['format']> = new Set(['3mf', 'stl'])

/** The active features whose status is error, in timeline order. */
function failedFeatures(state: AppState): readonly string[] {
  const statuses = state.document?.featureStatus ?? {}
  return selectActiveFeatureIds(state).filter((id) => statuses[id]?.status === 'error')
}

/**
 * The text that says which features an export leaves out: `export_failed_feature` for one,
 * `export_failed_features` for several, or null when no active feature is in error.
 */
export function exportFailedText(state: AppState): string | null {
  const document = state.document
  if (document === null) {
    return null
  }
  const failed = failedFeatures(state)
  const [only] = failed
  if (only === undefined) {
    return null
  }
  if (failed.length > 1) {
    return MESSAGES.export_failed_features(failed.length)
  }
  return MESSAGES.export_failed_feature(featureLabel(document.recipe, only, state.catalog))
}

/** "Print check: {state}": the badge's text, "Not checked" before any report, and " (out of date)" when stale. */
function printCheckText(badge: HealthBadge): string {
  const text = badge.state === 'none' ? 'Not checked' : healthBadgeText(badge)
  return `Print check: ${withOutdatedSuffix(badge, text)}`
}

/** What the status lines are made from, each read from the state by its own selector. */
export type ExportStatusInputs = {
  /** `exportFailedText`. */
  readonly failedText: string | null
  /** `selectHealthBadge`. */
  readonly badge: HealthBadge
  /** `selectOrientationBody`. */
  readonly orientationBody: string | null
}

/** The status lines of the export popover for a format (3.18). */
export function statusLines(
  format: ExportRequest['format'],
  { failedText, badge, orientationBody }: ExportStatusInputs,
): readonly ExportStatusLine[] {
  const lines: ExportStatusLine[] = []
  if (failedText !== null) {
    lines.push({ kind: 'failed', text: failedText })
  }
  if (!PRINT_FORMATS.has(format)) {
    return lines
  }
  lines.push({
    kind: 'print_check',
    text: printCheckText(badge),
    badge,
    checkOffered: badge.state === 'none' || healthBadgeOutdated(badge),
  })
  if (orientationBody !== null) {
    lines.push({ kind: 'orientation', text: MESSAGES.orientation_set(orientationBody) })
  }
  return lines
}

/** The status lines of the export popover for a format, read from the state (3.18). */
export function exportStatus(
  state: AppState,
  format: ExportRequest['format'],
): readonly ExportStatusLine[] {
  return statusLines(format, {
    failedText: exportFailedText(state),
    badge: selectHealthBadge(state),
    orientationBody: selectOrientationBody(state),
  })
}
