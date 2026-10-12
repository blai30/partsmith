import { afterEach, describe, expect, it } from 'vitest'
import { hidePopover, placeNextTo, showPopover } from './floating'
import { installPopoverPolyfill, isPopoverOpen } from './popover-polyfill.testing'

// The declarations of an element's style attribute, read through the attribute.
function declarations(element: Element): string[] {
  return (element.getAttribute('style') ?? '')
    .split(';')
    .map((declaration) => declaration.trim().replaceAll(/\s+/g, ' '))
    .filter((declaration) => declaration !== '')
}

function withBounds(element: Element, x: number, y: number, width: number, height: number): void {
  element.getBoundingClientRect = () => DOMRect.fromRect({ x, y, width, height })
}

function setWindowSize(width: number, height: number): void {
  const root = document.documentElement
  Object.defineProperty(root, 'clientWidth', { configurable: true, value: width })
  Object.defineProperty(root, 'clientHeight', { configurable: true, value: height })
}

afterEach(() => {
  const root = document.documentElement
  Reflect.deleteProperty(root, 'clientWidth')
  Reflect.deleteProperty(root, 'clientHeight')
  root.removeAttribute('style')
  document.body.replaceChildren()
})

describe('placeNextTo', () => {
  it('moves the element under its anchor with a 0.25rem gap and marks it positioned', () => {
    setWindowSize(800, 600)
    const anchor = document.createElement('button')
    const element = document.createElement('div')
    withBounds(anchor, 100, 50, 40, 20)
    withBounds(element, 0, 0, 120, 60)
    placeNextTo(element, anchor, 'below')
    expect(declarations(element)).toEqual(['translate: 100px 74px'])
    expect(element.hasAttribute('data-positioned')).toBe(true)
  })

  it('scales the gap with the root font size (the UI scale)', () => {
    setWindowSize(800, 600)
    document.documentElement.setAttribute('style', 'font-size: 32px')
    const anchor = document.createElement('button')
    const element = document.createElement('div')
    withBounds(anchor, 100, 50, 40, 20)
    withBounds(element, 0, 0, 120, 60)
    placeNextTo(element, anchor, 'below')
    expect(declarations(element)).toEqual(['translate: 100px 78px'])
  })

  it('centers the element in the window without an anchor', () => {
    setWindowSize(800, 600)
    const element = document.createElement('div')
    withBounds(element, 0, 0, 200, 100)
    placeNextTo(element, null, 'below')
    expect(declarations(element)).toEqual(['translate: 300px 250px'])
  })
})

describe('showPopover and hidePopover', () => {
  it('do nothing where the Popover API is missing', () => {
    const element = document.createElement('div')
    element.setAttribute('popover', 'auto')
    document.body.append(element)
    expect(() => {
      showPopover(element, null)
      hidePopover(element)
    }).not.toThrow()
  })

  it('show and hide a popover through the Popover API', () => {
    const uninstall = installPopoverPolyfill()
    try {
      const element = document.createElement('div')
      element.setAttribute('popover', 'auto')
      document.body.append(element)
      showPopover(element, null)
      expect(isPopoverOpen(element)).toBe(true)
      hidePopover(element)
      expect(isPopoverOpen(element)).toBe(false)
    } finally {
      uninstall()
    }
  })

  it('hiding a hidden popover does nothing', () => {
    const uninstall = installPopoverPolyfill()
    try {
      const element = document.createElement('div')
      element.setAttribute('popover', 'auto')
      document.body.append(element)
      expect(() => {
        hidePopover(element)
      }).not.toThrow()
      expect(isPopoverOpen(element)).toBe(false)
    } finally {
      uninstall()
    }
  })
})
