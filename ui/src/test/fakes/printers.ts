import type { PrinterProfile, ProfileInput } from '../../generated/protocol'

/** The built-in generic FDM profile exactly as print.md 2.2 lists it. */
export const GENERIC_FDM: PrinterProfile = {
  id: 'generic-fdm',
  name: 'Generic FDM 0.4 mm',
  technology: 'fdm',
  bed: { kind: 'rectangle', width: 256, depth: 256, height: 256 },
  nozzle_diameter: 0.4,
  layer_height: 0.2,
  max_overhang_angle: 0.7853981633974483,
  max_bridge_length: 10,
  fit: { press: 0.05, snug: 0.1, slip: 0.2, loose: 0.4 },
}

/** The built-in generic resin profile exactly as print.md 2.2 lists it. */
export const GENERIC_RESIN: PrinterProfile = {
  id: 'generic-resin',
  name: 'Generic resin MSLA',
  technology: 'resin',
  bed: { kind: 'rectangle', width: 130, depth: 80, height: 160 },
  layer_height: 0.05,
  max_overhang_angle: 0.5235987755982988,
  max_bridge_length: 5,
  fit: { press: 0.02, snug: 0.05, slip: 0.1, loose: 0.2 },
}

/** The `list_printer_profiles` result of the test core: built-in profiles sorted by id. */
export const TEST_PRINTERS: readonly PrinterProfile[] = [GENERIC_FDM, GENERIC_RESIN]

/** The written form of file-format.md's E11 profile, "My MK4" (ui.md 6.4, before U-252). */
export const MK4: PrinterProfile = {
  id: 'my-mk4',
  name: 'My MK4',
  technology: 'fdm',
  bed: { kind: 'rectangle', width: 250, depth: 210, height: 220 },
  nozzle_diameter: 0.4,
  layer_height: 0.15,
  max_overhang_angle: 0.7853981633974483,
  max_bridge_length: 10,
  fit: { press: 0.05, snug: 0.1, slip: 0.15, loose: 0.3 },
}

/** `MK4` with other fit classes: the library's copy that differs from a document's (U-258). */
export const MK4B: PrinterProfile = {
  ...MK4,
  fit: { press: 0.05, snug: 0.1, slip: 0.3, loose: 0.4 },
}

/** The generic resin profile's written form under the id `my-mars` and the name "My Mars" (U-254). */
export const MY_MARS: PrinterProfile = { ...GENERIC_RESIN, id: 'my-mars', name: 'My Mars' }

/** The `unknown_printer_profile` warning of engine.md 3.4 for a printer id. */
export function unknownPrinterMessage(id: string): string {
  return `Printer profile "${id}" is not among your printers. Printer values and checks use the default FDM profile until you add that printer or pick another one.`
}

/**
 * The written form of a profile in print.md's input form, as the core lists and returns it: each
 * field the input leaves out comes from its technology's generic profile, and resin has no nozzle
 * (print.md 3.1).
 */
export function writtenProfile(input: ProfileInput): PrinterProfile {
  const generic = input.technology === 'resin' ? GENERIC_RESIN : GENERIC_FDM
  return {
    id: input.id,
    name: input.name,
    technology: input.technology,
    bed: input.bed ?? generic.bed,
    ...(input.technology === 'resin'
      ? {}
      : { nozzle_diameter: input.nozzle_diameter ?? generic.nozzle_diameter ?? null }),
    layer_height: input.layer_height ?? generic.layer_height,
    max_overhang_angle: input.max_overhang_angle ?? generic.max_overhang_angle,
    max_bridge_length: input.max_bridge_length ?? generic.max_bridge_length,
    fit: {
      press: input.fit?.press ?? generic.fit.press,
      snug: input.fit?.snug ?? generic.fit.snug,
      slip: input.fit?.slip ?? generic.fit.slip,
      loose: input.fit?.loose ?? generic.fit.loose,
    },
  }
}
