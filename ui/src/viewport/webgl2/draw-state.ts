// The fixed-function state and per-draw shader amounts of a draw. Pure, so it is unit-tested without WebGL.
import type { DrawCall, Geometry } from '../backend'
import type { Pass } from './shaders'

/** Which WebGL capabilities a draw turns on. */
export type DrawState = {
  readonly depthTest: boolean
  readonly depthWrite: boolean
  readonly blend: boolean
  readonly cullFace: boolean
  readonly polygonOffset: boolean
}

/** How far hover mixes a B-rep edge toward the highlight (3.6). */
export const EDGE_HOVER_HIGHLIGHT = 0.7

/**
 * The state of a draw in a pass. `sectioned` is whether the section clips this draw. The ID pass depth-tests everything (3.6), so overlay draws, which the color pass draws on top (3.12 pass 7), cannot be picked through bodies. Faces always use the polygon offset so coplanar edges and sketch geometry win, and with a section nothing is culled because the back faces are the cut cap.
 */
export function drawState(
  drawCall: DrawCall,
  kind: Geometry['kind'],
  pass: Pass,
  sectioned: boolean,
): DrawState {
  const triangles = kind === 'triangles'
  const surface = triangles || kind === 'fill'
  const faces = { cullFace: triangles && !sectioned, polygonOffset: triangles }
  if (pass === 'id') {
    return { ...faces, depthTest: true, depthWrite: true, blend: false }
  }
  const opaqueSurface = triangles && drawCall.layer === 'opaque' && drawCall.opacity >= 1
  return {
    ...faces,
    depthTest: drawCall.depthTest,
    depthWrite: surface && drawCall.layer === 'opaque',
    blend: !opaqueSurface,
  }
}

/**
 * How far hover mixes a line toward the highlight in the color pass: B-rep edges at 70 percent, sketch curves (the overlay layer) fully (3.6).
 */
export function hoverHighlight(drawCall: DrawCall): number {
  return drawCall.layer === 'overlay' ? 1 : EDGE_HOVER_HIGHLIGHT
}
