import type { PrinterProfile, PrintReport } from '../generated/protocol'
import type { AppState } from '../state/store'
import { encodedFromHex, VIEWPORT_COLORS, type ThemeName } from '../theme/roles'
import type { RenderFrame } from '../viewport/backend'
import { findPrinter } from './printer'

// The overhang color map (ui.md 3.12 pass 3, 3.17): which bodies show it, its ramp and its legend.

/** The overhang angle the ramp ends at: a horizontal ceiling, π/2 from vertical. */
const HORIZONTAL = Math.PI / 2

/**
 * The overhang limit in radians from vertical: the `limit` of the first finding of an `overhangs`
 * check in the report, else the `max_overhang_angle` of the report's profile, else null when that
 * profile is not installed.
 */
export function overhangLimit(
  report: PrintReport,
  printers: readonly PrinterProfile[],
): number | null {
  for (const body of report.bodies) {
    for (const check of body.report.checks) {
      if (check.check !== 'overhangs') {
        continue
      }
      const limit = check.findings.find((finding) => finding.limit !== undefined)?.limit
      if (limit !== undefined) {
        return limit
      }
    }
  }
  return findPrinter(printers, report.profile)?.max_overhang_angle ?? null
}

/**
 * Whether the map is on: Print context and the Overhang map toggle on. The toggle alone shows or
 * hides the map and its legend (3.17); the Overhangs check switch only hides findings.
 */
export function overhangMapShown(state: AppState): boolean {
  return state.context === 'print' && state.print.overhangMap
}

/**
 * The scalar field of each body that shows the map: the body's overhang buffer from the last
 * print_health event, when the report analyzed the mesh the body shows now. A body whose mesh
 * changed since the report shows no map until the next report. The buffers are passed on, not copied.
 */
export function overhangFields(
  state: AppState,
  overhang: Readonly<Record<string, Float32Array>>,
): Readonly<Record<string, Float32Array>> {
  const report = state.print.report
  const meshes = state.document?.meshes
  if (report === null || meshes === undefined || !overhangMapShown(state)) {
    return {}
  }
  const fields: Record<string, Float32Array> = {}
  for (const body of report.bodies) {
    const values = overhang[body.body]
    const meshId = meshes[body.body]?.meshId
    if (values !== undefined && meshId !== undefined && body.mesh_id === meshId) {
      fields[body.body] = values
    }
  }
  return fields
}

/** The ramp of `RenderFrame.analysis`: `analysisLow` at the limit to `analysisHigh` at 90 degrees; null while the map is off. */
export function overhangAnalysis(state: AppState, theme: ThemeName): RenderFrame['analysis'] {
  const limit = overhangLegendLimit(state)
  if (limit === null) {
    return null
  }
  const colors = VIEWPORT_COLORS[theme]
  return {
    low: encodedFromHex(colors.analysisLow),
    high: encodedFromHex(colors.analysisHigh),
    threshold: limit,
    maximum: HORIZONTAL,
  }
}

/** The limit the legend shows, or null when there is no legend: the map is off or no report has a limit. */
export function overhangLegendLimit(state: AppState): number | null {
  const report = state.print.report
  return report === null || !overhangMapShown(state) ? null : overhangLimit(report, state.printers)
}
