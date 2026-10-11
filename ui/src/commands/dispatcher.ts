import type { App } from '../app/createApp'
import { chordFromEvent, chordToString } from './shortcuts'

/** Chords native text editing keeps inside editable targets. */
const NATIVE_EDITING_CHORDS: ReadonlySet<string> = new Set([
  'Mod+Z',
  'Mod+Shift+Z',
  'Mod+Y',
  'Mod+A',
  'Mod+C',
  'Mod+X',
  'Mod+V',
])

/** The one chord considered while the palette, a menu or a popover is open (it closes the palette). */
const PALETTE_CHORD = 'Mod+K'

/** Input types that take no typed text, so their keys stay shortcuts. */
const NON_TEXT_INPUT_TYPES: ReadonlySet<string> = new Set([
  'button',
  'checkbox',
  'color',
  'file',
  'hidden',
  'image',
  'radio',
  'range',
  'reset',
  'submit',
])

/** An input of a text-like type, a textarea, a select, or content-editable element. */
function isEditable(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) {
    return !NON_TEXT_INPUT_TYPES.has(target.type)
  }
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return true
  }
  return target instanceof HTMLElement && target.isContentEditable
}

/** Whether a menu or a popover (light-dismiss popovers; tooltips are manual) is open. */
function hasOpenPopover(document: Document): boolean {
  return document.querySelector('[popover=""]:popover-open, [popover="auto"]:popover-open') !== null
}

/** Runs registry commands for key presses on the window (ui.md 2.10 dispatch rules); returns the uninstaller. */
export function installShortcutDispatcher(app: App, target: Window): () => void {
  function onKeyDown(event: KeyboardEvent): void {
    // Components that handle a key themselves prevent its default.
    if (event.defaultPrevented || event.isComposing || event.repeat) {
      return
    }
    const chord = chordFromEvent(event, app.platform)
    if (chord === null) {
      return
    }
    const name = chordToString(chord)
    if (isEditable(event.target) && (!chord.primaryModifier || NATIVE_EDITING_CHORDS.has(name))) {
      return
    }
    const state = app.store.getState()
    if (
      (state.panels.paletteOpen || hasOpenPopover(target.document)) &&
      name !== PALETTE_CHORD
    ) {
      return
    }
    const command = app.commands.forChord(app, name, state.context)
    if (command === undefined) {
      return
    }
    event.preventDefault()
    app.commands.run(app, command.id, 'shortcut').catch((error: unknown) => {
      console.error(`The command ${command.id} failed.`, error)
    })
  }
  target.addEventListener('keydown', onKeyDown)
  return () => {
    target.removeEventListener('keydown', onKeyDown)
  }
}
