import type { CoreEvent } from '../core/events'
import type { DocHandle, Document } from '../generated/protocol'
import { measureTiming, TIMING_NAMES } from '../performance/marks'
import type { AppStore, DocumentState, FileReference } from './store'

// Applies core events to the store (ui.md 2.7). The store shows one document, the one an action
// adopted. Events for a document that is not adopted yet (the reply events of new_document and
// open_document arrive before the request resolves) are kept until an action adopts it. A
// document_changed followed by the recomputed of the same document is measured as ps:apply-events
// (2.18).

/** What an action knows about a document that the core does not. */
export type DocumentDetails = {
  readonly name: string
  readonly file: FileReference | null
  readonly localUnsaved: boolean
  /**
   * The recovery session the document continues, as after a core restart (ui.md 3.19, 4.4), or
   * the one an action named for it; adopting a document without one starts a new session.
   */
  readonly recoveryId?: string
}

/** The details of an adopted document, with its recovery session. */
type AdoptedDetails = DocumentDetails & { readonly recoveryId: string }

type DocumentRegistry = {
  /** Documents the core reported that no action has adopted yet. */
  readonly pending: Map<DocHandle, DocumentState>
  /** An adopted document whose first document_changed has not arrived yet. */
  awaiting: ({ readonly documentHandle: DocHandle } & AdoptedDetails) | null
  /**
   * The document_changed that was the last event applied, and when its `applyCoreEvent` call
   * started: the start of ps:apply-events if the recomputed of that document comes next.
   */
  changed: { readonly documentHandle: DocHandle; readonly startedAt: number } | null
}

const registries = new WeakMap<AppStore, DocumentRegistry>()

function registryOf(store: AppStore): DocumentRegistry {
  let registry = registries.get(store)
  if (registry === undefined) {
    registry = { pending: new Map(), awaiting: null, changed: null }
    registries.set(store, registry)
  }
  return registry
}

function newDocumentState(
  documentHandle: DocHandle,
  revision: number,
  recipe: Document,
  details: AdoptedDetails,
): DocumentState {
  return {
    documentHandle,
    name: details.name,
    file: details.file,
    revision,
    recipe,
    dirty: false,
    localUnsaved: details.localUnsaved,
    recoveryId: details.recoveryId,
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
    bareNumbers: 0,
    ghosts: {},
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
        bareNumbers: event.bareNumbers,
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
        ghosts: event.ghosts,
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

/**
 * Applies one core event to the store, following the revision rule of ui.md 2.7, and records
 * ps:apply-events from a document_changed to the end of the recomputed of the same document that
 * comes right after it (2.18).
 */
export function applyCoreEvent(store: AppStore, event: CoreEvent): void {
  const startedAt = performance.now()
  const registry = registryOf(store)
  const changed = registry.changed
  registry.changed =
    event.type === 'document_changed' ? { documentHandle: event.documentHandle, startedAt } : null
  applyEvent(store, registry, event)
  if (event.type === 'recomputed' && changed?.documentHandle === event.documentHandle) {
    measureTiming(TIMING_NAMES.applyEvents, changed.startedAt)
  }
}

/** Applies one event to the shown document, the awaited one or a pending one. */
function applyEvent(store: AppStore, registry: DocumentRegistry, event: CoreEvent): void {
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
      // Adoption fills in the details, the recovery session included.
      const document = newDocumentState(event.documentHandle, event.revision, event.document, {
        name: '',
        file: null,
        localUnsaved: false,
        recoveryId: '',
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
  const adopted: AdoptedDetails = {
    ...details,
    recoveryId: details.recoveryId ?? crypto.randomUUID(),
  }
  const pending = registry.pending.get(documentHandle)
  registry.pending.clear()
  if (pending === undefined) {
    registry.awaiting = { documentHandle, ...adopted }
    store.setState({ document: null })
    return
  }
  registry.awaiting = null
  store.setState({ document: { ...pending, ...adopted } })
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
  registry.changed = null
  store.setState({ document: null })
}
