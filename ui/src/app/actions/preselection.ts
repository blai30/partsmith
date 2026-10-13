import type { SelectionKind } from '../../generated/protocol'
import {
  buildInspectorModel,
  filledByCommand,
  getParam,
  setParam,
  type InspectorField,
  type InspectorModel,
} from '../../inspector/model'
import { acceptsTarget, selectionValue } from '../../inspector/selection-values'
import { pointSelectionItem } from '../../sketch/hit'
import type { AppState, CatalogFeature, SelectionItem } from '../../state/store'
import { isEmptyValue, parentOf, siblingPath, type Params } from './command-values'
import { sketchFaceBody, sketchRecordOf } from './sketch'

// The selection fields of a feature command: which field takes picks, and how the selection fills
// them when the command opens (ui.md 3.7 steps 4 and 5, 2.12).

/** The model's fields: essential ones, then those under More options. */
export function fieldsOf(model: InspectorModel): readonly InspectorField[] {
  return [...model.essential, ...model.more]
}

/** Selection fields of the model, in manifest order. */
function selectionFields(model: InspectorModel, feature: CatalogFeature): InspectorField[] {
  const fields = fieldsOf(model)
  return feature.manifest.fields.flatMap((hint) => {
    const field = fields.find((candidate) => candidate.path === hint.path)
    return field?.control.type === 'selection' ? [field] : []
  })
}

/** Whether a body field fills itself from the first pick of a face or edge field beside it (2.12). */
function fillsFromSiblingPick(field: InspectorField, fields: readonly InspectorField[]): boolean {
  if (field.control.type !== 'selection' || field.control.accepts !== 'body') {
    return false
  }
  const parent = parentOf(field.path)
  return fields.some(
    (other) =>
      other.control.type === 'selection' &&
      (other.control.accepts === 'face' || other.control.accepts === 'edge') &&
      parentOf(other.path) === parent,
  )
}

/**
 * The first required, empty selection field that waits for a pick, or null: a body field that the
 * first face or edge pick fills, or that the command fills itself (op new), waits for none.
 */
export function firstEmptySelectionField(
  model: InspectorModel,
  feature: CatalogFeature,
  params: Params,
): string | null {
  const fields = selectionFields(model, feature)
  const next = fields.find(
    (field) =>
      field.required &&
      isEmptyValue(field.value) &&
      !fillsFromSiblingPick(field, fields) &&
      !filledByCommand(field, params, feature),
  )
  return next?.path ?? null
}

/**
 * The field that takes picks when a command opens (3.7 step 5): the first required empty
 * selection field, else the first essential multiple selection field, else none.
 */
export function openingActiveField(
  model: InspectorModel,
  feature: CatalogFeature,
  params: Params,
): string | null {
  const empty = firstEmptySelectionField(model, feature, params)
  if (empty !== null) {
    return empty
  }
  const multiple = model.essential.find(
    (field) => field.control.type === 'selection' && field.control.multiple,
  )
  return multiple?.path ?? null
}

/** Whether a field takes the item; a feature row only counts as a sketch when it is one. */
export function takes(state: AppState, accepts: SelectionKind, item: SelectionItem): boolean {
  if (!acceptsTarget(accepts, item)) {
    return false
  }
  if (accepts !== 'sketch' || item.kind !== 'feature') {
    return true
  }
  return sketchRecordOf(state, item.feature) !== null
}

/** The one closed region of a sketch, as an item, when it has exactly one. */
function onlyRegionOf(state: AppState, sketchId: string): SelectionItem | null {
  const regions = state.document?.sketches[sketchId]?.solution.regions ?? []
  const [region] = regions
  if (regions.length !== 1 || region === undefined) {
    return null
  }
  return {
    key: `region:${sketchId}:${region.reference.key}`,
    kind: 'sketch_region',
    label: 'Region 1',
    body: null,
    feature: sketchId,
    element: null,
    sketchItem: { kind: 'region', sketch: sketchId, region },
    origin: null,
  }
}

/**
 * The hole centers a sketch offers (3.3): its non-construction point entities, or, when it has
 * none, the centers of its non-construction circles and arcs.
 */
function sketchPointsOf(state: AppState, sketchId: string): SelectionItem[] {
  const entities = state.document?.sketches[sketchId]?.solution.entities ?? []
  const drawn = entities.filter((entity) => entity.construction !== true)
  const points = drawn.filter((entity) => entity.type === 'point')
  if (points.length > 0) {
    return points.map((entity) => pointSelectionItem(sketchId, entity.id))
  }
  return drawn
    .filter((entity) => entity.type === 'circle' || entity.type === 'arc')
    .map((entity) => pointSelectionItem(sketchId, `${entity.id}.center`))
}

/** What a whole sketch stands for in a region or point field (3.3, 3.7 step 4), or null for other fields. */
function sketchStandIns(
  state: AppState,
  sketchId: string,
  accepts: SelectionKind,
): SelectionItem[] | null {
  if (accepts === 'sketch_region') {
    const region = onlyRegionOf(state, sketchId)
    return region === null ? [] : [region]
  }
  if (accepts === 'sketch_point') {
    return sketchPointsOf(state, sketchId)
  }
  return null
}

/**
 * Fills selection fields from the selection, in manifest order, each item used once (3.7 step 4).
 * A selected sketch, or the sketch just finished when nothing is selected, stands for its region
 * or its points in region and point fields.
 */
export function preselected(
  state: AppState,
  feature: CatalogFeature,
  params: Params,
  items: readonly SelectionItem[],
  finishedSketch: string | null,
): Record<string, unknown> {
  let next: Record<string, unknown> = { ...params }
  const used = new Set<string>()
  const model = buildInspectorModel({ feature, params, errors: {} })
  for (const field of selectionFields(model, feature)) {
    if (field.control.type !== 'selection') {
      continue
    }
    const { accepts, multiple } = field.control
    const candidates = items.flatMap((item) => {
      const sketch = item.kind === 'feature' ? sketchRecordOf(state, item.feature) : null
      const standIns = sketch === null ? null : sketchStandIns(state, sketch.id, accepts)
      return standIns === null
        ? [{ source: item.key, item }]
        : standIns.map((standIn) => ({ source: item.key, item: standIn }))
    })
    if (items.length === 0 && finishedSketch !== null) {
      for (const standIn of sketchStandIns(state, finishedSketch, accepts) ?? []) {
        candidates.push({ source: `feature:${finishedSketch}`, item: standIn })
      }
    }
    const accepted = candidates.filter(
      (candidate) => !used.has(candidate.source) && takes(state, accepts, candidate.item),
    )
    const taken = multiple ? accepted : accepted.slice(0, 1)
    const value = selectionValue(
      accepts,
      taken.map((candidate) => candidate.item),
      multiple,
    )
    if (value === null || isEmptyValue(value)) {
      continue
    }
    for (const candidate of taken) {
      used.add(candidate.source)
    }
    next = setParam(feature, next, field.path, value)
    next = withSiblingBody(state, feature, next, field, taken[0]?.item ?? null)
  }
  return next
}

/**
 * Fills the fields beside a picked element (2.12): an empty body beside a face or edge field from
 * the item's body; the sketch beside a point field from the points' sketch, and the body beside
 * them from that sketch's plane when it lies on a face.
 */
export function withSiblingBody(
  state: AppState,
  feature: CatalogFeature,
  params: Record<string, unknown>,
  field: InspectorField,
  item: SelectionItem | null,
): Record<string, unknown> {
  if (item === null || field.control.type !== 'selection') {
    return params
  }
  const accepts = field.control.accepts
  const hintOf = (path: string) => feature.manifest.fields.find((hint) => hint.path === path)
  const bodyPath = siblingPath(field.path, 'body')
  if (accepts === 'face' || accepts === 'edge') {
    if (
      item.body !== null &&
      hintOf(bodyPath)?.selection === 'body' &&
      isEmptyValue(getParam(params, bodyPath))
    ) {
      return setParam(feature, params, bodyPath, item.body)
    }
    return params
  }
  if (accepts !== 'sketch_point') {
    return params
  }
  const sketch = item.sketchItem?.sketch ?? null
  const sketchPath = siblingPath(field.path, 'sketch')
  if (sketch === null || hintOf(sketchPath)?.selection !== 'sketch') {
    return params
  }
  let next = setParam(feature, params, sketchPath, sketch)
  const planeBody = sketchFaceBody(state, sketch)
  if (planeBody !== null && hintOf(bodyPath)?.selection === 'body') {
    // A hole drills the body its sketch was drawn on.
    next = setParam(feature, next, bodyPath, planeBody)
  }
  return next
}
