import { describe, expect, it } from 'vitest'
import { keepInViewHorizontally, keepInViewVertically } from './scroll'

/** A container whose layout sizes jsdom does not compute, set by hand. */
function container(size: { readonly clientHeight?: number; readonly clientWidth?: number }) {
  const element = document.createElement('div')
  Object.defineProperty(element, 'clientHeight', { value: size.clientHeight ?? 0 })
  Object.defineProperty(element, 'clientWidth', { value: size.clientWidth ?? 0 })
  return element
}

function item(box: {
  readonly top?: number
  readonly height?: number
  readonly left?: number
  readonly width?: number
}) {
  const element = document.createElement('div')
  Object.defineProperty(element, 'offsetTop', { value: box.top ?? 0 })
  Object.defineProperty(element, 'offsetHeight', { value: box.height ?? 0 })
  Object.defineProperty(element, 'offsetLeft', { value: box.left ?? 0 })
  Object.defineProperty(element, 'offsetWidth', { value: box.width ?? 0 })
  return element
}

describe('keepInViewVertically', () => {
  it('scrolls up to an item above the visible part', () => {
    const list = container({ clientHeight: 100 })
    list.scrollTop = 200
    keepInViewVertically(list, item({ top: 150, height: 36 }))
    expect(list.scrollTop).toBe(150)
  })

  it('scrolls down just enough to show an item below the visible part', () => {
    const list = container({ clientHeight: 100 })
    keepInViewVertically(list, item({ top: 180, height: 36 }))
    expect(list.scrollTop).toBe(116)
  })

  it('leaves a visible item where it is', () => {
    const list = container({ clientHeight: 100 })
    list.scrollTop = 40
    keepInViewVertically(list, item({ top: 60, height: 36 }))
    expect(list.scrollTop).toBe(40)
  })
})

describe('keepInViewHorizontally', () => {
  it('scrolls left to an item before the visible part', () => {
    const strip = container({ clientWidth: 200 })
    strip.scrollLeft = 300
    keepInViewHorizontally(strip, item({ left: 100, width: 40 }))
    expect(strip.scrollLeft).toBe(100)
  })

  it('scrolls right just enough to show an item after the visible part', () => {
    const strip = container({ clientWidth: 200 })
    keepInViewHorizontally(strip, item({ left: 400, width: 40 }))
    expect(strip.scrollLeft).toBe(240)
  })
})
