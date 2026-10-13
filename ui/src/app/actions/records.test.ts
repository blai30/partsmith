import { describe, expect, it } from 'vitest'
import type { Body } from '../../generated/protocol'
import { withOptional } from './records'

describe('withOptional', () => {
  const BODY: Body = { id: 'b-cube', name: 'Cube', color: '#ff0000' }

  it('sets the property on a copy, keeping the key order of the record', () => {
    const changed = withOptional(BODY, 'color', '#00ff00')
    expect(Object.entries(changed)).toEqual([
      ['id', 'b-cube'],
      ['name', 'Cube'],
      ['color', '#00ff00'],
    ])
    expect(BODY.color).toBe('#ff0000')
  })

  it('removes the property for undefined', () => {
    expect(withOptional(BODY, 'color', undefined)).toEqual({ id: 'b-cube', name: 'Cube' })
    expect('color' in withOptional(BODY, 'color', undefined)).toBe(false)
  })
})
