import { describe, expect, it } from 'vitest'
import type { Platform } from '../app/createApp'
import { chordFromEvent, chordToString, formatShortcut, parseShortcut } from './shortcuts'

const MAC: Platform = { operatingSystem: 'mac', host: 'web' }
const LINUX: Platform = { operatingSystem: 'linux', host: 'web' }
const WINDOWS: Platform = { operatingSystem: 'windows', host: 'desktop' }

function keydown(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init })
}

function chordString(init: KeyboardEventInit, platform: Platform): string | null {
  const chord = chordFromEvent(keydown(init), platform)
  return chord === null ? null : chordToString(chord)
}

describe('formatShortcut', () => {
  it('U-21 Mod+Shift+S is ⇧⌘S on macOS', () => {
    expect(formatShortcut('Mod+Shift+S', MAC)).toBe('⇧⌘S')
  })

  it('U-21 Mod+Shift+S is Ctrl+Shift+S on Linux', () => {
    expect(formatShortcut('Mod+Shift+S', LINUX)).toBe('Ctrl+Shift+S')
  })

  it('U-21 Mod+Enter is ⌘↩ on macOS', () => {
    expect(formatShortcut('Mod+Enter', MAC)).toBe('⌘↩')
  })

  it('U-21 Mod+Enter is Ctrl+Enter on Linux', () => {
    expect(formatShortcut('Mod+Enter', LINUX)).toBe('Ctrl+Enter')
  })

  it('U-21 Delete is ⌦ on macOS', () => {
    expect(formatShortcut('Delete', MAC)).toBe('⌦')
  })

  it('U-21 Delete is Del on Linux', () => {
    expect(formatShortcut('Delete', LINUX)).toBe('Del')
  })

  it('U-21 F6 is F6 on macOS and Linux', () => {
    expect(formatShortcut('F6', MAC)).toBe('F6')
    expect(formatShortcut('F6', LINUX)).toBe('F6')
  })

  it('formats Windows like Linux', () => {
    expect(formatShortcut('Mod+Shift+S', WINDOWS)).toBe('Ctrl+Shift+S')
    expect(formatShortcut('Delete', WINDOWS)).toBe('Del')
  })

  it('orders macOS modifier symbols ⌃ ⌥ ⇧ ⌘ whatever the spec order', () => {
    expect(formatShortcut('Mod+Alt+Shift+S', MAC)).toBe('⌥⇧⌘S')
    expect(formatShortcut('Ctrl+Alt+Shift+S', MAC)).toBe('⌃⌥⇧S')
  })

  it('writes Ctrl as ⌃ on macOS and Ctrl elsewhere', () => {
    expect(formatShortcut('Ctrl+B', MAC)).toBe('⌃B')
    expect(formatShortcut('Ctrl+B', LINUX)).toBe('Ctrl+B')
  })

  it('writes Alt as words joined by + outside macOS', () => {
    expect(formatShortcut('Mod+Alt+S', LINUX)).toBe('Ctrl+Alt+S')
  })

  it.each([
    ['Backspace', '⌫', 'Backspace'],
    ['Escape', 'Esc', 'Esc'],
    ['Space', 'Space', 'Space'],
    ['Enter', '↩', 'Enter'],
    ['ArrowUp', '↑', '↑'],
    ['ArrowDown', '↓', '↓'],
    ['ArrowLeft', '←', '←'],
    ['ArrowRight', '→', '→'],
    ['K', 'K', 'K'],
    ['7', '7', '7'],
    ['F12', 'F12', 'F12'],
    ['=', '=', '='],
    ['-', '-', '-'],
  ])('writes the key %s as %s on macOS and %s elsewhere', (key, mac, linux) => {
    expect(formatShortcut(key, MAC)).toBe(mac)
    expect(formatShortcut(key, LINUX)).toBe(linux)
  })

  it('writes Shift+F as ⇧F on macOS and Shift+F elsewhere', () => {
    expect(formatShortcut('Shift+F', MAC)).toBe('⇧F')
    expect(formatShortcut('Shift+F', LINUX)).toBe('Shift+F')
  })

  it('writes Mod+K as ⌘K on macOS and Ctrl+K elsewhere', () => {
    expect(formatShortcut('Mod+K', MAC)).toBe('⌘K')
    expect(formatShortcut('Mod+K', WINDOWS)).toBe('Ctrl+K')
  })
})

describe('parseShortcut', () => {
  it.each(['Shift+', 'Shift+Mod+S', 'Mod+Mod+S', 'Mod+Ctrl+S'])(
    'U-21 throws Invalid shortcut for %s',
    (shortcut) => {
      expect(() => parseShortcut(shortcut)).toThrow(new Error(`Invalid shortcut "${shortcut}".`))
    },
  )

  it.each(['', 'Mod+s', 'F13', 'Mod+Plus', 'Hyper+S', 'Alt+Mod+S', 'Shift+Alt+S', 'Mod+S+K'])(
    'throws Invalid shortcut for %j',
    (shortcut) => {
      expect(() => parseShortcut(shortcut)).toThrow(new Error(`Invalid shortcut "${shortcut}".`))
    },
  )

  it('parses Mod+Shift+S', () => {
    expect(parseShortcut('Mod+Shift+S')).toEqual({
      primaryModifier: true,
      control: false,
      alt: false,
      shift: true,
      key: 'S',
    })
  })

  it('parses Ctrl+Alt+Delete', () => {
    expect(parseShortcut('Ctrl+Alt+Delete')).toEqual({
      primaryModifier: false,
      control: true,
      alt: true,
      shift: false,
      key: 'Delete',
    })
  })

  it('parses a bare key', () => {
    expect(parseShortcut('E')).toEqual({
      primaryModifier: false,
      control: false,
      alt: false,
      shift: false,
      key: 'E',
    })
  })

  it.each([
    'Mod+K',
    'S',
    'Mod+Z',
    'Mod+Shift+Z',
    'Mod+Y',
    'Mod+Shift+S',
    'Delete',
    'Backspace',
    'F2',
    'F6',
    'Shift+F',
    'Mod+Enter',
    'Mod+Alt+Shift+0',
    'Ctrl+Space',
    'Escape',
    'Tab',
    'Home',
    'End',
    'ArrowLeft',
    'Mod+=',
    'Mod+-',
  ])('chordToString(parseShortcut(%s)) gives the shortcut back', (shortcut) => {
    expect(chordToString(parseShortcut(shortcut))).toBe(shortcut)
  })
})

describe('chordFromEvent', () => {
  it('U-23 {key: s, metaKey, shiftKey} on macOS gives Mod+Shift+S', () => {
    expect(chordString({ key: 's', metaKey: true, shiftKey: true }, MAC)).toBe('Mod+Shift+S')
  })

  it('U-23 {key: s, ctrlKey, shiftKey} on Linux gives Mod+Shift+S', () => {
    expect(chordString({ key: 's', ctrlKey: true, shiftKey: true }, LINUX)).toBe('Mod+Shift+S')
  })

  it('U-23 {key: д, code: KeyL} gives L', () => {
    expect(chordString({ key: 'д', code: 'KeyL' }, LINUX)).toBe('L')
  })

  it('U-23 a lone Shift gives null', () => {
    expect(
      chordFromEvent(keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true }), LINUX),
    ).toBe(null)
  })

  it('U-23 {key: " "} gives Space', () => {
    expect(chordString({ key: ' ', code: 'Space' }, LINUX)).toBe('Space')
  })

  it.each([
    { key: 'Control', ctrlKey: true },
    { key: 'Alt', altKey: true },
    { key: 'Meta', metaKey: true },
  ])('a lone $key press gives null', (init) => {
    expect(chordFromEvent(keydown(init), MAC)).toBe(null)
    expect(chordFromEvent(keydown(init), LINUX)).toBe(null)
  })

  it('gives the normalized chord object on macOS: Cmd is the primary modifier and Ctrl is control', () => {
    expect(chordFromEvent(keydown({ key: 'b', metaKey: true, ctrlKey: true }), MAC)).toEqual({
      primaryModifier: true,
      control: true,
      alt: false,
      shift: false,
      key: 'B',
    })
  })

  it('gives Ctrl+B for the Control key on macOS', () => {
    expect(chordString({ key: 'b', ctrlKey: true }, MAC)).toBe('Ctrl+B')
  })

  it('never sets control outside macOS', () => {
    const chord = chordFromEvent(keydown({ key: 'b', ctrlKey: true }), WINDOWS)
    expect(chord).toEqual({
      primaryModifier: true,
      control: false,
      alt: false,
      shift: false,
      key: 'B',
    })
  })

  it('uppercases a single letter key', () => {
    expect(chordString({ key: 'e', code: 'KeyE' }, LINUX)).toBe('E')
  })

  it('uses the letter of event.code when Alt changes the character on macOS', () => {
    expect(chordString({ key: '¬', code: 'KeyL', altKey: true }, MAC)).toBe('Alt+L')
  })

  it('keeps named keys as event.key gives them', () => {
    expect(chordString({ key: 'Enter', code: 'Enter', metaKey: true }, MAC)).toBe('Mod+Enter')
    expect(chordString({ key: 'Delete', code: 'Delete' }, LINUX)).toBe('Delete')
    expect(chordString({ key: 'F2', code: 'F2' }, LINUX)).toBe('F2')
    expect(chordString({ key: 'ArrowUp', code: 'ArrowUp', shiftKey: true }, LINUX)).toBe(
      'Shift+ArrowUp',
    )
  })

  it('keeps digits as typed', () => {
    expect(chordString({ key: '1', code: 'Digit1', ctrlKey: true }, LINUX)).toBe('Mod+1')
  })
})
