import { useEffect } from 'react'
import { cancelCommand, confirmCommand } from '../app/actions/command'
import { clearSelection } from '../app/actions/selection'
import { finishSketch, setSketchTool } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import { buildInspectorModel } from '../inspector/model'
import type { AppState } from '../state/store'
import { isNarrowWindow } from './use-overlay-dismiss'

// The Esc and Enter ladders (ui.md 3.5): what the two keys do when no focused control takes them.

/**
 * One step of the Esc ladder after the open layers, which close themselves (3.5): exit the sketch
 * tool, cancel the command, cancel a new sketch's plane pick, then clear the selection. Esc never
 * leaves sketch mode once a sketch is being edited. Returns whether a step applied.
 */
export function escapeStep(app: App): boolean {
  const state = app.store.getState()
  const sketch = state.context === 'sketch' ? state.sketch : null
  if (sketch !== null) {
    if (sketch.sketchId === null) {
      void finishSketch(app)
      return true
    }
    if (sketch.tool !== null) {
      setSketchTool(app, null)
      return true
    }
    if (sketch.selection.length > 0) {
      app.store.setState((current) =>
        current.sketch === null ? {} : { sketch: { ...current.sketch, selection: [] } },
      )
      return true
    }
    return false
  }
  if (state.command !== null) {
    cancelCommand(app).catch((error: unknown) => {
      console.error('Canceling the command failed.', error)
    })
    return true
  }
  if (state.selection.length > 0) {
    clearSelection(app)
    return true
  }
  return false
}

/**
 * Runs the Esc ladder for presses no open layer took. Menus, popovers, the palette and inline
 * editors close on Esc themselves: they either prevent the key's default or are popovers the
 * browser light-dismisses, which are tracked through their toggle events.
 */
export function useEscapeLadder(app: App): void {
  useEffect(() => {
    const openLayers = new Set<Element>()
    let layerOpenAtPress = false
    const onToggle = (event: Event): void => {
      const target = event.target
      // Tooltips are manual popovers: they never take an Esc press from the ladder.
      if (!(target instanceof Element) || target.getAttribute('popover') === 'manual') {
        return
      }
      const newState: unknown = Reflect.get(event, 'newState')
      if (newState === 'open') {
        openLayers.add(target)
      } else if (newState === 'closed') {
        openLayers.delete(target)
      }
    }
    // The capture phase runs before the browser or a popover closes the layer for this press.
    const onKeyDownCapture = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') {
        return
      }
      for (const layer of openLayers) {
        if (!layer.isConnected) {
          openLayers.delete(layer)
        }
      }
      layerOpenAtPress = openLayers.size > 0
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing) {
        return
      }
      const { panels } = app.store.getState()
      // An overlay panel in a narrow window closes on this press itself, whenever it registered.
      const overlayOpen = isNarrowWindow() && (panels.browserOpen || panels.side !== null)
      if (layerOpenAtPress || panels.paletteOpen || overlayOpen) {
        return
      }
      if (escapeStep(app)) {
        event.preventDefault()
      }
    }
    document.addEventListener('toggle', onToggle, true)
    window.addEventListener('keydown', onKeyDownCapture, true)
    window.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('toggle', onToggle, true)
      window.removeEventListener('keydown', onKeyDownCapture, true)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [app])
}

/** Whether the active feature command can be confirmed: every required field set, no field error. */
export function commandComplete(state: AppState): boolean {
  const command = state.command
  const feature = command === null ? undefined : state.catalog?.byType[command.featureType]
  return (
    command !== null &&
    feature !== undefined &&
    buildInspectorModel({ feature, params: command.params, errors: command.fieldErrors }).complete
  )
}

/** The Enter ladder's last step (3.5): confirms the active feature command when it is complete. Returns whether it did. */
export function confirmIfComplete(app: App): boolean {
  if (!commandComplete(app.store.getState())) {
    return false
  }
  confirmCommand(app).catch((error: unknown) => {
    console.error('Confirming the command failed.', error)
  })
  return true
}

/** Whether Enter on the element belongs to the element itself (a button activates, a list opens). */
function handlesOwnEnter(element: Element): boolean {
  return (
    element.closest(
      'button, a, select, textarea, [role="button"], [role="menu"], [role="listbox"]',
    ) !== null
  )
}

/**
 * The last step of the Enter ladder (3.5): with focus in the inspector (not on a button) or on the
 * viewport, Enter confirms the active feature command when it is complete. Inline editors and the
 * line tool take Enter first and prevent its default; an inspector expression field confirms
 * through `confirmIfComplete` itself, after its text is evaluated.
 */
export function useEnterLadder(app: App): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const target = event.target
      if (
        event.key !== 'Enter' ||
        event.defaultPrevented ||
        event.isComposing ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        !(target instanceof Element)
      ) {
        return
      }
      const inInspector =
        target.closest('[data-testid="inspector"][data-mode="command"]') !== null &&
        !handlesOwnEnter(target)
      const onViewport =
        target.getAttribute('role') === 'application' &&
        target.closest('[data-testid="viewport"]') !== null
      if ((inInspector || onViewport) && confirmIfComplete(app)) {
        event.preventDefault()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [app])
}
