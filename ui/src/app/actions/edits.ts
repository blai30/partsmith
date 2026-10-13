import { CoreError } from '../../core/client'
import { CoreFatalError } from '../../core/transport'
import type { Edit, FeatureStatus } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectEditLabel, selectFeatureLabel } from '../../state/selectors'
import type { DocumentState } from '../../state/store'
import type { App } from '../createApp'
import { editFeature } from './command'
import { undo } from './document'
import { notify } from './notifications'
import { showProblemsAction } from './panels'

// Helpers shared by the actions that change the document: labeled edits through the mutation
// queue, and the error and failure notifications of ui.md 4.2.

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

export type RefusalOptions = {
  /** Receives the core's refusal instead of an error notification, for errors shown next to a field. */
  readonly onRefused?: (message: string) => void
}

/**
 * Shows a failed core call: a CoreError's message verbatim as an error notification (4.2), a fatal
 * error through the fatal state the store already shows; anything else is a bug and is rethrown.
 */
export function showCoreError(app: App, error: unknown, options: RefusalOptions = {}): void {
  if (error instanceof CoreError) {
    console.info(`The core refused a request: ${error.code}.`)
    if (options.onRefused !== undefined) {
      options.onRefused(error.message)
      return
    }
    notify(app, { severity: 'error', message: error.message, actions: [], code: error.code })
    return
  }
  if (error instanceof CoreFatalError) {
    return
  }
  throw error
}

/** Statuses by feature id, to compare before and after a change. */
function statusesOf(app: App): Readonly<Record<string, FeatureStatus>> {
  return documentOf(app)?.featureStatus ?? {}
}

/**
 * Shows the failure notification of 4.2 for features that a user change turned from not failing
 * into failing: "{label} failed. {message}" with Edit and Undo, or a count with Show problems.
 */
export function reportNewFailures(app: App, before: Readonly<Record<string, FeatureStatus>>): void {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  const failed = (document.recipe.features ?? []).filter(
    (feature) =>
      document.featureStatus[feature.id]?.status === 'error' &&
      before[feature.id]?.status !== 'error',
  )
  const [first] = failed
  if (first === undefined) {
    return
  }
  const undoAction = {
    label: 'Undo',
    run: () => {
      void undo(app)
    },
  }
  // The notification names the features, so it goes once none of them is in error (3.22).
  const features = failed.map((feature) => feature.id)
  if (failed.length > 1) {
    notify(app, {
      severity: 'error',
      message: MESSAGES.features_failed(failed.length),
      code: 'features_failed',
      features,
      actions: [showProblemsAction(app), undoAction],
    })
    return
  }
  const label = selectFeatureLabel(app.store.getState(), first.id)
  const message = document.featureStatus[first.id]?.messages[0]?.message ?? ''
  notify(app, {
    severity: 'error',
    message: MESSAGES.feature_failed(label, message).trimEnd(),
    code: 'feature_failed',
    features,
    actions: [
      {
        label: selectEditLabel(app.store.getState(), first.id),
        run: () => {
          void editFeature(app, first.id)
        },
      },
      undoAction,
    ],
  })
}

/** Labeled edits built from the document as it is when they are sent. */
export type BuiltEdits = { readonly label: string; readonly edits: readonly Edit[] }

/**
 * Builds edits from the document as it is when their turn in the mutation queue comes, so an edit
 * made from one record never undoes another change queued before it (a rename, then a suppress of
 * the same feature). Null sends nothing, such as for a record that is gone by then.
 */
export type EditsBuilder = (document: DocumentState) => BuiltEdits | null

/**
 * Applies labeled edits as one undo entry through the mutation queue. A refusal shows the core's
 * message and resolves false; features the change made fail are reported (4.2).
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
 * Applies edits built inside their task of the mutation queue (`EditsBuilder`) as one undo entry.
 * Resolves false when nothing was sent or the core refused it, whose message is then shown.
 */
export async function commitBuiltEdits(
  app: App,
  build: EditsBuilder,
  options: RefusalOptions = {},
): Promise<boolean> {
  const handle = documentOf(app)?.documentHandle ?? null
  if (handle === null) {
    return false
  }
  const before = statusesOf(app)
  let sent: boolean
  try {
    sent = await app.mutations.run(async () => {
      // Edits never follow their document into another one that replaced it meanwhile.
      const document = documentOf(app)
      const built = document?.documentHandle === handle ? build(document) : null
      if (document === null || built === null) {
        return false
      }
      await app.core.applyEdits(document.documentHandle, built.label, built.edits)
      return true
    })
  } catch (error) {
    showCoreError(app, error, options)
    return false
  }
  if (sent) {
    reportNewFailures(app, before)
  }
  return sent
}

/** Runs a core call that changes the document through the mutation queue, reporting failures; resolves null when it failed. */
export async function mutate<Result>(
  app: App,
  task: (document: DocumentState) => Promise<Result>,
  options: RefusalOptions = {},
): Promise<Result | null> {
  const document = documentOf(app)
  if (document === null) {
    return null
  }
  try {
    return await app.mutations.run(() => task(document))
  } catch (error) {
    showCoreError(app, error, options)
    return null
  }
}
