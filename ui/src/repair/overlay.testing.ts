// Readers of the overlay's SVG marks, for tests.

/** The numbers of an SVG path's `d`, in order. */
export function pathNumbers(element: Element): number[] {
  return (element.getAttribute('d')?.match(/-?\d+(?:\.\d+)?(?:e-?\d+)?/g) ?? []).map(Number)
}
