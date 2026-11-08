import { formatNumber } from '../expression/format'
import type { Bed, Document, PrinterProfile, Technology } from '../generated/protocol'
import type { AppState } from '../state/store'

// Printer profiles as the Print context shows them (ui.md 3.17, 3.21, print.md 2.2).

/** The profile the core uses for a document that names none or names one it does not know (print.md `DEFAULT_PROFILE_ID`). */
export const DEFAULT_PRINTER_ID = 'generic-fdm'

/** The built-in generic profile of each technology (print.md 2.2), whose values a new printer starts from (3.17). */
export const GENERIC_PRINTER_IDS: Readonly<Record<Technology, string>> = {
  fdm: DEFAULT_PRINTER_ID,
  resin: 'generic-resin',
}

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

/**
 * The printer in effect (3.17, engine.md 3.7): the profile the document stores, else the listed
 * profile with its printer id, else the default profile; null without the default listed.
 */
export function documentPrinter(
  document: Document,
  printers: readonly PrinterProfile[],
): PrinterProfile | null {
  return (
    document.printer_profile ??
    findPrinter(printers, document.printer) ??
    findPrinter(printers, DEFAULT_PRINTER_ID) ??
    null
  )
}

/**
 * The profiles a print report of a document can name (3.17): the listed ones, with `stored`, the
 * profile the document stores, in place of a listed one with its id.
 */
export function reportPrinters(
  stored: PrinterProfile | null | undefined,
  printers: readonly PrinterProfile[],
): readonly PrinterProfile[] {
  return stored === null || stored === undefined
    ? printers
    : [stored, ...printers.filter((printer) => printer.id !== stored.id)]
}

/** The name of the profile the print check panel describes: the report's, else the one the next analysis will use. */
export function healthProfileName(state: AppState): string {
  const report = state.print.report
  const recipe = state.document?.recipe ?? null
  if (report !== null) {
    return printerName(reportPrinters(recipe?.printer_profile, state.printers), report.profile)
  }
  // Without the document's profile or the default listed, the id stands for its name.
  return (
    (recipe === null ? null : documentPrinter(recipe, state.printers))?.name ??
    recipe?.printer ??
    DEFAULT_PRINTER_ID
  )
}

/** The name of the default profile, which the engine uses for a document without a printer. */
export function defaultPrinterName(printers: readonly PrinterProfile[]): string {
  return printerName(printers, DEFAULT_PRINTER_ID)
}

/**
 * The document's printer as the status bar names it (3.21): the name of the profile it stores or
 * of the listed profile it names, or "{default name} (default)" when it has none or no document is
 * open. The profile the checks use is always named.
 */
export function printerLabel(
  document: Document | null,
  printers: readonly PrinterProfile[],
): string {
  const id = document?.printer ?? null
  if (document === null || id === null) {
    return `${defaultPrinterName(printers)} (default)`
  }
  return document.printer_profile?.name ?? printerName(printers, id)
}

/** The text of the Print toolbar's printer button (3.17): "Printer: {name}" or "Printer: {default name} (default)". */
export function printerButtonText(
  document: Document | null,
  printers: readonly PrinterProfile[],
): string {
  return `Printer: ${printerLabel(document, printers)}`
}

/**
 * The library profile the printer menu's "Edit '{name}'…" opens (3.17): the listed profile the
 * document's printer names, unless it is a built-in profile, which the library never holds.
 */
export function editablePrinter(
  document: Document | null,
  printers: readonly PrinterProfile[],
): PrinterProfile | null {
  const id = document?.printer ?? null
  if (id === null || Object.values(GENERIC_PRINTER_IDS).includes(id)) {
    return null
  }
  return findPrinter(printers, id) ?? null
}

/** The longest id `printerIdFromName` makes before a number is appended (3.17). */
const MAX_NAME_ID_LENGTH = 56

/**
 * A free profile id for a printer name (3.17): the name in lowercase with each run of characters
 * other than a to z and 0 to 9 as one `-`, trimmed of `-`, cut to 56 characters, `printer` when
 * nothing is left, then `-2`, `-3` and so on while the id is in `taken`.
 */
export function printerIdFromName(name: string, taken: readonly string[]): string {
  const base =
    name
      .toLowerCase()
      .replaceAll(/[^a-z0-9]+/g, '-')
      .replaceAll(/^-+|-+$/g, '')
      .slice(0, MAX_NAME_ID_LENGTH) || 'printer'
  let id = base
  for (let number = 2; taken.includes(id); number += 1) {
    id = `${base}-${String(number)}`
  }
  return id
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

/** The number fields of the printer dialog, in its order (3.17). */
export const PRINTER_NUMBER_FIELDS = [
  'width',
  'depth',
  'height',
  'diameter',
  'nozzle',
  'layerHeight',
  'overhang',
  'bridge',
  'press',
  'snug',
  'slip',
  'loose',
] as const

export type PrinterNumberField = (typeof PRINTER_NUMBER_FIELDS)[number]

/** The printer dialog's fields as typed (3.17): lengths in mm and the overhang in degrees. */
export type PrinterForm = {
  readonly name: string
  readonly technology: Technology
  readonly bed: Bed['kind']
} & Readonly<Record<PrinterNumberField, string>>

/** The values a profile gives the number fields: lengths in mm and the overhang in radians. */
function fieldValues(profile: PrinterProfile): Readonly<Record<PrinterNumberField, number | null>> {
  const { bed, fit } = profile
  // A rectangle's width fills Diameter and a round bed's diameter fills Width and Depth. A switch
  // of bed shape starts from the same size.
  const width = bed.kind === 'rectangle' ? bed.width : bed.diameter
  return {
    width,
    depth: bed.kind === 'rectangle' ? bed.depth : bed.diameter,
    height: bed.height,
    diameter: width,
    nozzle: profile.nozzle_diameter ?? null,
    layerHeight: profile.layer_height,
    overhang: profile.max_overhang_angle,
    bridge: profile.max_bridge_length,
    press: fit.press,
    snug: fit.snug,
    slip: fit.slip,
    loose: fit.loose,
  }
}

/** The dialog's fields for `profile`, named `name` (3.17). */
export function printerForm(profile: PrinterProfile, name: string = profile.name): PrinterForm {
  const values = fieldValues(profile)
  const text = (field: PrinterNumberField): string => {
    const value = values[field]
    return value === null
      ? ''
      : formatNumber(value, field === 'overhang' ? 'angle' : 'length', 'mm')
  }
  return {
    name,
    technology: profile.technology,
    bed: profile.bed.kind,
    width: text('width'),
    depth: text('depth'),
    height: text('height'),
    diameter: text('diameter'),
    nozzle: text('nozzle'),
    layerHeight: text('layerHeight'),
    overhang: text('overhang'),
    bridge: text('bridge'),
    press: text('press'),
    snug: text('snug'),
    slip: text('slip'),
    loose: text('loose'),
  }
}

/** The number fields the dialog shows for its bed shape and technology, in its order (3.17). */
export function shownNumberFields(form: PrinterForm): readonly PrinterNumberField[] {
  return PRINTER_NUMBER_FIELDS.filter((field) => {
    switch (field) {
      case 'width':
      case 'depth':
        return form.bed === 'rectangle'
      case 'diameter':
        return form.bed === 'circle'
      case 'nozzle':
        return form.technology === 'fdm'
      default:
        return true
    }
  })
}

/** A plain decimal number, such as `0.4`, `-0.05` or `.5`; not an expression. */
const DECIMAL = /^[+-]?(?:\d+\.?\d*|\.\d+)$/

/** The message key of a number field's text: `field_required` when empty, `field_not_number` for other text that is not a decimal number, else null (3.17). */
export function numberFieldProblem(text: string): 'field_required' | 'field_not_number' | null {
  const trimmed = text.trim()
  if (trimmed === '') {
    return 'field_required'
  }
  return DECIMAL.test(trimmed) ? null : 'field_not_number'
}

/**
 * The profile the dialog's fields describe (3.17), under `id`: lengths as typed in mm, the
 * overhang as degrees × π / 180 and the nozzle for FDM only. A field whose text is still the one
 * `printerForm(opened)` gave keeps `opened`'s exact value: an untouched 45° stays π / 4.
 */
export function printerFromForm(
  form: PrinterForm,
  opened: PrinterProfile,
  id: string,
): PrinterProfile {
  const openedForm = printerForm(opened)
  const openedValues = fieldValues(opened)
  const value = (field: PrinterNumberField): number => {
    const kept = openedValues[field]
    if (form[field] === openedForm[field] && kept !== null) {
      return kept
    }
    const typed = Number(form[field].trim())
    return field === 'overhang' ? (typed * Math.PI) / 180 : typed
  }
  return {
    id,
    name: form.name,
    technology: form.technology,
    bed:
      form.bed === 'rectangle'
        ? {
            kind: 'rectangle',
            width: value('width'),
            depth: value('depth'),
            height: value('height'),
          }
        : { kind: 'circle', diameter: value('diameter'), height: value('height') },
    ...(form.technology === 'fdm' ? { nozzle_diameter: value('nozzle') } : {}),
    layer_height: value('layerHeight'),
    max_overhang_angle: value('overhang'),
    max_bridge_length: value('bridge'),
    fit: { press: value('press'), snug: value('snug'), slip: value('slip'), loose: value('loose') },
  }
}
