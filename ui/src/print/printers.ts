import type { PrinterProfile, Technology } from '../generated/protocol'
import type { AppState } from '../state/store'

// Printer profiles as the Print context shows them (ui.md 3.17, print.md 2.2).

/** The profile the core uses for a document that names none or names one it does not know (print.md `DEFAULT_PROFILE_ID`). */
export const DEFAULT_PRINTER_ID = 'generic-fdm'

/** The profiles of one technology under their menu heading. */
export type PrinterGroup = {
  readonly technology: Technology
  readonly title: string
  readonly printers: readonly PrinterProfile[]
}

const TECHNOLOGY_TITLES: readonly { readonly technology: Technology; readonly title: string }[] = [
  { technology: 'fdm', title: 'FDM' },
  { technology: 'resin', title: 'Resin' },
]

/** The profiles grouped by technology, FDM first, each in list order; technologies without a profile are left out. */
export function printerGroups(printers: readonly PrinterProfile[]): readonly PrinterGroup[] {
  return TECHNOLOGY_TITLES.flatMap(({ technology, title }) => {
    const members = printers.filter((printer) => printer.technology === technology)
    return members.length === 0 ? [] : [{ technology, title, printers: members }]
  })
}

/** The display name of a profile, or its id when it is not installed. */
export function printerName(printers: readonly PrinterProfile[], id: string): string {
  return printers.find((printer) => printer.id === id)?.name ?? id
}

/** The profile the core resolves for a document printer id (engine.md 3.7). */
export function resolvedPrinter(
  printers: readonly PrinterProfile[],
  id: string | null | undefined,
): PrinterProfile | null {
  return (
    printers.find((printer) => printer.id === id) ??
    printers.find((printer) => printer.id === DEFAULT_PRINTER_ID) ??
    null
  )
}

/** The name of the profile the health list describes: the report's, else the one the next analysis will use. */
export function healthProfileName(state: AppState): string {
  const report = state.print.report
  if (report !== null) {
    return printerName(state.printers, report.profile)
  }
  const printer = state.document?.recipe.printer
  return (
    resolvedPrinter(state.printers, printer)?.name ??
    printerName(state.printers, printer ?? DEFAULT_PRINTER_ID)
  )
}
