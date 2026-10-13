import { describe, expect, it } from 'vitest'
import { isView, VIEW_ID } from './focus'

describe('isView', () => {
  it("is true for the 3D view's surface only", () => {
    const view = document.createElement('div')
    view.id = VIEW_ID
    expect(isView(view)).toBe(true)
    expect(isView(document.createElement('div'))).toBe(false)
  })
})
