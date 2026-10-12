import type { SketchSolution } from '../generated/protocol'
import type { AppState, SelectionItem } from '../state/store'

// The next steps the selection inspector offers (ui.md 3.8): the feature commands that fit what is
// selected, run with the selection preselected. Palette-only commands (Datum plane, Move body,
// Split body) are offered where they apply, so the toolbars do not grow.

function isPlane(item: SelectionItem, state: AppState): boolean {
  return (
    item.kind === 'origin_plane' ||
    item.kind === 'datum_plane' ||
    (item.kind === 'feature' && featureType(state, item.feature) === 'datum_plane')
  )
}

function featureType(state: AppState, id: string | null): string | null {
  if (id === null) {
    return null
  }
  return state.document?.recipe.features?.find((feature) => feature.id === id)?.type ?? null
}

/** Extrude and Revolve for a sketch with regions, Hole for one with points that are not construction. */
function sketchSuggestions(solution: SketchSolution | undefined): readonly string[] {
  if (solution === undefined) {
    return []
  }
  const hasPoints = solution.entities.some(
    (entity) => entity.type === 'point' && entity.construction !== true,
  )
  return [
    ...(solution.regions.length > 0 ? ['extrude', 'revolve'] : []),
    ...(hasPoints ? ['hole'] : []),
  ]
}

/** The feature types the rows of 3.8 suggest for the selection, in display order. */
function suggestedTypes(state: AppState, items: readonly SelectionItem[]): readonly string[] {
  const [first, second] = items
  if (first === undefined || state.context === 'sketch') {
    return []
  }
  const every = (test: (item: SelectionItem) => boolean): boolean => items.every(test)
  if (items.length === 1) {
    if (first.kind === 'body') {
      return ['move_body', 'mirror', 'split_body']
    }
    if (isPlane(first, state)) {
      return ['sketch', 'datum_plane']
    }
    if (first.kind === 'feature' && featureType(state, first.feature) === 'sketch') {
      const sketch = first.feature === null ? undefined : state.document?.sketches[first.feature]
      return sketchSuggestions(sketch?.solution)
    }
    if (first.kind === 'face') {
      return first.element?.surface === 'plane' ? ['sketch', 'hole', 'shell'] : ['shell']
    }
  }
  if (
    items.length === 2 &&
    second !== undefined &&
    ((first.kind === 'body' && isPlane(second, state)) ||
      (second.kind === 'body' && isPlane(first, state)))
  ) {
    return ['mirror', 'split_body']
  }
  if (every((item) => item.kind === 'body')) {
    return ['boolean']
  }
  if (every((item) => item.kind === 'edge')) {
    return ['fillet', 'chamfer']
  }
  if (every((item) => item.kind === 'face')) {
    return ['shell']
  }
  if (every((item) => item.kind === 'sketch_region')) {
    return ['extrude', 'revolve']
  }
  if (every((item) => item.kind === 'sketch_point')) {
    return ['hole']
  }
  return []
}

/**
 * The commands the selection inspector suggests (3.8), as `feature.<type>` ids in display order.
 * A type the catalog does not have is left out; the inspector also leaves out a suggestion whose
 * command is disabled now (2.10).
 */
export function suggestedCommands(
  state: AppState,
  items: readonly SelectionItem[],
): readonly string[] {
  return suggestedTypes(state, items)
    .filter((type) => state.catalog?.byType[type] !== undefined)
    .map((type) => `feature.${type}`)
}
