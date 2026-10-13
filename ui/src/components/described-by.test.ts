import { describe, expect, it } from 'vitest'
import { describedBy } from './described-by'

describe('describedBy', () => {
  it('joins the ids that are shown', () => {
    expect(describedBy('a', null, undefined, '', 'b')).toBe('a b')
    expect(describedBy(null, undefined)).toBeUndefined()
  })
})
