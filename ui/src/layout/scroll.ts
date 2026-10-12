// Keeping an item of a scrolling list in view (ui.md 3.4, 3.9) by moving its container's scroll
// position only, so the window and other scrolling ancestors never move. The container must be the
// item's offset parent (positioned).

/** Scrolls the container vertically just enough to show the whole item. */
export function keepInViewVertically(container: HTMLElement, item: HTMLElement): void {
  const top = item.offsetTop
  const bottom = top + item.offsetHeight
  if (top < container.scrollTop) {
    container.scrollTop = top
  } else if (bottom > container.scrollTop + container.clientHeight) {
    container.scrollTop = bottom - container.clientHeight
  }
}

/** Scrolls the container horizontally just enough to show the whole item. */
export function keepInViewHorizontally(container: HTMLElement, item: HTMLElement): void {
  const left = item.offsetLeft
  const right = left + item.offsetWidth
  if (left < container.scrollLeft) {
    container.scrollLeft = left
  } else if (right > container.scrollLeft + container.clientWidth) {
    container.scrollLeft = right - container.clientWidth
  }
}
