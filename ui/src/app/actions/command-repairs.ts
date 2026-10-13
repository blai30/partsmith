import type { FeatureStatus, SelectionKind } from '../../generated/protocol'
import { getParam } from '../../inspector/model'
import { selectionValue } from '../../inspector/selection-values'
import { fieldForPointer, fieldPointer } from '../../inspector/status'
import type { CatalogFeature, SelectionItem } from '../../state/store'
import { propertyOf, sameValue, valueAt, type Params } from './command-values'

// Picking a replacement for a reference the core could not resolve, in an edited feature's
// selection field (ui.md 3.7, 4.2).

/**
 * A reference of the edited feature the core could not resolve (4.2): the selection field it is in,
 * where in that field's value it sits (`['0']` for the first edge, `['regions', '1']` for a profile's
 * second region) and the stored value, which the next pick in that field replaces.
 */
export type RepairSlot = {
  readonly field: string
  readonly segments: readonly string[]
  readonly lost: unknown
}

/** The lost references of a feature's status as slots of its selection fields (4.2). */
export function repairSlots(
  feature: CatalogFeature,
  params: Params,
  status: FeatureStatus | undefined,
): readonly RepairSlot[] {
  const paths = feature.manifest.fields
    .filter((hint) => (hint.selection ?? null) !== null)
    .map((hint) => hint.path)
  return (status?.repairs ?? []).flatMap((repair): RepairSlot[] => {
    const field = fieldForPointer(paths, repair.path)
    if (field === null) {
      return []
    }
    const segments = repair.path
      .slice(fieldPointer(field).length)
      .split('/')
      .filter((segment) => segment !== '')
    // A whole single value is replaced by any pick already; only an entry of a list needs a slot.
    return segments.length === 0
      ? []
      : [{ field, segments, lost: valueAt(getParam(params, field), segments) }]
  })
}

/** The repair slot of a field whose lost reference is still in its value, or null. */
export function pendingRepair(
  repairs: readonly RepairSlot[],
  field: string,
  value: unknown,
): RepairSlot | null {
  return (
    repairs.find(
      (slot) =>
        slot.field === field &&
        slot.lost !== undefined &&
        sameValue(valueAt(value, slot.segments), slot.lost),
    ) ?? null
  )
}

/**
 * The list a repair slot sits in with its lost entry replaced by `entries` (4.2): entries the list
 * already holds elsewhere are not added twice. Null when the slot's list cannot be found.
 */
export function replacedInList(
  value: unknown,
  slot: RepairSlot,
  entries: readonly unknown[],
): unknown[] | null {
  const list = valueAt(value, slot.segments.slice(0, -1))
  const index = Number(slot.segments.at(-1))
  if (!Array.isArray(list) || !Number.isInteger(index) || index < 0 || index >= list.length) {
    return null
  }
  const others = list.filter((_entry, position) => position !== index)
  const added = entries.filter((entry) => !others.some((other) => sameValue(other, entry)))
  return [...others.slice(0, index), ...added, ...others.slice(index)]
}

/** A value with the list a repair slot sits in replaced. */
function withList(value: unknown, slot: RepairSlot, list: readonly unknown[]): unknown {
  const parents = slot.segments.slice(0, -1)
  if (parents.length === 0) {
    return list
  }
  // The lists of selection values sit one level down at most (a profile's `regions`).
  const [key] = parents
  return typeof value === 'object' && value !== null && key !== undefined
    ? { ...value, [key]: list }
    : value
}

/**
 * The field's value after a pick that replaces a lost reference (3.7, 4.2): the picked item takes
 * the lost entry's place. Null when no lost reference of this field is waiting, or when the pick
 * cannot go in its list (a region of another sketch replaces the whole profile instead).
 */
export function repairedValue(
  repairs: readonly RepairSlot[],
  accepts: SelectionKind,
  multiple: boolean,
  field: string,
  current: unknown,
  item: SelectionItem,
): { readonly value: unknown } | null {
  const slot = pendingRepair(repairs, field, current)
  if (slot === null) {
    return null
  }
  const picked = selectionValue(accepts, [item], multiple)
  if (
    accepts === 'sketch_region' &&
    propertyOf(picked, 'sketch') !== propertyOf(current, 'sketch')
  ) {
    return null
  }
  const entries = valueAt(picked, slot.segments.slice(0, -1))
  if (!Array.isArray(entries)) {
    return null
  }
  const list = replacedInList(current, slot, entries)
  return list === null ? null : { value: withList(current, slot, list) }
}
