import { runOfferedFix } from '../app/actions/fixes'
import { setHover } from '../app/actions/selection'
import type { App } from '../app/createApp'
import type { MeshElement } from '../generated/protocol'
import type { HoverTarget } from '../state/store'
import { meshElementKey } from '../viewport/scene'
import type { RepairCandidateView } from './repair'

// What a candidate's button in the banner and its marker on the model do (ui.md 3.7).

/** The hover target of a candidate's element: the mesh key the scene draws hovered (3.6). */
function elementHover(element: MeshElement): HoverTarget {
  return {
    key: meshElementKey(element.mesh_id, element.kind, element.index),
    kind: element.kind,
  }
}

/**
 * The handlers of a candidate's button or marker (3.7): hovering or focusing it draws its element
 * hovered, and a click runs the message action that stores it, unless it waits for the preview.
 */
export function candidateHandlers(app: App, candidate: RepairCandidateView, onRan?: () => void) {
  const hover = candidate.element === null ? null : elementHover(candidate.element)
  function enter(): void {
    if (hover !== null) {
      setHover(app, hover)
    }
  }
  function leave(): void {
    if (hover !== null && app.store.getState().hover?.key === hover.key) {
      setHover(app, null)
    }
  }
  return {
    onPointerEnter: enter,
    onPointerLeave: leave,
    onFocus: enter,
    onBlur: leave,
    onClick: () => {
      if (candidate.action !== null) {
        runOfferedFix(app, candidate.action, 'banner', { onRan })
      }
    },
  }
}
