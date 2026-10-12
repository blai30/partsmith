import { scalePlainNumber } from '../expression/step'

// Starting values that work on printed parts (ui.md 2.12): explicit units, so a first fillet,
// chamfer or shell on a 20 to 60 mm part previews instead of failing, and the variants of tagged
// unions a new feature starts with.

/** Start values by feature type and field path: expression texts, and variant names for tag paths. */
export const START_VALUES: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  fillet: { radius: '1 mm' },
  chamfer: { 'distance.expr': '1 mm', 'distance.second_expr': '1 mm' },
  shell: { thickness: '2 mm' },
  hole: {
    diameter: '3.2 mm',
    'extent.kind': 'through_all',
    'hole_type.depth': '3 mm',
    'hole_type.angle': '90 deg',
  },
  revolve: { 'extent.kind': 'full' },
  datum_plane: { offset: '10 mm' },
}

/** The head of a counterbore or countersink starts at twice the hole's top-level Diameter (2.12). */
const HEAD_DIAMETER = { type: 'hole', path: 'hole_type.diameter', source: 'diameter' } as const

/**
 * The start value at a path: the table's entry, or for a hole's head diameter twice Diameter (the
 * doubled number with the same unit word and spacing for a plain number, else `2 * ({text})`).
 */
export function startValue(
  type: string,
  path: string,
  params: Readonly<Record<string, unknown>>,
): string | undefined {
  if (type === HEAD_DIAMETER.type && path === HEAD_DIAMETER.path) {
    const diameter = params[HEAD_DIAMETER.source]
    if (typeof diameter === 'string' && diameter.trim() !== '') {
      return scalePlainNumber(diameter, 2) ?? `2 * (${diameter})`
    }
  }
  return START_VALUES[type]?.[path]
}
