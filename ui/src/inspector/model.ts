import type {
  Body,
  Document,
  FeatureStatus,
  FieldHint,
  FieldUnit,
  SelectionKind,
} from '../generated/protocol'
import type { ActiveCommand, AppState, CatalogFeature } from '../state/store'
import { fieldHelp } from './help'
import { isRecord, parentPath, parsePath, writtenParams } from './param-path'
import {
  isExpression,
  locate,
  propertiesOf,
  requiredOf,
  resolved,
  schemaOrNull,
  tagOf,
  typesOf,
  unwrapped,
  variantsOf,
  type JsonSchema,
  type Located,
} from './schema'
import { startValue } from './start-values'

// The schema-driven inspector model (ui.md 2.12): manifest field hints mapped onto the params JSON
// Schema (draft 2020-12 as schemars 1 emits it) to controls, defaults and param updates.

export type { JsonSchema } from './schema'

export type FieldControl =
  | { readonly type: 'expression'; readonly unit: FieldUnit }
  | {
      readonly type: 'selection'
      readonly accepts: SelectionKind
      readonly multiple: boolean
      readonly minimum: number
    }
  | {
      readonly type: 'choice'
      readonly options: readonly { value: string; label: string }[]
      readonly presentation: 'segmented' | 'select'
    }
  | { readonly type: 'switch' }
  | { readonly type: 'toggle' }
  | { readonly type: 'text' }
  | { readonly type: 'number'; readonly integer: boolean }
  | { readonly type: 'unsupported' }

export type InspectorField = {
  readonly path: string
  readonly label: string
  readonly help: string | null
  readonly essential: boolean
  readonly required: boolean
  readonly control: FieldControl
  readonly value: unknown
  readonly error: string | null
  /** For an inline toggle, the path of the field whose row it sits on; otherwise null. */
  readonly attachedTo: string | null
}

export type InspectorModel = {
  readonly essential: readonly InspectorField[]
  readonly more: readonly InspectorField[]
  /**
   * True when a field in `more` has an error (including a mapped status message, 3.7), or a
   * required field in `more` is empty and not waiting for a pick; it forces the More section open
   * (2.12).
   */
  readonly forceMore: boolean
  /** Labels of shown More fields whose value differs from its starting value, in display order; `body`, `sketch` and `target` fields never count. */
  readonly changedMore: readonly string[]
  /** True when every required field has a value and no field has an error. */
  readonly complete: boolean
}

/** The active feature command with its catalog feature and inspector model. */
export type ActiveCommandModel = {
  readonly command: ActiveCommand
  readonly feature: CatalogFeature
  readonly model: InspectorModel
}

export type DefaultsContext = {
  readonly bodies: readonly Body[]
  readonly bodiesWithGeometry: readonly string[]
  readonly lastTargetBody: string | null
  /** The recipe, for the reuse rule; null skips it. */
  readonly document: Document | null
  /** Feature statuses by id, so the reuse rule skips records in error. */
  readonly featureStatus: Readonly<Record<string, FeatureStatus>>
}

/** Choices shown as a segmented control; more become a select. */
const MAXIMUM_SEGMENTS = 4

/** The starting text of an expression field without a schema default, by unit. */
const EXPRESSION_DEFAULTS: Readonly<Record<FieldUnit, string>> = {
  length: '10 mm',
  angle: '90 deg',
  scalar: '1',
  count: '2',
}

type Params = Readonly<Record<string, unknown>>

function isChoiceSchema(located: Located, root: JsonSchema): boolean {
  if (located.tag || Array.isArray(located.schema['enum'])) {
    return true
  }
  const choices = located.schema['oneOf']
  return (
    Array.isArray(choices) &&
    choices.length > 0 &&
    choices.every(
      (choice) =>
        typeof schemaOrNull(resolved(schemaOrNull(choice) ?? {}, root))?.['const'] === 'string',
    )
  )
}

function controlFor(hint: FieldHint, located: Located, root: JsonSchema): FieldControl {
  const unit = hint.unit ?? null
  if (unit !== null && isExpression(located.written, root)) {
    return { type: 'expression', unit }
  }
  const selection = hint.selection ?? null
  if (selection !== null) {
    return {
      type: 'selection',
      accepts: selection,
      multiple: typesOf(located.schema).includes('array') || selection === 'sketch_region',
      minimum: located.required ? 1 : 0,
    }
  }
  const options = hint.options ?? []
  if (options.length > 0 && isChoiceSchema(located, root)) {
    return {
      type: 'choice',
      options: options.map((option) => ({ value: option.value, label: option.label })),
      presentation: options.length <= MAXIMUM_SEGMENTS ? 'segmented' : 'select',
    }
  }
  const types = typesOf(located.schema)
  if (types.includes('boolean')) {
    return { type: 'switch' }
  }
  if (types.includes('string')) {
    return { type: 'text' }
  }
  if (types.includes('integer')) {
    return { type: 'number', integer: true }
  }
  if (types.includes('number')) {
    return { type: 'number', integer: false }
  }
  return { type: 'unsupported' }
}

/** The value at a dotted and indexed path such as `extent.expr` or `size[1]`, or undefined. */
export function getParam(params: Params, path: string): unknown {
  const segments = parsePath(path)
  if (segments === null) {
    return undefined
  }
  let value: unknown = params
  for (const segment of segments) {
    if (typeof segment === 'number') {
      value = Array.isArray(value) ? value[segment] : undefined
    } else {
      value = isRecord(value) ? value[segment] : undefined
    }
  }
  return value
}

/** The params object that holds a field: the top level, or its parent object (empty when that is no object). */
export function siblingsOf(params: Params, path: string): Params {
  const parent = parentPath(path)
  if (parent === '') {
    return params
  }
  const value = getParam(params, parent)
  return isRecord(value) ? value : {}
}

/** Whether a field value is missing: nothing, empty text, an empty list, or a profile without regions. */
export function isEmptyValue(value: unknown): boolean {
  if (value === undefined || value === null || value === '') {
    return true
  }
  if (Array.isArray(value)) {
    return value.length === 0
  }
  return (
    typeof value === 'object' &&
    Array.isArray(Reflect.get(value, 'regions')) &&
    Reflect.get(value, 'regions').length === 0
  )
}

function hasValue(control: FieldControl, value: unknown): boolean {
  if (value === undefined || value === null) {
    return false
  }
  switch (control.type) {
    case 'selection':
      if (Array.isArray(value)) {
        return value.length >= Math.max(1, control.minimum)
      }
      if (isRecord(value) && Array.isArray(value['regions'])) {
        return value['regions'].length > 0
      }
      return value !== ''
    case 'expression':
    case 'text':
      return typeof value === 'string' && value.trim() !== ''
    case 'number':
      return typeof value === 'number' && Number.isFinite(value)
    default:
      return true
  }
}

/**
 * How a feature creates a body through its `target` field (2.12): `op` when its op options include
 * `new` (Mirror, Linear pattern, Circular pattern: the target is shown only while op is new), and
 * `always` when it has no op and an optional target (Split body: it starts set to a new body).
 * Null for every other feature, whose `body` field receives the body op `new` creates.
 */
export function bodyCreatingTarget(feature: CatalogFeature): 'op' | 'always' | null {
  const hints = feature.manifest.fields
  const target = hints.find((hint) => hint.path === 'target')
  if (target?.selection !== 'body') {
    return null
  }
  const op = hints.find((hint) => hint.path === 'op')
  if (op === undefined) {
    return requiredOf(feature.paramsSchema).includes('target') ? null : 'always'
  }
  return op.options?.some((option) => option.value === 'new') === true ? 'op' : null
}

/** A body field the command fills itself: an empty body beside op `new` gets a new body (3.7). */
export function filledByCommand(
  field: InspectorField,
  params: Params,
  feature: CatalogFeature,
): boolean {
  if (
    field.control.type !== 'selection' ||
    field.control.accepts !== 'body' ||
    bodyCreatingTarget(feature) !== null
  ) {
    return false
  }
  const parent = parentPath(field.path)
  return getParam(params, parent === '' ? 'op' : `${parent}.op`) === 'new'
}

const PICKED_ELEMENT_KINDS: ReadonlySet<string> = new Set(['face', 'edge', 'sketch_point'])

/**
 * A `body` or `sketch` field that the first pick of an empty sibling `face`, `edge` or
 * `sketch_point` field fills (2.12).
 */
function waitsForPick(
  field: InspectorField,
  fields: readonly InspectorField[],
  params: Params,
): boolean {
  if (
    field.control.type !== 'selection' ||
    (field.control.accepts !== 'body' && field.control.accepts !== 'sketch')
  ) {
    return false
  }
  const parent = parentPath(field.path)
  return fields.some(
    (sibling) =>
      sibling.control.type === 'selection' &&
      PICKED_ELEMENT_KINDS.has(sibling.control.accepts) &&
      parentPath(sibling.path) === parent &&
      !hasValue(sibling.control, getParam(params, sibling.path)),
  )
}

/** A required field without a value that nothing else fills. */
function missing(field: InspectorField, params: Params, feature: CatalogFeature): boolean {
  return (
    field.required &&
    !hasValue(field.control, field.value) &&
    !filledByCommand(field, params, feature)
  )
}

/** Extent variants in which the feature ignores Reverse (features/extrude.md open issue 1). */
const REVERSE_IGNORED: ReadonlySet<string> = new Set(['symmetric', 'full'])

/** Whether the feature's params have a top-level boolean `reverse` and a top-level tagged union `extent` (2.12). */
function hasInlineReverse(feature: CatalogFeature): boolean {
  const root = feature.paramsSchema
  const properties = propertiesOf(root)
  const reverse = schemaOrNull(properties['reverse'])
  const extent = schemaOrNull(properties['extent'])
  return (
    reverse !== null &&
    extent !== null &&
    typesOf(unwrapped(reverse, root).schema).includes('boolean') &&
    variantsOf(unwrapped(extent, root).schema, root) !== null
  )
}

/**
 * Places the inline Reverse toggle right after the field it attaches to (the first shown expression
 * field under `extent.`, else `extent.kind`), or leaves it out while the extent ignores it (2.12).
 */
function withInlineReverse(
  essential: readonly InspectorField[],
  more: readonly InspectorField[],
  params: Params,
): { readonly essential: InspectorField[]; readonly more: InspectorField[] } {
  const reverse = [...essential, ...more].find((field) => field.path === 'reverse')
  const others = {
    essential: essential.filter((field) => field.path !== 'reverse'),
    more: more.filter((field) => field.path !== 'reverse'),
  }
  const kind = getParam(params, 'extent.kind')
  if (reverse === undefined || (typeof kind === 'string' && REVERSE_IGNORED.has(kind))) {
    return others
  }
  const anchor =
    others.essential.find(
      (field) => field.control.type === 'expression' && field.path.startsWith('extent.'),
    ) ?? others.essential.find((field) => field.path === 'extent.kind')
  if (anchor === undefined) {
    return { essential: [...essential], more: [...more] }
  }
  const toggle: InspectorField = {
    ...reverse,
    essential: true,
    control: { type: 'toggle' },
    attachedTo: anchor.path,
  }
  const index = others.essential.indexOf(anchor)
  return {
    essential: [
      ...others.essential.slice(0, index + 1),
      toggle,
      ...others.essential.slice(index + 1),
    ],
    more: others.more,
  }
}

/** Selection fields that rules fill, which never count as changed: body, sketch and target fields. */
function filledByRules(field: InspectorField): boolean {
  return (
    field.control.type === 'selection' &&
    (field.control.accepts === 'body' || field.control.accepts === 'sketch')
  )
}

/** The start params with every tagged union switched to the variant the params use, as setParam starts it. */
function alignedStart(feature: CatalogFeature, start: Params, params: Params): Params {
  let aligned: Params = start
  for (const hint of feature.manifest.fields) {
    const segments = parsePath(hint.path)
    const located = segments === null ? null : locate(feature.paramsSchema, params, segments)
    const value = getParam(params, hint.path)
    if (
      located?.tag === true &&
      typeof value === 'string' &&
      getParam(aligned, hint.path) !== value
    ) {
      aligned = setParam(feature, aligned, hint.path, value)
    }
  }
  return aligned
}

export function buildInspectorModel(input: {
  readonly feature: CatalogFeature
  readonly params: Params
  /** Field errors by path, including the status messages mapped to fields (3.7). */
  readonly errors: Readonly<Record<string, string>>
  /** The starting values the params are compared with for `changedMore`: `defaultParams` without the reuse step; without it, nothing counts as changed. */
  readonly startParams?: Params
}): InspectorModel {
  const { feature, params } = input
  const root = feature.paramsSchema
  const targetRule = bodyCreatingTarget(feature)
  const essentialFields: InspectorField[] = []
  const moreFields: InspectorField[] = []
  for (const hint of feature.manifest.fields) {
    const segments = parsePath(hint.path)
    const located = segments === null ? null : locate(root, params, segments)
    if (located === null) {
      continue
    }
    // A body-creating target of a feature with op is shown only while op is new (2.12).
    if (targetRule === 'op' && hint.path === 'target' && params['op'] !== 'new') {
      continue
    }
    const field: InspectorField = {
      path: hint.path,
      label: hint.label,
      // Help in the interface's words; schema descriptions are never shown (2.12).
      help: fieldHelp(feature.type, hint.path),
      essential: hint.essential ?? false,
      required: located.required,
      control: controlFor(hint, located, root),
      value: getParam(params, hint.path),
      error: input.errors[hint.path] ?? null,
      attachedTo: null,
    }
    if (field.essential) {
      essentialFields.push(field)
    } else {
      moreFields.push(field)
    }
  }
  const { essential, more } = hasInlineReverse(feature)
    ? withInlineReverse(essentialFields, moreFields, params)
    : { essential: essentialFields, more: moreFields }
  const fields = [...essential, ...more]
  const start =
    input.startParams === undefined ? null : alignedStart(feature, input.startParams, params)
  return {
    essential,
    more,
    forceMore: more.some(
      (field) =>
        field.error !== null ||
        (missing(field, params, feature) && !waitsForPick(field, fields, params)),
    ),
    changedMore:
      start === null
        ? []
        : more
            .filter(
              (field) =>
                !filledByRules(field) &&
                JSON.stringify(field.value) !== JSON.stringify(getParam(start, field.path)),
            )
            .map((field) => field.label),
    complete: fields.every((field) => field.error === null && !missing(field, params, feature)),
  }
}

/** The model's fields: essential ones, then those under More options. */
export function fieldsOf(model: InspectorModel): readonly InspectorField[] {
  return [...model.essential, ...model.more]
}

/** The catalog feature of the active feature command, or undefined without one. */
export function activeCommandFeature(state: AppState): CatalogFeature | undefined {
  return state.command === null ? undefined : state.catalog?.byType[state.command.featureType]
}

/** The active feature command's model, or null without a command. */
export function activeCommandModel(
  state: AppState,
  errors: Readonly<Record<string, string>> = {},
): ActiveCommandModel | null {
  const command = state.command
  const feature = activeCommandFeature(state)
  if (command === null || feature === undefined) {
    return null
  }
  return {
    command,
    feature,
    model: buildInspectorModel({ feature, params: command.params, errors }),
  }
}

function hintFor(feature: CatalogFeature, path: string): FieldHint | undefined {
  return feature.manifest.fields.find((hint) => hint.path === path)
}

/**
 * The starting value of one property, or undefined to leave it out (2.12): an expression's start
 * value, the variant a union's start value names, the schema default, then the unit's default.
 * `scope` holds the params known so far, for start values that follow another field.
 */
function defaultValue(
  feature: CatalogFeature,
  schema: JsonSchema,
  path: string,
  required: boolean,
  scope: Params,
): unknown {
  const root = feature.paramsSchema
  const direct = resolved(schema, root)
  const leaf = unwrapped(schema, root)
  const hint = hintFor(feature, path)
  const unit = hint?.unit ?? null
  const expression = unit !== null && isExpression(leaf.written, root)
  if (expression) {
    const start = startValue(feature.type, path, scope)
    if (start !== undefined) {
      return start
    }
  }
  const variants = variantsOf(leaf.schema, root)
  const startKind = variants === null ? undefined : startValue(feature.type, `${path}.kind`, scope)
  const named = variants?.find((variant) => tagOf(variant, root) === startKind)
  if (named !== undefined) {
    return variantDefaults(feature, named, path, scope)
  }
  const declared = schema['default'] ?? direct['default']
  if (declared !== undefined) {
    return structuredClone(declared)
  }
  if (!required || leaf.optional) {
    return undefined
  }
  // A constant, such as a variant's `kind` tag, has one possible value.
  if (leaf.schema['const'] !== undefined) {
    return leaf.schema['const']
  }
  if (hint?.selection !== undefined && hint.selection !== null) {
    // Selection fields start empty.
    return undefined
  }
  if (expression && unit !== null) {
    return EXPRESSION_DEFAULTS[unit]
  }
  if (variants !== null) {
    const [first] = variants
    return first === undefined ? undefined : variantDefaults(feature, first, path, scope)
  }
  const options = hint?.options ?? []
  if (options.length > 0) {
    return options[0]?.value
  }
  const items = schemaOrNull(leaf.schema['items'])
  const length = leaf.schema['maxItems']
  if (items !== null && typeof length === 'number' && length === leaf.schema['minItems']) {
    const values = Array.from({ length }, (_unused, index) =>
      defaultValue(feature, items, `${path}[${String(index)}]`, true, scope),
    )
    return values.every((item) => item !== undefined) ? values : undefined
  }
  if (typesOf(leaf.schema).includes('object')) {
    return objectDefaults(feature, leaf.schema, path, scope)
  }
  return undefined
}

/** Defaults of every property of an object schema; `prefix` is the object's path. */
function objectDefaults(
  feature: CatalogFeature,
  schema: JsonSchema,
  prefix: string,
  scope: Params,
): Record<string, unknown> {
  const result: Record<string, unknown> = {}
  const required = requiredOf(schema)
  for (const [name, property] of Object.entries(propertiesOf(schema))) {
    const propertySchema = schemaOrNull(property)
    if (propertySchema === null) {
      continue
    }
    const value = defaultValue(
      feature,
      propertySchema,
      prefix === '' ? name : `${prefix}.${name}`,
      required.includes(name),
      // At the top level, the properties started so far are the params later ones follow.
      prefix === '' ? result : scope,
    )
    if (value !== undefined) {
      result[name] = value
    }
  }
  return result
}

/** A tagged union's variant with its tag and its defaults. */
function variantDefaults(
  feature: CatalogFeature,
  variant: JsonSchema,
  path: string,
  scope: Params,
): Record<string, unknown> {
  const { kind, ...properties } = objectDefaults(feature, variant, path, scope)
  return { kind, ...properties }
}

/** Whether every tagged union above the path selects the same variant in both values. */
function sameVariants(first: Params, second: Params, path: string): boolean {
  const segments = path.split('.')
  for (let i = 1; i < segments.length; i++) {
    const tag = `${segments.slice(0, i).join('.')}.kind`
    const kind = getParam(second, tag)
    if (typeof kind === 'string' && getParam(first, tag) !== kind) {
      return false
    }
  }
  return true
}

/**
 * The reuse rule (2.12): each expression field takes the text at the same path of the latest
 * record of the same type that has a value there, is not in error, and uses the same variant.
 */
function reusedParams(
  feature: CatalogFeature,
  params: Record<string, unknown>,
  context: DefaultsContext,
): Record<string, unknown> {
  const records = (context.document?.features ?? [])
    .filter(
      (record) =>
        record.type === feature.type && context.featureStatus[record.id]?.status !== 'error',
    )
    .toReversed()
  const root = feature.paramsSchema
  let reused = params
  for (const hint of feature.manifest.fields) {
    const segments = parsePath(hint.path)
    const located = segments === null ? null : locate(root, reused, segments)
    if (
      segments === null ||
      located === null ||
      hint.unit === undefined ||
      hint.unit === null ||
      !isExpression(located.written, root)
    ) {
      continue
    }
    const source = records.find((record) => {
      const text = getParam(record, hint.path)
      return (
        typeof text === 'string' && text.trim() !== '' && sameVariants(record, reused, hint.path)
      )
    })
    const text = source === undefined ? undefined : getParam(source, hint.path)
    if (typeof text === 'string') {
      reused = writtenParams(reused, segments, text)
    }
  }
  return reused
}

/** Whether a field's parent object has a face or edge field, whose first pick fills its body (2.12). */
function besideElementField(feature: CatalogFeature, path: string): boolean {
  const parent = parentPath(path)
  return feature.manifest.fields.some(
    (hint) =>
      (hint.selection === 'face' || hint.selection === 'edge') && parentPath(hint.path) === parent,
  )
}

export function defaultParams(
  feature: CatalogFeature,
  context: DefaultsContext,
): Record<string, unknown> {
  const params = reusedParams(
    feature,
    objectDefaults(feature, feature.paramsSchema, '', {}),
    context,
  )
  const op = hintFor(feature, 'op')
  if (op?.options?.some((option) => option.value === 'new') === true) {
    params['op'] = context.bodiesWithGeometry.length === 0 ? 'new' : 'join'
  }
  const body = hintFor(feature, 'body')
  if (
    body?.selection === 'body' &&
    params['op'] !== 'new' &&
    context.lastTargetBody !== null &&
    !besideElementField(feature, 'body')
  ) {
    params['body'] = context.lastTargetBody
  }
  return params
}

/**
 * Writes a value at a path, creating intermediate objects; undefined removes the key. Changing a
 * tagged union's `kind` gives the new variant's defaults, keeping properties both variants have.
 */
export function setParam(
  feature: CatalogFeature,
  params: Params,
  path: string,
  value: unknown,
): Record<string, unknown> {
  const segments = parsePath(path)
  if (segments === null) {
    return { ...params }
  }
  const root = feature.paramsSchema
  const located = locate(root, params, segments)
  const unionPath = segments.slice(0, -1)
  if (located?.tag === true && typeof value === 'string' && unionPath.length > 0) {
    const unionPathText = parentPath(path)
    const union = locate(root, params, unionPath)
    const variants = union === null ? null : variantsOf(union.schema, root)
    const variant = variants?.find((candidate) => tagOf(candidate, root) === value)
    if (variant !== undefined) {
      const previous = getParam(params, unionPathText)
      const kept = isRecord(previous) ? previous : {}
      const declared = propertiesOf(variant)
      const next: Record<string, unknown> = variantDefaults(feature, variant, unionPathText, params)
      for (const [name, property] of Object.entries(kept)) {
        if (name !== 'kind' && name in declared && property !== undefined) {
          next[name] = property
        }
      }
      return writtenParams(params, unionPath, next)
    }
  }
  return writtenParams(params, segments, value)
}
