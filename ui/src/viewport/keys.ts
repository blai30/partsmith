import type { ElementKind } from '../generated/protocol'

/** The hover and pick key of a mesh element by display id: `mesh:{meshId}:{kind}:{index}` (3.6). */
export function meshElementKey(meshId: number, kind: ElementKind, index: number): string {
  return `mesh:${String(meshId)}:${kind}:${String(index)}`
}
