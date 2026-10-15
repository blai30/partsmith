// Where the floating inspector opens (ui.md 3.7): beside the items it is about, inside the free
// area, clear of the viewport controls and the top-center slot's content where it still fits.

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

/**
 * The bounds the inspector opens in (3.7): the free area, with its left side moved to the right
 * edge of the viewport controls, its right side to the left edge of the view cube's reach and its
 * top side to the bottom of the top-center slot's content, each only where that still leaves the
 * panel's size plus two gaps. All rectangles are in the free-area layer's coordinates; `controls`,
 * `cube` and `slot` are null when not shown.
 */
export function inspectorBounds(input: {
  readonly area: Rectangle
  readonly controls: Rectangle | null
  readonly cube: Rectangle | null
  readonly slot: Rectangle | null
  readonly panel: { readonly width: number; readonly height: number }
  readonly gap?: number
}): Rectangle {
  const { area, controls, cube, slot, panel } = input
  const gap = input.gap ?? DEFAULT_GAP
  const bottom = area.y + area.height
  const left =
    controls !== null &&
    area.x + area.width - (controls.x + controls.width) >= panel.width + 2 * gap
      ? controls.x + controls.width
      : area.x
  const right =
    cube !== null && cube.x - left >= panel.width + 2 * gap ? cube.x : area.x + area.width
  const top =
    slot !== null && bottom - (slot.y + slot.height) >= panel.height + 2 * gap
      ? slot.y + slot.height
      : area.y
  return { x: left, y: top, width: right - left, height: bottom - top }
}
