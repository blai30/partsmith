import { capturePick } from '../app/actions/command'
import { select } from '../app/actions/selection'
import type { App } from '../app/createApp'
import type { SelectionKind } from '../generated/protocol'
import { buildInspectorModel } from '../inspector/model'
import { acceptsTarget } from '../inspector/selection-values'
import type { AppState, SelectionItem } from '../state/store'

// Clicks on timeline items and browser rows (ui.md 3.9, 3.10): while a feature command's active
// selection field accepts the item, the click fills that field; otherwise it selects the item.

/** What the active command's active selection field accepts, or null without one. */
export function activeFieldAccepts(state: AppState): SelectionKind | null {
  const command = state.command
  const feature = command === null ? undefined : state.catalog?.byType[command.featureType]
  if (command === null || feature === undefined || command.activeField === null) {
    return null
  }
  const model = buildInspectorModel({ feature, params: command.params, errors: {} })
  const field = [...model.essential, ...model.more].find(
    (candidate) => candidate.path === command.activeField,
  )
  return field?.control.type === 'selection' ? field.control.accepts : null
}

/** Captures the item into the active field when it fits, else selects it (toggling with Shift or Mod). */
export function pickRow(app: App, item: SelectionItem, toggle: boolean): void {
  const accepts = activeFieldAccepts(app.store.getState())
  if (accepts !== null && acceptsTarget(accepts, item)) {
    capturePick(app, item).catch((error: unknown) => {
      console.error('Capturing the pick failed.', error)
    })
    return
  }
  select(app, [item], toggle ? 'toggle' : 'replace')
}

/** Whether a pointer or key event asks to toggle the selection: Shift, or Mod for the platform. */
export function togglesSelection(
  app: App,
  event: { readonly shiftKey: boolean; readonly metaKey: boolean; readonly ctrlKey: boolean },
): boolean {
  return (
    event.shiftKey || (app.platform.operatingSystem === 'mac' ? event.metaKey : event.ctrlKey)
  )
}
