import { capturePick } from '../app/actions/command'
import { select } from '../app/actions/selection'
import { pickPlane } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import type { SelectionKind } from '../generated/protocol'
import { activeCommandModel, fieldsOf } from '../inspector/model'
import { acceptsTarget } from '../inspector/selection-values'
import type { AppState, SelectionItem } from '../state/store'

// Clicks and keys on timeline items and browser rows (ui.md 3.9, 3.10): while a feature command's
// active selection field accepts the item, the click fills that field; while a plane is picked for
// a new sketch or a re-attached one, an origin plane or datum row places the sketch on it (3.16.1);
// otherwise it selects.

/** What the active command's active selection field accepts, or null without one. */
export function activeFieldAccepts(state: AppState): SelectionKind | null {
  const activeField = state.command?.activeField ?? null
  const active = activeField === null ? null : activeCommandModel(state)
  if (active === null) {
    return null
  }
  const field = fieldsOf(active.model).find((candidate) => candidate.path === activeField)
  return field?.control.type === 'selection' ? field.control.accepts : null
}

/** Whether a plane is being picked for a new sketch or a re-attached one (3.16.1). */
function pickingPlane(state: AppState): boolean {
  return state.context === 'sketch' && state.sketch !== null && state.sketch.sketchId === null
}

/**
 * Captures the item into the active field when it fits, places the new or re-attached sketch on a
 * plane row while a plane is picked, else selects the item (toggling with Shift or Mod).
 */
export function pickRow(app: App, item: SelectionItem, toggle: boolean): void {
  const state = app.store.getState()
  if (pickingPlane(state) && (item.kind === 'origin_plane' || item.kind === 'datum_plane')) {
    pickPlane(app, item).catch(reportFailure('Placing the sketch'))
    return
  }
  const accepts = activeFieldAccepts(state)
  if (accepts !== null && acceptsTarget(accepts, item)) {
    capturePick(app, item).catch(reportFailure('Capturing the pick'))
    return
  }
  select(app, [item], toggle ? 'toggle' : 'replace')
}

/** Whether a pointer or key event asks to toggle the selection: Shift, or Mod for the platform. */
export function togglesSelection(
  app: App,
  event: { readonly shiftKey: boolean; readonly metaKey: boolean; readonly ctrlKey: boolean },
): boolean {
  return event.shiftKey || (app.platform.operatingSystem === 'mac' ? event.metaKey : event.ctrlKey)
}

/**
 * Whether a key event activates a timeline item or browser row: Enter without Ctrl, Cmd or Alt, so
 * Mod+Enter (Finish sketch) stays a shortcut while a row has focus (3.5).
 */
export function activatesRow(event: {
  readonly key: string
  readonly ctrlKey: boolean
  readonly metaKey: boolean
  readonly altKey: boolean
}): boolean {
  return event.key === 'Enter' && !event.ctrlKey && !event.metaKey && !event.altKey
}
