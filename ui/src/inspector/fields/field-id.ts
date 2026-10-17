// Element ids of inspector fields, so a status message can focus the field it is about, and the
// focusing itself (ui.md 3.7).

/** The id of a field's input for its param path. */
export function fieldInputId(path: string): string {
  return `inspector-field-${path.replaceAll(/[^A-Za-z0-9_-]/g, '-')}`
}

/**
 * Focuses a field's input and scrolls its whole field, label included, into view; does nothing for
 * null, a field that is not shown. Focus alone would scroll only the input into view, which can
 * leave its label under the inspector's header.
 */
export function focusFieldInput(input: HTMLElement | null): void {
  if (input === null) {
    return
  }
  input.focus({ preventScroll: true })
  const field = input.closest('[data-testid="inspector-field"]') ?? input
  field.scrollIntoView({ block: 'nearest' })
}

/** Focuses the input of the field at a param path, as `focusFieldInput` does; nothing when it is not shown. */
export function focusField(path: string): void {
  focusFieldInput(document.getElementById(fieldInputId(path)))
}
