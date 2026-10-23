import type { SketchOverlay } from './SketchOverlay'

// Sketch mode's overlay is a chunk of its own, loaded with a dynamic import() (ui.md 3.1,
// delivery.md 2.6): the shell starts loading it as it first renders, and startup never waits
// for it.

type SketchOverlayComponent = typeof SketchOverlay

let chunk: Promise<SketchOverlayComponent> | null = null
let loaded: SketchOverlayComponent | null = null

/** Loads the sketch overlay's chunk, once, and resolves with its component. */
export function loadSketchOverlay(): Promise<SketchOverlayComponent> {
  chunk ??= import('./SketchOverlay').then((module) => {
    loaded = module.SketchOverlay
    return loaded
  })
  return chunk
}

/** The sketch overlay's component once its chunk has loaded, else null. */
export function loadedSketchOverlay(): SketchOverlayComponent | null {
  return loaded
}
