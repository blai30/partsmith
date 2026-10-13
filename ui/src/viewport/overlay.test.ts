import { describe, expect, it } from 'vitest'
import { orientElement, placeElement, positionElement, visiblePosition } from './overlay'

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
  const size = { width: 800, height: 600 }

  it('keeps a position on the canvas, edges included', () => {
    expect(visiblePosition({ x: 0, y: 600 }, size)).toEqual({ x: 0, y: 600 })
    expect(visiblePosition({ x: 800, y: 0 }, size)).toEqual({ x: 800, y: 0 })
  })

  it('is null behind the camera, off the canvas or for a position that is not a number', () => {
    expect(visiblePosition(null, size)).toBeNull()
    expect(visiblePosition({ x: -0.5, y: 10 }, size)).toBeNull()
    expect(visiblePosition({ x: 10, y: 600.5 }, size)).toBeNull()
    expect(visiblePosition({ x: Number.NaN, y: 10 }, size)).toBeNull()
  })
})
