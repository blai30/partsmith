import { isNumberWithoutUnit } from '../expression/step'
import type { Diagnostic, FeatureStatus, FieldHint, Units } from '../generated/protocol'

// Mapping a feature status message to the inspector field it is about (ui.md 3.7): a hint path
// `a.b[1]` is the JSON pointer `/a/b/1`; a message's `field` maps to the field whose pointer equals
// it, else to the field whose pointer is its longest whole-segment prefix.

/** The JSON pointer of a hint path: `extent.expr` is `/extent/expr`, `edges[1]` is `/edges/1`. */
export function fieldPointer(path: string): string {
  const segments = path
    .replaceAll(/\[(\d+)\]/g, '.$1')
    .split('.')
    .filter((segment) => segment !== '')
  return `/${segments.join('/')}`
}

/** Whether a JSON pointer is `container` or lies inside it, by whole segments (`/edges/1` in `/edges`). */
export function pointsInto(pointer: string, container: string): boolean {
  return pointer === container || pointer.startsWith(`${container}/`)
}

/** The path of the field a pointer maps to, or null when none does. */
export function fieldForPointer(paths: readonly string[], pointer: string): string | null {
  let best: { readonly path: string; readonly length: number } | null = null
  for (const path of paths) {
    const candidate = fieldPointer(path)
    if (pointsInto(pointer, candidate) && (best === null || candidate.length > best.length)) {
      best = { path, length: candidate.length }
    }
  }
  return best?.path ?? null
}

/** The selection field a JSON pointer maps to (3.7), such as `/edges/0` to Edges, or null. */
export function selectionFieldFor(fields: readonly FieldHint[], pointer: string): FieldHint | null {
  const selectionFields = fields.filter((field) => (field.selection ?? null) !== null)
  const path = fieldForPointer(
    selectionFields.map((field) => field.path),
    pointer,
  )
  return selectionFields.find((field) => field.path === path) ?? null
}

/**
 * The selection field the first repair of a feature's status maps to (4.2): the field to pick the
 * lost reference again in. Null without repairs or when the repair maps to no selection field.
 */
export function repairField(
  fields: readonly FieldHint[],
  status: FeatureStatus | undefined,
): FieldHint | null {
  const [repair] = status?.repairs ?? []
  return repair === undefined ? null : selectionFieldFor(fields, repair.path)
}

/** The first message for each field a status message maps to. */
export function messagesByField(
  paths: readonly string[],
  messages: readonly Diagnostic[],
): ReadonlyMap<string, Diagnostic> {
  const byField = new Map<string, Diagnostic>()
  for (const message of messages) {
    const pointer = message.field ?? null
    const path = pointer === null ? null : fieldForPointer(paths, pointer)
    if (path !== null && !byField.has(path)) {
      byField.set(path, message)
    }
  }
  return byField
}

/** The codes whose `suggestion` is the largest value the core's search found to work (features.md 4.3). */
const LIMIT_CODES: ReadonlySet<string> = new Set([
  'fillet_too_large',
  'chamfer_too_large',
  'shell_too_thick',
])

/**
 * The largest value that works for a field (3.7, limits under fields): the `suggestion` of the
 * "too large" message the field shows, or null. In a document whose units are not mm, a number
 * without a unit gets none: there the suggestion is that number in mm, which keeps the size the
 * user meant (features.md 4.4), not a limit.
 */
export function fieldLimit(
  message: Diagnostic | undefined,
  text: unknown,
  units: Units,
): string | null {
  if (message === undefined || !LIMIT_CODES.has(message.code)) {
    return null
  }
  const suggestion = message.suggestion ?? null
  if (suggestion === null) {
    return null
  }
  const keepsSize = units !== 'mm' && typeof text === 'string' && isNumberWithoutUnit(text)
  return keepsSize ? null : suggestion
}
