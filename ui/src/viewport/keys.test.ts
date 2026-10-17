import { describe, expect, it } from 'vitest'

import { meshElementKey } from './keys'

describe('meshElementKey (3.6)', () => {
  it('keys a mesh element by mesh id, kind and display id', () => {
    expect(meshElementKey(3, 'edge', 4)).toBe('mesh:3:edge:4')
  })
})
