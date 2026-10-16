import { createLatestOnly } from '../core/queue'
import type { DocHandle, ElementRef, Resolution } from '../generated/protocol'
import { parameterKey } from '../inspector/selection-values'
import type { AppState, DocumentState, SelectionItem } from '../state/store'
import type { App } from './createApp'

// Keeps the selection valid across rebuilds (ui.md 3.6): items whose feature, body, parameter or
// sketch item is gone leave the selection, and selected faces, edges and vertices are resolved again
// whenever a body gets a new model mesh. What a `show` action selected stops being shown once the
// selection changes (4.2).

/** Whether an item still names something in the document. */
function exists(document: DocumentState, item: SelectionItem): boolean {
  const recipe = document.recipe
  switch (item.kind) {
    case 'body':
      return (recipe.bodies ?? []).some((body) => body.id === item.body)
    case 'feature':
    case 'datum_plane':
      return (recipe.features ?? []).some((feature) => feature.id === item.feature)
    case 'parameter':
      return (recipe.parameters ?? []).some((parameter) => parameterKey(parameter.id) === item.key)
    default:
      break
  }
  const sketchItem = item.sketchItem
  if (sketchItem === null) {
    return true
  }
  const solution = document.sketches[sketchItem.sketch]?.solution
  if (!(recipe.features ?? []).some((feature) => feature.id === sketchItem.sketch)) {
    return false
  }
  if (solution === undefined) {
    return true
  }
  switch (sketchItem.kind) {
    case 'entity':
      return solution.entities.some((entity) => entity.id === sketchItem.entity)
    case 'point':
      return solution.points.some((point) => point.point === sketchItem.point)
    case 'constraint':
      return solution.constraints.some((constraint) => constraint.id === sketchItem.constraint)
    case 'region':
      return solution.regions.some(
        (region) => region.reference.key === sketchItem.region.reference.key,
      )
    case 'origin':
      return true
  }
}

function isElement(item: SelectionItem): boolean {
  return (
    item.element !== null &&
    (item.kind === 'face' || item.kind === 'edge' || item.kind === 'vertex')
  )
}

/** Whether some body got a new model-layer mesh between two document states. */
function remeshed(document: DocumentState, previous: DocumentState | null): boolean {
  return Object.entries(document.meshes).some(
    ([body, mesh]) => previous?.meshes[body]?.meshId !== mesh.meshId,
  )
}

type ResolutionRequest = {
  readonly documentHandle: DocHandle
  readonly items: readonly SelectionItem[]
}

/** Starts keeping the selection valid; returns the function that stops it. */
export function installSelectionUpkeep(app: App): () => void {
  const resolutions = createLatestOnly<ResolutionRequest, readonly Resolution[]>(
    (request) =>
      app.core.resolveReferences(
        request.documentHandle,
        request.items.flatMap((item): ElementRef[] =>
          item.element === null
            ? []
            : [{ body: item.element.body, reference: item.element.reference }],
        ),
      ),
    (results, request) => {
      const lost = new Set(
        request.items
          .filter((_item, index) => results[index]?.status !== 'resolved')
          .map((item) => item.key),
      )
      if (lost.size > 0) {
        app.store.setState((state) => ({
          selection: state.selection.filter((item) => !lost.has(item.key)),
        }))
      }
    },
    (error) => {
      console.warn('Partsmith could not check the selection after a rebuild.', error)
    },
  )

  function prune(state: AppState, document: DocumentState): void {
    const selection = state.selection.filter((item) => exists(document, item))
    const sketchSelection = state.sketch?.selection.filter((item) => exists(document, item))
    const selectionChanged = selection.length !== state.selection.length
    const sketchChanged =
      state.sketch !== null &&
      sketchSelection !== undefined &&
      sketchSelection.length !== state.sketch.selection.length
    if (selectionChanged || sketchChanged) {
      app.store.setState({
        ...(selectionChanged ? { selection } : {}),
        ...(sketchChanged && state.sketch !== null && sketchSelection !== undefined
          ? { sketch: { ...state.sketch, selection: sketchSelection } }
          : {}),
      })
    }
  }

  return app.store.subscribe((state, previous) => {
    if (
      state.shown !== null &&
      state.shown === previous.shown &&
      state.selection !== previous.selection
    ) {
      app.store.setState({ shown: null })
    }
    const document = state.document
    if (document === null || document === previous.document) {
      return
    }
    if (
      document.recipe !== previous.document?.recipe ||
      document.sketches !== previous.document?.sketches
    ) {
      prune(state, document)
    }
    if (!remeshed(document, previous.document)) {
      return
    }
    // Each body's mesh arrives in its own event, and a newer request replaces a waiting one, so
    // every request covers all selected elements, not only those of the body just remeshed.
    const items = app.store.getState().selection.filter(isElement)
    if (items.length > 0) {
      resolutions.submit({ documentHandle: document.documentHandle, items })
    }
  })
}
