import { openRecent } from '../app/actions/document'
import { select } from '../app/actions/selection'
import type { App } from '../app/createApp'
import { featureLabel } from '../core/labels'
import type { RecentFile } from '../files/recent'
import { requestUi } from '../layout/requests'
import { MESSAGES } from '../messages'
import type { AppState, SelectionItem } from '../state/store'
import type { PaletteItem } from './fuzzy'
import type { CommandDefinition } from './registry'
import { formatShortcut } from './shortcuts'

// The items the command palette searches (ui.md 3.4): commands and settings, then the document's
// features, bodies and parameters, then recent files.

function selectionItem(
  key: string,
  kind: SelectionItem['kind'],
  label: string,
  ids: { readonly body?: string; readonly feature?: string },
): SelectionItem {
  return {
    key,
    kind,
    label,
    body: ids.body ?? null,
    feature: ids.feature ?? null,
    element: null,
    sketchItem: null,
    origin: null,
  }
}

/** Whether a command is listed in the palette now: its context and host, and not hidden. */
function listed(app: App, state: AppState, definition: CommandDefinition): boolean {
  return (
    definition.contexts.includes(state.context) &&
    (definition.host === undefined || definition.host === app.platform.host) &&
    definition.availability(app).state !== 'hidden'
  )
}

function commandItem(app: App, definition: CommandDefinition): PaletteItem {
  const availability = definition.availability(app)
  const [shortcut] = definition.shortcuts
  return {
    id: definition.id,
    kind: definition.category === 'Settings' ? 'setting' : 'command',
    category: definition.category,
    title: definition.title,
    keywords: definition.keywords,
    detail: definition.detail?.(app) ?? null,
    shortcut: shortcut === undefined ? null : formatShortcut(shortcut, app.platform),
    disabledReason: availability.state === 'disabled' ? availability.reason : null,
    run: () => {
      app.commands.run(app, definition.id, 'palette').catch((error: unknown) => {
        console.error(`The command ${definition.id} failed.`, error)
      })
    },
  }
}

function documentItems(app: App, state: AppState): PaletteItem[] {
  const document = state.document
  if (document === null) {
    return []
  }
  const recipe = document.recipe
  const plain = { category: null, keywords: [], shortcut: null, disabledReason: null } as const
  const features = (recipe.features ?? []).map((feature): PaletteItem => {
    const label = featureLabel(recipe, feature.id, state.catalog)
    return {
      ...plain,
      id: `feature:${feature.id}`,
      kind: 'feature',
      title: label,
      detail: null,
      run: () => {
        // The timeline scrolls the selected feature into view.
        select(
          app,
          [selectionItem(`feature:${feature.id}`, 'feature', label, { feature: feature.id })],
          'replace',
        )
      },
    }
  })
  const bodies = (recipe.bodies ?? []).map((body): PaletteItem => ({
    ...plain,
    id: `body:${body.id}`,
    kind: 'body',
    title: body.name,
    detail: null,
    run: () => {
      select(
        app,
        [selectionItem(`body:${body.id}`, 'body', body.name, { body: body.id })],
        'replace',
      )
      const bounds = app.store.getState().document?.meshes[body.id]?.boundingBox
      if (bounds !== undefined) {
        app.viewport()?.fit(bounds)
      }
    },
  }))
  const parameters = (recipe.parameters ?? []).map((parameter): PaletteItem => ({
    ...plain,
    id: `parameter:${parameter.id}`,
    kind: 'parameter',
    title: `${parameter.name} = ${parameter.expr}`,
    detail: parameter.comment ?? null,
    run: () => {
      app.store.setState((current) => ({ panels: { ...current.panels, side: 'parameters' } }))
      requestUi(app, { kind: 'parameter', id: parameter.id })
    },
  }))
  return [...features, ...bodies, ...parameters]
}

/** Every palette item for the current state and the recent files, in no particular order. */
export function paletteItems(app: App, recent: readonly RecentFile[]): PaletteItem[] {
  const state = app.store.getState()
  const commands = app.commands
    .list()
    .filter((definition) => listed(app, state, definition))
    .map((definition) => commandItem(app, definition))
  const recentItems = recent.map((entry): PaletteItem => ({
    id: `recent:${entry.id}`,
    kind: 'recent',
    category: null,
    title: `Open recent: ${entry.name}`,
    keywords: [],
    detail: null,
    shortcut: null,
    disabledReason: state.core.status === 'ready' ? null : MESSAGES.core_not_running(),
    run: () => {
      openRecent(app, entry).catch((error: unknown) => {
        console.error(`Opening ${entry.name} failed.`, error)
      })
    },
  }))
  return [...commands, ...documentItems(app, state), ...recentItems]
}
