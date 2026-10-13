import { describe, expect, it } from 'vitest'
import {
  isExpression,
  locate,
  resolved,
  tagOf,
  unwrapped,
  variantsOf,
  type JsonSchema,
} from './schema'

const DISTANCE = {
  type: 'object',
  properties: { kind: { type: 'string', const: 'distance' }, expr: { $ref: '#/$defs/Expr' } },
  required: ['kind', 'expr'],
}
const THROUGH_ALL = {
  type: 'object',
  properties: { kind: { type: 'string', const: 'through_all' } },
  required: ['kind'],
}

const ROOT: JsonSchema = {
  type: 'object',
  properties: {
    extent: { $ref: '#/$defs/Extent' },
    body: { anyOf: [{ type: 'string' }, { type: 'null' }] },
    size: { type: 'array', items: { $ref: '#/$defs/Expr' }, maxItems: 3 },
  },
  required: ['extent', 'size'],
  $defs: {
    Expr: { title: 'Expr', type: 'string' },
    Extent: { oneOf: [DISTANCE, THROUGH_ALL] },
  },
}

describe('resolved', () => {
  it('follows references into $defs, and leaves other schemas as they are', () => {
    expect(resolved({ $ref: '#/$defs/Expr' }, ROOT)).toEqual({ title: 'Expr', type: 'string' })
    expect(resolved({ $ref: '#/$defs/Missing' }, ROOT)).toEqual({ $ref: '#/$defs/Missing' })
    expect(resolved({ type: 'boolean' }, ROOT)).toEqual({ type: 'boolean' })
  })
})

describe('unwrapped', () => {
  it('unwraps an Option to its inner schema', () => {
    expect(unwrapped({ anyOf: [{ type: 'string' }, { type: 'null' }] }, ROOT)).toEqual({
      written: { type: 'string' },
      schema: { type: 'string' },
      optional: true,
    })
  })

  it('keeps any other schema, with references followed', () => {
    expect(unwrapped({ $ref: '#/$defs/Expr' }, ROOT)).toEqual({
      written: { $ref: '#/$defs/Expr' },
      schema: { title: 'Expr', type: 'string' },
      optional: false,
    })
  })
})

describe('tagged unions', () => {
  it('lists the variants of a oneOf of objects with a constant kind', () => {
    expect(variantsOf(resolved({ $ref: '#/$defs/Extent' }, ROOT), ROOT)).toEqual([
      DISTANCE,
      THROUGH_ALL,
    ])
    expect(tagOf(DISTANCE, ROOT)).toBe('distance')
  })

  it('finds no variants in any other schema', () => {
    expect(variantsOf({ type: 'string' }, ROOT)).toBeNull()
    expect(variantsOf({ oneOf: [{ type: 'string' }] }, ROOT)).toBeNull()
    expect(tagOf({ type: 'string' }, ROOT)).toBeNull()
  })
})

describe('isExpression', () => {
  it('recognizes the Expr text by reference or title', () => {
    expect(isExpression({ $ref: '#/$defs/Expr' }, ROOT)).toBe(true)
    expect(isExpression({ title: 'Expr', type: 'string' }, ROOT)).toBe(true)
    expect(isExpression({ type: 'string' }, ROOT)).toBe(false)
  })
})

describe('locate', () => {
  it("follows a path through the variant the params' kind selects", () => {
    const params = { extent: { kind: 'distance', expr: '10 mm' } }
    expect(locate(ROOT, params, ['extent', 'expr'])).toEqual({
      written: { $ref: '#/$defs/Expr' },
      schema: { title: 'Expr', type: 'string' },
      required: true,
      tag: false,
    })
    expect(locate(ROOT, params, ['extent', 'kind'])?.tag).toBe(true)
    expect(locate(ROOT, { extent: { kind: 'through_all' } }, ['extent', 'expr'])).toBeNull()
  })

  it('makes an optional leaf not required', () => {
    expect(locate(ROOT, {}, ['body'])?.required).toBe(false)
  })

  it('addresses array items up to maxItems', () => {
    expect(locate(ROOT, {}, ['size', 2])?.schema).toEqual({ title: 'Expr', type: 'string' })
    expect(locate(ROOT, {}, ['size', 3])).toBeNull()
  })
})
