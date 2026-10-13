// Where focus goes when the control that had it goes away (ui.md 3.24): a plane button replacing
// the Sketch button, the 3D view once a sketch is edited, a panel's opener once the panel closes.

/** The id of the 3D view's focusable surface (`role="application"`, 3.13). */
export const VIEW_ID = 'view'

/** Whether focus is on nothing, as after the focused element was removed. */
export function focusLost(): boolean {
  const active = document.activeElement
  return active === null || active === document.body
}

/** Whether an element is the 3D view's focusable surface. */
export function isView(target: Element): boolean {
  return target.id === VIEW_ID
}

/** Moves focus to the 3D view. */
export function focusView(): void {
  document.getElementById(VIEW_ID)?.focus()
}

/** Input types that take no typed text, so their keys stay shortcuts. */
const NON_TEXT_INPUT_TYPES: ReadonlySet<string> = new Set([
  'button',
  'checkbox',
  'color',
  'file',
  'hidden',
  'image',
  'radio',
  'range',
  'reset',
  'submit',
])

/** An input of a text-like type, a textarea, a select, or content-editable element. */
export function isEditable(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) {
    return !NON_TEXT_INPUT_TYPES.has(target.type)
  }
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return true
  }
  return target instanceof HTMLElement && target.isContentEditable
}

/** Whether a menu or a popover (light-dismiss popovers; tooltips are manual) is open. */
export function hasOpenPopover(document: Document): boolean {
  return document.querySelector('[popover=""]:popover-open, [popover="auto"]:popover-open') !== null
}
