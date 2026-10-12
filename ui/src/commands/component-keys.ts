import type { Platform } from '../app/createApp'

// Keys that components handle themselves, not the command registry (ui.md 3.5), as the data the
// keyboard and mouse panel renders. `Mod` stands for the platform's primary modifier.

/** A key, where it works and what it does, in the words the panel shows. */
export type ComponentKey = { readonly key: string; readonly place: string; readonly effect: string }

/** The component-key table of 3.5, in table order. */
export const COMPONENT_KEYS: readonly ComponentKey[] = [
  {
    key: 'Esc',
    place: 'Anywhere',
    effect:
      'Step back one level: close a menu or list, cancel a drawing, leave the tool, cancel the command, then clear the selection',
  },
  {
    key: 'Enter',
    place: 'Fields, inline editors, the line tool, the view',
    effect:
      'Commit the field or editor, end a line, confirm a complete command, or repeat the last command on the selection',
  },
  {
    key: 'Arrow keys',
    place: 'The view',
    effect:
      "Orbit 15 degrees per press: Left and Right about the vertical axis, Up and Down about the view's horizontal axis",
  },
  { key: 'Shift+arrow keys', place: 'The view', effect: "Pan by a tenth of the view's height" },
  {
    key: '= or +, -',
    place: 'The view',
    effect:
      'Zoom in or out at the center; during a command or while drawing a shape, - starts a negative value instead',
  },
  {
    key: 'Digit, ., -, (',
    place: 'The view or a selection field during a command',
    effect: "Type into the command's value field",
  },
  {
    key: 'Tab',
    place: 'The view during a command',
    effect: "Select the command's value to type over it",
  },
  {
    key: 'Digit, ., -, (',
    place: 'A sketch while drawing a shape',
    effect: "Type into the shape's first size input",
  },
  {
    key: 'Arrow keys, Home, End',
    place: 'Toolbars, timeline, browser, menus, segmented controls',
    effect: 'Move focus',
  },
  {
    key: 'Shift+F10, ContextMenu',
    place: 'Timeline item, browser row, the view',
    effect: 'Open the context menu',
  },
  { key: 'Space', place: 'Timeline item, browser row', effect: 'Select; with Mod, add or remove' },
  { key: 'Tab', place: 'Sketch size inputs', effect: 'Move to the next input' },
  {
    key: 'Hold Mod',
    place: 'A sketch while drawing',
    effect: 'Place freely, without snapping or automatic constraints',
  },
  {
    key: 'Shift or Mod with click',
    place: 'The view, timeline, browser',
    effect: 'Add to or remove from the selection',
  },
]

/** The keys of typed values: expression fields (2.13), sketch size inputs (3.16.4) and commands (3.7). */
export const TYPING_KEYS: readonly ComponentKey[] = [
  {
    key: 'ArrowUp, ArrowDown',
    place: 'Size and angle fields',
    effect: 'Step a plain number; with Shift ten times as far, with Alt a tenth',
  },
  {
    key: 'Tab',
    place: 'Size and angle fields while names are listed',
    effect: 'Complete the parameter name',
  },
  {
    key: 'Digit, ., -, (',
    place: 'The view during a command',
    effect: 'Type the value, then Enter confirms: pick an edge, type 2, Enter makes a 2 mm fillet',
  },
  {
    key: 'Digit, ., -, (',
    place: 'A sketch while drawing a shape',
    effect: 'Type its size; Tab moves to the next size and Enter places the shape',
  },
  { key: 'Esc', place: 'Sketch size inputs', effect: 'Clear the typed sizes' },
]

/** The line under the typing keys. */
export const TYPING_NOTE =
  'Any size or angle field takes a number, a value with a unit such as 1 in, or a formula with parameter names.'

/** A key or effect text with `Mod` written as the platform's key: `⌘` on macOS, else `Ctrl`. */
export function withPlatformKeys(text: string, platform: Platform): string {
  return text.replaceAll(/\bMod\b/g, platform.operatingSystem === 'mac' ? '⌘' : 'Ctrl')
}
