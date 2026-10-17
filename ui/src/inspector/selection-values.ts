import { featureLabel, originPlaneName, type OriginPlane } from '../core/labels'
import type { ElementKind, Reference, RegionInfo, SelectionKind } from '../generated/protocol'
import { regionSelectionItem } from '../sketch/hit'
import type { AppState, OriginName, SelectionItem } from '../state/store'
import { isRecord } from './param-path'

// Param values of selection fields from selection items, and back (ui.md 2.12, features.md 2.5).

const ROUND_SURFACES: ReadonlySet<string> = new Set(['cylinder', 'cone', 'sphere', 'torus'])
const ORIGIN_PLANES: ReadonlySet<string> = new Set(['XY', 'XZ', 'YZ'])
const ORIGIN_AXES: ReadonlySet<string> = new Set(['X', 'Y', 'Z'])
const ELEMENT_NOUNS: Readonly<Record<ElementKind, string>> = {
  face: 'Face',
  edge: 'Edge',
  vertex: 'Vertex',
}

function isReference(value: unknown): value is Reference {
  return isRecord(value) && typeof value['q'] === 'string' && typeof value['fp'] === 'string'
}

function isElement(item: SelectionItem): boolean {
  return item.kind === 'face' || item.kind === 'edge' || item.kind === 'vertex'
}

function isSketchLine(item: SelectionItem): boolean {
  return (
    item.kind === 'sketch_curve' &&
    item.sketchItem?.kind === 'entity' &&
    item.sketchItem.entityType === 'line'
  )
}

/** The sketch a sketch item or sketch feature belongs to. */
function sketchOf(item: SelectionItem): string | null {
  return item.sketchItem?.sketch ?? (item.kind === 'feature' ? item.feature : null)
}

/** Whether a selection field accepts the item. */
export function acceptsTarget(accepts: SelectionKind, item: SelectionItem): boolean {
  switch (accepts) {
    case 'body':
      return (item.kind === 'body' || isElement(item)) && item.body !== null
    case 'sketch':
      return (
        (item.kind === 'feature' && item.feature !== null) ||
        ((item.kind === 'sketch_curve' || item.kind === 'sketch_region') &&
          item.sketchItem !== null)
      )
    case 'sketch_region':
      return item.kind === 'sketch_region' && item.sketchItem?.kind === 'region'
    case 'sketch_point':
      return item.kind === 'sketch_point' && item.sketchItem?.kind === 'point'
    case 'face':
      return item.kind === 'face' && item.element !== null
    case 'edge':
      return item.kind === 'edge' && item.element !== null
    case 'plane':
      return (
        (item.kind === 'origin_plane' && ORIGIN_PLANES.has(item.origin ?? '')) ||
        (item.kind === 'datum_plane' && item.feature !== null) ||
        (item.kind === 'face' && item.element?.surface === 'plane')
      )
    case 'axis':
      return (
        (item.kind === 'origin_axis' && ORIGIN_AXES.has(item.origin ?? '')) ||
        isSketchLine(item) ||
        (item.kind === 'edge' && item.element?.curve === 'line') ||
        (item.kind === 'face' && ROUND_SURFACES.has(item.element?.surface ?? ''))
      )
  }
}

/** The items that belong to the sketch of the last one, in pick order: another sketch replaces the list. */
function ofOneSketch(items: readonly SelectionItem[]): readonly SelectionItem[] {
  let kept: SelectionItem[] = []
  let sketch: string | null = null
  for (const item of items) {
    const itemSketch = sketchOf(item)
    if (itemSketch !== sketch) {
      kept = []
      sketch = itemSketch
    }
    kept.push(item)
  }
  return kept
}

function planeValue(item: SelectionItem): unknown {
  if (item.kind === 'origin_plane') {
    return { kind: 'origin', plane: item.origin }
  }
  if (item.kind === 'datum_plane') {
    return { kind: 'datum', feature: item.feature }
  }
  return { kind: 'face', body: item.body, face: item.element?.reference }
}

/**
 * An axis value; a straight edge or a round face keeps the side it was picked on as `direction`,
 * its descriptor's `axis_direction` in body coordinates, left out when the descriptor has none
 * (features.md 2.5).
 */
function axisValue(item: SelectionItem): unknown {
  if (item.kind === 'origin_axis') {
    return { kind: 'origin', axis: item.origin }
  }
  if (item.sketchItem?.kind === 'entity') {
    return { kind: 'sketch_line', sketch: item.sketchItem.sketch, line: item.sketchItem.entity }
  }
  const direction = item.element?.axis_direction ?? null
  const side = direction === null ? {} : { direction }
  if (item.kind === 'edge') {
    return { kind: 'edge', body: item.body, edge: item.element?.reference, ...side }
  }
  return { kind: 'face', body: item.body, face: item.element?.reference, ...side }
}

/** The param value for the items, or null when they cannot form one. */
export function selectionValue(
  accepts: SelectionKind,
  items: readonly SelectionItem[],
  multiple: boolean,
): unknown {
  const accepted = items.filter((item) => acceptsTarget(accepts, item))
  const [first] = accepted
  if (first === undefined) {
    return null
  }
  switch (accepts) {
    case 'body': {
      const ids = [...new Set(accepted.flatMap((item) => (item.body === null ? [] : [item.body])))]
      return multiple ? ids : (ids[0] ?? null)
    }
    case 'sketch':
      return sketchOf(first)
    case 'sketch_region': {
      const regions = ofOneSketch(accepted)
      const [region] = regions
      return region === undefined
        ? null
        : {
            sketch: sketchOf(region),
            regions: regions.flatMap((item) =>
              item.sketchItem?.kind === 'region' ? [item.sketchItem.region.reference] : [],
            ),
          }
    }
    case 'sketch_point':
      return ofOneSketch(accepted).flatMap((item) =>
        item.sketchItem?.kind === 'point' ? [item.sketchItem.point] : [],
      )
    case 'face':
    case 'edge': {
      const references = accepted.flatMap((item) =>
        item.element === null ? [] : [item.element.reference],
      )
      return multiple ? references : (references[0] ?? null)
    }
    case 'plane':
      return planeValue(first)
    case 'axis':
      return axisValue(first)
  }
}

const NO_ITEM_DATA = {
  body: null,
  feature: null,
  element: null,
  sketchItem: null,
  origin: null,
} as const

/** The selection key of a body (3.6). */
export function bodyKey(id: string): string {
  return `body:${id}`
}

/** The selection key of a feature, such as a sketch or a datum plane (3.6). */
export function featureKey(id: string): string {
  return `feature:${id}`
}

/** The selection key of a parameter (3.6). */
export function parameterKey(id: string): string {
  return `parameter:${id}`
}

/** A body, feature or parameter selection key split into its kind and id. */
export type ParsedSelectionKey = {
  readonly kind: 'body' | 'feature' | 'parameter'
  readonly id: string
}

/** The kind and id of a body, feature or parameter key; null for any other key. */
export function parseSelectionKey(key: string): ParsedSelectionKey | null {
  const colon = key.indexOf(':')
  const kind = key.slice(0, colon)
  if (colon === -1 || (kind !== 'body' && kind !== 'feature' && kind !== 'parameter')) {
    return null
  }
  return { kind, id: key.slice(colon + 1) }
}

/** The selection item of a body. */
export function bodySelectionItem(id: string, label: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: bodyKey(id), kind: 'body', label, body: id }
}

/** The selection item of a feature: a sketch or another feature, or a datum plane. */
export function featureSelectionItem(
  id: string,
  label: string,
  kind: 'feature' | 'datum_plane' = 'feature',
): SelectionItem {
  return { ...NO_ITEM_DATA, key: featureKey(id), kind, label, feature: id }
}

/** The selection item of a parameter. */
export function parameterSelectionItem(id: string, label: string): SelectionItem {
  return { ...NO_ITEM_DATA, key: parameterKey(id), kind: 'parameter', label }
}

function isOriginPlane(origin: OriginName): origin is OriginPlane {
  return ORIGIN_PLANES.has(origin)
}

/**
 * The selection item of an origin plane, axis or point; the one place that names them, so browser
 * rows, selection titles, field boxes and Choose menus agree (planes by `originPlaneName`, 3.10).
 */
export function originItem(origin: OriginName): SelectionItem {
  if (isOriginPlane(origin)) {
    return {
      ...NO_ITEM_DATA,
      key: `origin:${origin}`,
      kind: 'origin_plane',
      label: originPlaneName(origin),
      origin,
    }
  }
  const axis = ORIGIN_AXES.has(origin)
  return {
    ...NO_ITEM_DATA,
    key: `origin:${origin}`,
    kind: axis ? 'origin_axis' : 'origin_point',
    label: axis ? `${origin} axis` : 'Origin',
    origin,
  }
}

function bodyName(state: AppState, bodyId: string): string {
  return state.document?.recipe.bodies?.find((body) => body.id === bodyId)?.name ?? bodyId
}

function featureName(state: AppState, featureId: string): string {
  return state.document === null
    ? featureId
    : featureLabel(state.document.recipe, featureId, state.catalog)
}

/** A face or edge item for a stored reference; its element name is the reference's query. */
function referenceItem(
  state: AppState,
  kind: ElementKind,
  bodyId: string,
  reference: Reference,
): SelectionItem {
  return {
    ...NO_ITEM_DATA,
    key: `element:${bodyId}:${reference.q}`,
    kind,
    label: `${ELEMENT_NOUNS[kind]} of ${bodyName(state, bodyId)}`,
    body: bodyId,
  }
}

function regionItems(state: AppState, value: unknown): readonly SelectionItem[] {
  if (!isRecord(value) || typeof value['sketch'] !== 'string' || !Array.isArray(value['regions'])) {
    return []
  }
  const sketch = value['sketch']
  const solved = state.document?.sketches[sketch]?.solution.regions ?? []
  return value['regions'].flatMap((reference: unknown) => {
    if (
      !isRecord(reference) ||
      typeof reference['key'] !== 'string' ||
      typeof reference['fp'] !== 'string'
    ) {
      return []
    }
    const index = solved.findIndex((region) => region.reference.key === reference['key'])
    const region: RegionInfo = solved[index] ?? {
      reference: { key: reference['key'], fp: reference['fp'] },
      area: 0,
      centroid: [0, 0],
    }
    return [regionSelectionItem(sketch, region, `Region ${String(index === -1 ? 1 : index + 1)}`)]
  })
}

function planeItems(
  state: AppState,
  value: Readonly<Record<string, unknown>>,
): readonly SelectionItem[] {
  const kind = value['kind']
  if (
    kind === 'origin' &&
    typeof value['plane'] === 'string' &&
    ORIGIN_PLANES.has(value['plane'])
  ) {
    return [originItem(value['plane'] as OriginName)]
  }
  if (kind === 'datum' && typeof value['feature'] === 'string') {
    const feature = value['feature']
    return [featureSelectionItem(feature, featureName(state, feature), 'datum_plane')]
  }
  if (kind === 'face' && typeof value['body'] === 'string' && isReference(value['face'])) {
    return [referenceItem(state, 'face', value['body'], value['face'])]
  }
  return []
}

function axisItems(
  state: AppState,
  value: Readonly<Record<string, unknown>>,
): readonly SelectionItem[] {
  const kind = value['kind']
  if (kind === 'origin' && typeof value['axis'] === 'string' && ORIGIN_AXES.has(value['axis'])) {
    return [originItem(value['axis'] as OriginName)]
  }
  if (
    kind === 'sketch_line' &&
    typeof value['sketch'] === 'string' &&
    typeof value['line'] === 'string'
  ) {
    const sketch = value['sketch']
    const line = value['line']
    return [
      {
        ...NO_ITEM_DATA,
        key: `sketch:${sketch}:${line}`,
        kind: 'sketch_curve',
        label: `Line ${line}`,
        feature: sketch,
        sketchItem: { kind: 'entity', sketch, entity: line, entityType: 'line' },
      },
    ]
  }
  if (typeof value['body'] !== 'string') {
    return []
  }
  if (kind === 'edge' && isReference(value['edge'])) {
    return [referenceItem(state, 'edge', value['body'], value['edge'])]
  }
  if (kind === 'face' && isReference(value['face'])) {
    return [referenceItem(state, 'face', value['body'], value['face'])]
  }
  return []
}

/**
 * Selection items for an existing param value, used when editing (labels from the document).
 * `siblings` is the object that holds the field, whose `body` or `sketch` qualifies face, edge and
 * point values.
 */
export function itemsFromValue(
  accepts: SelectionKind,
  value: unknown,
  state: AppState,
  siblings: Readonly<Record<string, unknown>> = {},
): readonly SelectionItem[] {
  const values: readonly unknown[] = Array.isArray(value)
    ? value
    : value === undefined || value === null
      ? []
      : [value]
  switch (accepts) {
    case 'body':
      return values.flatMap((id) =>
        typeof id === 'string' ? [bodySelectionItem(id, bodyName(state, id))] : [],
      )
    case 'sketch':
      return values.flatMap((id) =>
        typeof id === 'string' ? [featureSelectionItem(id, featureName(state, id))] : [],
      )
    case 'sketch_region':
      return regionItems(state, value)
    case 'sketch_point': {
      const sketch = siblings['sketch']
      if (typeof sketch !== 'string') {
        return []
      }
      return values.flatMap((point) =>
        typeof point === 'string'
          ? [
              {
                ...NO_ITEM_DATA,
                key: `point:${sketch}:${point}`,
                kind: 'sketch_point' as const,
                label: `Point ${point}`,
                feature: sketch,
                sketchItem: { kind: 'point' as const, sketch, point },
              },
            ]
          : [],
      )
    }
    case 'face':
    case 'edge': {
      const body = siblings['body']
      if (typeof body !== 'string') {
        return []
      }
      return values.flatMap((reference) =>
        isReference(reference) ? [referenceItem(state, accepts, body, reference)] : [],
      )
    }
    case 'plane':
      return isRecord(value) ? planeItems(state, value) : []
    case 'axis':
      return isRecord(value) ? axisItems(state, value) : []
  }
}
