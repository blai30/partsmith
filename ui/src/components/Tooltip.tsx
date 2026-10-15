import { clsx } from 'clsx/lite'
import {
  cloneElement,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type PointerEvent,
  type ReactElement,
} from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { describedBy } from './described-by'
import { useAnchoredPosition, usePopoverVisibility } from './floating'

/** How long the pointer rests on the trigger before the tooltip appears (ui.md 3.24). */
const SHOW_DELAY_MILLISECONDS = 500
/** How long the tooltip stays after the pointer leaves, so the pointer can move onto it (WCAG 1.4.13). */
const HIDE_DELAY_MILLISECONDS = 100

export type TooltipProps = {
  /**
   * The command title or the element's name; null shows no tooltip while keeping the wrapper, so a
   * trigger whose tooltip comes and goes keeps its identity (and its focus).
   */
  readonly label: string | null
  /** The formatted shortcut (formatShortcut), or null. */
  readonly shortcut?: string | null
  /** What the command does (2.10), on a second line in the muted text role, or null. */
  readonly description?: string | null
  /** The one element the tooltip describes; it must pass `aria-describedby` on to its DOM element. */
  readonly children: ReactElement<{ readonly 'aria-describedby'?: string | undefined }>
}

/** A mouse or pen rests on elements; a touch does not hover. */
function hovers(event: PointerEvent): boolean {
  return event.pointerType !== 'touch'
}

/**
 * Describes its trigger with a label and shortcut, and a description on a second line: shown after 500 ms of hover and at once on
 * keyboard focus, except focus that follows an Esc; closed by Esc, by pressing the trigger, and
 * when both the pointer and keyboard focus have left (ui.md 3.24). The tooltip stays in the DOM, referenced by the trigger's
 * aria-describedby. Trigger and tooltip share a `display: contents` wrapper that takes their
 * pointer and focus events, so the trigger lays out as a direct child of its container (use
 * `gap` there rather than `space-*` or `divide-*`, which style the wrapper).
 */
export function Tooltip({ label, shortcut = null, description = null, children }: TooltipProps) {
  const id = useId()
  const wrapper = useRef<HTMLSpanElement>(null)
  const tooltip = useRef<HTMLSpanElement>(null)
  const trigger = useRef<Element | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  // Whether the last interaction was a key press other than Esc, so the next focus counts as
  // keyboard focus. Esc never opens a tooltip: the browser gives focus back to the trigger of the
  // popover an Esc closes while it is still closing it, and showing a tooltip then throws (3.24).
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
      const escape = event.key === 'Escape'
      keyboardInteraction.current = !escape
      if (escape) {
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

  // Showing records the trigger, so the tooltip is placed under it.
  usePopoverVisibility(tooltip, visible, null, () => {
    trigger.current = wrapper.current?.firstElementChild ?? null
  })
  useAnchoredPosition(tooltip, trigger, visible, 'below')
  // A label that goes away closes the tooltip, so it never reopens on its own when one returns.
  if (label === null && visible) {
    setVisible(false)
  }

  // Spans, so a tooltip is valid wherever its trigger (phrasing content such as a button) is.
  return (
    <span
      ref={wrapper}
      className="contents"
      onPointerEnter={(event) => {
        if (!hovers(event) || label === null) {
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
        if (keyboardInteraction.current && label !== null) {
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
      {label === null
        ? children
        : cloneElement(children, {
            'aria-describedby': describedBy(children.props['aria-describedby'], id),
          })}
      {label === null ? null : (
        <span
          ref={tooltip}
          id={id}
          role="tooltip"
          popover="manual"
          // `hidden open:flex`: a display class would override the browser's hiding of a closed popover.
          className={clsx(
            'fixed top-0 left-0 m-0 hidden max-w-80 flex-col gap-0.5 open:flex rounded-md border px-2 py-1 text-xs shadow-lg transition-opacity duration-150 ease-out starting:open:opacity-0 motion-reduce:transition-none',
            COLOR_CLASSES.surfaceRaised.background,
            COLOR_CLASSES.border.border,
            COLOR_CLASSES.text.foreground,
          )}
        >
          <span className="flex items-center gap-3">
            <span>{label}</span>
            {shortcut === null ? null : (
              <span className={COLOR_CLASSES.textMuted.foreground}>{shortcut}</span>
            )}
          </span>
          {description === null ? null : (
            <span className={COLOR_CLASSES.textMuted.foreground}>{description}</span>
          )}
        </span>
      )}
    </span>
  )
}
