import type { ElementDescriptor, RegionInfo } from '../../generated/protocol'
import type { OriginName, SelectionItem } from '../../state/store'

// Selection items with the keys of ui.md 3.6, written out apart from the app's own builders so
// tests check them against an independent copy.

const NO_ITEM_DATA = {
  body: null,
  feature: null,
  element: null,
  sketchItem: null,
  origin: null,
} as const

/** A face, edge or vertex from its `pick_reference` descriptor, on the body named `bodyName`. */
export function elementItem(element: ElementDescriptor, bodyName = 'Cube'): SelectionItem {
  const noun = element.kind === 'face' ? 'Face' : element.kind === 'edge' ? 'Edge' : 'Vertex'
  return {
    ...NO_ITEM_DATA,
    key: `element:${element.body}:${element.name}`,
    kind: element.kind,
    label: `${noun} of ${bodyName}`,
    body: element.body,
    feature: element.feature,
    element,
  }
}

/** A body row. */
export function bodyItem(id: string, name: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `body:${id}`, kind: 'body', label: name, body: id }
}

/** A timeline item or a sketch row. */
export function featureItem(id: string, label: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `feature:${id}`, kind: 'feature', label, feature: id }
}

/** A datum plane row. */
export function datumItem(id: string, label: string): SelectionItem {
  return { ...featureItem(id, label), kind: 'datum_plane' }
}

/** A parameter row. */
export function parameterItem(id: string, name: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: `parameter:${id}`, kind: 'parameter', label: name }
}

/** A closed region of a sketch solution. */
export function regionItem(sketch: string, region: RegionInfo, label = 'Region 1'): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `region:${sketch}:${region.reference.key}`,
    kind: 'sketch_region',
    label,
    feature: sketch,
    sketchItem: { kind: 'region', sketch, region },
  }
}

/** A line of a sketch. */
export function sketchLineItem(sketch: string, line: string): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `sketch:${sketch}:${line}`,
    kind: 'sketch_curve',
    label: `Line ${line}`,
    feature: sketch,
    sketchItem: { kind: 'entity', sketch, entity: line, entityType: 'line' },
  }
}

/** A point of a sketch. */
export function sketchPointItem(sketch: string, point: string): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `point:${sketch}:${point}`,
    kind: 'sketch_point',
    label: `Point ${point}`,
    feature: sketch,
    sketchItem: { kind: 'point', sketch, point },
  }
}

/** Origin names by view and coordinates (ui.md 3.10). */
const ORIGIN_LABELS: Readonly<Record<OriginName, string>> = {
  XY: 'Top plane (XY)',
  XZ: 'Front plane (XZ)',
  YZ: 'Right plane (YZ)',
  X: 'X axis',
  Y: 'Y axis',
  Z: 'Z axis',
  O: 'Origin',
}

/** An origin plane, axis or point row. */
export function originItem(origin: OriginName): SelectionItem {
  const kind =
    origin === 'O'
      ? 'origin_point'
      : origin === 'XY' || origin === 'XZ' || origin === 'YZ'
        ? 'origin_plane'
        : 'origin_axis'
  return { ...NO_ITEM_DATA, key: `origin:${origin}`, kind, label: ORIGIN_LABELS[origin], origin }
}
