import { useEffect, useRef, useState, type PointerEvent, type RefObject } from 'react'
import type { RollbackDrag } from '../app/actions/timeline'
import { reportFailure } from '../app/report'
import { positionElement } from '../viewport/overlay'
import { gapPositions, nearestGap } from './strip'

// Dragging on the timeline strip (ui.md 3.9): an item to reorder it, or the rollback marker to roll
// back live. A press becomes a drag once it moves past a threshold; Esc cancels it.

/** A press must move this far (CSS pixels) before it drags an item or the marker. */
const DRAG_THRESHOLD_PIXELS = 4

/** The primary button's bit in `MouseEvent.buttons`. */
const PRIMARY_BUTTON_BIT = 1

type Press = {
  readonly pointerId: number
  readonly startX: number
  /** The dragged feature, or null for the rollback marker. */
  readonly featureId: string | null
  readonly index: number
  dragging: boolean
  gaps: readonly number[]
  gap: number
  rollback: RollbackDrag | null
}

export type StripDragOptions = {
  /** Whether a press may start a drag now. */
  readonly canPress: () => boolean
  /** Starts dragging the marker from its gap; null when the roll back cannot start. */
  readonly onRollback: (index: number) => RollbackDrag | null
  /** Moves the dropped feature to the timeline index `target`. */
  readonly onMove: (featureId: string, target: number) => void
}

/** The strip's drag: what is dragged, the drop bar and ghost elements, and the pointer handlers. */
export function useStripDrag(strip: RefObject<HTMLDivElement | null>, options: StripDragOptions) {
  const ghost = useRef<HTMLDivElement>(null)
  const bar = useRef<HTMLDivElement>(null)
  const press = useRef<Press | null>(null)
  const [dragging, setDragging] = useState<string | 'marker' | null>(null)

  // Esc cancels a drag (3.9).
  useEffect(() => {
    if (dragging === null) {
      return undefined
    }
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      if (event.key === 'Escape') {
        event.preventDefault()
        const current = press.current
        press.current = null
        setDragging(null)
        current?.rollback?.cancel().catch(reportFailure('Canceling the roll back'))
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
    }
  }, [dragging])

  function startPress(
    event: PointerEvent<HTMLElement>,
    featureId: string | null,
    index: number,
  ): void {
    if (event.button !== 0 || !options.canPress()) {
      return
    }
    press.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      featureId,
      index,
      dragging: false,
      gaps: [],
      gap: index,
      rollback: null,
    }
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>): void {
    const current = press.current
    const element = strip.current
    if (current === null || element === null || current.pointerId !== event.pointerId) {
      return
    }
    if (!current.dragging) {
      // Not captured yet, a press released outside the strip never reaches it; it ends here.
      if ((event.buttons & PRIMARY_BUTTON_BIT) === 0) {
        press.current = null
        return
      }
      if (Math.abs(event.clientX - current.startX) <= DRAG_THRESHOLD_PIXELS) {
        return
      }
      current.dragging = true
      // The strip captures the pointer only once a drag starts: captured at the press, the
      // release, click and double-click would go to the strip and never reach the item's button.
      // jsdom has no pointer capture.
      element.setPointerCapture?.(event.pointerId)
      current.gaps = gapPositions(element)
      if (current.featureId === null) {
        current.rollback = options.onRollback(current.index)
      }
      setDragging(current.featureId ?? 'marker')
    }
    current.gap = nearestGap(current.gaps, event.clientX)
    current.rollback?.move(current.gap)
    // The ghost and the bar sit in the strip's scrolled content.
    const left = element.getBoundingClientRect().left - element.scrollLeft
    if (bar.current !== null) {
      positionElement(bar.current, (current.gaps[current.gap] ?? left) - left, 0)
    }
    if (ghost.current !== null) {
      positionElement(ghost.current, event.clientX - left - ghost.current.offsetWidth / 2, 0)
    }
  }

  function onPointerUp(event: PointerEvent<HTMLDivElement>): void {
    const current = press.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    press.current = null
    if (!current.dragging) {
      return
    }
    setDragging(null)
    if (current.featureId === null) {
      current.rollback?.drop().catch(reportFailure('Rolling back'))
      return
    }
    // Dropping at gap g moves the feature to g before its own index, else to g - 1 (3.9).
    const target = current.gap < current.index ? current.gap : current.gap - 1
    if (target !== current.index) {
      options.onMove(current.featureId, target)
    }
  }

  function onPointerCancel(event: PointerEvent<HTMLDivElement>): void {
    const current = press.current
    if (current === null || current.pointerId !== event.pointerId) {
      return
    }
    press.current = null
    setDragging(null)
    current.rollback?.cancel().catch(reportFailure('Canceling the roll back'))
  }

  return { dragging, ghost, bar, startPress, onPointerMove, onPointerUp, onPointerCancel }
}
