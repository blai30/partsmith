// Where the floating inspector opens (ui.md 3.7): beside the items it is about, inside the viewport.

/** A rectangle in CSS pixels. */
export type Rectangle = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** The default distance between the inspector, its anchor and the viewport's edges, in CSS pixels. */
const DEFAULT_GAP = 16

/**
 * The inspector's top-left corner: right of the anchor when it fits, else left of it, else the
 * bounds' top-left corner; vertically level with the anchor but kept a gap inside the bounds.
 */
export function placeInspector(input: {
  readonly anchor: Rectangle | null
  readonly panel: { width: number; height: number }
  readonly bounds: Rectangle
  readonly gap?: number
}): { x: number; y: number } {
  const { anchor, panel, bounds } = input
  const gap = input.gap ?? DEFAULT_GAP
  const corner = { x: bounds.x + gap, y: bounds.y + gap }
  if (anchor === null) {
    return corner
  }
  const right = anchor.x + anchor.width + gap
  const left = anchor.x - gap - panel.width
  let x: number
  if (right + panel.width + gap <= bounds.x + bounds.width) {
    x = right
  } else if (left >= bounds.x + gap) {
    x = left
  } else {
    return corner
  }
  const top = bounds.y + gap
  const bottom = bounds.y + bounds.height - panel.height - gap
  const y = panel.height > bounds.height - 2 * gap ? top : Math.min(Math.max(anchor.y, top), bottom)
  return { x, y }
}
