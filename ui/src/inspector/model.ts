import type { Body, FieldHint, FieldUnit, SelectionKind } from '../generated/protocol'
import type { CatalogFeature } from '../state/store'

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
}

export type InspectorModel = {
  readonly essential: readonly InspectorField[]
  readonly more: readonly InspectorField[]
  /** True when a field in `more` has an error, which forces the More section open. */
  readonly forceMore: boolean
  /** True when every required field has a value and no field has an error. */
  readonly complete: boolean
}

export type DefaultsContext = {
  readonly bodies: readonly Body[]
  readonly bodiesWithGeometry: readonly string[]
  readonly lastTargetBody: string | null
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
  return Array.isArray(type) ? type.filter((entry): entry is string => typeof entry === 'string') : []
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
  readonly description: string | null
}

function descriptionOf(...schemas: readonly JsonSchema[]): string | null {
  for (const schema of schemas) {
    const description = schema['description']
    if (typeof description === 'string') {
      return description
    }
  }
  return null
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
    description: descriptionOf(raw, resolved(raw, root), leaf.written, leaf.schema),
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
    choices.every((choice) => typeof schemaOrNull(resolved(schemaOrNull(choice) ?? {}, root))?.['const'] === 'string')
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

/** A body field the command fills itself: an empty body beside op `new` gets a new body (3.7). */
function filledByCommand(field: InspectorField, params: Params): boolean {
  if (field.control.type !== 'selection' || field.control.accepts !== 'body') {
    return false
  }
  const parent = parentPath(field.path)
  return getParam(params, parent === '' ? 'op' : `${parent}.op`) === 'new'
}

function rootSchemaOf(feature: CatalogFeature): JsonSchema {
  return feature.paramsSchema
}

export function buildInspectorModel(input: {
  readonly feature: CatalogFeature
  readonly params: Params
  readonly errors: Readonly<Record<string, string>>
}): InspectorModel {
  const root = rootSchemaOf(input.feature)
  const essential: InspectorField[] = []
  const more: InspectorField[] = []
  for (const hint of input.feature.manifest.fields) {
    const segments = parsePath(hint.path)
    const located = segments === null ? null : locate(root, input.params, segments)
    if (located === null) {
      continue
    }
    const field: InspectorField = {
      path: hint.path,
      label: hint.label,
      help: located.description,
      essential: hint.essential ?? false,
      required: located.required,
      control: controlFor(hint, located, root),
      value: getParam(input.params, hint.path),
      error: input.errors[hint.path] ?? null,
    }
    if (field.essential) {
      essential.push(field)
    } else {
      more.push(field)
    }
  }
  const fields = [...essential, ...more]
  return {
    essential,
    more,
    forceMore: more.some((field) => field.error !== null),
    complete: fields.every(
      (field) =>
        field.error === null &&
        (!field.required ||
          hasValue(field.control, field.value) ||
          filledByCommand(field, input.params)),
    ),
  }
}

function hintFor(feature: CatalogFeature, path: string): FieldHint | undefined {
  return feature.manifest.fields.find((hint) => hint.path === path)
}

/** The starting value of one property, or undefined to leave it out. */
function defaultValue(
  feature: CatalogFeature,
  schema: JsonSchema,
  path: string,
  required: boolean,
): unknown {
  const root = rootSchemaOf(feature)
  const direct = resolved(schema, root)
  const declared = schema['default'] ?? direct['default']
  if (declared !== undefined) {
    return structuredClone(declared)
  }
  const leaf = unwrapped(schema, root)
  if (!required || leaf.optional) {
    return undefined
  }
  // A constant, such as a variant's `kind` tag, has one possible value.
  if (leaf.schema['const'] !== undefined) {
    return leaf.schema['const']
  }
  const hint = hintFor(feature, path)
  if (hint?.selection !== undefined && hint.selection !== null) {
    // Selection fields start empty.
    return undefined
  }
  if (hint?.unit !== undefined && hint.unit !== null && isExpression(leaf.written, root)) {
    return EXPRESSION_DEFAULTS[hint.unit]
  }
  const variants = variantsOf(leaf.schema, root)
  if (variants !== null) {
    const [first] = variants
    return first === undefined ? undefined : variantDefaults(feature, first, path)
  }
  const options = hint?.options ?? []
  if (options.length > 0) {
    return options[0]?.value
  }
  const items = schemaOrNull(leaf.schema['items'])
  const length = leaf.schema['maxItems']
  if (items !== null && typeof length === 'number' && length === leaf.schema['minItems']) {
    const values = Array.from({ length }, (_unused, index) =>
      defaultValue(feature, items, `${path}[${String(index)}]`, true),
    )
    return values.every((item) => item !== undefined) ? values : undefined
  }
  if (typesOf(leaf.schema).includes('object')) {
    return objectDefaults(feature, leaf.schema, path)
  }
  return undefined
}

/** Defaults of every property of an object schema; `prefix` is the object's path. */
function objectDefaults(
  feature: CatalogFeature,
  schema: JsonSchema,
  prefix: string,
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
): Record<string, unknown> {
  const { kind, ...properties } = objectDefaults(feature, variant, path)
  return { kind, ...properties }
}

export function defaultParams(
  feature: CatalogFeature,
  context: DefaultsContext,
): Record<string, unknown> {
  const params = objectDefaults(feature, rootSchemaOf(feature), '')
  const op = hintFor(feature, 'op')
  if (op?.options?.some((option) => option.value === 'new') === true) {
    params['op'] = context.bodiesWithGeometry.length === 0 ? 'new' : 'join'
  }
  const body = hintFor(feature, 'body')
  if (body?.selection === 'body' && params['op'] !== 'new' && context.lastTargetBody !== null) {
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
      const next: Record<string, unknown> = variantDefaults(feature, variant, unionPathText)
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
