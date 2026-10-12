import { documentOf, showCoreError } from '../app/actions/edits'
import type { App } from '../app/createApp'
import { createLatestOnly } from '../core/queue'
import type { Edit, FeatureRecord } from '../generated/protocol'

// Dragging the rollback marker (ui.md 3.9): a transaction labeled "Roll back" whose set_rollback
// follows the marker live, committed on drop and canceled by Esc or a drop where it started.

export type RollbackDrag = {
  /** The marker is over gap `position` (0 to N): the model follows. */
  move(position: number): void
  /** Commits the transaction, or cancels it when the marker is back where it started. */
  drop(): Promise<void>
  cancel(): Promise<void>
}

/** The set_rollback edit for marker position p: features 1 to p are active (3.9). */
export function rollbackEdit(records: readonly FeatureRecord[], position: number): Edit {
  const first = records[position]
  return first === undefined
    ? { type: 'set_rollback' }
    : { type: 'set_rollback', rollback: first.id }
}

/** Starts a marker drag from `start`; null without a document. */
export function startRollbackDrag(app: App, start: number): RollbackDrag | null {
  const document = documentOf(app)
  if (document === null) {
    return null
  }
  const handle = document.documentHandle
  const records = document.recipe.features ?? []
  let position = start
  let finished = false
  const began = app.mutations
    .run(() => app.core.beginTransaction(handle, 'Roll back'))
    .then(
      () => true,
      (error: unknown) => {
        showCoreError(app, error)
        return false
      },
    )
  const preview = createLatestOnly<number, number>(
    async (target) => {
      if (!(await began)) {
        return 0
      }
      return app.mutations.run(() =>
        app.core.applyEdits(handle, null, [rollbackEdit(records, target)]),
      )
    },
    () => undefined,
    (error) => {
      showCoreError(app, error)
    },
  )

  async function cancel(): Promise<void> {
    if (finished) {
      return
    }
    finished = true
    preview.cancel()
    await preview.flush()
    if (!(await began)) {
      return
    }
    try {
      await app.mutations.run(() => app.core.cancelTransaction(handle))
    } catch (error) {
      showCoreError(app, error)
    }
  }

  return {
    move(next) {
      if (finished || next === position) {
        return
      }
      position = next
      preview.submit(next)
    },
    async drop() {
      if (finished) {
        return
      }
      if (position === start) {
        await cancel()
        return
      }
      finished = true
      await preview.flush()
      if (!(await began)) {
        return
      }
      try {
        await app.mutations.run(() => app.core.commitTransaction(handle))
      } catch (error) {
        showCoreError(app, error)
      }
    },
    cancel,
  }
}
