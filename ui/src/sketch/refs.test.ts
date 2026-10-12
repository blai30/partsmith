import { describe, expect, it } from 'vitest'
import { ORIGIN_POINT, X_AXIS, Y_AXIS } from './refs'

describe('reserved sketch references (sketch.md 3.16)', () => {
  it('ORIGIN_POINT is the point reference origin', () => {
    expect(ORIGIN_POINT).toBe('origin')
  })

  it('X_AXIS is the line reference x_axis', () => {
    expect(X_AXIS).toBe('x_axis')
  })

  it('Y_AXIS is the line reference y_axis', () => {
    expect(Y_AXIS).toBe('y_axis')
  })
})
