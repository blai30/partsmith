import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { focusOnPointerMove, nextIndex, typeaheadIndex } from './menu-navigation'

describe('nextIndex', () => {
  it('moves one item forward or back', () => {
    expect(nextIndex(4, 1, 1)).toBe(2)
    expect(nextIndex(4, 1, -1)).toBe(0)
  })

  it('wraps around both ends', () => {
    expect(nextIndex(4, 3, 1)).toBe(0)
    expect(nextIndex(4, 0, -1)).toBe(3)
  })

  it('starts before the first item at -1 and after the last at the count', () => {
    expect(nextIndex(4, -1, 1)).toBe(0)
    expect(nextIndex(4, 4, -1)).toBe(3)
  })

  it('skips items that do not take focus', () => {
    const separators = new Set([1, 2])
    expect(nextIndex(4, 0, 1, (index) => !separators.has(index))).toBe(3)
    expect(nextIndex(4, 3, -1, (index) => !separators.has(index))).toBe(0)
  })

  it('keeps the start when no item takes focus', () => {
    expect(nextIndex(3, 1, 1, () => false)).toBe(1)
    expect(nextIndex(0, -1, 1)).toBe(-1)
  })
})

describe('typeaheadIndex', () => {
  const labels = ['New', null, 'Open', 'Open recent', 'Save']

  it('finds the next label starting with the character, ignoring case', () => {
    expect(typeaheadIndex(labels, -1, 'o')).toBe(2)
    expect(typeaheadIndex(labels, 2, 'O')).toBe(3)
  })

  it('wraps around past the last label', () => {
    expect(typeaheadIndex(labels, 3, 'n')).toBe(0)
  })

  it('returns null when no label matches', () => {
    expect(typeaheadIndex(labels, 0, 'x')).toBeNull()
  })
})

// A menu item that takes focus as the pointer moves over it.
function Item() {
  return (
    <button type="button" onPointerMove={focusOnPointerMove}>
      Item
    </button>
  )
}

describe('focusOnPointerMove', () => {
  it('focuses the item under a mouse', () => {
    render(<Item />)
    fireEvent.pointerMove(screen.getByRole('button'), { pointerType: 'mouse' })
    expect(screen.getByRole('button')).toHaveFocus()
  })

  it('leaves focus where it is for touch', () => {
    render(<Item />)
    fireEvent.pointerMove(screen.getByRole('button'), { pointerType: 'touch' })
    expect(screen.getByRole('button')).not.toHaveFocus()
  })
})
