// Native popovers (menus, popovers, tooltips) anchored to their triggers (ui.md 3.2, 3.12). They
// sit in the top layer at `fixed top-0 left-0` and are moved next to their anchor at runtime.

import { useEffectEvent, useLayoutEffect, useRef, type RefObject } from 'react'
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

/**
 * Shows a popover element while `open` and hides it once closed, never touching one that was never
 * shown. `source` is the invoker `showPopover` names; `onShow` runs after each showing and again
 * whenever `source` changes while open.
 */
export function usePopoverVisibility(
  element: RefObject<HTMLElement | null>,
  open: boolean,
  source: RefObject<HTMLElement | null> | null,
  onShow?: () => void,
): void {
  const shown = useRef(false)
  const afterShow = useEffectEvent(() => {
    onShow?.()
  })
  useLayoutEffect(() => {
    const popover = element.current
    if (popover === null) {
      return
    }
    if (open) {
      showPopover(popover, source?.current ?? null)
      shown.current = true
      afterShow()
    } else if (shown.current) {
      hidePopover(popover)
    }
  }, [element, open, source])
}

/**
 * The box a floating element is placed against: its anchor's, spanning the whole height of the
 * one-row island (`data-bar`: top bar, timeline, status bar) the anchor sits in, so a menu or
 * tooltip of a bar's control keeps the gap from the island's edge instead of touching it (3.2).
 * An anchor inside a popup opened from a bar, such as a submenu's item in the Document menu, is
 * not a control of the bar and keeps its own box.
 */
function anchorBox(anchor: Element): DOMRect {
  const box = anchor.getBoundingClientRect()
  const bar = anchor.closest('[data-bar], [popover]')
  if (bar === null || !bar.hasAttribute('data-bar')) {
    return box
  }
  const island = bar.getBoundingClientRect()
  return DOMRect.fromRect({ x: box.x, y: island.y, width: box.width, height: island.height })
}

/** Moves a floating element next to its anchor (centered in the window without one). */
export function placeNextTo(
  element: HTMLElement,
  anchor: Element | null,
  placement: Placement,
): void {
  const document = element.ownerDocument
  const rem = remPixels(document)
  const position = placeFloating(
    anchor === null ? null : anchorBox(anchor),
    element.getBoundingClientRect(),
    {
      width: document.documentElement.clientWidth,
      height: document.documentElement.clientHeight,
    },
    { placement, gap: GAP_REM * rem, margin: MARGIN_REM * rem },
  )
  positionElement(element, position.x, position.y)
}

export type AnchoredPositionOptions = {
  /**
   * Also places the element again when its anchor moves without changing size, which no event
   * reports (content shown above an input while the user types moves it), by checking the anchor's
   * box once per animation frame while open; default false.
   */
  readonly followMoves?: boolean
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
  options: AnchoredPositionOptions = {},
): void {
  const followMoves = options.followMoves === true
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
    let frame: number | undefined
    let placedAt = anchor.current?.getBoundingClientRect() ?? null
    const follow = (): void => {
      const box = anchor.current?.getBoundingClientRect() ?? null
      if (box !== null && (placedAt === null || box.x !== placedAt.x || box.y !== placedAt.y)) {
        update()
      }
      placedAt = box
      frame = requestAnimationFrame(follow)
    }
    if (followMoves) {
      frame = requestAnimationFrame(follow)
    }
    return () => {
      view?.removeEventListener('resize', update)
      document.removeEventListener('scroll', update, { capture: true })
      observer?.disconnect()
      if (frame !== undefined) {
        cancelAnimationFrame(frame)
      }
    }
  }, [floating, anchor, open, placement, followMoves])
}
