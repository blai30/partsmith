import { clsx } from 'clsx/lite'
import {
  cloneElement,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent,
  type ReactElement,
} from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { hidePopover, showPopover, useAnchoredPosition } from './floating'

/** How long the pointer rests on the trigger before the tooltip appears (ui.md 3.24). */
const SHOW_DELAY_MILLISECONDS = 500
/** How long the tooltip stays after the pointer leaves, so the pointer can move onto it (WCAG 1.4.13). */
const HIDE_DELAY_MILLISECONDS = 100

export type TooltipProps = {
  /** The command title or the element's name. */
  readonly label: string
  /** The formatted shortcut (formatShortcut), or null. */
  readonly shortcut?: string | null
  /** The one element the tooltip describes; it must pass `aria-describedby` on to its DOM element. */
  readonly children: ReactElement<{ readonly 'aria-describedby'?: string | undefined }>
}

/** A mouse or pen rests on elements; a touch does not hover. */
function hovers(event: PointerEvent): boolean {
  return event.pointerType !== 'touch'
}

/**
 * Describes its trigger with a label and shortcut: shown after 500 ms of hover and at once on
 * keyboard focus; closed by Esc, by pressing the trigger, and when both the pointer and keyboard
 * focus have left (ui.md 3.24). The tooltip stays in the DOM, referenced by the trigger's
 * aria-describedby. Trigger and tooltip share a `display: contents` wrapper that takes their
 * pointer and focus events, so the trigger lays out as a direct child of its container.
 */
export function Tooltip({ label, shortcut = null, children }: TooltipProps) {
  const id = useId()
  const wrapper = useRef<HTMLDivElement>(null)
  const tooltip = useRef<HTMLDivElement>(null)
  const trigger = useRef<Element | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  // Whether the last interaction was a key press, so the next focus counts as keyboard focus.
  const keyboardInteraction = useRef(false)
  // Whether the trigger has keyboard focus, which keeps the tooltip open when the pointer leaves.
  const keyboardFocus = useRef(false)
  const [visible, setVisible] = useState(false)

  const clearTimer = useCallback(() => {
    clearTimeout(timer.current)
    timer.current = undefined
  }, [])

  const setVisibleAfter = useCallback(
    (next: boolean, delay: number) => {
      clearTimer()
      timer.current = setTimeout(() => {
        timer.current = undefined
        setVisible(next)
      }, delay)
    },
    [clearTimer],
  )

  const hideNow = useCallback(() => {
    clearTimer()
    setVisible(false)
  }, [clearTimer])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      keyboardInteraction.current = true
      if (event.key === 'Escape') {
        hideNow()
      }
    }
    const onPointerDown = (): void => {
      keyboardInteraction.current = false
    }
    document.addEventListener('keydown', onKeyDown, true)
    document.addEventListener('pointerdown', onPointerDown, true)
    return () => {
      document.removeEventListener('keydown', onKeyDown, true)
      document.removeEventListener('pointerdown', onPointerDown, true)
      clearTimer()
    }
  }, [clearTimer, hideNow])

  // Whether the tooltip has been shown, so hiding never touches a popover that never opened.
  const shown = useRef(false)
  useLayoutEffect(() => {
    const element = tooltip.current
    if (element === null) {
      return
    }
    if (visible) {
      trigger.current = wrapper.current?.firstElementChild ?? null
      showPopover(element, null)
      shown.current = true
    } else if (shown.current) {
      hidePopover(element)
    }
  }, [visible])
  useAnchoredPosition(tooltip, trigger, visible, 'below')

  const describedBy = children.props['aria-describedby']
  return (
    <div
      ref={wrapper}
      className="contents"
      onPointerEnter={(event) => {
        if (!hovers(event)) {
          return
        }
        if (visible) {
          clearTimer()
        } else {
          setVisibleAfter(true, SHOW_DELAY_MILLISECONDS)
        }
      }}
      onPointerLeave={(event) => {
        if (hovers(event) && !keyboardFocus.current) {
          setVisibleAfter(false, HIDE_DELAY_MILLISECONDS)
        }
      }}
      onPointerDown={() => {
        keyboardFocus.current = false
        hideNow()
      }}
      onFocus={() => {
        if (keyboardInteraction.current) {
          keyboardFocus.current = true
          clearTimer()
          setVisible(true)
        }
      }}
      onBlur={() => {
        keyboardFocus.current = false
        hideNow()
      }}
    >
      {cloneElement(children, {
        'aria-describedby': describedBy === undefined ? id : `${describedBy} ${id}`,
      })}
      <div
        ref={tooltip}
        id={id}
        role="tooltip"
        popover="manual"
        className={clsx(
          'fixed top-0 left-0 m-0 flex max-w-80 items-center gap-3 rounded-md border px-2 py-1 text-xs shadow-lg transition-opacity duration-150 ease-out starting:open:opacity-0 motion-reduce:transition-none',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.text.foreground,
        )}
      >
        <span>{label}</span>
        {shortcut === null ? null : (
          <span className={COLOR_CLASSES.textMuted.foreground}>{shortcut}</span>
        )}
      </div>
    </div>
  )
}
