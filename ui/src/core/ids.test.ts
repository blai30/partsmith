import { describe, expect, it, vi } from 'vitest'
import { CUBE_DOCUMENT, PARAMETRIC_DOCUMENT, RECT_SKETCH_DOCUMENT } from '../test/fakes/documents'
import { generateId, usedIds } from './ids'

describe('generateId', () => {
  it('U-19 formats the random value as 12 lowercase hex digits after the prefix', () => {
    expect(
      generateId(
        'f',
        () => false,
        () => 0x3fa91c0e72b4,
      ),
    ).toBe('f-3fa91c0e72b4')
  })

  it('U-19 zero-pads small values', () => {
    expect(
      generateId(
        'b',
        () => false,
        () => 1,
      ),
    ).toBe('b-000000000001')
  })

  it('U-19 a predicate that rejects the first candidate gets the second', () => {
    const values = [1, 2]
    const random = vi.fn<() => number>(() => values.shift() ?? 3)
    expect(generateId('f', (id) => id === 'f-000000000001', random)).toBe('f-000000000002')
    expect(random).toHaveBeenCalledTimes(2)
  })

  it('U-19 throws after 64 rejected candidates', () => {
    const random = vi.fn<() => number>(() => 5)
    expect(() => generateId('e', () => true, random)).toThrow(
      new Error('Could not generate a free id after 64 attempts.'),
    )
    expect(random).toHaveBeenCalledTimes(64)
  })

  it('uses every prefix of model.md 3.2', () => {
    for (const prefix of ['p', 'b', 'f', 'e', 'c'] as const) {
      expect(
        generateId(
          prefix,
          () => false,
          () => 0xabc,
        ),
      ).toBe(`${prefix}-000000000abc`)
    }
  })

  it('draws 48 random bits from crypto.getRandomValues without an injected source', () => {
    const getRandomValues = vi.spyOn(crypto, 'getRandomValues')
    const id = generateId('p', () => false)
    expect(id).toMatch(/^p-[0-9a-f]{12}$/)
    expect(getRandomValues).toHaveBeenCalled()
    getRandomValues.mockRestore()
  })
})

describe('usedIds', () => {
  it('holds the ids of bodies and features and the entity and constraint ids of sketches', () => {
    const ids = usedIds(CUBE_DOCUMENT)
    for (const id of ['b-cube', 'f-sk1', 'f-ext1', 'e-1', 'e-2', 'e-3', 'e-4', 'c-1', 'c-12']) {
      expect(ids.has(id)).toBe(true)
    }
  })

  it('holds parameter ids', () => {
    expect(usedIds(PARAMETRIC_DOCUMENT).has('p-size')).toBe(true)
  })

  it('holds nothing else', () => {
    expect([...usedIds(RECT_SKETCH_DOCUMENT)].toSorted()).toEqual(
      [
        'f-sk1',
        'e-1',
        'e-2',
        'e-3',
        'e-4',
        'c-1',
        'c-2',
        'c-3',
        'c-4',
        'c-5',
        'c-6',
        'c-7',
        'c-8',
        'c-9',
        'c-10',
        'c-11',
      ].toSorted(),
    )
  })
})
