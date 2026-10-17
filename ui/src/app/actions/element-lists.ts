import type { ElementDescriptor, ElementKind } from '../../generated/protocol'
import type { AppStore } from '../../state/store'
import type { App } from '../createApp'

// The face, edge and vertex lists of bodies (`list_elements`), asked for once per model-layer mesh
// (ui.md 3.6, 3.7): a display id is a position in its kind's list for the mesh it was listed for.

type ListedElements = {
  readonly meshId: number
  readonly list: Promise<readonly ElementDescriptor[]>
}

const listsByStore = new WeakMap<AppStore, Map<string, ListedElements>>()

/**
 * The elements of `kind` of a body as its model-layer mesh `meshId` shows them, listed once per
 * mesh and body; null without a document or when `meshId` is not the body's current model mesh.
 * A failed list is asked for again next time.
 */
export function listedElements(
  app: App,
  body: string,
  kind: ElementKind,
  meshId: number,
): Promise<readonly ElementDescriptor[]> | null {
  const document = app.store.getState().document
  if (document === null || document.meshes[body]?.meshId !== meshId) {
    return null
  }
  let lists = listsByStore.get(app.store)
  if (lists === undefined) {
    lists = new Map()
    listsByStore.set(app.store, lists)
  }
  const key = `${body}:${kind}`
  const cached = lists.get(key)
  if (cached?.meshId === meshId) {
    return cached.list
  }
  const list = app.core.listElements(document.documentHandle, body, kind)
  const entry = { meshId, list }
  lists.set(key, entry)
  list.catch(() => {
    if (lists.get(key) === entry) {
      lists.delete(key)
    }
  })
  return list
}

/** The elements `listedElements` gives, or none without them or when the list failed. */
export function listedElementsOrNone(
  app: App,
  body: string,
  kind: ElementKind,
  meshId: number,
): Promise<readonly ElementDescriptor[]> {
  return (listedElements(app, body, kind, meshId) ?? Promise.resolve([])).catch(() => [])
}
