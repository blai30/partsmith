import { Shapes } from 'lucide-react'
import { startFeatureCommand } from '../app/actions/command'
import type { App } from '../app/createApp'
import { activeFeatures } from '../core/recipe'
import type { Category } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { AppContext, AppState, Catalog, CatalogFeature } from '../state/store'
import { FEATURE_DESCRIPTIONS } from './descriptions'
import { FEATURE_ICONS } from './icons'
import { keywordsFor } from './keywords'
import {
  disabledWith,
  type Availability,
  type CommandCategory,
  type CommandDefinition,
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

/** Whether some active sketch (not suppressed, not rolled back) has a closed region, or the edited one does. */
function hasSketchRegion(state: AppState): boolean {
  const document = state.document
  if (document === null) {
    return false
  }
  const active = activeFeatures(document.recipe)
  const edited = state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null
  const hasRegion = (id: string) => (document.sketches[id]?.solution.regions.length ?? 0) > 0
  return (
    (edited !== null && hasRegion(edited)) ||
    active.some(
      (feature) =>
        feature.type === 'sketch' && feature.suppressed !== true && hasRegion(feature.id),
    )
  )
}

/** How many bodies have model-layer geometry. */
function solidCount(state: AppState): number {
  return Object.keys(state.document?.meshes ?? {}).length
}

/** The 4.3 messages that say what a new feature still needs. */
type Requirement = 'needs_sketch_region' | 'needs_solid' | 'needs_two_bodies'

function needsSolid(state: AppState): Requirement | null {
  return solidCount(state) === 0 ? 'needs_solid' : null
}

/**
 * What a new feature of each type needs before it can start (2.10), as the id of the reason shown
 * while it is missing; types without an entry are always available. Editing a feature is never
 * affected.
 */
export const FEATURE_REQUIREMENTS: Readonly<
  Record<string, (state: AppState) => Requirement | null>
> = {
  extrude: (state) => (hasSketchRegion(state) ? null : 'needs_sketch_region'),
  revolve: (state) => (hasSketchRegion(state) ? null : 'needs_sketch_region'),
  hole: needsSolid,
  fillet: needsSolid,
  chamfer: needsSolid,
  shell: needsSolid,
  mirror: needsSolid,
  linear_pattern: needsSolid,
  circular_pattern: needsSolid,
  move_body: needsSolid,
  split_body: needsSolid,
  boolean: (state) => (solidCount(state) < 2 ? 'needs_two_bodies' : null),
}

/** A feature command is available while the core runs with a document open and its type's requirement is met. */
function availabilityFor(type: string): (app: App) => Availability {
  return (app) => {
    const state = app.store.getState()
    if (state.core.status !== 'ready' || state.document === null) {
      return disabledWith('core_not_running', MESSAGES.core_not_running())
    }
    const missing = FEATURE_REQUIREMENTS[type]?.(state) ?? null
    return missing === null ? { state: 'enabled' } : disabledWith(missing, MESSAGES[missing]())
  }
}

/** The command of one catalog feature type. */
function featureCommand(feature: CatalogFeature): CommandDefinition {
  const bound = FEATURE_SHORTCUTS[feature.type]
  const definition: CommandDefinition = {
    id: `feature.${feature.type}`,
    title: feature.manifest.label,
    category: CATEGORIES[feature.manifest.category],
    icon: FEATURE_ICONS[feature.manifest.icon] ?? Shapes,
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
