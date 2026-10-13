import { useSyncExternalStore } from 'react'
import { useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { runCommand, type Availability, type CommandDefinition } from '../commands/registry'
import { formatShortcut } from '../commands/shortcuts'
import type { MenuEntry } from '../components/Menu'

// Commands as the chrome shows them (ui.md 3.3, 3.24): a toolbar button or menu item re-renders
// when a command registers or when its availability or checked state changes, not on every store
// change.

/** A registered command with what its button shows now. */
export type CommandView = {
  readonly definition: CommandDefinition
  readonly availability: Availability
  /** The toggle state for commands with `checked`, else null. */
  readonly checked: boolean | null
  /** The first shortcut, formatted for the platform, or null. */
  readonly shortcut: string | null
}

/** The registered command definitions; re-renders when commands register or unregister. */
function useCommandList(app: App): readonly CommandDefinition[] {
  return useSyncExternalStore(app.commands.subscribe, app.commands.list, app.commands.list)
}

function viewOf(app: App, id: string): CommandView | null {
  const definition = app.commands.get(id)
  if (definition === undefined) {
    return null
  }
  const [shortcut] = definition.shortcuts
  return {
    definition,
    availability: definition.availability(app),
    checked: definition.checked?.(app) ?? null,
    shortcut: shortcut === undefined ? null : formatShortcut(shortcut, app.platform),
  }
}

/** A text that changes exactly when one of the commands' availability or checked state changes. */
function stateKey(app: App, ids: readonly string[]): string {
  return ids
    .map((id) => {
      const view = viewOf(app, id)
      if (view === null) {
        return `${id} unregistered`
      }
      const { availability, checked } = view
      const reason = availability.state === 'disabled' ? availability.reason : ''
      return `${id} ${availability.state} ${String(checked)} ${reason}`
    })
    .join('\n')
}

/** The views of the commands with these ids that are registered, in the given order. */
export function useCommandViews(app: App, ids: readonly string[]): readonly CommandView[] {
  // Subscribing to the list and to the state key re-renders the caller when a view changes.
  useCommandList(app)
  useAppState(() => stateKey(app, ids))
  return ids.flatMap((id) => {
    const view = viewOf(app, id)
    return view === null ? [] : [view]
  })
}

/** The menu item that runs a command: its icon, title as shown now (or the given label) and shortcut. */
export function commandMenuItem(app: App, view: CommandView, label?: string): MenuEntry {
  const { definition } = view
  return {
    kind: 'item',
    label: label ?? definition.label?.(app) ?? definition.title,
    ...(definition.icon === null ? {} : { icon: definition.icon }),
    ...(view.shortcut === null ? {} : { shortcut: view.shortcut }),
    disabled: view.availability.state === 'disabled',
    onSelect: () => {
      runCommand(app, definition.id, 'menu')
    },
  }
}
