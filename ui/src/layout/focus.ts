// Where focus goes when the control that had it goes away (ui.md 3.24): a plane button replacing
// the Sketch button, the 3D view once a sketch is edited, a panel's opener once the panel closes.

/** The id of the 3D view's focusable surface (`role="application"`, 3.13). */
export const VIEW_ID = 'view'

/** Whether focus is on nothing, as after the focused element was removed. */
export function focusLost(): boolean {
  const active = document.activeElement
  return active === null || active === document.body
}

/** Moves focus to the 3D view. */
export function focusView(): void {
  document.getElementById(VIEW_ID)?.focus()
}
