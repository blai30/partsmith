import type { SelectionKind } from '../../generated/protocol'
import { parentPath } from '../../inspector/param-path'
import { selectionValue } from '../../inspector/selection-values'
import type { SelectionItem } from '../../state/store'

// Reading and changing the params values of a feature command (ui.md 3.7).

/** A feature command's params. */
export type Params = Readonly<Record<string, unknown>>

export function siblingPath(path: string, name: string): string {
  const parent = parentPath(path)
  return parent === '' ? name : `${parent}.${name}`
}

export function sameValue(first: unknown, second: unknown): boolean {
  return JSON.stringify(first) === JSON.stringify(second)
}

/** A property of an object value, or undefined. */
export function propertyOf(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null ? Reflect.get(value, key) : undefined
}

/** The value at a path of segments inside a value, or undefined. */
export function valueAt(value: unknown, segments: readonly string[]): unknown {
  let current = value
  for (const segment of segments) {
    if (Array.isArray(current)) {
      current = current[Number(segment)]
    } else {
      current = propertyOf(current, segment)
    }
  }
  return current
}

/** The field's value with the picked item toggled (added, replacing a single value, or removed). */
export function toggledValue(
  accepts: SelectionKind,
  multiple: boolean,
  current: unknown,
  item: SelectionItem,
  currentSketch: unknown,
): unknown {
  const picked = selectionValue(accepts, [item], multiple)
  if (accepts === 'sketch_region') {
    // A region of another sketch replaces the profile (2.12).
    const sketch = propertyOf(current, 'sketch')
    const regions = propertyOf(current, 'regions')
    const pickedRegions = propertyOf(picked, 'regions')
    if (
      sketch !== propertyOf(picked, 'sketch') ||
      !Array.isArray(regions) ||
      !Array.isArray(pickedRegions)
    ) {
      return picked
    }
    const toggled = toggledList(regions, pickedRegions)
    return toggled.length === 0 ? undefined : { sketch, regions: toggled }
  }
  if (accepts === 'sketch_point' && item.sketchItem?.sketch !== currentSketch) {
    // A point of another sketch replaces the list (2.12).
    return picked
  }
  if (multiple) {
    const list = Array.isArray(current) ? current : []
    const additions = Array.isArray(picked) ? picked : [picked]
    const toggled = toggledList(list, additions)
    return toggled.length === 0 ? undefined : toggled
  }
  return sameValue(current, picked) ? undefined : picked
}

function toggledList(list: readonly unknown[], additions: readonly unknown[]): unknown[] {
  let toggled = [...list]
  for (const addition of additions) {
    toggled = toggled.some((entry) => sameValue(entry, addition))
      ? toggled.filter((entry) => !sameValue(entry, addition))
      : [...toggled, addition]
  }
  return toggled
}
