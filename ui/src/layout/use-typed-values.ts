import { useEffect } from 'react'
import { commandValueField, setCommandField } from '../app/actions/command'
import type { App } from '../app/createApp'
import { fieldInputId } from '../inspector/fields/field-id'
import { isView } from './focus'

// Typing values (ui.md 3.7): while a feature command is active, a value typed with focus on the
// viewport or on a selection field box goes to the command's value field, so pick, type 2, Enter
// makes a 2 mm fillet. Letters keep their shortcuts.

/** The characters that start a typed value: digits, a decimal point, a minus sign, a parenthesis. */
const VALUE_START = /^[0-9.\-(]$/

/** The input of the active command's value field, or null without one. */
function valueInput(app: App): { readonly path: string; readonly input: HTMLInputElement } | null {
  const path = commandValueField(app.store.getState())
  const input = path === null ? null : document.getElementById(fieldInputId(path))
  return path === null || !(input instanceof HTMLInputElement) ? null : { path, input }
}

/**
 * Sends typed values to the command's value field: a digit, `.`, `-` or `(` without Mod, Ctrl or
 * Alt on the viewport or a selection field box replaces the field's text with that character, and
 * Tab on the viewport selects the field's text. Runs in the capture phase, before the viewport's
 * own keys (where `-` zooms) and the shortcut dispatcher.
 */
export function useTypedValues(app: App): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const target = event.target
      if (
        event.defaultPrevented ||
        event.isComposing ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        app.store.getState().command === null ||
        !(target instanceof Element)
      ) {
        return
      }
      const viewport = isView(target)
      if (event.key === 'Tab' && !event.shiftKey && viewport) {
        const field = valueInput(app)
        if (field !== null) {
          event.preventDefault()
          field.input.focus()
          field.input.select()
        }
        return
      }
      const selectionBox = target.closest('[data-selection-field]') !== null
      if (!VALUE_START.test(event.key) || !(viewport || selectionBox)) {
        return
      }
      const field = valueInput(app)
      if (field === null) {
        return
      }
      // The character goes into the field here, not through the default key action.
      event.preventDefault()
      field.input.focus()
      setCommandField(app, field.path, event.key, { typing: true })
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
    }
  }, [app])
}
