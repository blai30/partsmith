import type { Cause, Diagnostic, Document, FeatureStatus, Repair } from '../generated/protocol'
import type { AppState } from '../state/store'

// The Problems panel's rows grouped by root cause (ui.md 3.9), and the notifications a user change
// raises for new losses and failures (4.2, 3.22), both from the statuses alone.

/** A feature listed under its root's row: its warnings and errors, shown without their actions. */
export type NestedProblem = {
  readonly feature: string
  readonly root: string
  readonly messages: readonly Diagnostic[]
}

/** A top-level row: a feature (or a root that heads a group) with its warnings and errors, or a document diagnostic (`feature` null). */
export type ProblemRow = {
  readonly feature: string | null
  readonly messages: readonly Diagnostic[]
  readonly nested: readonly NestedProblem[]
}

/** An info notice of a feature, listed under Notes and never counted as a problem. */
export type ProblemNote = { readonly feature: string; readonly message: Diagnostic }

export type ProblemRows = {
  readonly rows: readonly ProblemRow[]
  readonly notes: readonly ProblemNote[]
}

/** A notification a change raises (4.2), before its actions are bound. `problems` are loss keys `{feature} {code} {field}`. */
export type ChangeNotice =
  | {
      readonly kind: 'lost'
      readonly root: string
      readonly description: string
      readonly failed: number
      readonly severity: 'warning' | 'error'
      readonly cause: Cause | null
      readonly features: readonly string[]
      readonly problems: readonly string[]
    }
  | {
      readonly kind: 'check'
      readonly root: string
      readonly cause: Cause | null
      readonly features: readonly string[]
      readonly problems: readonly string[]
    }
  | { readonly kind: 'failed'; readonly features: readonly string[] }

type Statuses = Readonly<Record<string, FeatureStatus>>

/** The codes of lost, guessed and widened references and regions, face fallbacks and turned-over faces (4.2). */
const LOSS_CODES: ReadonlySet<string> = new Set([
  'reference_missing',
  'reference_ambiguous',
  'reference_guessed',
  'reference_widened',
  'region_missing',
  'region_ambiguous',
  'region_resized',
  'region_remapped',
  'plane_fallback',
  'sketch_turned_over',
  'references_not_found',
  'reference_not_found',
  'profile_not_found',
])

/** Loss codes whose reference is still used, so the feature needs a check rather than lost something (4.2). */
const CHECK_CODES: ReadonlySet<string> = new Set([
  'reference_guessed',
  'reference_widened',
  'region_resized',
  'region_remapped',
  'sketch_turned_over',
])

/** Whether a message is a warning or an error: info notices are never problems (3.9). */
function isProblem(message: Diagnostic): boolean {
  return message.severity !== 'info'
}

/** The key that tells one loss from another across a change: feature, code and field (4.2). */
function lossKey(feature: string, message: Diagnostic): string {
  return `${feature} ${message.code} ${message.field ?? ''}`
}

/** The loss messages of a status: warnings and errors whose code reports a loss (4.2). */
function lossMessages(status: FeatureStatus | undefined): readonly Diagnostic[] {
  return (status?.messages ?? []).filter(
    (message) => isProblem(message) && LOSS_CODES.has(message.code),
  )
}

/** The loss keys of every status. */
export function lossKeys(statuses: Statuses): ReadonlySet<string> {
  return new Set(
    Object.values(statuses).flatMap((status) =>
      lossMessages(status).map((message) => lossKey(status.id, message)),
    ),
  )
}

/** The feature ids of a document in timeline order. */
function featureIds(document: Document | undefined): readonly string[] {
  return (document?.features ?? []).map((feature) => feature.id)
}

/** `root` when it names another feature of the document, else null. */
function otherFeature(
  root: string | null | undefined,
  feature: string,
  ids: ReadonlySet<string>,
): string | null {
  return root !== null && root !== undefined && root !== feature && ids.has(root) ? root : null
}

/** The rows of the Problems panel from the latest statuses and diagnostics, in timeline order (3.9). */
export function problemRows(state: AppState): ProblemRows {
  const document = state.document
  return groupedRows(document?.recipe, document?.featureStatus ?? {}, document?.diagnostics ?? [])
}

/**
 * The rows for a recipe's statuses and document diagnostics (3.9): a feature whose first message
 * with a `root` names another feature of the document is listed under that root's row, which a
 * root heads whatever its own status; info notices of features are notes, and each document
 * diagnostic is a row of its own.
 */
export function groupedRows(
  recipe: Document | undefined,
  statuses: Statuses,
  diagnostics: readonly Diagnostic[],
): ProblemRows {
  const order = featureIds(recipe)
  const ids = new Set(order)
  const notes: ProblemNote[] = []
  const problems = new Map<string, readonly Diagnostic[]>()
  const roots = new Map<string, string>()
  for (const feature of order) {
    const status = statuses[feature]
    if (status === undefined) {
      continue
    }
    for (const message of status.messages) {
      if (!isProblem(message)) {
        notes.push({ feature, message })
      }
    }
    if (status.status !== 'warning' && status.status !== 'error') {
      continue
    }
    const messages = status.messages.filter(isProblem)
    problems.set(feature, messages)
    const withRoot = messages.find((message) => (message.root ?? null) !== null)
    const root = otherFeature(withRoot?.root, feature, ids)
    if (root !== null) {
      roots.set(feature, root)
    }
  }
  /** The root that heads a feature's group, following roots that have roots of their own. */
  function headOf(feature: string): string {
    const seen = new Set([feature])
    let head = feature
    for (let next = roots.get(head); next !== undefined && !seen.has(next);) {
      seen.add(next)
      head = next
      next = roots.get(head)
    }
    return head
  }
  const nested = new Map<string, NestedProblem[]>()
  for (const [feature, messages] of problems) {
    const head = headOf(feature)
    if (head !== feature) {
      const group = nested.get(head) ?? []
      group.push({ feature, root: head, messages })
      nested.set(head, group)
    }
  }
  const rows: ProblemRow[] = order.flatMap((feature): ProblemRow[] => {
    const group = nested.get(feature) ?? []
    const messages = problems.get(feature)
    const heads = messages !== undefined && headOf(feature) === feature
    return heads || group.length > 0 ? [{ feature, messages: messages ?? [], nested: group }] : []
  })
  for (const diagnostic of diagnostics) {
    rows.push({ feature: null, messages: [diagnostic], nested: [] })
  }
  return { rows, notes }
}

/** The severity a problem shows (3.9): an error when any of its messages is one, else a warning, never info. */
export function problemSeverity(messages: readonly Diagnostic[]): 'error' | 'warning' {
  return messages.some((message) => message.severity === 'error') ? 'error' : 'warning'
}

/**
 * The severity a top-level row shows (3.9): an error when its feature's status is one or when any
 * message of the row or of the problems nested under it is, so a root whose own status is fine
 * shows those of its nested features; else a warning.
 */
export function rowSeverity(row: ProblemRow, statuses: Statuses): 'error' | 'warning' {
  if (row.feature !== null && statuses[row.feature]?.status === 'error') {
    return 'error'
  }
  return problemSeverity([...row.messages, ...row.nested.flatMap((problem) => problem.messages)])
}

/** The repair a loss message describes: the one at its field, else the status's first (4.2). */
function repairOf(status: FeatureStatus, message: Diagnostic): Repair | undefined {
  return status.repairs.find((repair) => repair.path === message.field) ?? status.repairs[0]
}

/**
 * The notices of one change (4.2): one per root with a new loss of its own, in timeline order of
 * their roots, then one `failed` notice for the features newly in error that no loss notice names.
 * A loss is new when its key was not in the statuses before; a feature belongs to the root its
 * first message's `root` names when that is another feature of the document, else to itself.
 */
export function changeNotifications(
  before: Statuses,
  after: Statuses,
  document: Document,
): readonly ChangeNotice[] {
  const order = featureIds(document)
  const ids = new Set(order)
  const known = lossKeys(before)
  const newlyFailed = order.filter(
    (feature) => after[feature]?.status === 'error' && before[feature]?.status !== 'error',
  )
  const rootOf = (feature: string): string =>
    otherFeature(after[feature]?.messages[0]?.root, feature, ids) ?? feature
  const notices: ChangeNotice[] = []
  const reported = new Set<string>()
  for (const root of order) {
    const status = after[root]
    if (status === undefined || rootOf(root) !== root) {
      continue
    }
    const losses = lossMessages(status).filter((message) => !known.has(lossKey(root, message)))
    const [first] = losses
    if (first === undefined) {
      continue
    }
    const group = newlyFailed.filter((feature) => feature !== root && rootOf(feature) === root)
    const features = [root, ...group]
    const problems = losses.map((message) => lossKey(root, message))
    const cause = first.cause ?? null
    for (const feature of features) {
      reported.add(feature)
    }
    const repair = repairOf(status, first)
    if (repair === undefined || CHECK_CODES.has(first.code)) {
      notices.push({ kind: 'check', root, cause, features, problems })
      continue
    }
    const failing = features.some((feature) => after[feature]?.status === 'error')
    notices.push({
      kind: 'lost',
      root,
      description: repair.description,
      failed: group.length,
      severity: failing ? 'error' : 'warning',
      cause,
      features,
      problems,
    })
  }
  const failed = newlyFailed.filter((feature) => !reported.has(feature))
  if (failed.length > 0) {
    notices.push({ kind: 'failed', features: failed })
  }
  return notices
}
