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

/** A canvas position when there is one (null is behind the camera) and it lies in `area`, the free area (3.2), else null (3.12). */
export function visiblePosition(
  position: { readonly x: number; readonly y: number } | null,
  area: { readonly x: number; readonly y: number; readonly width: number; readonly height: number },
): { readonly x: number; readonly y: number } | null {
  const inside =
    position !== null &&
    position.x >= area.x &&
    position.y >= area.y &&
    position.x <= area.x + area.width &&
    position.y <= area.y + area.height
  return inside ? position : null
}

/**
 * Positions an element at (x, y) CSS pixels, or hides it with the `hidden` attribute when its anchor has no position (behind the camera or outside the free area, 3.12).
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

/** Whether two boxes overlap by more than an edge. */
function overlaps(first: DOMRectReadOnly, second: DOMRectReadOnly): boolean {
  return (
    first.left < second.right &&
    second.left < first.right &&
    first.top < second.bottom &&
    second.top < first.bottom
  )
}

/**
 * Hides each shown label whose box meets a card or chip of the free area (`data-card`, 3.12): half
 * under a card of its own tone, a label would read as part of it. A label centered on its anchor
 * is a wrapper around its chip, whose box is the one measured. Every box is read before any label
 * is hidden, so the layout is computed once.
 */
export function hideLabelsUnderCards(
  labels: Iterable<HTMLElement>,
  freeArea: Element | null,
): void {
  if (freeArea === null) {
    return
  }
  const cards = Array.from(freeArea.querySelectorAll('[data-card]'), (card) =>
    card.getBoundingClientRect(),
  )
  const covered = Array.from(labels).filter((label) => {
    if (label.hidden) {
      return false
    }
    const box = (label.firstElementChild ?? label).getBoundingClientRect()
    return cards.some((card) => overlaps(box, card))
  })
  for (const label of covered) {
    label.hidden = true
  }
}
