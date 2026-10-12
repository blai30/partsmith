import { Shapes } from 'lucide-react'
import { startFeatureCommand } from '../app/actions/command'
import type { App } from '../app/createApp'
import type { Category } from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { AppContext, Catalog } from '../state/store'
import { FEATURE_ICONS } from './icons'
import type { Availability, CommandCategory, CommandDefinition } from './registry'

// One command per catalog feature type (ui.md 2.10).

/** Extra palette search words per feature type. */
export const FEATURE_KEYWORDS: Readonly<Record<string, readonly string[]>> = {
  extrude: ['pad', 'pocket', 'extrusion'],
  revolve: ['revolution', 'groove', 'lathe'],
  hole: ['drill', 'counterbore', 'countersink'],
  fillet: ['round', 'radius'],
  chamfer: ['bevel'],
  shell: ['hollow', 'thickness'],
  boolean: ['combine', 'union', 'subtract', 'intersect', 'join', 'cut'],
  mirror: ['mirrored', 'symmetry'],
  linear_pattern: ['array', 'rectangular pattern'],
  circular_pattern: ['polar pattern', 'array'],
  datum_plane: ['construction plane', 'offset plane', 'work plane'],
  move_body: ['placement', 'translate', 'rotate'],
  split_body: ['slice', 'cut body'],
  box: ['cube', 'block'],
  cylinder: ['rod', 'pin'],
  sphere: ['ball'],
  cone: ['taper'],
  torus: ['ring', 'donut'],
  sketch: ['drawing', 'profile'],
}

/** The feature shortcuts of 3.5, with the contexts they are active in. */
export const FEATURE_SHORTCUTS: Readonly<
  Record<
    string,
    { readonly shortcuts: readonly string[]; readonly contexts: readonly AppContext[] }
  >
> = {
  extrude: { shortcuts: ['E'], contexts: ['model', 'sketch', 'print'] },
  // Sketch mode has its own H (Horizontal), and these features need a finished model anyway.
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

function availability(app: App): Availability {
  const state = app.store.getState()
  return state.core.status === 'ready' && state.document !== null
    ? { state: 'enabled' }
    : { state: 'disabled', reason: MESSAGES.core_not_running() }
}

/** The commands of the catalog's feature types, in catalog order. */
export function featureCommands(catalog: Catalog): CommandDefinition[] {
  return catalog.features
    .filter((feature) => !WITHOUT_COMMAND.has(feature.type))
    .map((feature) => {
      const bound = FEATURE_SHORTCUTS[feature.type]
      return {
        id: `feature.${feature.type}`,
        title: feature.manifest.label,
        category: CATEGORIES[feature.manifest.category],
        icon: FEATURE_ICONS[feature.manifest.icon] ?? Shapes,
        keywords: FEATURE_KEYWORDS[feature.type] ?? [],
        shortcuts: bound?.shortcuts ?? [],
        contexts: bound?.contexts ?? ALL_CONTEXTS,
        availability,
        // The sketch type starts a new sketch; startFeatureCommand switches to Model first (3.3).
        run: (app: App) => startFeatureCommand(app, feature.type),
      }
    })
}
