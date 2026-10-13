// The counted nouns of selection titles (ui.md 3.8).

/** "1 face", "3 edges": a count with its noun. */
export function counted(count: number, nouns: readonly [string, string]): string {
  return `${String(count)} ${count === 1 ? nouns[0] : nouns[1]}`
}

/** "1 item", "3 items": a count of mixed items. */
export function itemsCount(count: number): string {
  return `${String(count)} ${count === 1 ? 'item' : 'items'}`
}
