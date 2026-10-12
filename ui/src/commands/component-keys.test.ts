import { describe, expect, it } from 'vitest'
import { LINUX_WEB, MAC_WEB } from '../test/fakes/app'
import { COMPONENT_KEYS, TYPING_KEYS, TYPING_NOTE, withPlatformKeys } from './component-keys'

// The component-key table of ui.md 3.5 as data: Key, Where, Effect.
const TABLE: readonly (readonly [string, string, string])[] = [
  [
    'Esc',
    'Anywhere',
    'Step back one level: close a menu or list, cancel a drawing, leave the tool, cancel the command, then clear the selection',
  ],
  [
    'Enter',
    'Fields, inline editors, the line tool, the view',
    'Commit the field or editor, end a line, confirm a complete command, or repeat the last command on the selection',
  ],
  [
    'Arrow keys',
    'The view',
    "Orbit 15 degrees per press: Left and Right about the vertical axis, Up and Down about the view's horizontal axis",
  ],
  ['Shift+arrow keys', 'The view', "Pan by a tenth of the view's height"],
  [
    '= or +, -',
    'The view',
    'Zoom in or out at the center; during a command or while drawing a shape, - starts a negative value instead',
  ],
  [
    'Digit, ., -, (',
    'The view or a selection field during a command',
    "Type into the command's value field",
  ],
  ['Tab', 'The view during a command', "Select the command's value to type over it"],
  ['Digit, ., -, (', 'A sketch while drawing a shape', "Type into the shape's first size input"],
  ['Arrow keys, Home, End', 'Toolbars, timeline, browser, menus, segmented controls', 'Move focus'],
  ['Shift+F10, ContextMenu', 'Timeline item, browser row, the view', 'Open the context menu'],
  ['Space', 'Timeline item, browser row', 'Select; with Mod, add or remove'],
  ['Tab', 'Sketch size inputs', 'Move to the next input'],
  ['Hold Mod', 'A sketch while drawing', 'Place freely, without snapping or automatic constraints'],
  ['Shift or Mod with click', 'The view, timeline, browser', 'Add to or remove from the selection'],
]

describe('COMPONENT_KEYS', () => {
  it('U-125 has one entry per row of the 3.5 component-key table', () => {
    expect(COMPONENT_KEYS).toHaveLength(TABLE.length)
  })

  it.each(TABLE.map(([key, place, effect], index) => ({ index, key, place, effect })))(
    'U-125 row $index is "$key" in "$place"',
    ({ index, key, place, effect }) => {
      expect(COMPONENT_KEYS[index]).toEqual({ key, place, effect })
    },
  )
})

describe('TYPING_KEYS', () => {
  it('covers stepping and completing in expression fields, typed values and sketch size inputs', () => {
    expect(TYPING_KEYS.map((row) => row.key)).toEqual([
      'ArrowUp, ArrowDown',
      'Tab',
      'Digit, ., -, (',
      'Digit, ., -, (',
      'Esc',
    ])
    expect(TYPING_NOTE).toBe(
      'Any size or angle field takes a number, a value with a unit such as 1 in, or a formula with parameter names.',
    )
  })
})

describe('withPlatformKeys', () => {
  it('writes Mod as Ctrl outside macOS and as ⌘ on macOS', () => {
    expect(withPlatformKeys('Shift or Mod with click', LINUX_WEB)).toBe('Shift or Ctrl with click')
    expect(withPlatformKeys('Hold Mod', MAC_WEB)).toBe('Hold ⌘')
  })

  it('leaves words that only contain Mod alone', () => {
    expect(withPlatformKeys('Model', LINUX_WEB)).toBe('Model')
  })
})
