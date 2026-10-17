import { describe, expect, it } from 'vitest'
import { pointSelectionItem } from '../../sketch/hit'
import { bodyItem } from '../../test/fakes/selection'
import {
  differingFields,
  propertyOf,
  sameValue,
  siblingPath,
  toggledValue,
  valueAt,
} from './command-values'

describe('field paths', () => {
  it('siblingPath names a field beside another', () => {
    expect(siblingPath('points', 'sketch')).toBe('sketch')
    expect(siblingPath('axis.line', 'sketch')).toBe('axis.sketch')
  })
})

describe('value reads', () => {
  it('sameValue compares by JSON text', () => {
    expect(sameValue({ q: 'a', fp: 'b' }, { q: 'a', fp: 'b' })).toBe(true)
    expect(sameValue({ q: 'a', fp: 'b' }, { fp: 'b', q: 'a' })).toBe(false)
  })

  it('propertyOf and valueAt read inside objects and lists', () => {
    const value = { regions: [{ key: 'r1' }, { key: 'r2' }] }
    expect(propertyOf(value, 'regions')).toBe(value.regions)
    expect(propertyOf('text', 'length')).toBeUndefined()
    expect(valueAt(value, ['regions', '1', 'key'])).toBe('r2')
    expect(valueAt(value, ['missing', '0'])).toBeUndefined()
  })
})

describe('differingFields', () => {
  it('keeps the fields whose values differ, in the order given', () => {
    const fields = [{ path: 'radius' }, { path: 'extent.expr' }, { path: 'edges' }]
    const before = { radius: '2 mm', extent: { expr: '5 mm' }, edges: ['a'] }
    const after = { radius: '2 mm', extent: { expr: '8 mm' }, edges: ['b'] }
    expect(differingFields(fields, before, after)).toEqual([
      { path: 'extent.expr' },
      { path: 'edges' },
    ])
    expect(differingFields(fields, before, before)).toEqual([])
  })
})

describe('toggledValue', () => {
  it('adds an item to a multiple field, and removes it when it is already there', () => {
    const peg = bodyItem('b-peg', 'Peg')
    expect(toggledValue('body', true, ['b-cube'], peg, undefined)).toEqual(['b-cube', 'b-peg'])
    expect(toggledValue('body', true, ['b-peg'], peg, undefined)).toBeUndefined()
  })

  it('replaces a single value, and clears it when the same item is picked again', () => {
    const peg = bodyItem('b-peg', 'Peg')
    expect(toggledValue('body', false, 'b-cube', peg, undefined)).toBe('b-peg')
    expect(toggledValue('body', false, 'b-peg', peg, undefined)).toBeUndefined()
  })

  it('a point of another sketch replaces the list', () => {
    const point = pointSelectionItem('f-sk2', 'e-7')
    expect(toggledValue('sketch_point', true, ['e-1'], point, 'f-sk1')).toEqual(['e-7'])
  })
})
