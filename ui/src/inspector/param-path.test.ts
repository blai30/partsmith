import { describe, expect, it } from 'vitest'
import { isRecord, parentPath, parsePath, writtenParams } from './param-path'

describe('parsePath', () => {
  it('splits names and array indices', () => {
    expect(parsePath('extent.expr')).toEqual(['extent', 'expr'])
    expect(parsePath('size[1]')).toEqual(['size', 1])
    expect(parsePath('a.b[1][2].c')).toEqual(['a', 'b', 1, 2, 'c'])
  })

  it('is null for a path it cannot address', () => {
    expect(parsePath('edges[*]')).toBeNull()
    expect(parsePath('1st')).toBeNull()
  })
})

describe('parentPath', () => {
  it('is the path of the object holding the field', () => {
    expect(parentPath('profile.sketch')).toBe('profile')
    expect(parentPath('body')).toBe('')
  })

  it('keeps every object above the field', () => {
    expect(parentPath('a.b.c')).toBe('a.b')
  })
})

describe('writtenParams', () => {
  it('writes a value at a path, creating objects and arrays on the way', () => {
    const params = { op: 'new' }
    expect(writtenParams(params, ['extent', 'expr'], '5 mm')).toEqual({
      op: 'new',
      extent: { expr: '5 mm' },
    })
    expect(writtenParams({}, ['size', 1], '2 mm')).toEqual({ size: [undefined, '2 mm'] })
    expect(params).toEqual({ op: 'new' })
  })

  it('removes the key or array item that undefined is written at', () => {
    expect(writtenParams({ op: 'new', body: 'b1' }, ['body'], undefined)).toEqual({ op: 'new' })
    expect(writtenParams({ edges: ['e1', 'e2'] }, ['edges', 0], undefined)).toEqual({
      edges: ['e2'],
    })
  })
})

describe('isRecord', () => {
  it('is true for objects only', () => {
    expect(isRecord({})).toBe(true)
    expect(isRecord({ kind: 'distance' })).toBe(true)
    expect(isRecord(null)).toBe(false)
    expect(isRecord(['e1'])).toBe(false)
    expect(isRecord('10 mm')).toBe(false)
  })
})
