// Where a floating element (menu, popover, tooltip) goes next to its anchor, in CSS pixels of the
// window. The result is applied with positionElement (ui.md 3.12).

export type Rectangle = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export type Size = { readonly width: number; readonly height: number }

/**
 * `below`: under the anchor, aligned to its start, flipped above when only that side has room
 * (menus, popovers, tooltips). `right`: beside the anchor, aligned to its top, flipped to the left
 * when only that side has room (submenus).
 */
export type Placement = 'below' | 'right'

export type PlacementOptions = {
  readonly placement: Placement
  /** Distance between the anchor and the floating element. */
  readonly gap: number
  /** The least distance kept from the window's edges. */
  readonly margin: number
}

/** Clamps a value into [minimum, maximum], preferring minimum when the range is empty. */
function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(value, maximum))
}

/**
 * The start of the floating element along one axis: after the anchor when it fits there or has
 * more room than before it, else before it; then kept inside the window.
 */
function along(
  anchorStart: number,
  anchorEnd: number,
  size: number,
  window: number,
  gap: number,
  margin: number,
): number {
  const after = anchorEnd + gap
  const before = anchorStart - gap - size
  const roomAfter = window - margin - after
  const roomBefore = anchorStart - gap - margin
  const start = roomAfter >= size || roomAfter >= roomBefore ? after : before
  return clamp(start, margin, window - margin - size)
}

/** The top-left corner of a floating element of `size` next to `anchor` inside a window of `window`. */
export function placeFloating(
  anchor: Rectangle | null,
  size: Size,
  window: Size,
  options: PlacementOptions,
): { readonly x: number; readonly y: number } {
  const { gap, margin } = options
  const maximumX = window.width - margin - size.width
  const maximumY = window.height - margin - size.height
  if (anchor === null) {
    return {
      x: clamp((window.width - size.width) / 2, margin, maximumX),
      y: clamp((window.height - size.height) / 2, margin, maximumY),
    }
  }
  if (options.placement === 'below') {
    return {
      x: clamp(anchor.x, margin, maximumX),
      y: along(anchor.y, anchor.y + anchor.height, size.height, window.height, gap, margin),
    }
  }
  return {
    x: along(anchor.x, anchor.x + anchor.width, size.width, window.width, gap, margin),
    y: clamp(anchor.y, margin, maximumY),
  }
}
