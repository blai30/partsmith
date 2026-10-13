// Roving tab stops (ui.md 3.24): one item of a toolbar, the timeline strip or the browser tree is in
// the tab order, and arrow keys, Home and End move focus between the items.

/** Makes one item the container's tab stop: the focused one while it is there, else the first. */
export function rovingTabStop(
  container: HTMLElement,
  selector: string,
  focused: Element | null,
): void {
  const items = [...container.querySelectorAll<HTMLElement>(selector)]
  const stop = items.find((item) => item === focused) ?? items[0]
  for (const item of items) {
    item.tabIndex = item === stop ? 0 : -1
  }
}

/**
 * The index of the item a key moves focus to from the item at `index`, of items up to `last`; null
 * for a key that does not move focus. With `wrap` the arrows go around the ends, else they stop there.
 */
export function arrowTarget(
  key: string,
  index: number,
  last: number,
  options: { readonly wrap: boolean },
): number | null {
  switch (key) {
    case 'ArrowRight':
      return options.wrap ? (index === last ? 0 : index + 1) : Math.min(index + 1, last)
    case 'ArrowLeft':
      return options.wrap ? (index === 0 ? last : index - 1) : Math.max(index - 1, 0)
    case 'Home':
      return 0
    case 'End':
      return last
    default:
      return null
  }
}
