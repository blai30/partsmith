import { useEffect, useRef, type RefObject } from 'react'

// Narrow windows (ui.md 3.2): below the lg breakpoint the browser and the side panel open as
// overlays above the viewport, and close on Esc or on a pointer press outside them.

/** Tailwind's lg breakpoint; media queries measure rem in the browser's default font size. */
const NARROW_QUERY = '(width < 64rem)'

function narrow(): boolean {
  return typeof matchMedia === 'function' && matchMedia(NARROW_QUERY).matches
}

/**
 * Closes the panel on Esc or a press outside it while the window is narrower than lg. A press on a
 * control of the panel (`aria-controls` naming the panel's id) is left to that control.
 */
export function useOverlayDismiss(
  panel: RefObject<HTMLElement | null>,
  onDismiss: () => void,
): void {
  const latest = useRef(onDismiss)
  useEffect(() => {
    latest.current = onDismiss
  })
  useEffect(() => {
    const onPointerDown = (event: PointerEvent): void => {
      const element = panel.current
      const target = event.target
      if (
        !narrow() ||
        element === null ||
        !(target instanceof Element) ||
        element.contains(target)
      ) {
        return
      }
      const control = target.closest('[aria-controls]')
      if (
        element.id !== '' &&
        control?.getAttribute('aria-controls')?.split(' ').includes(element.id)
      ) {
        return
      }
      latest.current()
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      // Menus and editors inside the panel take their Esc first and prevent its default.
      if (event.key === 'Escape' && !event.defaultPrevented && narrow()) {
        event.preventDefault()
        latest.current()
      }
    }
    document.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [panel])
}
