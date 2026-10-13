import { openRecent } from '../app/actions/document'
import { openSidePanel } from '../app/actions/panels'
import { select } from '../app/actions/selection'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { featureLabel } from '../core/labels'
import type { RecentFile } from '../files/recent'
import {
  bodyKey,
  bodySelectionItem,
  featureKey,
  featureSelectionItem,
  parameterKey,
} from '../inspector/selection-values'
import { requestUi } from '../layout/requests'
import { MESSAGES } from '../messages'
import type { AppState } from '../state/store'
import type { PaletteItem } from './fuzzy'
import { runCommand, shortcutsActiveIn, type CommandDefinition } from './registry'
import { formatShortcut } from './shortcuts'

// The items the command palette searches (ui.md 3.4): commands and settings, then the document's
// features, bodies and parameters, then recent files.

/** Whether a command is listed in the palette now: its context and host, and not hidden. */
function listed(app: App, state: AppState, definition: CommandDefinition): boolean {
  return (
    definition.contexts.includes(state.context) &&
    (definition.host === undefined || definition.host === app.platform.host) &&
    definition.availability(app).state !== 'hidden'
  )
}

/** The detail of a command: its own, or, for a feature command in Sketch, what running it does first (2.10). */
function commandDetail(app: App, state: AppState, definition: CommandDefinition): string | null {
  const own = definition.detail?.(app) ?? null
  if (own === null && state.context === 'sketch' && definition.id.startsWith('feature.')) {
    return FINISHES_THE_SKETCH
  }
  return own
}

const FINISHES_THE_SKETCH = 'Finishes the sketch'

function commandItem(app: App, state: AppState, definition: CommandDefinition): PaletteItem {
  const availability = definition.availability(app)
  // A shortcut that does something else in this context is not shown (3.5).
  const [shortcut] = shortcutsActiveIn(definition, state.context) ? definition.shortcuts : []
  return {
    id: definition.id,
    kind: definition.category === 'Settings' ? 'setting' : 'command',
    category: definition.category,
    title: definition.label?.(app) ?? definition.title,
    keywords: definition.keywords,
    detail: commandDetail(app, state, definition),
    description: definition.description ?? null,
    shortcut: shortcut === undefined ? null : formatShortcut(shortcut, app.platform),
    disabledReason: availability.state === 'disabled' ? availability.reason : null,
    run: () => {
      runCommand(app, definition.id, 'palette')
    },
  }
}

/** A palette item for something in the document: no category, keywords, shortcut or reason. */
function documentItem(
  id: string,
  kind: 'feature' | 'body' | 'parameter',
  title: string,
  detail: string | null,
  run: () => void,
): PaletteItem {
  return {
    id,
    kind,
    category: null,
    title,
    keywords: [],
    detail,
    description: null,
    shortcut: null,
    disabledReason: null,
    run,
  }
}

function documentItems(app: App, state: AppState): PaletteItem[] {
  const document = state.document
  if (document === null) {
    return []
  }
  const recipe = document.recipe
  const features = (recipe.features ?? []).map((feature) => {
    const label = featureLabel(recipe, feature.id, state.catalog)
    return documentItem(featureKey(feature.id), 'feature', label, null, () => {
      select(app, [featureSelectionItem(feature.id, label)], 'replace')
      requestUi(app, { kind: 'reveal', feature: feature.id })
    })
  })
  const bodies = (recipe.bodies ?? []).map((body) => {
    return documentItem(bodyKey(body.id), 'body', body.name, null, () => {
      select(app, [bodySelectionItem(body.id, body.name)], 'replace')
      const bounds = app.store.getState().document?.meshes[body.id]?.boundingBox
      if (bounds !== undefined) {
        app.viewport()?.fit(bounds)
      }
    })
  })
  const parameters = (recipe.parameters ?? []).map((parameter) =>
    documentItem(
      parameterKey(parameter.id),
      'parameter',
      `${parameter.name} = ${parameter.expr}`,
      parameter.comment ?? null,
      () => {
        openSidePanel(app, 'parameters')
        requestUi(app, { kind: 'parameter', id: parameter.id })
      },
    ),
  )
  return [...features, ...bodies, ...parameters]
}

/** Every palette item for the current state and the recent files, in no particular order. */
export function paletteItems(app: App, recent: readonly RecentFile[]): PaletteItem[] {
  const state = app.store.getState()
  const commands = app.commands
    .list()
    .filter((definition) => listed(app, state, definition))
    .map((definition) => commandItem(app, state, definition))
  const recentItems = recent.map((entry): PaletteItem => ({
    id: `recent:${entry.id}`,
    kind: 'recent',
    category: null,
    title: `Open recent: ${entry.name}`,
    keywords: [],
    detail: null,
    description: null,
    shortcut: null,
    disabledReason: state.core.status === 'ready' ? null : MESSAGES.core_not_running(),
    run: () => {
      openRecent(app, entry).catch(reportFailure(`Opening ${entry.name}`))
    },
  }))
  return [...commands, ...documentItems(app, state), ...recentItems]
}
