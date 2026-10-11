import type { CommandCategory } from './registry'

/** Previous text characters that make the next character a word start. */
const WORD_SEPARATORS: ReadonlySet<string> = new Set([' ', '-', '_', '/', '.', ':', '('])
const LOWERCASE_LETTER = /^\p{Ll}$/u
const UPPERCASE_LETTER = /^\p{Lu}$/u

const MATCH_POINTS = 1
const FIRST_POSITION_POINTS = 8
const WORD_START_POINTS = 6
const CONSECUTIVE_POINTS = 4
const LENGTH_PENALTY = 0.01
const KEYWORD_PENALTY = 2

const MAXIMUM_RESULTS = 50
const MAXIMUM_RECENT_FIRST = 5

/** Category order of commands with an empty query (ui.md 2.10). */
const CATEGORY_ORDER: readonly CommandCategory[] = [
  'File',
  'Edit',
  'View',
  'Create',
  'Modify',
  'Pattern',
  'Construct',
  'Sketch',
  'Constrain',
  'Print',
  'Settings',
]

function lowercase(text: string): string {
  return text.toLocaleLowerCase('en')
}

/** The query as matched: trimmed, inner whitespace runs as single spaces, lowercase. */
function normalizedQuery(query: string): string {
  return lowercase(query.trim().replaceAll(/\s+/gu, ' '))
}

/** The text in lowercase with one UTF-16 unit per unit of the text, so positions line up. */
function lowercaseAligned(text: string): string {
  const lowered = lowercase(text)
  if (lowered.length === text.length) {
    return lowered
  }
  // A few characters lowercase to another length (İ); such a character keeps its own case.
  return text
    .split('')
    .map((unit) => {
      const lower = lowercase(unit)
      return lower.length === 1 ? lower : unit
    })
    .join('')
}

function isSubsequence(query: string, text: string): boolean {
  let matched = 0
  for (let position = 0; position < text.length && matched < query.length; position++) {
    if (text[position] === query[matched]) {
      matched++
    }
  }
  return matched === query.length
}

function pointsAt(text: string, position: number): number {
  if (position === 0) {
    return MATCH_POINTS + FIRST_POSITION_POINTS
  }
  const previous = text[position - 1] ?? ''
  const camelCase =
    LOWERCASE_LETTER.test(previous) && UPPERCASE_LETTER.test(text[position] ?? '')
  return MATCH_POINTS + (WORD_SEPARATORS.has(previous) || camelCase ? WORD_START_POINTS : 0)
}

/** Best score of an already normalized query against text, or null when it is not a subsequence. */
function scoreNormalized(query: string, text: string): number | null {
  const lowered = lowercaseAligned(text)
  if (query.length === 0 || !isSubsequence(query, lowered)) {
    return null
  }
  const length = lowered.length
  // best[j]: the best score so far with the latest query character at text position j.
  let best = new Float64Array(length).fill(Number.NEGATIVE_INFINITY)
  for (let position = 0; position < length; position++) {
    if (lowered[position] === query[0]) {
      best[position] = pointsAt(text, position)
    }
  }
  for (let i = 1; i < query.length; i++) {
    const next = new Float64Array(length).fill(Number.NEGATIVE_INFINITY)
    // The best score of the previous query character at a position before j - 1.
    let earlier = Number.NEGATIVE_INFINITY
    for (let position = 1; position < length; position++) {
      const adjacent = best[position - 1] ?? Number.NEGATIVE_INFINITY
      if (lowered[position] === query[i]) {
        const previous = Math.max(earlier, adjacent + CONSECUTIVE_POINTS)
        if (previous > Number.NEGATIVE_INFINITY) {
          next[position] = previous + pointsAt(text, position)
        }
      }
      earlier = Math.max(earlier, adjacent)
    }
    best = next
  }
  const top = best.reduce((highest, score) => Math.max(highest, score), Number.NEGATIVE_INFINITY)
  return top === Number.NEGATIVE_INFINITY ? null : top - LENGTH_PENALTY * length
}

/** Best score of query against text, or null when query is not a subsequence of text. */
export function fuzzyScore(query: string, text: string): number | null {
  return scoreNormalized(normalizedQuery(query), text)
}

export type PaletteItem = {
  readonly id: string
  readonly kind: 'command' | 'feature' | 'body' | 'parameter' | 'recent' | 'setting'
  readonly title: string
  readonly keywords: readonly string[]
  readonly detail: string | null
  readonly shortcut: string | null
  readonly disabledReason: string | null
  readonly run: () => void
  /** The command's category, which orders commands when the query is empty. */
  readonly category?: CommandCategory
}

/** Sort group of an item with an empty query: command categories, settings, then the document. */
function groupOf(item: PaletteItem): number {
  switch (item.kind) {
    case 'command':
      // A command without a category sorts after the command categories, before Settings.
      return item.category === undefined
        ? CATEGORY_ORDER.indexOf('Settings') - 0.5
        : CATEGORY_ORDER.indexOf(item.category)
    case 'setting':
      return CATEGORY_ORDER.indexOf('Settings')
    case 'feature':
      return CATEGORY_ORDER.length
    case 'body':
      return CATEGORY_ORDER.length + 1
    case 'parameter':
      return CATEGORY_ORDER.length + 2
    case 'recent':
      return CATEGORY_ORDER.length + 3
  }
}

function byTitle(first: PaletteItem, second: PaletteItem): number {
  return first.title.localeCompare(second.title, 'en')
}

/** An item's score: the best of its title and its keywords, keywords counting 2 less. */
function itemScore(query: string, item: PaletteItem): number | null {
  let best = scoreNormalized(query, item.title)
  for (const keyword of item.keywords) {
    const score = scoreNormalized(query, keyword)
    if (score !== null && (best === null || score - KEYWORD_PENALTY > best)) {
      best = score - KEYWORD_PENALTY
    }
  }
  return best
}

function rankEmpty(
  items: readonly PaletteItem[],
  recentIds: readonly string[],
): readonly PaletteItem[] {
  const byId = new Map(items.map((item) => [item.id, item]))
  const recent: PaletteItem[] = []
  for (const id of new Set(recentIds)) {
    const item = byId.get(id)
    if (item !== undefined && recent.length < MAXIMUM_RECENT_FIRST) {
      recent.push(item)
    }
  }
  const shown = new Set(recent)
  const rest = items
    .filter((item) => !shown.has(item))
    .toSorted((first, second) => groupOf(first) - groupOf(second) || byTitle(first, second))
  return [...recent, ...rest].slice(0, MAXIMUM_RESULTS)
}

/** Palette items matching a query, best first (ui.md 2.10); at most 50. */
export function rankPaletteItems(
  query: string,
  items: readonly PaletteItem[],
  recentIds: readonly string[],
): readonly PaletteItem[] {
  const normalized = normalizedQuery(query)
  if (normalized === '') {
    return rankEmpty(items, recentIds)
  }
  const recency = new Map<string, number>()
  for (const [position, id] of recentIds.entries()) {
    if (!recency.has(id)) {
      recency.set(id, position)
    }
  }
  const scored: { readonly item: PaletteItem; readonly rank: number; readonly recent: number }[] =
    []
  for (const item of items) {
    const score = itemScore(normalized, item)
    if (score !== null) {
      // Scores are whole numbers less hundredths, so hundredths compare exactly.
      const rank = Math.round(score * 100)
      scored.push({ item, rank, recent: recency.get(item.id) ?? Number.POSITIVE_INFINITY })
    }
  }
  return scored
    .toSorted(
      (first, second) =>
        second.rank - first.rank ||
        compareRecency(first.recent, second.recent) ||
        byTitle(first.item, second.item),
    )
    .slice(0, MAXIMUM_RESULTS)
    .map((entry) => entry.item)
}

function compareRecency(first: number, second: number): number {
  if (first === second) {
    return 0
  }
  return first < second ? -1 : 1
}
