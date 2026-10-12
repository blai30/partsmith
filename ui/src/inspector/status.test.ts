import { describe, expect, it } from 'vitest'
import type { Diagnostic } from '../generated/protocol'
import { fieldForPointer, fieldPointer, messagesByField } from './status'

describe('fieldPointer', () => {
  it('turns a hint path into a JSON pointer', () => {
    expect(fieldPointer('radius')).toBe('/radius')
    expect(fieldPointer('extent.expr')).toBe('/extent/expr')
    expect(fieldPointer('a.b[1]')).toBe('/a/b/1')
  })
})

describe('fieldForPointer', () => {
  const paths = ['edges', 'radius', 'extent.kind', 'extent.expr']

  it('maps an exact pointer to its field', () => {
    expect(fieldForPointer(paths, '/radius')).toBe('radius')
    expect(fieldForPointer(paths, '/extent/expr')).toBe('extent.expr')
  })

  it('maps a pointer below a field to the field with the longest whole-segment prefix', () => {
    expect(fieldForPointer(paths, '/edges/1')).toBe('edges')
  })

  it('does not match a field that is only a prefix of a segment', () => {
    expect(fieldForPointer(['edge'], '/edges/1')).toBeNull()
  })

  it('maps nothing for an unknown pointer', () => {
    expect(fieldForPointer(paths, '/body')).toBeNull()
  })
})

describe('messagesByField', () => {
  it('keeps the first message per field and skips messages without a field', () => {
    const first: Diagnostic = { severity: 'error', code: 'a', message: 'First.', field: '/radius' }
    const second: Diagnostic = { severity: 'warning', code: 'b', message: 'Second.', field: '/radius' }
    const general: Diagnostic = { severity: 'warning', code: 'c', message: 'General.' }
    const byField = messagesByField(['radius'], [general, first, second])
    expect(byField.get('radius')).toBe(first)
    expect(byField.size).toBe(1)
  })
})
