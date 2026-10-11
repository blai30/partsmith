// The only module in src/ that writes element styles: runtime positions of 3.12, through these two functions.

/** Moves an element to (x, y) CSS pixels from its layer's origin: sets only `element.style.translate` (`${x}px ${y}px`) and marks the element `data-positioned`. */
export function positionElement(element: HTMLElement, x: number, y: number): void {
  element.style.translate = `${String(x)}px ${String(y)}px`
  element.setAttribute('data-positioned', '')
}

/** Sets only `element.style.transform` to the CSS `matrix3d` of a column-major 4 × 4 matrix and marks the element `data-positioned`. */
export function orientElement(element: HTMLElement, matrix: Float32Array): void {
  element.style.transform = `matrix3d(${Array.from(matrix, String).join(', ')})`
  element.setAttribute('data-positioned', '')
}

/**
 * Positions an element at (x, y) CSS pixels, or hides it with the `hidden` attribute when its anchor has no position (behind the camera or outside the canvas, 3.12).
 */
export function placeElement(
  element: HTMLElement,
  position: { readonly x: number; readonly y: number } | null,
): void {
  if (position === null) {
    element.hidden = true
    return
  }
  element.hidden = false
  positionElement(element, position.x, position.y)
}
