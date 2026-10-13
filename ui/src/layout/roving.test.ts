import { describe, expect, it } from 'vitest'
import { arrowTarget, rovingTabStop } from './roving'

function container(count: number): { element: HTMLElement; items: HTMLButtonElement[] } {
  const element = document.createElement('div')
  const items = Array.from({ length: count }, () => {
    const item = document.createElement('button')
    item.setAttribute('data-item', '')
    element.append(item)
    return item
  })
  // An element outside the selector keeps its own tab index.
  element.append(document.createElement('button'))
  return { element, items }
}

describe('rovingTabStop', () => {
  it('makes the focused item the only tab stop', () => {
    const { element, items } = container(3)
    rovingTabStop(element, '[data-item]', items[1] ?? null)
    expect(items.map((item) => item.tabIndex)).toEqual([-1, 0, -1])
  })

  it('makes the first item the tab stop when the focused one is gone', () => {
    const { element, items } = container(3)
    rovingTabStop(element, '[data-item]', document.createElement('button'))
    expect(items.map((item) => item.tabIndex)).toEqual([0, -1, -1])
    rovingTabStop(element, '[data-item]', null)
    expect(items.map((item) => item.tabIndex)).toEqual([0, -1, -1])
  })

  it('leaves elements the selector does not match alone', () => {
    const { element } = container(2)
    rovingTabStop(element, '[data-item]', null)
    expect(element.lastElementChild).toHaveProperty('tabIndex', 0)
    expect(element.lastElementChild?.hasAttribute('tabindex')).toBe(false)
  })
})

describe('arrowTarget', () => {
  it('moves to the next and previous item', () => {
    for (const wrap of [true, false]) {
      expect(arrowTarget('ArrowRight', 1, 3, { wrap })).toBe(2)
      expect(arrowTarget('ArrowLeft', 1, 3, { wrap })).toBe(0)
    }
  })

  it('goes around the ends with wrap', () => {
    expect(arrowTarget('ArrowRight', 3, 3, { wrap: true })).toBe(0)
    expect(arrowTarget('ArrowLeft', 0, 3, { wrap: true })).toBe(3)
  })

  it('stops at the ends without wrap', () => {
    expect(arrowTarget('ArrowRight', 3, 3, { wrap: false })).toBe(3)
    expect(arrowTarget('ArrowLeft', 0, 3, { wrap: false })).toBe(0)
  })

  it('moves to the first item with Home and the last with End', () => {
    expect(arrowTarget('Home', 2, 3, { wrap: false })).toBe(0)
    expect(arrowTarget('End', 0, 3, { wrap: true })).toBe(3)
  })

  it('does not move for other keys', () => {
    expect(arrowTarget('ArrowDown', 1, 3, { wrap: true })).toBeNull()
    expect(arrowTarget('Enter', 1, 3, { wrap: false })).toBeNull()
  })
})
