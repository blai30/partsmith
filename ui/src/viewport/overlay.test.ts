import { describe, expect, it } from 'vitest'
import {
  hideLabelsUnderCards,
  orientElement,
  placeElement,
  positionElement,
  visiblePosition,
} from './overlay'

// The declarations of the element's style attribute, read through the attribute so this file has no style property access.
function declarations(element: HTMLElement): string[] {
  return (element.getAttribute('style') ?? '')
    .split(';')
    .map((declaration) => declaration.trim().replaceAll(/\s+/g, ' '))
    .filter((declaration) => declaration !== '')
}

describe('positionElement', () => {
  it('U-91 positionElement(element, 12.5, 40) leaves a style attribute that declares only translate: 12.5px 40px and sets data-positioned', () => {
    const element = document.createElement('div')
    positionElement(element, 12.5, 40)
    expect(declarations(element)).toEqual(['translate: 12.5px 40px'])
    expect(element.hasAttribute('data-positioned')).toBe(true)
  })

  it('replaces the earlier position', () => {
    const element = document.createElement('div')
    positionElement(element, 1, 2)
    positionElement(element, -3.25, 7)
    expect(declarations(element)).toEqual(['translate: -3.25px 7px'])
  })

  it('leaves the classes and other attributes alone', () => {
    const element = document.createElement('div')
    element.className = 'absolute left-0 top-0'
    element.dataset.testid = 'sketch-dimension'
    positionElement(element, 5, 6)
    expect(element.className).toBe('absolute left-0 top-0')
    expect(element.dataset.testid).toBe('sketch-dimension')
  })
})

describe('orientElement', () => {
  it('leaves a style attribute that declares only the matrix3d transform of the column-major matrix and sets data-positioned', () => {
    const element = document.createElement('div')
    const matrix = new Float32Array([1, 0.5, 0, 0, -0.5, 1, 0, 0, 0, 0, 1, 0, 13.5, -14, 15, 1])
    orientElement(element, matrix)
    const declared = declarations(element)
    expect(declared).toHaveLength(1)
    const match = /^transform: matrix3d\((.*)\)$/.exec(declared[0] ?? '')
    expect(match).not.toBeNull()
    const values = (match?.[1] ?? '').split(',').map((value) => Number(value.trim()))
    expect(values).toEqual(Array.from(matrix))
    expect(element.hasAttribute('data-positioned')).toBe(true)
  })

  it('replaces the earlier orientation', () => {
    const element = document.createElement('div')
    orientElement(element, new Float32Array([2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 1]))
    const identity = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
    orientElement(element, identity)
    const declared = declarations(element)
    expect(declared).toHaveLength(1)
    const values = (/matrix3d\((.*)\)/.exec(declared[0] ?? '')?.[1] ?? '').split(',').map(Number)
    expect(values).toEqual(Array.from(identity))
  })
})

describe('placeElement', () => {
  it('positions an element that has a position and shows it', () => {
    const element = document.createElement('div')
    element.hidden = true
    placeElement(element, { x: 3, y: 4 })
    expect(element.hidden).toBe(false)
    expect(declarations(element)).toEqual(['translate: 3px 4px'])
  })

  it('hides an element without a position and keeps its last position', () => {
    const element = document.createElement('div')
    placeElement(element, { x: 3, y: 4 })
    placeElement(element, null)
    expect(element.hidden).toBe(true)
    expect(declarations(element)).toEqual(['translate: 3px 4px'])
  })
})

describe('visiblePosition', () => {
  const area = { x: 280, y: 64, width: 640, height: 600 }

  it('keeps a position in the free area, edges included', () => {
    expect(visiblePosition({ x: 280, y: 664 }, area)).toEqual({ x: 280, y: 664 })
    expect(visiblePosition({ x: 920, y: 64 }, area)).toEqual({ x: 920, y: 64 })
  })

  it('is null behind the camera, outside the free area or for a position that is not a number (3.12)', () => {
    expect(visiblePosition(null, area)).toBeNull()
    expect(visiblePosition({ x: 100, y: 300 }, area)).toBeNull()
    expect(visiblePosition({ x: 279.5, y: 300 }, area)).toBeNull()
    expect(visiblePosition({ x: 600, y: 664.5 }, area)).toBeNull()
    expect(visiblePosition({ x: Number.NaN, y: 300 }, area)).toBeNull()
  })
})

function withBounds(element: Element, x: number, y: number, width: number, height: number): void {
  element.getBoundingClientRect = () => DOMRect.fromRect({ x, y, width, height })
}

/** A free area with one card at (100, 100, 200, 80). */
function freeAreaWithCard(): HTMLElement {
  const area = document.createElement('main')
  const card = document.createElement('div')
  card.setAttribute('data-card', '')
  withBounds(card, 100, 100, 200, 80)
  area.append(card)
  return area
}

/** A 16 by 16 label at (x, y). */
function label(x: number, y: number): HTMLElement {
  const element = document.createElement('span')
  withBounds(element, x, y, 16, 16)
  return element
}

describe('hideLabelsUnderCards', () => {
  it('U-214 hides a shown label whose box meets a card of the free area and keeps the others (3.12)', () => {
    const under = label(290, 170)
    const beside = label(300, 100)
    const clear = label(20, 20)
    hideLabelsUnderCards([under, beside, clear], freeAreaWithCard())
    expect(under.hidden).toBe(true)
    // Touching an edge is not meeting.
    expect(beside.hidden).toBe(false)
    expect(clear.hidden).toBe(false)
  })

  it('U-214 measures the chip of a wrapper centered on its anchor', () => {
    const wrapper = document.createElement('div')
    withBounds(wrapper, 20, 20, 16, 16)
    const chip = document.createElement('span')
    withBounds(chip, 120, 120, 16, 16)
    wrapper.append(chip)
    hideLabelsUnderCards([wrapper], freeAreaWithCard())
    expect(wrapper.hidden).toBe(true)
  })

  it('U-214 leaves labels alone without a free area or a card', () => {
    const under = label(150, 150)
    hideLabelsUnderCards([under], null)
    expect(under.hidden).toBe(false)
    hideLabelsUnderCards([under], document.createElement('main'))
    expect(under.hidden).toBe(false)
  })
})
