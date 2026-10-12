import type { ElementDescriptor, RegionInfo } from '../../generated/protocol'
import type { OriginName, SelectionItem } from '../../state/store'

// Selection items with the keys of ui.md 3.6.

/** A face, edge or vertex from its `pick_reference` descriptor. */
export function elementItem(element: ElementDescriptor, bodyName: string): SelectionItem {
  return {
    key: `element:${element.body}:${element.name}`,
    kind: element.kind,
    label: `${element.kind === 'face' ? 'Face' : element.kind === 'edge' ? 'Edge' : 'Vertex'} of ${bodyName}`,
    body: element.body,
    feature: element.feature,
    element,
    sketchItem: null,
    origin: null,
  }
}

/** A body row. */
export function bodyItem(id: string, name: string): SelectionItem {
  return {
    key: `body:${id}`,
    kind: 'body',
    label: name,
    body: id,
    feature: null,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

/** A timeline item or a sketch row. */
export function featureItem(id: string, label: string): SelectionItem {
  return {
    key: `feature:${id}`,
    kind: 'feature',
    label,
    body: null,
    feature: id,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

/** A closed region of a sketch solution. */
export function regionItem(sketch: string, region: RegionInfo, number = 1): SelectionItem {
  return {
    key: `region:${sketch}:${region.reference.key}`,
    kind: 'sketch_region',
    label: `Region ${String(number)}`,
    body: null,
    feature: sketch,
    element: null,
    sketchItem: { kind: 'region', sketch, region },
    origin: null,
  }
}

/** An origin plane, axis or point row. */
export function originItem(origin: OriginName): SelectionItem {
  const kind = origin === 'O' ? 'origin_point' : origin.length === 2 ? 'origin_plane' : 'origin_axis'
  const label =
    origin === 'O' ? 'Origin' : origin.length === 2 ? `${origin} plane` : `${origin} axis`
  return {
    key: `origin:${origin}`,
    kind,
    label,
    body: null,
    feature: null,
    element: null,
    sketchItem: null,
    origin,
  }
}
