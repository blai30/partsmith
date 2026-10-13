// Navigating a params JSON Schema (draft 2020-12 as schemars 1 emits it, ui.md 2.12): references,
// Options, tagged unions and the schema a path leads to for the current params.

import { isRecord, type PathSegment } from './param-path'

export type JsonSchema = Readonly<Record<string, unknown>>

/** The value as a schema when it is a JSON object, else null. */
export function schemaOrNull(value: unknown): JsonSchema | null {
  return isRecord(value) ? value : null
}

/** Follows `$ref`s into the root schema's `$defs`. */
export function resolved(schema: JsonSchema, root: JsonSchema): JsonSchema {
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
export type Unwrapped = {
  /** The schema as written, below any Option. */
  readonly written: JsonSchema
  readonly schema: JsonSchema
  readonly optional: boolean
}

/** The schema below any Option, with references followed. */
export function unwrapped(schema: JsonSchema, root: JsonSchema): Unwrapped {
  const direct = resolved(schema, root)
  const inner = optionInner(direct)
  if (inner === null) {
    return { written: schema, schema: direct, optional: false }
  }
  return { written: inner, schema: resolved(inner, root), optional: true }
}

/** The schema's `type` as a list: one type, several, or none. */
export function typesOf(schema: JsonSchema): readonly string[] {
  const type = schema['type']
  if (typeof type === 'string') {
    return [type]
  }
  return Array.isArray(type)
    ? type.filter((entry): entry is string => typeof entry === 'string')
    : []
}

/** The schema's `properties`, empty without them. */
export function propertiesOf(schema: JsonSchema): Readonly<Record<string, unknown>> {
  return schemaOrNull(schema['properties']) ?? {}
}

/** The names in the schema's `required`, empty without them. */
export function requiredOf(schema: JsonSchema): readonly unknown[] {
  const required = schema['required']
  return Array.isArray(required) ? required : []
}

/** The value of a variant's `kind` tag, or null when the schema is not a tagged variant. */
export function tagOf(variant: JsonSchema, root: JsonSchema): string | null {
  const kind = schemaOrNull(propertiesOf(resolved(variant, root))['kind'])
  const value = kind?.['const']
  return typeof value === 'string' ? value : null
}

/** The variants of a tagged union (a `oneOf` of objects with a constant `kind`), or null. */
export function variantsOf(schema: JsonSchema, root: JsonSchema): readonly JsonSchema[] | null {
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

/** Whether the schema is the `Expr` text of an expression. */
export function isExpression(schema: JsonSchema, root: JsonSchema): boolean {
  return schema['$ref'] === '#/$defs/Expr' || resolved(schema, root)['title'] === 'Expr'
}

/** Where a path leads in the schema, for the current params. */
export type Located = {
  /** The leaf schema as written (below any Option). */
  readonly written: JsonSchema
  /** The leaf schema with references followed. */
  readonly schema: JsonSchema
  readonly required: boolean
  /** The leaf is the `kind` tag of a tagged union. */
  readonly tag: boolean
}

/** Follows a path through the schema; null when the current value's branch does not declare it. */
export function locate(
  root: JsonSchema,
  params: Readonly<Record<string, unknown>>,
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
