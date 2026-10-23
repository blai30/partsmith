import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent,
  type RefObject,
} from 'react'
import type { RollbackDrag } from '../app/actions/timeline'
import { reportFailure } from '../app/report'
import { measureTiming, TIMING_NAMES } from '../performance/marks'
import { positionElement } from '../viewport/overlay'
import { dropIndex, gapPositions, nearestGap } from './strip'
import type { DropWarning } from './uses'

// Dragging on the timeline strip (ui.md 3.9): an item to reorder it, or the rollback marker to roll
// back live. A press becomes a drag once it moves past a threshold; Esc cancels it. While an item
// is dragged, each pointer move checks what a drop at the nearest gap would break
// (`ps:drop-check`).

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
  /** The client x of the nearest gap, where the insertion bar is. */
  barX: number
  /** What a drop at `gap` would break, as last checked. */
  warning: DropWarning | null
  rollback: RollbackDrag | null
}

export type StripDragOptions = {
  /** Whether a press may start a drag now. */
  readonly canPress: () => boolean
  /** Starts dragging the marker from its gap; null when the roll back cannot start. */
  readonly onRollback: (index: number) => RollbackDrag | null
  /** Starts dragging a feature; resolves once what every feature uses is known (3.9). */
  readonly onFeatureDrag: (featureId: string) => Promise<void>
  /** What a drop of the feature at gap `gap` would break, or null (3.9). */
  readonly dropWarning: (featureId: string, gap: number) => DropWarning | null
  /** Moves the dropped feature to the timeline index `target`. */
  readonly onMove: (featureId: string, target: number) => void
}

function sameWarning(first: DropWarning | null, second: DropWarning | null): boolean {
  return (
    first === second ||
    (first !== null &&
      second !== null &&
      first.user === second.user &&
      first.feature === second.feature &&
      first.what === second.what &&
      first.moved === second.moved)
  )
}

/**
 * Places the drop hint over the insertion bar at client x `barX`, inside its parent (3.12), whose
 * padding box it is positioned in.
 */
function placeHint(element: HTMLElement | null, barX: number): void {
  const container = element?.parentElement ?? null
  if (element === null || container === null) {
    return
  }
  const left = container.getBoundingClientRect().left + container.clientLeft
  const centered = barX - left - element.offsetWidth / 2
  const rightmost = Math.max(0, container.clientWidth - element.offsetWidth)
  positionElement(element, Math.min(Math.max(centered, 0), rightmost), 0)
}

/**
 * The strip's drag: what is dragged and what a drop there would break, the drop bar, ghost and
 * hint elements, and the pointer handlers. The hint lies outside the strip, in its parent, so the
 * strip's scrolling never clips it.
 */
export function useStripDrag(strip: RefObject<HTMLDivElement | null>, options: StripDragOptions) {
  const ghost = useRef<HTMLDivElement>(null)
  const bar = useRef<HTMLDivElement>(null)
  const hint = useRef<HTMLDivElement>(null)
  const press = useRef<Press | null>(null)
  const [dragging, setDragging] = useState<string | 'marker' | null>(null)
  const [warning, setWarning] = useState<DropWarning | null>(null)

  /** Ends the drag: nothing dragged, nothing warned. */
  function endDrag(): void {
    setDragging(null)
    setWarning(null)
  }

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
        setWarning(null)
        current?.rollback?.cancel().catch(reportFailure('Canceling the roll back'))
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
    }
  }, [dragging])

  // A warning that a check just showed or changed is placed once it is drawn.
  useLayoutEffect(() => {
    const current = press.current
    if (warning !== null && current !== null) {
      placeHint(hint.current, current.barX)
    }
  }, [warning])

  /** Shows what a drop at the nearest gap would break, when that changed. */
  function showWarning(current: Press, found: DropWarning | null): void {
    if (!sameWarning(found, current.warning)) {
      current.warning = found
      setWarning(found)
    }
  }

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
      barX: event.clientX,
      warning: null,
      rollback: null,
    }
  }

  /** Starts the drag of a press that moved past the threshold. */
  function startDrag(current: Press, element: HTMLDivElement, pointerId: number): void {
    current.dragging = true
    // The strip captures the pointer only once a drag starts: captured at the press, the release,
    // click and double-click would go to the strip and never reach the item's button. jsdom has no
    // pointer capture.
    element.setPointerCapture?.(pointerId)
    current.gaps = gapPositions(element)
    const { featureId } = current
    if (featureId === null) {
      current.rollback = options.onRollback(current.index)
    } else {
      // Uses that arrive during the drag apply to the gap the pointer is over already.
      options
        .onFeatureDrag(featureId)
        .then(() => {
          if (press.current === current) {
            showWarning(current, options.dropWarning(featureId, current.gap))
          }
        })
        .catch(reportFailure('Asking what the features use'))
    }
    setDragging(featureId ?? 'marker')
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>): void {
    const moved = performance.now()
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
      startDrag(current, element, event.pointerId)
    }
    current.gap = nearestGap(current.gaps, event.clientX)
    current.rollback?.move(current.gap)
    if (current.featureId !== null) {
      const found = options.dropWarning(current.featureId, current.gap)
      measureTiming(TIMING_NAMES.dropCheck, moved)
      showWarning(current, found)
    }
    // The ghost and the bar sit in the strip's scrolled content.
    const left = element.getBoundingClientRect().left - element.scrollLeft
    current.barX = current.gaps[current.gap] ?? left
    if (bar.current !== null) {
      positionElement(bar.current, current.barX - left, 0)
    }
    if (ghost.current !== null) {
      positionElement(ghost.current, event.clientX - left - ghost.current.offsetWidth / 2, 0)
    }
    placeHint(hint.current, current.barX)
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
    endDrag()
    if (current.featureId === null) {
      current.rollback?.drop().catch(reportFailure('Rolling back'))
      return
    }
    // A drop that would break a use is allowed: the warning asked, and Undo restores (3.9).
    const target = dropIndex(current.index, current.gap)
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
    endDrag()
    current.rollback?.cancel().catch(reportFailure('Canceling the roll back'))
  }

  return {
    dragging,
    warning,
    ghost,
    bar,
    hint,
    startPress,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
  }
}
