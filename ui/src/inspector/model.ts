import type {
  Body,
  Document,
  FeatureStatus,
  FieldHint,
  FieldUnit,
  SelectionKind,
} from '../generated/protocol'
import type { CatalogFeature } from '../state/store'
import { fieldHelp } from './help'
import { startValue } from './start-values'

// The schema-driven inspector model (ui.md 2.12): manifest field hints mapped onto the params JSON
// Schema (draft 2020-12 as schemars 1 emits it) to controls, defaults and param updates.

export type JsonSchema = Readonly<Record<string, unknown>>

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

type PathSegment = string | number
type Params = Readonly<Record<string, unknown>>

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function schemaOrNull(value: unknown): JsonSchema | null {
  return isRecord(value) ? value : null
}

/** Splits `a.b[1].c` into `['a', 'b', 1, 'c']`; null for paths this model cannot address (`[*]`). */
function parsePath(path: string): PathSegment[] | null {
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

/** Follows `$ref`s into the root schema's `$defs`. */
function resolved(schema: JsonSchema, root: JsonSchema): JsonSchema {
  let current = schema
  for (let depth = 0; depth < 32; depth += 1) {
    const reference = current['$ref']
    if (typeof reference !== 'string') {
      return current
    }
    const name = /^#\/\$defs\/(.+)$/.exec(reference)?.[1]
    const target = name === undefined ? null : schemaOrNull(schemaOrNull(root['$defs'])?.[name])
    if (target === null) {
      return current
    }
    current = target
  }
  return current
}

/** The inner schema of an `Option<T>` (`anyOf: [T, {type: null}]`), or null for anything else. */
function optionInner(schema: JsonSchema): JsonSchema | null {
  const choices = schema['anyOf']
  if (!Array.isArray(choices) || choices.length !== 2) {
    return null
  }
  const [first, second] = choices.map(schemaOrNull)
  if (second?.['type'] === 'null') {
    return first ?? null
  }
  if (first?.['type'] === 'null') {
    return second ?? null
  }
  return null
}

/** A schema with references followed and an Option unwrapped. */
type Unwrapped = {
  /** The schema as written, below any Option. */
  readonly written: JsonSchema
  readonly schema: JsonSchema
  readonly optional: boolean
}

function unwrapped(schema: JsonSchema, root: JsonSchema): Unwrapped {
  const direct = resolved(schema, root)
  const inner = optionInner(direct)
  if (inner === null) {
    return { written: schema, schema: direct, optional: false }
  }
  return { written: inner, schema: resolved(inner, root), optional: true }
}

function typesOf(schema: JsonSchema): readonly string[] {
  const type = schema['type']
  if (typeof type === 'string') {
    return [type]
  }
  return Array.isArray(type)
    ? type.filter((entry): entry is string => typeof entry === 'string')
    : []
}

function propertiesOf(schema: JsonSchema): Readonly<Record<string, unknown>> {
  return schemaOrNull(schema['properties']) ?? {}
}

function requiredOf(schema: JsonSchema): readonly unknown[] {
  const required = schema['required']
  return Array.isArray(required) ? required : []
}

/** The value of a variant's `kind` tag, or null when the schema is not a tagged variant. */
function tagOf(variant: JsonSchema, root: JsonSchema): string | null {
  const kind = schemaOrNull(propertiesOf(resolved(variant, root))['kind'])
  const value = kind?.['const']
  return typeof value === 'string' ? value : null
}

/** The variants of a tagged union (a `oneOf` of objects with a constant `kind`), or null. */
function variantsOf(schema: JsonSchema, root: JsonSchema): readonly JsonSchema[] | null {
  const choices = schema['oneOf']
  if (!Array.isArray(choices) || choices.length === 0) {
    return null
  }
  const variants = choices.map(schemaOrNull)
  if (variants.some((variant) => variant === null || tagOf(variant, root) === null)) {
    return null
  }
  return variants.map((variant) => resolved(variant ?? {}, root))
}

/** The variant a value selects by its `kind`, else the first. */
function variantFor(variants: readonly JsonSchema[], value: unknown, root: JsonSchema): JsonSchema {
  const kind = isRecord(value) ? value['kind'] : undefined
  return variants.find((variant) => tagOf(variant, root) === kind) ?? variants[0] ?? {}
}

function isExpression(schema: JsonSchema, root: JsonSchema): boolean {
  return schema['$ref'] === '#/$defs/Expr' || resolved(schema, root)['title'] === 'Expr'
}

/** Where a path leads in the schema, for the current params. */
type Located = {
  /** The leaf schema as written (below any Option). */
  readonly written: JsonSchema
  /** The leaf schema with references followed. */
  readonly schema: JsonSchema
  readonly required: boolean
  /** The leaf is the `kind` tag of a tagged union. */
  readonly tag: boolean
}

/** Follows a path through the schema; null when the current value's branch does not declare it. */
function locate(
  root: JsonSchema,
  params: Params,
  segments: readonly PathSegment[],
): Located | null {
  let current: JsonSchema = root
  let value: unknown = params
  let required = true
  let tag = false
  let raw: JsonSchema = root
  for (const segment of segments) {
    const parent = unwrapped(current, root)
    if (parent.optional) {
      required = false
    }
    let next: JsonSchema | null
    if (typeof segment === 'number') {
      const items = parent.schema['items']
      const maximum = parent.schema['maxItems']
      next =
        typeof maximum === 'number' && segment >= maximum
          ? null
          : schemaOrNull(Array.isArray(items) ? items[segment] : items)
      value = Array.isArray(value) ? value[segment] : undefined
      tag = false
    } else {
      const variants = variantsOf(parent.schema, root)
      const object = variants === null ? parent.schema : variantFor(variants, value, root)
      next = schemaOrNull(propertiesOf(object)[segment])
      required = required && requiredOf(object).includes(segment)
      tag = variants !== null && segment === 'kind'
      value = isRecord(value) ? value[segment] : undefined
    }
    if (next === null) {
      return null
    }
    raw = next
    current = next
  }
  const leaf = unwrapped(raw, root)
  return {
    written: leaf.written,
    schema: leaf.schema,
    required: required && !leaf.optional,
    tag,
  }
}

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

/** The path of the parent object of a field path (`''` for top-level fields). */
function parentPath(path: string): string {
  const dot = path.lastIndexOf('.')
  return dot === -1 ? '' : path.slice(0, dot)
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
    return requiredOf(rootSchemaOf(feature)).includes('target') ? null : 'always'
  }
  return op.options?.some((option) => option.value === 'new') === true ? 'op' : null
}

/** A body field the command fills itself: an empty body beside op `new` gets a new body (3.7). */
function filledByCommand(field: InspectorField, params: Params, feature: CatalogFeature): boolean {
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

function rootSchemaOf(feature: CatalogFeature): JsonSchema {
  return feature.paramsSchema
}

/** Extent variants in which the feature ignores Reverse (features/extrude.md open issue 1). */
const REVERSE_IGNORED: ReadonlySet<string> = new Set(['symmetric', 'full'])

/** Whether the feature's params have a top-level boolean `reverse` and a top-level tagged union `extent` (2.12). */
function hasInlineReverse(feature: CatalogFeature): boolean {
  const root = rootSchemaOf(feature)
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
    const located = segments === null ? null : locate(rootSchemaOf(feature), params, segments)
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
  const root = rootSchemaOf(feature)
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
  const root = rootSchemaOf(feature)
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
  const root = rootSchemaOf(feature)
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
    objectDefaults(feature, rootSchemaOf(feature), '', {}),
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

/** The params with `value` written at `segments`. */
function writtenParams(
  params: Params,
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
  const root = rootSchemaOf(feature)
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
