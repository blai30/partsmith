import { clsx } from 'clsx/lite'
import { useEffect, useLayoutEffect, useRef, type ReactNode, type RefObject } from 'react'
import { COLOR_CLASSES } from '../theme/roles'
import { hidePopover, showPopover, useAnchoredPosition } from './floating'

/** Elements that take keyboard focus, in the order a popover tries them when it opens. */
const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])'

export type PopoverProps = {
  /** The dialog's accessible name. */
  readonly label: string
  readonly open: boolean
  /** The trigger the popover is placed under and returns focus to. */
  readonly anchor: RefObject<HTMLElement | null>
  /** Called with false when the browser light-dismisses the popover (Esc or a press outside). */
  readonly onOpenChange: (open: boolean) => void
  readonly children: ReactNode
  /** Layout classes for the content, such as a width or padding. */
  readonly className?: string
}

/**
 * A non-modal dialog in a native light-dismiss popover, anchored under its trigger. It stays in
 * the DOM while closed. Opening moves focus into it: to an element marked `data-autofocus` (React
 * never writes the `autofocus` attribute), else its first focusable element. Esc returns focus to
 * the trigger.
 */
export function Popover({ label, open, anchor, onOpenChange, children, className }: PopoverProps) {
  const element = useRef<HTMLDialogElement>(null)
  // The latest props for the toggle listener, which the browser calls outside React's render.
  const latest = useRef({ open, onOpenChange })
  useLayoutEffect(() => {
    latest.current = { open, onOpenChange }
  })

  useEffect(() => {
    const popover = element.current
    if (popover === null) {
      return undefined
    }
    let focusWasInside = false
    const onBeforeToggle = (event: ToggleEvent): void => {
      if (event.newState === 'closed') {
        focusWasInside = popover.contains(popover.ownerDocument.activeElement)
      }
    }
    const onToggle = (event: ToggleEvent): void => {
      if (event.newState !== 'closed') {
        return
      }
      // Browsers return focus only to an element that had it at opening, which Safari never
      // gives a clicked button; focus that would fall to the body goes back to the trigger.
      const active = popover.ownerDocument.activeElement
      if (focusWasInside && (active === null || active === popover.ownerDocument.body)) {
        anchor.current?.focus()
      }
      if (latest.current.open) {
        latest.current.onOpenChange(false)
      }
    }
    popover.addEventListener('beforetoggle', onBeforeToggle)
    popover.addEventListener('toggle', onToggle)
    return () => {
      popover.removeEventListener('beforetoggle', onBeforeToggle)
      popover.removeEventListener('toggle', onToggle)
    }
  }, [anchor])

  // Whether the popover has been shown, so closing never touches a popover that never opened.
  const shown = useRef(false)
  useLayoutEffect(() => {
    const popover = element.current
    if (popover === null) {
      return
    }
    if (!open) {
      if (shown.current) {
        hidePopover(popover)
      }
      return
    }
    showPopover(popover, anchor.current)
    shown.current = true
    if (popover.contains(popover.ownerDocument.activeElement)) {
      return
    }
    const target =
      popover.querySelector<HTMLElement>('[data-autofocus], [autofocus]') ??
      popover.querySelector<HTMLElement>(FOCUSABLE) ??
      popover
    target.focus()
  }, [open, anchor])
  useAnchoredPosition(element, anchor, open, 'below')

  return (
    <dialog
      ref={element}
      aria-label={label}
      popover="auto"
      tabIndex={-1}
      className={clsx(
        'fixed top-0 left-0 m-0 rounded-md border p-4 text-sm shadow-lg transition-opacity duration-150 ease-out starting:open:opacity-0 motion-reduce:transition-none',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
        className,
      )}
    >
      {children}
    </dialog>
  )
}
