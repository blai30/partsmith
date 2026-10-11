import { describe, expect, it } from 'vitest'
import { utf16Range } from './span'

// 10 UTF-8 bytes and 9 UTF-16 units: Ø takes 2 bytes and 1 unit.
const SOURCE = '2 * Ø + x'

describe('utf16Range', () => {
  it('U-92 the span 9..10 of x gives 8..9', () => {
    expect(utf16Range(SOURCE, { start: 9, end: 10 })).toEqual({ start: 8, end: 9 })
  })

  it('U-92 the span 4..6 of Ø gives 4..5', () => {
    expect(utf16Range(SOURCE, { start: 4, end: 6 })).toEqual({ start: 4, end: 5 })
  })

  it('U-92 the span 0..10 of the whole source gives 0..9', () => {
    expect(utf16Range(SOURCE, { start: 0, end: 10 })).toEqual({ start: 0, end: 9 })
  })

  it('U-92 an offset of 5, inside Ø, gives null', () => {
    expect(utf16Range(SOURCE, { start: 5, end: 10 })).toBeNull()
    expect(utf16Range(SOURCE, { start: 0, end: 5 })).toBeNull()
  })

  it('U-92 an end of 11, past the end, gives null', () => {
    expect(utf16Range(SOURCE, { start: 9, end: 11 })).toBeNull()
  })

  it('U-92 for a😀b the span 5..6 of b gives 3..4', () => {
    expect(utf16Range('a😀b', { start: 5, end: 6 })).toEqual({ start: 3, end: 4 })
  })

  it('maps a code point above U+FFFF to its two UTF-16 units', () => {
    expect(utf16Range('a😀b', { start: 1, end: 5 })).toEqual({ start: 1, end: 3 })
  })

  it('gives null for an offset inside a 4-byte character', () => {
    expect(utf16Range('a😀b', { start: 2, end: 5 })).toBeNull()
  })

  it('maps a 3-byte character to one UTF-16 unit', () => {
    expect(utf16Range('€1', { start: 3, end: 4 })).toEqual({ start: 1, end: 2 })
  })

  it('maps an empty span at the end to the source length', () => {
    expect(utf16Range(SOURCE, { start: 10, end: 10 })).toEqual({ start: 9, end: 9 })
  })

  it('counts a lone surrogate as the 3 bytes of U+FFFD', () => {
    expect(utf16Range('\uD800x', { start: 3, end: 4 })).toEqual({ start: 1, end: 2 })
  })

  it('maps ASCII offsets to the same indices', () => {
    expect(utf16Range('width * 2', { start: 0, end: 5 })).toEqual({ start: 0, end: 5 })
  })
})
