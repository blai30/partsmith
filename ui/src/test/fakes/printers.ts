import type { PrinterProfile } from '../../generated/protocol'

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
