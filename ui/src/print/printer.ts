import { formatNumber } from '../expression/format'
import type { Bed, PrinterProfile, Technology } from '../generated/protocol'
import type { AppState } from '../state/store'

// Printer profiles as the Print context shows them (ui.md 3.17, 3.21, print.md 2.2).

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

/** The installed profile with this id, if there is one. */
export function findPrinter(
  printers: readonly PrinterProfile[],
  id: string | null | undefined,
): PrinterProfile | undefined {
  return printers.find((printer) => printer.id === id)
}

/** The display name of a profile, or its id when it is not installed. */
export function printerName(printers: readonly PrinterProfile[], id: string): string {
  return findPrinter(printers, id)?.name ?? id
}

/** The profile the core resolves for a document printer id (engine.md 3.7). */
export function resolvedPrinter(
  printers: readonly PrinterProfile[],
  id: string | null | undefined,
): PrinterProfile | null {
  return findPrinter(printers, id) ?? findPrinter(printers, DEFAULT_PRINTER_ID) ?? null
}

/** The name of the profile the print check panel describes: the report's, else the one the next analysis will use. */
export function healthProfileName(state: AppState): string {
  const report = state.print.report
  if (report !== null) {
    return printerName(state.printers, report.profile)
  }
  // Without the document's profile or the default installed, the id stands for its name.
  const printer = state.document?.recipe.printer
  return resolvedPrinter(state.printers, printer)?.name ?? printer ?? DEFAULT_PRINTER_ID
}

/** The name of the default profile, which the engine uses for a document without a printer. */
export function defaultPrinterName(printers: readonly PrinterProfile[]): string {
  return printerName(printers, DEFAULT_PRINTER_ID)
}

/**
 * The document's printer as the status bar names it (3.21): the profile's name, or "{default
 * name} (default)" when the document has none, so the profile the checks use is always named.
 */
export function printerLabel(
  printers: readonly PrinterProfile[],
  id: string | null | undefined,
): string {
  return id === null || id === undefined
    ? `${defaultPrinterName(printers)} (default)`
    : printerName(printers, id)
}

/** The text of the Print toolbar's printer button (3.17): "Printer: {name}" or "Printer: {default name} (default)". */
export function printerButtonText(
  printers: readonly PrinterProfile[],
  id: string | null | undefined,
): string {
  return `Printer: ${printerLabel(printers, id)}`
}

/** A length in mm with `formatNumber`; profiles are always in mm, whatever the document unit. */
function millimeters(value: number): string {
  return formatNumber(value, 'length', 'mm')
}

/** The build volume: "{width} × {depth} × {height}", or "⌀{diameter} × {height}" for a round bed. */
function buildVolume(bed: Bed): string {
  return bed.kind === 'rectangle'
    ? `${millimeters(bed.width)} × ${millimeters(bed.depth)} × ${millimeters(bed.height)}`
    : `⌀${millimeters(bed.diameter)} × ${millimeters(bed.height)}`
}

/**
 * The key numbers of a profile under its name in the printer menu (3.17). FDM: "{width} × {depth} ×
 * {height} mm bed · {nozzle} mm nozzle" (a round bed reads "⌀{diameter} × {height} mm bed");
 * resin: "{width} × {depth} × {height} mm". An FDM profile without a nozzle shows its bed alone.
 */
export function printerDetail(printer: PrinterProfile): string {
  const volume = `${buildVolume(printer.bed)} mm`
  if (printer.technology === 'resin') {
    return volume
  }
  const nozzle = printer.nozzle_diameter ?? null
  return nozzle === null ? `${volume} bed` : `${volume} bed · ${millimeters(nozzle)} mm nozzle`
}
