/** The ids of a control's descriptions that are shown, joined, or undefined when none is. */
export function describedBy(...ids: readonly (string | null | undefined)[]): string | undefined {
  const shown = ids.filter((id) => id !== null && id !== undefined && id !== '')
  return shown.length === 0 ? undefined : shown.join(' ')
}
