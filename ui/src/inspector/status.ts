import type { Diagnostic } from '../generated/protocol'

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
