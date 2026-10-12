// Element ids of inspector fields, so a status message can focus the field it is about (ui.md 3.7).

/** The id of a field's input for its param path. */
export function fieldInputId(path: string): string {
  return `inspector-field-${path.replaceAll(/[^A-Za-z0-9_-]/g, '-')}`
}
