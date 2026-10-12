import type { Orientation } from '../generated/protocol'
import type { AppState } from '../state/store'

// The session print orientation of Lay on face (ui.md 3.17): how the bodies are placed on the bed
// for print checks and 3MF and STL export. Null prints as modeled.

/** The orientation the core is sent for a session orientation: null is as modeled. */
export function orientationToSend(orientation: Orientation | null): Orientation {
  return orientation ?? { kind: 'as_modeled' }
}

/** Whether two session orientations place the bodies the same way; null and as modeled are one. */
export function sameOrientation(first: Orientation | null, second: Orientation | null): boolean {
  return (
    first === second ||
    JSON.stringify(orientationToSend(first)) === JSON.stringify(orientationToSend(second))
  )
}

/** The name of the body whose picked face rests on the bed, or null while printing as modeled. */
export function selectOrientationBody(state: AppState): string | null {
  const orientation = state.print.orientation
  if (orientation?.kind !== 'face_down') {
    return null
  }
  const body = state.document?.recipe.bodies?.find((candidate) => candidate.id === orientation.body)
  return body?.name ?? orientation.body
}
