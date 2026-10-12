import type { Diagnostic, FeatureRecord, FeatureStatus, FieldHint } from '../generated/protocol'

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

/** The path of the field a pointer maps to, or null when none does. */
export function fieldForPointer(paths: readonly string[], pointer: string): string | null {
  let best: { readonly path: string; readonly length: number } | null = null
  for (const path of paths) {
    const candidate = fieldPointer(path)
    const matches = pointer === candidate || pointer.startsWith(`${candidate}/`)
    if (matches && (best === null || candidate.length > best.length)) {
      best = { path, length: candidate.length }
    }
  }
  return best?.path ?? null
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
  if (repair === undefined) {
    return null
  }
  const selectionFields = fields.filter((field) => (field.selection ?? null) !== null)
  const path = fieldForPointer(
    selectionFields.map((field) => field.path),
    repair.path,
  )
  return selectionFields.find((field) => field.path === path) ?? null
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

/** A JSON pointer's reference tokens (RFC 6901), or null for a pointer that does not start with `/`. */
function pointerTokens(pointer: string): string[] | null {
  if (!pointer.startsWith('/')) {
    return null
  }
  return pointer
    .slice(1)
    .split('/')
    .map((token) => token.replaceAll('~1', '/').replaceAll('~0', '~'))
}

/** A copy of a value with the value at the tokens replaced, or null when the path is not in it. */
function replacedAt(value: unknown, tokens: readonly string[], replacement: unknown): unknown {
  const [token, ...rest] = tokens
  if (token === undefined) {
    return replacement
  }
  if (Array.isArray(value)) {
    const index = /^\d+$/.test(token) ? Number(token) : -1
    if (index < 0 || index >= value.length) {
      return null
    }
    const inner = replacedAt(value[index], rest, replacement)
    return inner === null
      ? null
      : value.map((entry, position) => (position === index ? inner : entry))
  }
  if (typeof value !== 'object' || value === null || (rest.length > 0 && !(token in value))) {
    return null
  }
  const inner = replacedAt(Reflect.get(value, token), rest, replacement)
  return inner === null ? null : { ...value, [token]: inner }
}

/**
 * The record a status message's suggestion makes at rest (4.2): the value at its `field` replaced
 * by its `suggestion`; null when the message has no suggestion or its field is not in the record.
 */
export function suggestedRecord(record: FeatureRecord, message: Diagnostic): FeatureRecord | null {
  const suggestion = message.suggestion ?? null
  const pointer = message.field ?? null
  const tokens = pointer === null ? null : pointerTokens(pointer)
  const [first, ...rest] = tokens ?? []
  // The record's own id and type are not params.
  if (suggestion === null || first === undefined || first === 'id' || first === 'type') {
    return null
  }
  if (rest.length > 0 && !(first in record)) {
    return null
  }
  const inner = replacedAt(record[first], rest, suggestion)
  return inner === null ? null : { ...record, [first]: inner }
}
