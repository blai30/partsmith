import { createContext, useContext } from 'react'

// The viewport's canvas-aligned host (ui.md 3.2): layers that work in canvas pixels, such as the
// sketch overlay, render into it through a portal, outside the free-area layer, while their
// chrome stays in the free area.

/** The canvas-aligned host of the enclosing viewport, or null outside a viewport. */
export const CanvasLayerContext = createContext<HTMLElement | null>(null)

/** The canvas-aligned host of the enclosing viewport, or null outside one (or before it mounts). */
export function useCanvasLayer(): HTMLElement | null {
  return useContext(CanvasLayerContext)
}
