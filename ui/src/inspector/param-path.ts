// Paths into feature params (ui.md 2.12): `a.b[1].c` names the value at `a`, then `b`, then its
// item 1, then `c`.

/** One step of a path: a property name or an array index. */
export type PathSegment = string | number

/** Whether a value is a JSON object (not null, not an array). */
export function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** Splits `a.b[1].c` into `['a', 'b', 1, 'c']`; null for paths this model cannot address (`[*]`). */
export function parsePath(path: string): PathSegment[] | null {
  const segments: PathSegment[] = []
  for (const part of path.split('.')) {
    const match = /^([A-Za-z_][A-Za-z0-9_]*)((?:\[\d+\])*)$/.exec(part)
    if (match === null) {
      return null
    }
    segments.push(match[1] ?? '')
    for (const index of (match[2] ?? '').matchAll(/\[(\d+)\]/g)) {
      segments.push(Number(index[1]))
    }
  }
  return segments
}

/** The path of the parent object of a field path (`''` for top-level fields). */
export function parentPath(path: string): string {
  const dot = path.lastIndexOf('.')
  return dot === -1 ? '' : path.slice(0, dot)
}

/** The params with `value` written at `segments`. */
export function writtenParams(
  params: Readonly<Record<string, unknown>>,
  segments: readonly PathSegment[],
  value: unknown,
): Record<string, unknown> {
  const result = written(params, segments, value)
  return isRecord(result) ? { ...result } : {}
}

/** A copy of `container` with `value` written at `segments`, creating objects on the way. */
function written(container: unknown, segments: readonly PathSegment[], value: unknown): unknown {
  const [segment, ...rest] = segments
  if (segment === undefined) {
    return value
  }
  if (typeof segment === 'number') {
    const array = Array.isArray(container) ? [...container] : []
    const item = written(array[segment], rest, value)
    if (item === undefined && rest.length === 0) {
      array.splice(segment, 1)
    } else {
      array[segment] = item
    }
    return array
  }
  const object: Record<string, unknown> = isRecord(container) ? { ...container } : {}
  const property = written(object[segment], rest, value)
  if (property === undefined) {
    Reflect.deleteProperty(object, segment)
  } else {
    object[segment] = property
  }
  return object
}
