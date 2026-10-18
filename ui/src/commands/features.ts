import { startFeatureCommand } from '../app/actions/command'
import { showOpenEnds } from '../app/actions/sketch'
import type { App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { activeFeatures } from '../core/recipe'
import type { Category, FeatureRecord } from '../generated/protocol'
import { MESSAGES } from '../messages'
import { openEnds } from '../sketch/issues'
import { selectFeatureLabel } from '../state/selectors'
import type { AppContext, AppState, Catalog, CatalogFeature, DocumentState } from '../state/store'
import { FEATURE_DESCRIPTIONS } from './descriptions'
import { featureIcon } from './icons'
import { keywordsFor } from './keywords'
import {
  disabledWith,
  type Availability,
  type CommandCategory,
  type CommandDefinition,
  type DisabledAvailability,
} from './registry'

// One command per catalog feature type (ui.md 2.10).

/** The feature shortcuts of 3.5, with the contexts they are active in. */
export const FEATURE_SHORTCUTS: Readonly<
  Record<
    string,
    { readonly shortcuts: readonly string[]; readonly contexts: readonly AppContext[] }
  >
> = {
  extrude: { shortcuts: ['E'], contexts: ['model', 'sketch', 'print'] },
  // Sketch mode has its own H (Horizontal), F and M stay free there too, and these features act
  // on a solid anyway; the commands themselves stay in the Sketch palette (2.10).
  hole: { shortcuts: ['H'], contexts: ['model', 'print'] },
  fillet: { shortcuts: ['F'], contexts: ['model', 'print'] },
  move_body: { shortcuts: ['M'], contexts: ['model', 'print'] },
}

const CATEGORIES: Readonly<Record<Category, CommandCategory>> = {
  create: 'Create',
  modify: 'Modify',
  pattern: 'Pattern',
  construct: 'Construct',
  sketch: 'Create',
}

const ALL_CONTEXTS: readonly AppContext[] = ['model', 'sketch', 'print']

/** Types without a command: STEP import has no UI in M1 (2.10). */
const WITHOUT_COMMAND: ReadonlySet<string> = new Set(['import'])

/**
 * The sketches whose regions a new Extrude or Revolve can use, in timeline order: the active ones
 * (not suppressed, not rolled back) and the one being edited in a sketch session (2.10).
 */
function usableSketches(state: AppState, document: DocumentState): readonly FeatureRecord[] {
  const edited = state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null
  const active = new Set(activeFeatures(document.recipe).map((feature) => feature.id))
  return (document.recipe.features ?? []).filter(
    (feature) =>
      feature.type === 'sketch' &&
      (feature.id === edited || (active.has(feature.id) && feature.suppressed !== true)),
  )
}

/** A sketch without a region whose outline has open ends, and how many (2.14 `openEnds`). */
type OpenSketch = { readonly id: string; readonly openEnds: number }

/**
 * Why Extrude and Revolve cannot start (2.10), or null when a usable sketch has a region. A usable
 * sketch whose outline has open ends is named, the one with the fewest (the latest in timeline
 * order among equals), with "Show '{sketch}'", which edits it and frames its open ends.
 */
function needsClosedShape(state: AppState, document: DocumentState): DisabledAvailability | null {
  let fewest: OpenSketch | null = null
  for (const sketch of usableSketches(state, document)) {
    const solution = document.sketches[sketch.id]?.solution ?? null
    if (solution === null) {
      continue
    }
    if (solution.regions.length > 0) {
      return null
    }
    const ends = openEnds(solution)
    if (ends > 0 && (fewest === null || ends <= fewest.openEnds)) {
      fewest = { id: sketch.id, openEnds: ends }
    }
  }
  if (fewest === null) {
    return disabledWith('needs_sketch_region', MESSAGES.needs_sketch_region())
  }
  const { id } = fewest
  const label = selectFeatureLabel(state, id)
  return {
    ...disabledWith('needs_closed_shape', MESSAGES.needs_closed_shape(label, fewest.openEnds)),
    action: {
      label: `Show '${label}'`,
      run: (app) => {
        showOpenEnds(app, id).catch(reportFailure(`Showing the open ends of ${id}`))
      },
    },
  }
}

/** How many bodies have model-layer geometry. */
function solidCount(document: DocumentState): number {
  return Object.keys(document.meshes).length
}

function needsSolid(_state: AppState, document: DocumentState): DisabledAvailability | null {
  return solidCount(document) === 0 ? disabledWith('needs_solid', MESSAGES.needs_solid()) : null
}

/**
 * What a new feature of each type needs before it can start (2.10): the reason shown while it is
 * missing, or null once it is there; types without an entry are always available. Editing a
 * feature is never affected.
 */
export const FEATURE_REQUIREMENTS: Readonly<
  Record<string, (state: AppState, document: DocumentState) => DisabledAvailability | null>
> = {
  extrude: needsClosedShape,
  revolve: needsClosedShape,
  hole: needsSolid,
  fillet: needsSolid,
  chamfer: needsSolid,
  shell: needsSolid,
  mirror: needsSolid,
  linear_pattern: needsSolid,
  circular_pattern: needsSolid,
  move_body: needsSolid,
  split_body: needsSolid,
  boolean: (_state, document) =>
    solidCount(document) < 2 ? disabledWith('needs_two_bodies', MESSAGES.needs_two_bodies()) : null,
}

/** A feature command is available while the core runs with a document open and its type's requirement is met. */
function availabilityFor(type: string): (app: App) => Availability {
  return (app) => {
    const state = app.store.getState()
    if (state.core.status !== 'ready' || state.document === null) {
      return disabledWith('core_not_running', MESSAGES.core_not_running())
    }
    return FEATURE_REQUIREMENTS[type]?.(state, state.document) ?? { state: 'enabled' }
  }
}

/** The command of one catalog feature type. */
function featureCommand(feature: CatalogFeature): CommandDefinition {
  const bound = FEATURE_SHORTCUTS[feature.type]
  const definition: CommandDefinition = {
    id: `feature.${feature.type}`,
    title: feature.manifest.label,
    category: CATEGORIES[feature.manifest.category],
    icon: featureIcon(feature.manifest.icon),
    keywords: keywordsFor(`feature.${feature.type}`),
    shortcuts: bound?.shortcuts ?? [],
    // Every feature command is in every context (3.3); some of their letters are not (3.5).
    contexts: ALL_CONTEXTS,
    availability: availabilityFor(feature.type),
    // The sketch type starts a new sketch; startFeatureCommand switches to Model first (3.3).
    run: (app: App) => startFeatureCommand(app, feature.type),
  }
  const description = FEATURE_DESCRIPTIONS[definition.id]
  return {
    ...definition,
    ...(description === undefined ? {} : { description }),
    ...(bound === undefined ? {} : { shortcutContexts: bound.contexts }),
  }
}

/** The commands of the catalog's feature types, in catalog order. */
export function featureCommands(catalog: Catalog): CommandDefinition[] {
  return catalog.features
    .filter((feature) => !WITHOUT_COMMAND.has(feature.type))
    .map(featureCommand)
}
