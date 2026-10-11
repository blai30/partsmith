import { afterEach, describe, expect, it, vi } from 'vitest'
import { createWebGl2Backend } from './backend'

afterEach(() => {
  vi.restoreAllMocks()
})

describe('createWebGl2Backend', () => {
  it('returns null when the canvas cannot create a WebGL2 context', () => {
    const canvas = document.createElement('canvas')
    const getContext = vi.spyOn(canvas, 'getContext').mockReturnValue(null)
    expect(createWebGl2Backend(canvas)).toBeNull()
    expect(getContext.mock.calls.map((call) => call[0])).toContain('webgl2')
  })
})
