import type { CoreEvent } from '../core/events'
import type { DocHandle, Document } from '../generated/protocol'
import type { AppStore, DocumentState, FileReference } from './store'

// Applies core events to the store (ui.md 2.7). The store shows one document, the one an action
// adopted. Events for a document that is not adopted yet (the reply events of new_document and
// open_document arrive before the request resolves) are kept until an action adopts it.

/** What an action knows about a document that the core does not. */
export type DocumentDetails = {
  readonly name: string
  readonly file: FileReference | null
  readonly localUnsaved: boolean
}

type DocumentRegistry = {
  /** Documents the core reported that no action has adopted yet. */
  readonly pending: Map<DocHandle, DocumentState>
  /** An adopted document whose first document_changed has not arrived yet. */
  awaiting: ({ readonly documentHandle: DocHandle } & DocumentDetails) | null
}

const registries = new WeakMap<AppStore, DocumentRegistry>()

function registryOf(store: AppStore): DocumentRegistry {
  let registry = registries.get(store)
  if (registry === undefined) {
    registry = { pending: new Map(), awaiting: null }
    registries.set(store, registry)
  }
  return registry
}

function newDocumentState(
  documentHandle: DocHandle,
  revision: number,
  recipe: Document,
  details: DocumentDetails,
): DocumentState {
  return {
    documentHandle,
    name: details.name,
    file: details.file,
    revision,
    recipe,
    dirty: false,
    localUnsaved: details.localUnsaved,
    undoLabel: null,
    redoLabel: null,
    transactionOpen: false,
    focus: null,
    featureStatus: {},
    parameterValues: {},
    diagnostics: [],
    sketches: {},
    datums: {},
    meshes: {},
    lastRebuildMilliseconds: null,
  }
}

/** Keeps only the entries whose key passes `keep`. */
function filtered<Value>(
  record: Readonly<Record<string, Value>>,
  keep: (key: string) => boolean,
): Readonly<Record<string, Value>> {
  const entries = Object.entries(record)
  const kept = entries.filter(([key]) => keep(key))
  return kept.length === entries.length ? record : Object.fromEntries(kept)
}

/** The document with one event applied, or null when the revision rule ignores the event. */
function applied(document: DocumentState, event: CoreEvent): DocumentState | null {
  if (event.revision < document.revision) {
    return null
  }
  const revision = event.revision
  switch (event.type) {
    case 'document_changed': {
      const bodies = new Set((event.document.bodies ?? []).map((body) => body.id))
      const features = new Set((event.document.features ?? []).map((feature) => feature.id))
      return {
        ...document,
        revision,
        recipe: event.document,
        dirty: event.dirty,
        undoLabel: event.undo,
        redoLabel: event.redo,
        transactionOpen: event.transactionOpen,
        focus: event.focus,
        // Results of bodies and features the recipe no longer has are gone.
        meshes: filtered(document.meshes, (body) => bodies.has(body)),
        sketches: filtered(document.sketches, (feature) => features.has(feature)),
        datums: filtered(document.datums, (feature) => features.has(feature)),
      }
    }
    case 'recomputed':
      return {
        ...document,
        revision,
        featureStatus: Object.fromEntries(event.features.map((status) => [status.id, status])),
        parameterValues: Object.fromEntries(event.parameters.map((value) => [value.id, value])),
        diagnostics: event.diagnostics,
      }
    case 'sketch_solved':
      return {
        ...document,
        revision,
        sketches: {
          ...document.sketches,
          [event.solution.feature]: { solution: event.solution, display: event.display },
        },
      }
    case 'datum_plane':
      return {
        ...document,
        revision,
        datums: { ...document.datums, [event.feature]: event.placement },
      }
    case 'body_mesh': {
      if (event.layer !== 'model') {
        // Preview meshes are the scene's; the store keeps the pickable model layer only.
        return { ...document, revision }
      }
      const others = filtered(document.meshes, (body) => body !== event.body)
      const mesh = event.mesh
      return {
        ...document,
        revision,
        meshes:
          mesh === null
            ? others
            : {
                ...others,
                [event.body]: {
                  meshId: event.meshId,
                  boundingBox: mesh.boundingBox,
                  faceCount: mesh.faceCount,
                  edgeCount: mesh.edgeCount,
                  vertexCount: mesh.vertexCount,
                },
              },
      }
    }
    case 'print_health':
      return { ...document, revision }
  }
}

/** Applies one core event to the store, following the revision rule of ui.md 2.7. */
export function applyCoreEvent(store: AppStore, event: CoreEvent): void {
  const registry = registryOf(store)
  const current = store.getState().document
  if (current !== null && current.documentHandle === event.documentHandle) {
    const next = applied(current, event)
    if (next === null) {
      return
    }
    if (event.type === 'print_health') {
      store.setState((state) => ({
        document: next,
        print: { ...state.print, report: event.report, reportRevision: event.revision },
      }))
      return
    }
    store.setState({ document: next })
    return
  }
  const awaiting = registry.awaiting
  if (awaiting !== null && awaiting.documentHandle === event.documentHandle) {
    if (event.type !== 'document_changed') {
      return
    }
    registry.awaiting = null
    const document = newDocumentState(
      event.documentHandle,
      event.revision,
      event.document,
      awaiting,
    )
    store.setState({ document: applied(document, event) ?? document })
    return
  }
  const pending = registry.pending.get(event.documentHandle)
  if (pending === undefined) {
    if (event.type === 'document_changed') {
      const document = newDocumentState(event.documentHandle, event.revision, event.document, {
        name: '',
        file: null,
        localUnsaved: false,
      })
      registry.pending.set(event.documentHandle, applied(document, event) ?? document)
    }
    return
  }
  const next = applied(pending, event)
  if (next !== null) {
    registry.pending.set(event.documentHandle, next)
  }
}

/**
 * Makes the document with this handle the one the store shows, with the details only the UI
 * knows. Events the core already sent for it are applied; otherwise the document appears with its
 * first document_changed. Other documents the core reported are forgotten.
 */
export function adoptDocument(
  store: AppStore,
  documentHandle: DocHandle,
  details: DocumentDetails,
): void {
  const registry = registryOf(store)
  const pending = registry.pending.get(documentHandle)
  registry.pending.clear()
  if (pending === undefined) {
    registry.awaiting = { documentHandle, ...details }
    store.setState({ document: null })
    return
  }
  registry.awaiting = null
  store.setState({ document: { ...pending, ...details } })
}

/**
 * Records how long the core took to rebuild a document (3.21), on the shown document or on one the
 * core reported that an action has not adopted yet (an opened file's first rebuild).
 */
export function recordRebuildTime(
  store: AppStore,
  documentHandle: DocHandle,
  milliseconds: number,
): void {
  const current = store.getState().document
  if (current !== null && current.documentHandle === documentHandle) {
    store.setState({ document: { ...current, lastRebuildMilliseconds: milliseconds } })
    return
  }
  const registry = registryOf(store)
  const pending = registry.pending.get(documentHandle)
  if (pending !== undefined) {
    registry.pending.set(documentHandle, { ...pending, lastRebuildMilliseconds: milliseconds })
  }
}

/** Forgets the shown document and every document the core reported, as after a core restart. */
export function releaseDocuments(store: AppStore): void {
  const registry = registryOf(store)
  registry.pending.clear()
  registry.awaiting = null
  store.setState({ document: null })
}
