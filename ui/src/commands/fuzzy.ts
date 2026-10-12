import type { AppContext } from '../state/store'
import type { CommandCategory, Keyword } from './registry'

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
const NO_SCORE = Number.NEGATIVE_INFINITY

const MAXIMUM_RESULTS = 50
const MAXIMUM_RECENT_FIRST = 5

/** Every category, in the order of `CommandCategory`. */
const CATEGORIES: readonly CommandCategory[] = [
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

/** The order of the categories the context works with first, with an empty query (ui.md 2.10). */
const CONTEXT_ORDER: Readonly<Record<AppContext, readonly CommandCategory[]>> = {
  model: ['Create', 'Modify', 'Pattern', 'Construct', 'Edit', 'View', 'File', 'Print', 'Settings'],
  sketch: [
    'Sketch',
    'Constrain',
    'Edit',
    'View',
    'Create',
    'Modify',
    'Pattern',
    'Construct',
    'File',
    'Print',
    'Settings',
  ],
  print: ['Print', 'View', 'Edit', 'File', 'Create', 'Modify', 'Pattern', 'Construct', 'Settings'],
}

/** Categories that rank 3 higher in a context, so its own tools come first (2.10). */
const CONTEXT_BONUS_CATEGORIES: Readonly<Record<AppContext, ReadonlySet<CommandCategory>>> = {
  model: new Set(),
  sketch: new Set(['Sketch', 'Constrain']),
  print: new Set(['Print']),
}
const CONTEXT_BONUS = 3

/** The categories in a context's order: the ones it names, then the rest in `CommandCategory` order. */
export function categoryOrder(context: AppContext): readonly CommandCategory[] {
  const named = CONTEXT_ORDER[context]
  return [...named, ...CATEGORIES.filter((category) => !named.includes(category))]
}

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
  const camelCase = LOWERCASE_LETTER.test(previous) && UPPERCASE_LETTER.test(text[position] ?? '')
  return MATCH_POINTS + (WORD_SEPARATORS.has(previous) || camelCase ? WORD_START_POINTS : 0)
}

/** Two rows of the scoring table, at least as long as every text scored with them. */
type ScoreRows = { readonly first: Float64Array; readonly second: Float64Array }

function scoreRows(length: number): ScoreRows {
  return { first: new Float64Array(length), second: new Float64Array(length) }
}

/** Best score of an already normalized query against text, or null when it is not a subsequence. */
function scoreNormalized(query: string, text: string, rows: ScoreRows): number | null {
  const lowered = lowercaseAligned(text)
  if (query.length === 0 || !isSubsequence(query, lowered)) {
    return null
  }
  const length = lowered.length
  // best[position]: the best score so far with the latest query character at that text position.
  let best = rows.first
  let next = rows.second
  for (let position = 0; position < length; position++) {
    best[position] = lowered[position] === query[0] ? pointsAt(text, position) : NO_SCORE
  }
  for (let i = 1; i < query.length; i++) {
    next[0] = NO_SCORE
    // The best score of the previous query character at a position before position - 1.
    let earlier = NO_SCORE
    for (let position = 1; position < length; position++) {
      const adjacent = best[position - 1] ?? NO_SCORE
      const previous = Math.max(earlier, adjacent + CONSECUTIVE_POINTS)
      next[position] =
        lowered[position] === query[i] && previous > NO_SCORE
          ? previous + pointsAt(text, position)
          : NO_SCORE
      earlier = Math.max(earlier, adjacent)
    }
    const filled = next
    next = best
    best = filled
  }
  let top = NO_SCORE
  for (let position = 0; position < length; position++) {
    top = Math.max(top, best[position] ?? NO_SCORE)
  }
  return top === NO_SCORE ? null : top - LENGTH_PENALTY * length
}

/** Best score of query against text, or null when query is not a subsequence of text. */
export function fuzzyScore(query: string, text: string): number | null {
  return scoreNormalized(normalizedQuery(query), text, scoreRows(text.length))
}

/** Fields every palette item has. */
type PaletteItemFields = {
  readonly id: string
  readonly title: string
  readonly keywords: readonly Keyword[]
  readonly detail: string | null
  /** The command's description (shown in the palette footer, 3.4), or null. */
  readonly description: string | null
  readonly shortcut: string | null
  readonly disabledReason: string | null
  readonly run: () => void
}

/**
 * A palette entry. Commands (settings are commands of category Settings) carry their category,
 * which orders them when the query is empty; document items and recent files have none.
 */
export type PaletteItem = PaletteItemFields &
  (
    | { readonly kind: 'command' | 'setting'; readonly category: CommandCategory }
    | { readonly kind: 'feature' | 'body' | 'parameter' | 'recent'; readonly category: null }
  )

function byTitle(first: PaletteItem, second: PaletteItem): number {
  return first.title.localeCompare(second.title, 'en')
}

/** An item's score, and the tagged keyword it came from, if any. */
type ItemScore = { readonly score: number; readonly tagged: Exclude<Keyword, string> | null }

/**
 * An item's score: the best of its title and its keywords, keywords counting 2 less, plus 3 for
 * the context's own categories (2.10).
 */
function itemScore(
  query: string,
  item: PaletteItem,
  rows: ScoreRows,
  context: AppContext,
): ItemScore | null {
  let best: ItemScore | null = null
  const title = scoreNormalized(query, item.title, rows)
  if (title !== null) {
    best = { score: title, tagged: null }
  }
  for (const keyword of item.keywords) {
    const word = typeof keyword === 'string' ? keyword : keyword.word
    const score = scoreNormalized(query, word, rows)
    if (score !== null && (best === null || score - KEYWORD_PENALTY > best.score)) {
      best = {
        score: score - KEYWORD_PENALTY,
        tagged: typeof keyword === 'string' ? null : keyword,
      }
    }
  }
  if (best === null) {
    return null
  }
  const bonus =
    item.category !== null && CONTEXT_BONUS_CATEGORIES[context].has(item.category)
      ? CONTEXT_BONUS
      : 0
  return { score: best.score + bonus, tagged: best.tagged }
}

/** The length of the longest title or keyword. */
function longestText(items: readonly PaletteItem[]): number {
  let longest = 0
  for (const item of items) {
    longest = Math.max(longest, item.title.length)
    for (const keyword of item.keywords) {
      longest = Math.max(longest, (typeof keyword === 'string' ? keyword : keyword.word).length)
    }
  }
  return longest
}

/** The items of `recentIds` in that order, at most 5: the "Recent" group of an empty query. */
export function recentItems(
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
  return recent
}

/**
 * With an empty query: up to 5 recent items, then every command and setting grouped by category
 * in the context's order and by title within a category, so every command can be browsed.
 */
function rankEmpty(
  items: readonly PaletteItem[],
  recentIds: readonly string[],
  context: AppContext,
): readonly PaletteItem[] {
  const recent = recentItems(items, recentIds)
  const shown = new Set(recent)
  const order = categoryOrder(context)
  const groupOf = (item: PaletteItem): number =>
    item.category === null ? order.length : order.indexOf(item.category)
  const rest = items
    .filter((item) => !shown.has(item) && (item.kind === 'command' || item.kind === 'setting'))
    .toSorted((first, second) => groupOf(first) - groupOf(second) || byTitle(first, second))
  return [...recent, ...rest]
}

/** Palette items matching a query, best first (ui.md 2.10); at most 50 for a non-empty query. */
export function rankPaletteItems(
  query: string,
  items: readonly PaletteItem[],
  recentIds: readonly string[],
  context: AppContext,
): readonly PaletteItem[] {
  const normalized = normalizedQuery(query)
  if (normalized === '') {
    return rankEmpty(items, recentIds, context)
  }
  const recency = new Map<string, number>()
  for (const [position, id] of recentIds.entries()) {
    if (!recency.has(id)) {
      recency.set(id, position)
    }
  }
  // One pair of rows for every text, so a keystroke allocates no table per item.
  const rows = scoreRows(longestText(items))
  const scored: { readonly item: PaletteItem; readonly rank: number; readonly recent: number }[] =
    []
  for (const item of items) {
    const found = itemScore(normalized, item, rows, context)
    if (found !== null) {
      // Another app's name that matched names the mapping, unless a reason takes the place (2.10).
      const shownItem =
        found.tagged !== null && item.disabledReason === null
          ? { ...item, detail: `${found.tagged.app}: ${found.tagged.word}` }
          : item
      // Scores are whole numbers less hundredths, so hundredths compare exactly.
      const rank = Math.round(found.score * 100)
      scored.push({
        item: shownItem,
        rank,
        recent: recency.get(item.id) ?? Number.POSITIVE_INFINITY,
      })
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
