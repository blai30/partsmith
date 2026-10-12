import type { CheckKind } from '../generated/protocol'

// The printability checks as the UI presents them (ui.md 3.17, print.md 3.9).

export type PrintCheck = {
  readonly check: CheckKind
  readonly title: string
}

/**
 * Every check in display order: the order of the Checks popover's switches and the tie-break order
 * of the health list. Mesh validity comes last because it cannot be switched off.
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
