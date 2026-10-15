import { useEffect, useEffectEvent, type RefObject } from 'react'
import { isNarrowWindow } from './docking'

// Narrow windows (ui.md 3.2): below 65rem the browser and the side panel float over the free area
// as overlay islands, and close on Esc or on a pointer press outside them.

/**
 * Closes the panel on Esc or a press outside it while the window is narrower than 65rem. A press on a
 * control of the panel (`aria-controls` naming the panel's id) is left to that control.
 */
export function useOverlayDismiss(
  panel: RefObject<HTMLElement | null>,
  onDismiss: () => void,
): void {
  const dismiss = useEffectEvent(onDismiss)
  useEffect(() => {
    const onPointerDown = (event: PointerEvent): void => {
      const element = panel.current
      const target = event.target
      if (
        !isNarrowWindow() ||
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
      dismiss()
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      // Menus and editors inside the panel take their Esc first and prevent its default.
      if (event.key === 'Escape' && !event.defaultPrevented && isNarrowWindow()) {
        event.preventDefault()
        dismiss()
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
