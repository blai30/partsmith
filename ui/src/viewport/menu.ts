import { cancelCommand, commandComplete, confirmCommand, editFeature } from '../app/actions/command'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { runCommand, shortcutsActiveIn } from '../commands/registry'
import { formatShortcut } from '../commands/shortcuts'
import { featureLabel } from '../core/labels'
import { suggestedCommands } from '../inspector/suggestions'
import { CONSTRAINT_TITLES, applicableConstraints } from '../sketch/constraints'
import type { AppState, SelectionItem } from '../state/store'
import { isSessionVisible } from '../state/visibility'

// The viewport context menu (ui.md 3.13): what fits the selection under the pointer, one right
// click away. The items are data, so the plain menu of M1 and the radial marking menu of 1.2 can
// share them; disabled commands are left out.

export type ViewportMenuItem =
  | {
      readonly kind: 'command'
      /** The registry command the item runs, or `command.confirm` and `command.cancel` for OK and Cancel. */
      readonly command: string
      readonly label: string
      /** The formatted shortcut, or null. */
      readonly shortcut: string | null
      readonly run: () => void
    }
  | { readonly kind: 'separator' }

type CommandItem = Extract<ViewportMenuItem, { readonly kind: 'command' }>

const SEPARATOR: ViewportMenuItem = { kind: 'separator' }

/** The item of an enabled registry command, with its title as shown now; null when it is not enabled. */
function commandItem(app: App, state: AppState, id: string, label?: string): CommandItem | null {
  const definition = app.commands.get(id)
  if (definition === undefined || definition.availability(app).state !== 'enabled') {
    return null
  }
  const [shortcut] = shortcutsActiveIn(definition, state.context) ? definition.shortcuts : []
  return {
    kind: 'command',
    command: id,
    label: label ?? definition.label?.(app) ?? definition.title,
    shortcut: shortcut === undefined ? null : formatShortcut(shortcut, app.platform),
    run: () => {
      runCommand(app, id, 'menu')
    },
  }
}

/** An item that runs `action` for `command`, which is not a registry command run (OK, Cancel, Edit). */
function actionItem(
  command: string,
  label: string,
  shortcut: string | null,
  action: () => Promise<unknown>,
): CommandItem {
  return {
    kind: 'command',
    command,
    label,
    shortcut,
    run: () => {
      action().catch(reportFailure(`The command ${command}`))
    },
  }
}

function present<Item>(items: readonly (Item | null)[]): Item[] {
  return items.filter((item) => item !== null)
}

/** The items, then `last` after a separator; just the items while `last` is not enabled. */
function withTrailing(
  above: readonly ViewportMenuItem[],
  last: CommandItem | null,
): readonly ViewportMenuItem[] {
  if (last === null) {
    return above
  }
  return above.length === 0 ? [last] : [...above, SEPARATOR, last]
}

/** OK (when complete) and Cancel of the active feature command. */
function commandItems(app: App, state: AppState): readonly ViewportMenuItem[] {
  const ok = actionItem('command.confirm', 'OK', formatShortcut('Enter', app.platform), () =>
    confirmCommand(app),
  )
  const cancel = actionItem(
    'command.cancel',
    'Cancel',
    formatShortcut('Escape', app.platform),
    () => cancelCommand(app),
  )
  return commandComplete(state) ? [ok, cancel] : [cancel]
}

/** The constraints that fit the sketch selection, Construction and Delete, then Finish sketch. */
function sketchItems(app: App, state: AppState): readonly ViewportMenuItem[] {
  const sketchId = state.sketch?.sketchId ?? null
  const selection = state.sketch?.selection ?? []
  const solution = sketchId === null ? undefined : state.document?.sketches[sketchId]?.solution
  const constraints =
    solution === undefined || selection.length === 0
      ? []
      : applicableConstraints(selection, solution).map((type) =>
          commandItem(app, state, `sketch.constrain_${type}`, CONSTRAINT_TITLES[type]),
        )
  const editing =
    selection.length === 0
      ? []
      : [commandItem(app, state, 'sketch.construction'), commandItem(app, state, 'edit.delete')]
  return withTrailing(
    present([...constraints, ...editing]),
    commandItem(app, state, 'sketch.finish'),
  )
}

/** The feature that "Edit {label}" opens: one feature, or the feature that made one element. */
function editedFeature(state: AppState, selection: readonly SelectionItem[]): string | null {
  const [only] = selection
  if (selection.length !== 1 || only === undefined || only.feature === null) {
    return null
  }
  const element = only.kind === 'face' || only.kind === 'edge' || only.kind === 'vertex'
  const feature = only.kind === 'feature' || only.kind === 'datum_plane'
  if (!element && !feature) {
    return null
  }
  const record = state.document?.recipe.features?.find((candidate) => candidate.id === only.feature)
  // Imported geometry has no settings to edit (4.3).
  return record === undefined || record.type === 'import' ? null : record.id
}

/** "Hide" while some of the selection's visibility targets are shown, else "Show". */
function visibilityLabel(state: AppState): string {
  const shown = state.selection.some((item) => {
    if (item.kind === 'body') {
      const body = state.document?.recipe.bodies?.find((candidate) => candidate.id === item.body)
      return body?.visible !== false
    }
    return isSessionVisible(state, item.key)
  })
  return shown ? 'Hide' : 'Show'
}

/** Repeat, the suggested commands, Edit, Hide and Look at, then Fit to view. */
function modelItems(app: App, state: AppState): readonly ViewportMenuItem[] {
  const featureId = editedFeature(state, state.selection)
  const document = state.document
  const edit =
    featureId === null || document === null
      ? null
      : actionItem(
          'edit.edit_feature',
          `Edit ${featureLabel(document.recipe, featureId, state.catalog)}`,
          null,
          () => editFeature(app, featureId),
        )
  const above = present([
    commandItem(app, state, 'edit.repeat'),
    ...suggestedCommands(state, state.selection).map((id) => commandItem(app, state, id)),
    edit,
    commandItem(app, state, 'view.visibility', visibilityLabel(state)),
    commandItem(app, state, 'view.look_at'),
  ])
  return withTrailing(above, commandItem(app, state, 'view.fit'))
}

/** The items of the viewport context menu for the current state (3.13), disabled commands left out. */
export function viewportMenuItems(app: App): readonly ViewportMenuItem[] {
  const state = app.store.getState()
  if (state.command !== null) {
    return commandItems(app, state)
  }
  if (state.context === 'sketch' && state.sketch !== null) {
    return sketchItems(app, state)
  }
  return modelItems(app, state)
}
