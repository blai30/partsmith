import { createLatestOnly } from '../../core/queue'
import type { Diagnostic, Edit, FeatureRecord } from '../../generated/protocol'
import { suggestedRecord } from '../../inspector/status'
import { MESSAGES } from '../../messages'
import { selectFeatureLabel } from '../../state/selectors'
import type { App } from '../createApp'
import { coreUndo } from './document'
import {
  commitBuiltEdits,
  commitEdits,
  documentOf,
  refusedDuringCommand,
  showCoreError,
} from './edits'
import { notify } from './notifications'
import { withOptional } from './records'

// Timeline edits (ui.md 3.9): rollback marker, reorder, suppress, delete and rename.

function recordsOf(app: App): readonly FeatureRecord[] {
  return documentOf(app)?.recipe.features ?? []
}

/** The set_rollback edit for marker position p: features 1 to p are active (3.9). */
function rollbackEdit(records: readonly FeatureRecord[], position: number): Edit {
  const first = records[position]
  return first === undefined
    ? { type: 'set_rollback' }
    : { type: 'set_rollback', rollback: first.id }
}

/** Moves the rollback marker to position 0 to N, labeled "Roll back". */
export async function setRollback(app: App, position: number): Promise<void> {
  if (refusedDuringCommand(app)) {
    return
  }
  await commitEdits(app, 'Roll back', [rollbackEdit(recordsOf(app), position)])
}

/** A drag of the rollback marker in progress (3.9). */
export type RollbackDrag = {
  /** The marker is over gap `position` (0 to N): the model follows it live. */
  move(position: number): void
  /** Commits the drag as one undo entry, or cancels it when the marker is back where it started. */
  drop(): Promise<void>
  /** Cancels the drag (Esc): nothing is recorded and the model is as before. */
  cancel(): Promise<void>
}

/**
 * Starts dragging the rollback marker from gap `start` (3.9): a transaction labeled "Roll back"
 * whose set_rollback follows the marker through a LatestOnly on the mutation queue, so the model
 * updates live and newer gaps replace waiting ones. Null without a document.
 */
export function startRollbackDrag(app: App, start: number): RollbackDrag | null {
  const document = documentOf(app)
  if (document === null || refusedDuringCommand(app)) {
    return null
  }
  const { documentHandle } = document
  // Gaps count the timeline as it was when the drag started; the drag reorders nothing.
  const records = document.recipe.features ?? []
  let position = start
  let ended = false
  const began = app.mutations
    .run(() => app.core.beginTransaction(documentHandle, 'Roll back'))
    .then(
      () => true,
      (error: unknown) => {
        showCoreError(app, error)
        return false
      },
    )
  const rollbacks = createLatestOnly<number, void>(
    async (gap) => {
      if (await began) {
        await app.mutations.run(() =>
          app.core.applyEdits(documentHandle, null, [rollbackEdit(records, gap)]),
        )
      }
    },
    () => undefined,
    (error) => {
      showCoreError(app, error)
    },
  )

  /** Ends the transaction after the last gap was applied: committed, or canceled. */
  async function end(commit: boolean): Promise<void> {
    ended = true
    if (!commit) {
      rollbacks.cancel()
    }
    await rollbacks.flush()
    if (!(await began)) {
      return
    }
    try {
      if (commit) {
        await app.mutations.run(() => app.core.commitTransaction(documentHandle))
      } else {
        await app.mutations.run(() => app.core.cancelTransaction(documentHandle))
      }
    } catch (error) {
      showCoreError(app, error)
    }
  }

  return {
    move(gap) {
      if (ended || gap === position) {
        return
      }
      position = gap
      rollbacks.submit(gap)
    },
    async drop() {
      if (!ended) {
        await end(position !== start)
      }
    },
    async cancel() {
      if (!ended) {
        await end(false)
      }
    },
  }
}

/** Moves a feature to a timeline index, labeled "Reorder"; its own index changes nothing. */
export async function moveFeature(app: App, featureId: string, toIndex: number): Promise<void> {
  const index = recordsOf(app).findIndex((record) => record.id === featureId)
  if (index === -1 || index === toIndex || refusedDuringCommand(app)) {
    return
  }
  await commitEdits(app, 'Reorder', [{ type: 'move_feature', id: featureId, index: toIndex }])
}

/** Suppresses or unsuppresses features in one edit. */
export async function setSuppressed(
  app: App,
  featureIds: readonly string[],
  suppressed: boolean,
): Promise<void> {
  const chosen = new Set(featureIds)
  if (!recordsOf(app).some((record) => chosen.has(record.id)) || refusedDuringCommand(app)) {
    return
  }
  const verb = suppressed ? 'Suppress' : 'Unsuppress'
  await commitBuiltEdits(app, (document) => {
    // The records as they are then, so a rename queued before this keeps its name.
    const records = (document.recipe.features ?? []).filter((record) => chosen.has(record.id))
    const [first] = records
    if (first === undefined) {
      return null
    }
    const subject =
      records.length === 1
        ? selectFeatureLabel(app.store.getState(), first.id)
        : `${String(records.length)} features`
    return {
      label: `${verb} ${subject}`,
      edits: records.map((record) => ({
        type: 'update_feature',
        // Suppressed is omitted when false (model.md).
        feature: withOptional(record, 'suppressed', suppressed ? true : undefined),
      })),
    }
  })
}

/**
 * Deletes features in one edit. When the rollback feature goes, the marker first moves to the
 * nearest later feature that stays, or to the end; "Deleted {label}." then offers Undo.
 */
export async function deleteFeatures(app: App, featureIds: readonly string[]): Promise<void> {
  const document = documentOf(app)
  if (document === null) {
    return
  }
  const doomed = new Set(featureIds)
  const records = document.recipe.features ?? []
  const deleted = records.filter((record) => doomed.has(record.id))
  const [first] = deleted
  if (first === undefined || refusedDuringCommand(app)) {
    return
  }
  const edits: Edit[] = []
  const rollback = document.recipe.rollback ?? null
  if (rollback !== null && doomed.has(rollback)) {
    const rollbackIndex = records.findIndex((record) => record.id === rollback)
    const next = records.slice(rollbackIndex + 1).find((record) => !doomed.has(record.id))
    edits.push(
      next === undefined ? { type: 'set_rollback' } : { type: 'set_rollback', rollback: next.id },
    )
  }
  for (const record of deleted) {
    edits.push({ type: 'remove_feature', id: record.id })
  }
  const subject =
    deleted.length === 1
      ? selectFeatureLabel(app.store.getState(), first.id)
      : `${String(deleted.length)} features`
  if (!(await commitEdits(app, `Delete ${subject}`, edits))) {
    return
  }
  notify(app, {
    severity: 'info',
    message: MESSAGES.deleted(subject),
    code: 'deleted',
    actions: [
      {
        label: 'Undo',
        run: () => {
          void coreUndo(app)
        },
      },
    ],
  })
}

/** Renames a feature, labeled "Rename"; an empty name brings back the default label. */
export async function renameFeature(app: App, featureId: string, name: string): Promise<void> {
  if (
    !recordsOf(app).some((candidate) => candidate.id === featureId) ||
    refusedDuringCommand(app)
  ) {
    return
  }
  const trimmed = name.trim()
  await commitBuiltEdits(app, (document) => {
    // Only the name changes on the record as it is then, so a change queued before this stays.
    const record = document.recipe.features?.find((candidate) => candidate.id === featureId)
    return record === undefined
      ? null
      : {
          label: 'Rename',
          edits: [
            {
              type: 'update_feature',
              feature: withOptional(record, 'name', trimmed === '' ? undefined : trimmed),
            },
          ],
        }
  })
}

/**
 * "Use {suggestion}" for a feature message at rest (4.2): one `update_feature` with the value at the
 * message's field replaced, labeled "Change {label}". Refused during a feature command (3.7), and
 * built from the record as it is when the edit is sent.
 */
export async function applySuggestion(
  app: App,
  featureId: string,
  message: Diagnostic,
): Promise<boolean> {
  if (refusedDuringCommand(app)) {
    return false
  }
  return commitBuiltEdits(app, (document) => {
    const record = document.recipe.features?.find((candidate) => candidate.id === featureId)
    const updated = record === undefined ? null : suggestedRecord(record, message)
    return updated === null
      ? null
      : {
          label: `Change ${selectFeatureLabel(app.store.getState(), featureId)}`,
          edits: [{ type: 'update_feature', feature: updated }],
        }
  })
}
