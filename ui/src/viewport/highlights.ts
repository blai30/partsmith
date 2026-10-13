import type { App } from '../app/createApp'
import { createLatestOnly, type LatestOnly } from '../core/queue'
import type {
  DocHandle,
  ElementDescriptor,
  ElementRef,
  MeshElement,
  Reference,
  Resolution,
} from '../generated/protocol'
import type { AppState } from '../state/store'
import type { PickResult } from './picking'
import {
  commandSelectionFields,
  selectionFieldAt,
  type CommandSelectionField,
} from './picking-rules'
import type { Scene } from './scene'

// The model elements the viewport draws selected or hovered that need the core (ui.md 3.6, 3.7):
// selected faces, edges and vertices and the command's field values resolve to display ids of the
// current meshes, and selected features light up the faces they made.

export type Highlights = {
  /** Follows a store change: references are resolved again when the document, selection or command changed. `previous` is null on start. */
  storeChanged(state: AppState, previous: AppState | null): void
  /** Hands the scene the display ids to draw selected and hovered, and asks for a frame. */
  update(): void
  /** The keys of the element references the state highlights, taken before a click. */
  referenceKeys(state: AppState): ReadonlySet<string>
  /** Remembers a clicked element's display id, so it is highlighted without a core round trip. */
  rememberClick(before: ReadonlySet<string>, result: Extract<PickResult, { kind: 'mesh' }>): void
  /** Whether a mesh pick is one of the elements the state highlights as selected. */
  isPicked(state: AppState, result: Extract<PickResult, { kind: 'mesh' }>): boolean
  dispose(): void
}

/** A resolved reference: its display ids in the mesh its body had when it was resolved. */
type ResolvedReference = {
  readonly body: string
  readonly meshId: number
  /** Null until the reference is resolved; selected elements found by a click are known at once. */
  readonly matches: readonly MeshElement[] | null
}

/** A face list of one body, in list_elements order (display ids are positions). */
type FaceList = { readonly meshId: number; readonly faces: readonly ElementDescriptor[] }

type FaceRequest = { readonly documentHandle: DocHandle; readonly meshId: number }

/** `ElementHighlights` while they are collected. */
type MutableElementHighlights = Record<
  string,
  { meshId: number; face: number[]; edge: number[]; vertex: number[] }
>

/** The entry of a body for a mesh, made (replacing one for another mesh) when there is none. */
function highlightEntry(
  highlights: MutableElementHighlights,
  body: string,
  meshId: number,
): MutableElementHighlights[string] {
  const existing = highlights[body]
  if (existing !== undefined && existing.meshId === meshId) {
    return existing
  }
  const created = { meshId, face: [], edge: [], vertex: [] }
  highlights[body] = created
  return created
}

function referenceKey(body: string, reference: Reference): string {
  return `${body}|${reference.q}|${reference.fp}`
}

function isReference(value: unknown): value is Reference {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof Reflect.get(value, 'q') === 'string' &&
    typeof Reflect.get(value, 'fp') === 'string'
  )
}

/** The element references the viewport highlights: the app selection's elements, or the command's field values. */
function highlightedReferences(state: AppState): {
  readonly references: readonly ElementRef[]
  /** Selection keys of the app selection's elements by reference key, to drop lost ones. */
  readonly selectionKeys: ReadonlyMap<string, string>
} {
  const references: ElementRef[] = []
  const selectionKeys = new Map<string, string>()
  if (state.command === null) {
    for (const item of state.selection) {
      if (item.element !== null) {
        const key = referenceKey(item.element.body, item.element.reference)
        references.push({ body: item.element.body, reference: item.element.reference })
        selectionKeys.set(key, item.key)
      }
    }
    return { references, selectionKeys }
  }
  for (const field of commandSelectionFields(state)) {
    references.push(...fieldReferences(field))
  }
  return { references, selectionKeys }
}

/** The model elements a command selection field names: its faces or edges, or the face or edge of a plane or axis. */
function fieldReferences(field: CommandSelectionField): ElementRef[] {
  const references: ElementRef[] = []
  const values = Array.isArray(field.value) ? field.value : [field.value]
  const body = field.siblings['body']
  for (const value of values) {
    if ((field.accepts === 'face' || field.accepts === 'edge') && typeof body === 'string') {
      if (isReference(value)) {
        references.push({ body, reference: value })
      }
      continue
    }
    if (field.accepts !== 'plane' && field.accepts !== 'axis') {
      continue
    }
    if (typeof value !== 'object' || value === null) {
      continue
    }
    const valueBody: unknown = Reflect.get(value, 'body')
    const element: unknown = Reflect.get(value, 'face') ?? Reflect.get(value, 'edge')
    if (typeof valueBody === 'string' && isReference(element)) {
      references.push({ body: valueBody, reference: element })
    }
  }
  return references
}

/** The model elements of the selection field whose box the pointer is over (3.7), or none. */
function hoveredFieldReferences(state: AppState): readonly ElementRef[] {
  const field = selectionFieldAt(state, state.command?.hoveredField ?? null)
  return field === undefined ? [] : fieldReferences(field)
}

/** The features whose faces are highlighted: the selected ones while no command is active (3.6). */
function selectedFeatures(state: AppState): ReadonlySet<string> {
  if (state.command !== null) {
    return new Set()
  }
  return new Set(
    state.selection.flatMap((item) =>
      (item.kind === 'feature' || item.kind === 'datum_plane') && item.feature !== null
        ? [item.feature]
        : [],
    ),
  )
}

/** Tracks the highlights that need the core; `onChange` asks for a frame after the scene got new ones. */
export function createHighlights(options: {
  app: App
  scene: Scene
  onChange: () => void
}): Highlights {
  const { app, scene, onChange } = options
  const store = app.store
  const resolved = new Map<string, ResolvedReference>()
  const faceLists = new Map<string, FaceList>()
  /** The mesh id each body's face list was last asked for. */
  const requestedFaces = new Map<string, number>()
  const faceRequests = new Map<string, LatestOnly<FaceRequest>>()

  const resolutions = createLatestOnly<
    {
      readonly documentHandle: DocHandle
      readonly references: readonly ElementRef[]
      readonly meshIds: readonly number[]
    },
    readonly Resolution[]
  >(
    (request) => app.core.resolveReferences(request.documentHandle, request.references),
    (results, request) => {
      const lost = new Set<string>()
      request.references.forEach((reference, index) => {
        const key = referenceKey(reference.body, reference.reference)
        const resolution = results[index]
        const found = resolution?.status === 'resolved'
        resolved.set(key, {
          body: reference.body,
          meshId: request.meshIds[index] ?? 0,
          matches: found ? resolution.matches : [],
        })
        if (!found) {
          lost.add(key)
        }
      })
      dropLostSelection(lost)
      update()
    },
    (error) => {
      console.warn('Partsmith could not highlight the selection after a rebuild.', error)
    },
  )

  /** Selected faces, edges and vertices that no longer resolve leave the selection silently (3.6). */
  function dropLostSelection(lost: ReadonlySet<string>): void {
    if (lost.size === 0) {
      return
    }
    const state = store.getState()
    if (state.command !== null) {
      return
    }
    const { selectionKeys } = highlightedReferences(state)
    const keys = new Set(
      Array.from(lost).flatMap((key) => {
        const selectionKey = selectionKeys.get(key)
        return selectionKey === undefined ? [] : [selectionKey]
      }),
    )
    if (keys.size > 0) {
      store.setState((current) => ({
        selection: current.selection.filter((item) => !keys.has(item.key)),
      }))
    }
  }

  /** Resolves the highlighted references whose body got a new mesh, or that were never resolved during a command. */
  function resolveStale(state: AppState): void {
    const document = state.document
    if (document === null) {
      return
    }
    const { references } = highlightedReferences(state)
    const stale: ElementRef[] = []
    const meshIds: number[] = []
    for (const reference of references) {
      const meshId = document.meshes[reference.body]?.meshId
      if (meshId === undefined) {
        continue
      }
      const key = referenceKey(reference.body, reference.reference)
      const known = resolved.get(key)
      if (known === undefined && state.command === null) {
        // Selected before the viewport knew it: highlighted once its body is rebuilt.
        resolved.set(key, { body: reference.body, meshId, matches: null })
        continue
      }
      if (known === undefined || known.meshId !== meshId) {
        stale.push(reference)
        meshIds.push(meshId)
      }
    }
    if (stale.length > 0) {
      resolutions.submit({ documentHandle: document.documentHandle, references: stale, meshIds })
    }
  }

  function faceRequestOf(body: string): LatestOnly<FaceRequest> {
    let request = faceRequests.get(body)
    if (request === undefined) {
      request = createLatestOnly<FaceRequest, readonly ElementDescriptor[]>(
        (input) => app.core.listElements(input.documentHandle, body, 'face'),
        (faces, input) => {
          const current = store.getState().document?.meshes[body]?.meshId
          if (current !== input.meshId) {
            // The body was rebuilt while the list was on its way: ask again for the new mesh.
            requestedFaces.delete(body)
            requestFaces(store.getState())
            return
          }
          faceLists.set(body, { meshId: input.meshId, faces })
          update()
        },
        (error) => {
          console.warn('Partsmith could not list the faces to highlight.', error)
        },
      )
      faceRequests.set(body, request)
    }
    return request
  }

  /** Lists the faces of visible bodies while features are selected (3.6 feature highlighting). */
  function requestFaces(state: AppState): void {
    const document = state.document
    if (document === null || selectedFeatures(state).size === 0) {
      return
    }
    for (const body of document.recipe.bodies ?? []) {
      const mesh = document.meshes[body.id]
      if (body.visible === false || mesh === undefined) {
        continue
      }
      const cached = faceLists.get(body.id)
      if (cached?.meshId === mesh.meshId || requestedFaces.get(body.id) === mesh.meshId) {
        continue
      }
      faceLists.delete(body.id)
      requestedFaces.set(body.id, mesh.meshId)
      faceRequestOf(body.id).submit({
        documentHandle: document.documentHandle,
        meshId: mesh.meshId,
      })
    }
  }

  /** The display ids the resolved references stand for, per body, at the current meshes. */
  function resolvedIndices(
    state: AppState,
    references: readonly ElementRef[],
  ): MutableElementHighlights {
    const document = state.document
    const highlights: MutableElementHighlights = {}
    if (document === null) {
      return highlights
    }
    for (const reference of references) {
      const known = resolved.get(referenceKey(reference.body, reference.reference))
      for (const match of known?.matches ?? []) {
        if (document.meshes[match.body]?.meshId !== match.mesh_id) {
          continue
        }
        highlightEntry(highlights, match.body, match.mesh_id)[match.kind].push(match.index)
      }
    }
    return highlights
  }

  function update(): void {
    const state = store.getState()
    const document = state.document
    const highlights = resolvedIndices(state, highlightedReferences(state).references)
    if (document !== null) {
      const features = selectedFeatures(state)
      if (features.size > 0) {
        for (const [body, list] of faceLists) {
          if (document.meshes[body]?.meshId !== list.meshId) {
            continue
          }
          list.faces.forEach((face, index) => {
            if (features.has(face.feature)) {
              highlightEntry(highlights, body, list.meshId).face.push(index)
            }
          })
        }
      }
    }
    scene.setElementHighlights(highlights)
    scene.setFieldHover(resolvedIndices(state, hoveredFieldReferences(state)))
    onChange()
  }

  return {
    storeChanged(state, previous) {
      const changed =
        previous === null ||
        state.document !== previous.document ||
        state.selection !== previous.selection ||
        state.command !== previous.command
      if (!changed) {
        return
      }
      if (state.document?.documentHandle !== previous?.document?.documentHandle) {
        resolved.clear()
        faceLists.clear()
        requestedFaces.clear()
      }
      resolveStale(state)
      requestFaces(state)
      update()
    },

    update,

    referenceKeys(state) {
      return new Set(
        highlightedReferences(state).references.map((reference) =>
          referenceKey(reference.body, reference.reference),
        ),
      )
    },

    rememberClick(before, result) {
      const { references } = highlightedReferences(store.getState())
      const added = references.filter(
        (reference) =>
          !before.has(referenceKey(reference.body, reference.reference)) &&
          reference.body === result.body,
      )
      // A click adds one element; anything else resolves through the core.
      const [reference, ...others] = added
      if (reference === undefined || others.length > 0) {
        return
      }
      resolved.set(referenceKey(reference.body, reference.reference), {
        body: result.body,
        meshId: result.meshId,
        matches: [
          { body: result.body, mesh_id: result.meshId, kind: result.element, index: result.index },
        ],
      })
    },

    isPicked(state, result) {
      return highlightedReferences(state).references.some((reference) =>
        (resolved.get(referenceKey(reference.body, reference.reference))?.matches ?? []).some(
          (match) =>
            match.body === result.body &&
            match.mesh_id === result.meshId &&
            match.kind === result.element &&
            match.index === result.index,
        ),
      )
    },

    dispose() {
      resolutions.cancel()
      for (const request of faceRequests.values()) {
        request.cancel()
      }
    },
  }
}
