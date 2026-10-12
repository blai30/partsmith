import { CoreError } from '../../core/client'
import { featureLabel } from '../../core/labels'
import { CoreFatalError } from '../../core/transport'
import type { Edit, FeatureStatus } from '../../generated/protocol'
import { MESSAGES } from '../../messages'
import { selectEditLabel } from '../../state/selectors'
import type { DocumentState } from '../../state/store'
import type { App } from '../createApp'
import { editFeature } from './command'
import { undo } from './document'
import { notify } from './notifications'

// Helpers shared by the actions that change the document: labeled edits through the mutation
// queue, and the error and failure notifications of ui.md 4.2.

/** The open document, or null. */
export function documentOf(app: App): DocumentState | null {
  return app.store.getState().document
}

/** Whether the document has changes that are not in its file (core dirty or locally unsaved). */
export function isUnsaved(document: DocumentState): boolean {
  return document.dirty || document.localUnsaved
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
      actions: [
        {
          label: 'Show problems',
          run: () => {
            app.store.setState((state) => ({ panels: { ...state.panels, side: 'problems' } }))
          },
        },
        undoAction,
      ],
    })
    return
  }
  const label = featureLabel(document.recipe, first.id, app.store.getState().catalog)
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
  const document = documentOf(app)
  if (document === null) {
    return false
  }
  const before = statusesOf(app)
  try {
    await app.mutations.run(() => app.core.applyEdits(document.documentHandle, label, edits))
  } catch (error) {
    showCoreError(app, error, options)
    return false
  }
  reportNewFailures(app, before)
  return true
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
