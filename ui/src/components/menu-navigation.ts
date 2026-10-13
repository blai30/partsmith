// What every menu in a native popover shares (WAI-ARIA menu button pattern): showing it under its
// button, learning that the browser light-dismissed it, and moving focus between its items.

import { useEffect, useEffectEvent, type PointerEvent, type RefObject } from 'react'
import { useAnchoredPosition, usePopoverVisibility } from './floating'
import type { Placement } from './placement'

/**
 * Shows a menu's popover element under its anchor when the menu mounts and keeps it placed there.
 * `onDismiss` runs when the browser light-dismisses the menu (a press outside it); `onShow` runs
 * after each showing, on mounting and whenever the anchor changes.
 */
export function useMenuPopover(
  element: RefObject<HTMLElement | null>,
  anchor: RefObject<HTMLElement | null>,
  placement: Placement,
  onDismiss: () => void,
  onShow?: () => void,
): void {
  const dismissed = useEffectEvent(onDismiss)
  useEffect(() => {
    const menu = element.current
    if (menu === null) {
      return undefined
    }
    const onToggle = (event: ToggleEvent): void => {
      if (event.newState === 'closed') {
        dismissed()
      }
    }
    menu.addEventListener('toggle', onToggle)
    return () => {
      menu.removeEventListener('toggle', onToggle)
    }
  }, [element])

  usePopoverVisibility(element, true, anchor, onShow)
  useAnchoredPosition(element, anchor, true, placement)
}

/**
 * The index of the next item from `start` in `direction` that `isFocusable` accepts, wrapping
 * around the `count` items; `start` when there is none. A `start` of -1 or `count` begins before
 * the first or after the last item.
 */
export function nextIndex(
  count: number,
  start: number,
  direction: 1 | -1,
  isFocusable: (index: number) => boolean = () => true,
): number {
  for (let step = 1; step <= count; step++) {
    const index = (((start + direction * step) % count) + count) % count
    if (isFocusable(index)) {
      return index
    }
  }
  return start
}

/**
 * The index of the first item after `from`, wrapping around, whose label starts with the typed
 * character, ignoring case; null when none does. Items without a label (separators) are skipped.
 */
export function typeaheadIndex(
  labels: readonly (string | null)[],
  from: number,
  character: string,
): number | null {
  const wanted = character.toLocaleLowerCase()
  for (let step = 1; step <= labels.length; step++) {
    const index = (from + step) % labels.length
    if (labels[index]?.toLocaleLowerCase().startsWith(wanted) === true) {
      return index
    }
  }
  return null
}

/**
 * Focuses the item under a moving mouse or pen, as hovering does in a menu. Returns false for
 * touch, which never hovers and leaves focus where it is.
 */
export function focusOnPointerMove(event: PointerEvent<HTMLElement>): boolean {
  if (event.pointerType === 'touch') {
    return false
  }
  const item = event.currentTarget
  if (item.ownerDocument.activeElement !== item) {
    item.focus({ preventScroll: true })
  }
  return true
}
