import type { Platform } from '../app/createApp'

/** A parsed chord. `key` is normalized (ui.md 2.10). */
export type Chord = {
  readonly primaryModifier: boolean
  readonly control: boolean
  readonly alt: boolean
  readonly shift: boolean
  readonly key: string
}

type Modifier = 'Mod' | 'Ctrl' | 'Alt' | 'Shift'

/** Modifiers in the only order a shortcut may name them. */
const MODIFIER_ORDER: readonly Modifier[] = ['Mod', 'Ctrl', 'Alt', 'Shift']

const NAMED_KEYS: ReadonlySet<string> = new Set([
  'Enter',
  'Escape',
  'Delete',
  'Backspace',
  'Tab',
  'Space',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Home',
  'End',
  '=',
  '-',
])

const SINGLE_CHARACTER_KEY = /^[A-Z0-9]$/
const FUNCTION_KEY = /^F(?:[1-9]|1[0-2])$/

/** `event.key` values of modifier keys pressed on their own (UI Events key values). */
const MODIFIER_KEYS: ReadonlySet<string> = new Set([
  'Alt',
  'AltGraph',
  'CapsLock',
  'Control',
  'Fn',
  'FnLock',
  'Hyper',
  'Meta',
  'NumLock',
  'OS',
  'ScrollLock',
  'Shift',
  'Super',
  'Symbol',
  'SymbolLock',
])

const MAC_KEY_LABELS: Readonly<Record<string, string>> = {
  Enter: '↩',
  Delete: '⌦',
  Backspace: '⌫',
  Escape: 'Esc',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
}

const OTHER_KEY_LABELS: Readonly<Record<string, string>> = {
  Delete: 'Del',
  Escape: 'Esc',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
}

function isKey(key: string): boolean {
  return SINGLE_CHARACTER_KEY.test(key) || FUNCTION_KEY.test(key) || NAMED_KEYS.has(key)
}

function isModifier(part: string): part is Modifier {
  return MODIFIER_ORDER.some((modifier) => modifier === part)
}

/** Parses a shortcut spec such as `Mod+Shift+S`; throws for anything outside the grammar of ui.md 2.10. */
export function parseShortcut(shortcut: string): Chord {
  const invalid = (): never => {
    throw new Error(`Invalid shortcut "${shortcut}".`)
  }
  const parts = shortcut.split('+')
  const key = parts.pop() ?? ''
  if (!isKey(key)) {
    invalid()
  }
  let previous = -1
  for (const part of parts) {
    const position = isModifier(part) ? MODIFIER_ORDER.indexOf(part) : -1
    // Unknown, repeated or out-of-order modifiers are all a position not after the previous one.
    if (position <= previous) {
      invalid()
    }
    previous = position
  }
  const primaryModifier = parts.includes('Mod')
  const control = parts.includes('Ctrl')
  if (primaryModifier && control) {
    invalid()
  }
  return {
    primaryModifier,
    control,
    alt: parts.includes('Alt'),
    shift: parts.includes('Shift'),
    key,
  }
}

/** The canonical shortcut text of a chord, such as `Mod+Shift+S`. */
export function chordToString(chord: Chord): string {
  const parts: string[] = []
  if (chord.primaryModifier) {
    parts.push('Mod')
  }
  if (chord.control) {
    parts.push('Ctrl')
  }
  if (chord.alt) {
    parts.push('Alt')
  }
  if (chord.shift) {
    parts.push('Shift')
  }
  parts.push(chord.key)
  return parts.join('+')
}

/** A shortcut as the platform writes it: `⇧⌘S` on macOS, `Ctrl+Shift+S` elsewhere. */
export function formatShortcut(shortcut: string, platform: Platform): string {
  const chord = parseShortcut(shortcut)
  if (platform.operatingSystem === 'mac') {
    // Apple's order: Control, Option, Shift, Command, then the key, with no separators.
    return [
      chord.control ? '⌃' : '',
      chord.alt ? '⌥' : '',
      chord.shift ? '⇧' : '',
      chord.primaryModifier ? '⌘' : '',
      MAC_KEY_LABELS[chord.key] ?? chord.key,
    ].join('')
  }
  const parts: string[] = []
  if (chord.primaryModifier || chord.control) {
    parts.push('Ctrl')
  }
  if (chord.alt) {
    parts.push('Alt')
  }
  if (chord.shift) {
    parts.push('Shift')
  }
  parts.push(OTHER_KEY_LABELS[chord.key] ?? chord.key)
  return parts.join('+')
}

const ASCII_LETTER = /^[a-z]$/i
const LETTER_CODE = /^Key([A-Z])$/
const SINGLE_LETTER = /^\p{L}$/u

function normalizedKey(event: KeyboardEvent): string {
  if (ASCII_LETTER.test(event.key)) {
    return event.key.toUpperCase()
  }
  // Non-Latin layouts and macOS Option combinations: the physical letter key.
  const letter = LETTER_CODE.exec(event.code)?.[1]
  if (letter !== undefined) {
    return letter
  }
  if (event.key === ' ') {
    return 'Space'
  }
  return SINGLE_LETTER.test(event.key) ? event.key.toLocaleUpperCase() : event.key
}

/** The chord of a key press, or null for a lone modifier press. */
export function chordFromEvent(event: KeyboardEvent, platform: Platform): Chord | null {
  if (MODIFIER_KEYS.has(event.key)) {
    return null
  }
  const mac = platform.operatingSystem === 'mac'
  return {
    primaryModifier: mac ? event.metaKey : event.ctrlKey,
    control: mac && event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    key: normalizedKey(event),
  }
}
