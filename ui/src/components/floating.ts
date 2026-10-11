// Native popovers (menus, popovers, tooltips) anchored to their triggers (ui.md 3.2, 3.12). They
// sit in the top layer at `fixed top-0 left-0` and are moved next to their anchor at runtime.

import { useLayoutEffect, type RefObject } from 'react'
import { positionElement } from '../viewport/overlay'
import { placeFloating, type Placement } from './placement'

/** Distance between a floating element and its anchor, in rem. */
const GAP_REM = 0.25
/** The least distance a floating element keeps from the window's edges, in rem. */
const MARGIN_REM = 0.5
/** CSS pixels per rem when the root font size cannot be read. */
const DEFAULT_REM_PIXELS = 16

/** CSS pixels per rem, which follow the UI scale (ui.md 2.16.9). */
function remPixels(document: Document): number {
  const size = Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
  return Number.isFinite(size) && size > 0 ? size : DEFAULT_REM_PIXELS
}

/** Whether a popover element is in the top layer. */
export function isPopoverShowing(element: HTMLElement): boolean {
  return element.matches(':popover-open')
}

/**
 * Shows a popover element in the top layer. The source makes the anchor its invoker, so clicking
 * the anchor does not light-dismiss it. Browsers without the Popover API leave it as it is.
 */
export function showPopover(element: HTMLElement, source: HTMLElement | null): void {
  if (!('showPopover' in element) || isPopoverShowing(element)) {
    return
  }
  element.showPopover(source === null ? undefined : { source })
}

/** Hides a popover element; does nothing when it is hidden or the Popover API is missing. */
export function hidePopover(element: HTMLElement): void {
  if ('togglePopover' in element) {
    element.togglePopover(false)
  }
}

/** Moves a floating element next to its anchor (centered in the window without one). */
export function placeNextTo(
  element: HTMLElement,
  anchor: Element | null,
  placement: Placement,
): void {
  const document = element.ownerDocument
  const rem = remPixels(document)
  const anchorBounds = anchor?.getBoundingClientRect() ?? null
  const bounds = element.getBoundingClientRect()
  const position = placeFloating(
    anchorBounds === null
      ? null
      : {
          x: anchorBounds.x,
          y: anchorBounds.y,
          width: anchorBounds.width,
          height: anchorBounds.height,
        },
    { width: bounds.width, height: bounds.height },
    {
      width: document.documentElement.clientWidth,
      height: document.documentElement.clientHeight,
    },
    { placement, gap: GAP_REM * rem, margin: MARGIN_REM * rem },
  )
  positionElement(element, position.x, position.y)
}

/**
 * While `open`, keeps a floating element next to its anchor: placed before the first paint, and
 * again when the window resizes, anything scrolls or either element changes size. Declare it after
 * the effect that shows the element, so the element has its size when it is first placed.
 */
export function useAnchoredPosition(
  floating: RefObject<HTMLElement | null>,
  anchor: RefObject<Element | null>,
  open: boolean,
  placement: Placement,
): void {
  useLayoutEffect(() => {
    const element = floating.current
    if (!open || element === null) {
      return undefined
    }
    const document = element.ownerDocument
    const view = document.defaultView
    const update = (): void => {
      placeNextTo(element, anchor.current, placement)
    }
    update()
    view?.addEventListener('resize', update)
    document.addEventListener('scroll', update, { capture: true, passive: true })
    const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : null
    observer?.observe(element)
    if (anchor.current !== null) {
      observer?.observe(anchor.current)
    }
    return () => {
      view?.removeEventListener('resize', update)
      document.removeEventListener('scroll', update, { capture: true })
      observer?.disconnect()
    }
  }, [floating, anchor, open, placement])
}
