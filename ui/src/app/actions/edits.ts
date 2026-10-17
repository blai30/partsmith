import { CoreError } from '../../core/client'
import type { TaskKind } from '../../core/queue'
import { CoreFatalError } from '../../core/transport'
import type { Edit, FeatureStatus } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import type { DocumentState, Notification } from '../../state/store'
import type { App } from '../createApp'
import { copyDetails, fixNotificationAction, offeredActions } from './fixes'
import { reportChange } from './loss-notifications'
import { notify } from './notifications'
import { showProblemsAction } from './panels'

// Helpers shared by the actions that change the document: labeled edits through the mutation
// queue, the error notifications of ui.md 4.2, and the report of what a change broke.

/** The open document, or null. */
export function documentOf(app: App): DocumentState | null {
  return app.store.getState().document
}

/**
 * Whether an active feature command blocks other document edits (3.7): it shows
 * `finish_command_first` as an info notification and returns true, so the edit is not sent.
 */
export function refusedDuringCommand(app: App): boolean {
  const command = app.store.getState().command
  if (command === null) {
    return false
  }
  notify(app, {
    severity: 'info',
    message: MESSAGES.finish_command_first(command.title),
    actions: [],
    code: 'finish_command_first',
  })
  return true
}

/** The warning of an open, a new document, a save or an export that the core stopped before it ended (4.3). */
export type StoppedNotice = { readonly code: string; readonly message: string }

export type RefusalOptions = {
  /** Receives the core's refusal instead of an error notification, for errors shown next to a field. */
  readonly onRefused?: (message: string) => void
  /** Shown as a warning when the core stops before the call ends, for a call that is not a change (4.4). */
  readonly stopped?: StoppedNotice
}

/**
 * The actions of a CoreError's notification (4.2): the error's own actions but
 * `finish_transaction`, which the UI never shows because it refuses those requests itself (3.7),
 * "Show problems" for a fix that no longer applies, and Copy details last when it has a `detail`.
 */
function errorActions(app: App, error: CoreError): Notification['actions'] {
  const own = offeredActions(error.actions).map((action) => fixNotificationAction(app, action))
  const details =
    error.detail === null
      ? []
      : [
          {
            label: 'Copy details',
            run: () => {
              // Not the error itself: its `cause` is `Error.cause`, not a cause line.
              void copyDetails(app, {
                code: error.code,
                message: error.message,
                detail: error.detail,
              })
            },
          },
        ]
  return [...own, ...(error.code === 'stale_fix' ? [showProblemsAction(app)] : []), ...details]
}

/**
 * Shows a failed core call: a CoreError's message verbatim as an error notification with its
 * actions (4.2). A fatal error shows the call's `stopped` notice; a change shows nothing, since the
 * fatal state or the restart after a stop reports it (4.4). Anything else is a bug and is rethrown.
 */
export function showCoreError(app: App, error: unknown, options: RefusalOptions = {}): void {
  if (error instanceof CoreError) {
    console.info(`The core refused a request: ${error.code}.`)
    if (options.onRefused !== undefined) {
      options.onRefused(error.message)
      return
    }
    notify(app, {
      severity: 'error',
      message: error.message,
      actions: errorActions(app, error),
      code: error.code,
    })
    return
  }
  if (error instanceof CoreFatalError) {
    if (options.stopped !== undefined) {
      notify(app, { severity: 'warning', ...options.stopped, actions: [] })
    }
    return
  }
  throw error
}

/** Statuses by feature id, to compare before and after a change. */
function statusesOf(app: App): Readonly<Record<string, FeatureStatus>> {
  return documentOf(app)?.featureStatus ?? {}
}

/** Labeled edits built from the document as it is when they are sent. */
export type BuiltEdits = { readonly label: string; readonly edits: readonly Edit[] }

/**
 * Builds edits from the document as it is when their turn in the mutation queue comes, so an edit
 * made from one record never undoes another change queued before it (a rename, then a suppress of
 * the same feature). Null sends nothing, such as for a record that is gone by then.
 */
export type EditsBuilder = (document: DocumentState) => BuiltEdits | null

/** What a user change led to: whether it was sent, and whether it raised loss or failure notifications (4.2). */
export type ChangeOutcome = { readonly sent: boolean; readonly reported: boolean }

/**
 * Runs one user change on the open document through the mutation queue, then reports what it
 * broke (4.2). `change` runs when its turn comes, never on a document that replaced this one
 * meanwhile, and resolves false when it sent nothing. A refusal shows the core's message.
 */
export async function runChange(
  app: App,
  change: (document: DocumentState) => Promise<boolean>,
  options: RefusalOptions = {},
): Promise<ChangeOutcome> {
  const handle = documentOf(app)?.documentHandle ?? null
  if (handle === null) {
    return { sent: false, reported: false }
  }
  const before = statusesOf(app)
  let sent: boolean
  try {
    sent = await app.mutations.run(async () => {
      const document = documentOf(app)
      return document?.documentHandle === handle ? change(document) : false
    })
  } catch (error) {
    showCoreError(app, error, options)
    return { sent: false, reported: false }
  }
  return { sent, reported: sent && reportChange(app, before) > 0 }
}

/**
 * Applies labeled edits as one undo entry through the mutation queue. A refusal shows the core's
 * message and resolves false; what the change broke is reported (4.2).
 */
export async function commitEdits(
  app: App,
  label: string,
  edits: readonly Edit[],
  options: RefusalOptions = {},
): Promise<boolean> {
  return commitBuiltEdits(app, () => ({ label, edits }), options)
}

/**
 * Applies edits built inside their task of the mutation queue (`EditsBuilder`) as one undo entry,
 * and tells whether the change raised loss or failure notifications (4.2).
 */
export async function commitReportedEdits(
  app: App,
  build: EditsBuilder,
  options: RefusalOptions = {},
): Promise<ChangeOutcome> {
  return runChange(
    app,
    async (document) => {
      const built = build(document)
      if (built === null) {
        return false
      }
      await app.core.applyEdits(document.documentHandle, built.label, built.edits)
      return true
    },
    options,
  )
}

/**
 * Applies edits built inside their task of the mutation queue (`EditsBuilder`) as one undo entry.
 * Resolves false when nothing was sent or the core refused it, whose message is then shown.
 */
export async function commitBuiltEdits(
  app: App,
  build: EditsBuilder,
  options: RefusalOptions = {},
): Promise<boolean> {
  return (await commitReportedEdits(app, build, options)).sent
}

/**
 * Runs a core call on the open document through the mutation queue, as a task of `kind` (a change
 * unless given), reporting failures; resolves null when it failed.
 */
export async function mutate<Result>(
  app: App,
  task: (document: DocumentState) => Promise<Result>,
  options: RefusalOptions & { readonly kind?: TaskKind } = {},
): Promise<Result | null> {
  const document = documentOf(app)
  if (document === null) {
    return null
  }
  try {
    return await app.mutations.run(() => task(document), options.kind)
  } catch (error) {
    showCoreError(app, error, options)
    return null
  }
}
